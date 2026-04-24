use bevy::prelude::{
    App, IntoScheduleConfigs, Local, Reflect, ReflectResource, Res, ResMut, Resource, Time, Update,
};
use mlua::{Function, Lua, Table, Value};
use sidereal_game::{
    GeneratedComponentRegistry, ProceduralSprite,
    compute_collision_half_extents_from_procedural_sprite,
    compute_collision_half_extents_from_sprite_length,
    generate_rdp_collision_outline_from_procedural_sprite,
    generate_rdp_collision_outline_from_sprite_png,
};
use sidereal_persistence::{
    GraphEntityRecord, ScriptCatalogRecord, ensure_script_catalog_schema, infer_script_family,
    load_active_script_catalog, replace_active_script_catalog,
};
use sidereal_scripting::{
    LuaSandboxPolicy, PLANET_REGISTRY_SCRIPT_REL_PATH, ScriptAssetRegistryEntry, ScriptError,
    WORLD_INIT_SCRIPT_REL_PATH, WorldInitScriptConfig, decode_graph_entity_records,
    inject_script_logger, load_asset_registry_from_source, load_lua_module_from_source,
    load_planet_registry_from_sources, load_world_init_config_from_source, lua_value_to_json,
    resolve_scripts_root, table_get_required_string, table_get_required_string_list,
    validate_runtime_render_graph_records,
};
use std::collections::{HashMap, HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

fn remove_empty_array_like_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
) {
    match object.get_mut(key) {
        Some(serde_json::Value::Null) => {
            object.remove(key);
        }
        Some(serde_json::Value::Array(values)) if values.is_empty() => {
            object.remove(key);
        }
        Some(serde_json::Value::Object(map)) if map.is_empty() => {
            object.remove(key);
        }
        _ => {}
    }
}

pub fn scripts_root_dir() -> PathBuf {
    let resolved = resolve_scripts_root(env!("CARGO_MANIFEST_DIR"));
    bevy::log::info!(
        "replication scripting root resolved to {}",
        resolved.display()
    );
    resolved
}

#[derive(Resource, Reflect, Debug, Clone, Default)]
#[reflect(Resource)]
pub struct ScriptCatalogResource {
    pub entries: Vec<ScriptCatalogEntry>,
    pub revision: u64,
    pub root_dir: String,
}

#[derive(Reflect, Debug, Clone, Default)]
pub struct ScriptCatalogEntry {
    pub script_path: String,
    pub source: String,
    pub revision: u64,
    pub origin: String,
}

#[derive(Resource, Reflect, Debug, Clone, Default)]
#[reflect(Resource)]
pub struct ScriptCatalogControlResource {
    pub reload_all_from_disk_requested: bool,
    pub last_reload_succeeded: bool,
    pub last_reload_message: String,
    pub last_reload_at_s: f64,
    pub last_persist_succeeded: bool,
    pub last_persist_message: String,
    pub last_persist_at_s: f64,
    pub startup_loaded_from_disk_fallback: bool,
    pub startup_status_message: String,
}

#[derive(Resource, Reflect, Debug, Clone, Default)]
#[reflect(Resource)]
pub struct EntityRegistryResource {
    pub entries: Vec<EntityRegistryEntry>,
    pub revision: u64,
    pub script_path: String,
}

#[derive(Reflect, Debug, Clone, Default)]
pub struct EntityRegistryEntry {
    pub entity_id: String,
    pub entity_class: String,
    pub graph_records_script: String,
    pub required_component_kinds: Vec<String>,
}

#[derive(Resource, Reflect, Debug, Clone, Default)]
#[reflect(Resource)]
pub struct AssetRegistryResource {
    pub entries: Vec<AssetRegistryEntry>,
    pub revision: u64,
    pub script_path: String,
}

#[derive(Reflect, Debug, Clone, Default)]
pub struct AssetRegistryEntry {
    pub asset_id: String,
    pub shader_family: Option<String>,
    pub source_path: String,
    pub content_type: String,
    pub dependencies: Vec<String>,
    pub bootstrap_required: bool,
}

#[derive(Resource, Debug, Clone, Default)]
struct ScriptCatalogSyncState {
    fingerprints_by_path: HashMap<String, u64>,
    next_revision: u64,
}

#[derive(Resource, Debug, Clone)]
struct AssetRegistrySyncState {
    registry_script_path: PathBuf,
    last_catalog_revision: u64,
}

#[derive(Resource, Debug, Clone)]
struct PlanetRegistrySyncState {
    registry_script_path: PathBuf,
    last_catalog_revision: u64,
}

#[derive(Resource, Debug, Clone, Default)]
struct ScriptCatalogPersistenceState {
    last_persisted_catalog_revision: u64,
}

#[derive(Debug, Clone)]
struct ScriptCatalogLoadOutcome {
    catalog: ScriptCatalogResource,
    persisted_catalog_revision: u64,
    startup_loaded_from_disk_fallback: bool,
    startup_status_message: String,
}

const ENTITY_REGISTRY_SCRIPT_REL_PATH: &str = "bundles/bundle_registry.lua";
const ASSET_REGISTRY_SCRIPT_REL_PATH: &str = "assets/registry.lua";

pub fn init_resources(app: &mut App) {
    let scripts_root = scripts_root_dir();
    let asset_registry_script_path = scripts_root.join(ASSET_REGISTRY_SCRIPT_REL_PATH);
    let planet_registry_script_path = scripts_root.join(PLANET_REGISTRY_SCRIPT_REL_PATH);
    let load_outcome = match load_script_catalog_from_database_or_disk(&scripts_root) {
        Ok(outcome) => outcome,
        Err(err) => {
            bevy::log::error!(
                "replication script catalog startup failed root={}: {}",
                scripts_root.display(),
                err
            );
            let fallback_catalog =
                script_catalog_from_disk(&scripts_root).unwrap_or_else(|disk_err| {
                    bevy::log::error!(
                        "replication script catalog disk fallback also failed root={}: {}",
                        scripts_root.display(),
                        disk_err
                    );
                    ScriptCatalogResource {
                        entries: Vec::new(),
                        revision: 1,
                        root_dir: scripts_root.display().to_string(),
                    }
                });
            ScriptCatalogLoadOutcome {
                catalog: fallback_catalog,
                persisted_catalog_revision: 0,
                startup_loaded_from_disk_fallback: true,
                startup_status_message: format!(
                    "script catalog startup fell back to disk after failure: {}",
                    err
                ),
            }
        }
    };
    let mut catalog = load_outcome.catalog;
    if load_outcome.startup_loaded_from_disk_fallback {
        bevy::log::warn!(
            "replication script catalog booted in disk-fallback mode root={}: {}",
            scripts_root.display(),
            load_outcome.startup_status_message
        );
    } else {
        bevy::log::debug!(
            "replication script catalog startup ready root={}: {}",
            scripts_root.display(),
            load_outcome.startup_status_message
        );
    }
    let (catalog_revision, next_revision, fingerprints_by_path) =
        initialize_catalog_revisions(&catalog.entries);
    catalog.revision = catalog_revision;
    let entries = match load_entity_registry_entries_from_catalog(&catalog) {
        Ok(entries) => entries,
        Err(err) => {
            bevy::log::warn!("replication entity registry initial derive failed: {}", err);
            Vec::new()
        }
    };
    let (asset_entries, shader_entries) = match load_asset_registry_data_from_catalog(&catalog) {
        Ok(entries) => entries,
        Err(err) => {
            bevy::log::warn!("replication asset registry initial derive failed: {}", err);
            (Vec::new(), Vec::new())
        }
    };
    let planet_registry = match load_planet_registry_from_catalog(&catalog) {
        Ok(registry) => registry,
        Err(err) => {
            bevy::log::warn!("replication planet registry initial derive failed: {}", err);
            sidereal_game::PlanetRegistry::default()
        }
    };
    app.register_type::<ScriptCatalogEntry>();
    app.register_type::<ScriptCatalogResource>();
    app.register_type::<ScriptCatalogControlResource>();
    app.register_type::<EntityRegistryEntry>();
    app.register_type::<EntityRegistryResource>();
    app.register_type::<AssetRegistryEntry>();
    app.register_type::<AssetRegistryResource>();
    app.register_type::<sidereal_game::PlanetRegistryEntry>();
    app.register_type::<sidereal_game::PlanetSpawnDefinition>();
    app.register_type::<sidereal_game::PlanetDefinition>();
    app.register_type::<sidereal_game::PlanetRegistry>();
    app.insert_resource(catalog);
    app.insert_resource(ScriptCatalogControlResource {
        startup_loaded_from_disk_fallback: load_outcome.startup_loaded_from_disk_fallback,
        startup_status_message: load_outcome.startup_status_message.clone(),
        ..Default::default()
    });
    app.insert_resource(ScriptCatalogSyncState {
        fingerprints_by_path,
        next_revision,
    });
    app.insert_resource(ScriptCatalogPersistenceState {
        last_persisted_catalog_revision: load_outcome.persisted_catalog_revision,
    });
    app.insert_resource(EntityRegistryResource {
        entries,
        revision: 1,
        script_path: ENTITY_REGISTRY_SCRIPT_REL_PATH.to_string(),
    });
    app.insert_resource(AssetRegistryResource {
        entries: asset_entries,
        revision: 1,
        script_path: ASSET_REGISTRY_SCRIPT_REL_PATH.to_string(),
    });
    app.insert_resource(planet_registry);
    if let Some(mut generated_registry) = app
        .world_mut()
        .get_resource_mut::<GeneratedComponentRegistry>()
    {
        generated_registry.shader_entries = shader_entries;
    } else {
        bevy::log::warn!(
            "GeneratedComponentRegistry missing during replication scripting init; inserting shader entries without inferred component schemas"
        );
        app.insert_resource(GeneratedComponentRegistry {
            entries: Vec::new(),
            shader_entries,
        });
    }
    app.insert_resource(AssetRegistrySyncState {
        registry_script_path: asset_registry_script_path,
        last_catalog_revision: 0,
    });
    app.insert_resource(PlanetRegistrySyncState {
        registry_script_path: planet_registry_script_path,
        last_catalog_revision: 0,
    });
    app.add_systems(
        Update,
        (
            reload_all_scripts_from_disk_system,
            normalize_script_catalog_resource_system,
            persist_script_catalog_resource_system,
            sync_entity_registry_resource_system,
            sync_asset_registry_resource_system,
            sync_planet_registry_resource_system,
        )
            .chain(),
    );
}

pub fn load_entity_registry_entries(root: &Path) -> Result<Vec<EntityRegistryEntry>, String> {
    let catalog = script_catalog_from_disk(root)?;
    load_entity_registry_entries_from_catalog(&catalog)
}

pub fn load_script_catalog_entries_from_disk(
    root: &Path,
) -> Result<Vec<ScriptCatalogEntry>, String> {
    let mut entries = Vec::new();
    load_script_catalog_entries_from_disk_recursive(root, root, &mut entries)?;
    if entries.is_empty() {
        return Err(format!("{}: no .lua scripts discovered", root.display()));
    }
    entries.sort_by(|a, b| a.script_path.cmp(&b.script_path));
    Ok(entries)
}

fn assign_initial_entry_revisions(entries: &mut [ScriptCatalogEntry]) {
    for (idx, entry) in entries.iter_mut().enumerate() {
        if entry.revision == 0 {
            entry.revision = (idx as u64) + 1;
        }
    }
}

fn load_script_catalog_entries_from_disk_recursive(
    root: &Path,
    current_dir: &Path,
    out: &mut Vec<ScriptCatalogEntry>,
) -> Result<(), String> {
    let read_dir = std::fs::read_dir(current_dir)
        .map_err(|err| format!("read {} failed: {err}", current_dir.display()))?;
    for entry in read_dir {
        let entry = entry.map_err(|err| format!("read {} failed: {err}", current_dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            load_script_catalog_entries_from_disk_recursive(root, &path, out)?;
            continue;
        }
        if path.extension().and_then(|v| v.to_str()) != Some("lua") {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|err| format!("strip prefix {} failed: {err}", path.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(&path)
            .map_err(|err| format!("read {} failed: {err}", path.display()))?;
        out.push(ScriptCatalogEntry {
            script_path: relative,
            source,
            revision: 0,
            origin: "disk".to_string(),
        });
    }
    Ok(())
}

pub fn initialize_catalog_revisions(
    entries: &[ScriptCatalogEntry],
) -> (u64, u64, HashMap<String, u64>) {
    let mut fingerprints_by_path = HashMap::new();
    let mut next_revision = 1_u64;
    for entry in entries {
        fingerprints_by_path.insert(entry.script_path.clone(), catalog_entry_fingerprint(entry));
        next_revision = next_revision.max(entry.revision.saturating_add(1));
    }
    (1, next_revision.max(2), fingerprints_by_path)
}

fn replication_database_url() -> String {
    std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
}

fn persisted_record_from_entry(entry: &ScriptCatalogEntry) -> ScriptCatalogRecord {
    ScriptCatalogRecord {
        script_path: entry.script_path.clone(),
        source: entry.source.clone(),
        revision: entry.revision,
        origin: entry.origin.clone(),
        family: infer_script_family(&entry.script_path),
    }
}

fn catalog_entry_from_persisted(record: ScriptCatalogRecord) -> ScriptCatalogEntry {
    ScriptCatalogEntry {
        script_path: record.script_path,
        source: record.source,
        revision: record.revision,
        origin: record.origin,
    }
}

fn load_script_catalog_from_database_or_disk(
    root: &Path,
) -> Result<ScriptCatalogLoadOutcome, String> {
    load_script_catalog_from_database_or_disk_with_url(root, &replication_database_url())
}

fn load_script_catalog_from_database_or_disk_with_url(
    root: &Path,
    database_url: &str,
) -> Result<ScriptCatalogLoadOutcome, String> {
    let authoritative_load = (|| -> Result<ScriptCatalogLoadOutcome, String> {
        let mut client = postgres::Client::connect(database_url, postgres::NoTls)
            .map_err(|err| format!("script catalog postgres connect failed: {err}"))?;
        ensure_script_catalog_schema(&mut client)
            .map_err(|err| format!("ensure script catalog schema failed: {err}"))?;
        let persisted = load_active_script_catalog(&mut client)
            .map_err(|err| format!("load active script catalog failed: {err}"))?;
        if !persisted.is_empty() {
            let mut entries = persisted
                .into_iter()
                .map(catalog_entry_from_persisted)
                .collect::<Vec<_>>();
            entries.sort_by(|a, b| a.script_path.cmp(&b.script_path));
            return Ok(ScriptCatalogLoadOutcome {
                catalog: ScriptCatalogResource {
                    entries,
                    revision: 1,
                    root_dir: root.display().to_string(),
                },
                persisted_catalog_revision: 1,
                startup_loaded_from_disk_fallback: false,
                startup_status_message: "script catalog loaded from active database revisions"
                    .to_string(),
            });
        }
        let mut entries = load_script_catalog_entries_from_disk(root)?;
        assign_initial_entry_revisions(&mut entries);
        entries.sort_by(|a, b| a.script_path.cmp(&b.script_path));
        let persisted_records = entries
            .iter()
            .map(persisted_record_from_entry)
            .collect::<Vec<_>>();
        replace_active_script_catalog(&mut client, &persisted_records)
            .map_err(|err| format!("seed active script catalog failed: {err}"))?;
        Ok(ScriptCatalogLoadOutcome {
            catalog: ScriptCatalogResource {
                entries,
                revision: 1,
                root_dir: root.display().to_string(),
            },
            persisted_catalog_revision: 1,
            startup_loaded_from_disk_fallback: false,
            startup_status_message: "script catalog seeded from disk into empty database"
                .to_string(),
        })
    })();
    match authoritative_load {
        Ok(outcome) => Ok(outcome),
        Err(err) => {
            let disk_catalog = script_catalog_from_disk(root)
                .map_err(|disk_err| format!("{err}; disk fallback failed: {disk_err}"))?;
            Ok(ScriptCatalogLoadOutcome {
                catalog: disk_catalog,
                persisted_catalog_revision: 0,
                startup_loaded_from_disk_fallback: true,
                startup_status_message: format!(
                    "script catalog booted from disk because authoritative load failed: {err}"
                ),
            })
        }
    }
}

fn catalog_entry_fingerprint(entry: &ScriptCatalogEntry) -> u64 {
    let mut hasher = DefaultHasher::new();
    entry.script_path.hash(&mut hasher);
    entry.source.hash(&mut hasher);
    entry.origin.hash(&mut hasher);
    hasher.finish()
}

pub fn lookup_script_catalog_entry<'a>(
    catalog: &'a ScriptCatalogResource,
    script_path: &str,
) -> Result<&'a ScriptCatalogEntry, String> {
    catalog
        .entries
        .iter()
        .find(|entry| entry.script_path == script_path)
        .ok_or_else(|| format!("script catalog missing script_path={script_path}"))
}

pub fn load_entity_registry_entries_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<Vec<EntityRegistryEntry>, String> {
    let policy = LuaSandboxPolicy::from_env();
    let entry = lookup_script_catalog_entry(catalog, ENTITY_REGISTRY_SCRIPT_REL_PATH)?;
    let module = load_lua_module_from_source(
        &entry.source,
        Path::new(ENTITY_REGISTRY_SCRIPT_REL_PATH),
        &policy,
    )
    .map_err(map_script_err)?;
    let bundles = module
        .root()
        .get::<Table>("bundles")
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    let mut out = Vec::<EntityRegistryEntry>::new();
    for pair in bundles.pairs::<String, Table>() {
        let (entity_id, entity_table) =
            pair.map_err(|err| format!("{}: {err}", module.script_path().display()))?;
        let entity_class = table_get_required_string(&entity_table, "bundle_class", &entity_id)
            .map_err(map_script_err)?;
        let graph_records_script =
            table_get_required_string(&entity_table, "graph_records_script", &entity_id)
                .map_err(map_script_err)?;
        let required_component_kinds =
            table_get_required_string_list(&entity_table, "required_component_kinds", &entity_id)
                .map_err(map_script_err)?;
        out.push(EntityRegistryEntry {
            entity_id,
            entity_class,
            graph_records_script,
            required_component_kinds,
        });
    }
    if out.is_empty() {
        return Err(format!(
            "{}: bundles table must not be empty",
            ENTITY_REGISTRY_SCRIPT_REL_PATH
        ));
    }
    out.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
    Ok(out)
}

pub fn load_asset_registry_entries(root: &Path) -> Result<Vec<AssetRegistryEntry>, String> {
    let catalog = script_catalog_from_disk(root)?;
    load_asset_registry_entries_from_catalog(&catalog)
}

pub fn load_asset_registry_entries_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<Vec<AssetRegistryEntry>, String> {
    load_asset_registry_data_from_catalog(catalog).map(|(entries, _)| entries)
}

pub fn load_planet_registry_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<sidereal_game::PlanetRegistry, String> {
    let registry_entry = lookup_script_catalog_entry(catalog, PLANET_REGISTRY_SCRIPT_REL_PATH)?;
    let sources_by_script_path = catalog
        .entries
        .iter()
        .map(|entry| (entry.script_path.clone(), entry.source.clone()))
        .collect::<HashMap<_, _>>();
    load_planet_registry_from_sources(
        &registry_entry.source,
        Path::new(PLANET_REGISTRY_SCRIPT_REL_PATH),
        &sources_by_script_path,
    )
    .map_err(map_script_err)
}

fn load_asset_registry_data_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<
    (
        Vec<AssetRegistryEntry>,
        Vec<sidereal_game::ShaderEditorRegistryEntry>,
    ),
    String,
> {
    let entry = lookup_script_catalog_entry(catalog, ASSET_REGISTRY_SCRIPT_REL_PATH)?;
    let registry =
        load_asset_registry_from_source(&entry.source, Path::new(ASSET_REGISTRY_SCRIPT_REL_PATH))
            .map_err(map_script_err)?;
    let mut assets = registry
        .assets
        .iter()
        .map(map_script_asset_registry_entry)
        .collect::<Vec<_>>();
    assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    let shader_entries = shader_registry_entries_from_script_assets(&registry.assets)?;
    Ok((assets, shader_entries))
}

fn map_script_asset_registry_entry(entry: &ScriptAssetRegistryEntry) -> AssetRegistryEntry {
    AssetRegistryEntry {
        asset_id: entry.asset_id.clone(),
        shader_family: entry.shader_family.clone(),
        source_path: entry.source_path.clone(),
        content_type: entry.content_type.clone(),
        dependencies: entry.dependencies.clone(),
        bootstrap_required: entry.bootstrap_required,
    }
}

fn shader_registry_entries_from_script_assets(
    asset_entries: &[ScriptAssetRegistryEntry],
) -> Result<Vec<sidereal_game::ShaderEditorRegistryEntry>, String> {
    let mut entries = asset_entries
        .iter()
        .filter(|entry| entry.source_path.ends_with(".wgsl"))
        .map(map_script_shader_registry_entry)
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    Ok(entries)
}

fn map_script_shader_registry_entry(
    entry: &ScriptAssetRegistryEntry,
) -> Result<sidereal_game::ShaderEditorRegistryEntry, String> {
    let uniform_schema = entry
        .editor_schema
        .as_ref()
        .map(|schema| {
            schema
                .uniforms
                .iter()
                .map(|field| {
                    Ok(sidereal_game::ShaderEditorFieldSchema {
                        field_path: field.field_path.clone(),
                        display_name: field
                            .label
                            .clone()
                            .unwrap_or_else(|| field.field_path.clone()),
                        description: field.description.clone(),
                        value_kind: shader_editor_value_kind(&field.kind)?,
                        min: field.min,
                        max: field.max,
                        step: field.step,
                        options: field
                            .options
                            .iter()
                            .map(|option| sidereal_game::ShaderEditorOption {
                                value: option.value.clone(),
                                label: option.label.clone(),
                            })
                            .collect(),
                        default_value_json: field
                            .default_value
                            .as_ref()
                            .map(serde_json::to_string)
                            .transpose()
                            .map_err(|err| {
                                format!(
                                    "asset {} uniform {} default json serialize failed: {}",
                                    entry.asset_id, field.field_path, err
                                )
                            })?,
                        group: field.group.clone(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    let presets = entry
        .editor_schema
        .as_ref()
        .map(|schema| {
            schema
                .presets
                .iter()
                .map(|preset| {
                    Ok(sidereal_game::ShaderEditorPreset {
                        preset_id: preset.preset_id.clone(),
                        display_name: preset.label.clone(),
                        description: preset.description.clone(),
                        values_json: serde_json::to_string(&preset.values).map_err(|err| {
                            format!(
                                "asset {} preset {} json serialize failed: {}",
                                entry.asset_id, preset.preset_id, err
                            )
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(sidereal_game::ShaderEditorRegistryEntry {
        asset_id: entry.asset_id.clone(),
        source_path: entry.source_path.clone(),
        shader_family: entry.shader_family.clone(),
        dependencies: entry.dependencies.clone(),
        bootstrap_required: entry.bootstrap_required,
        uniform_schema,
        presets,
    })
}

fn shader_editor_value_kind(kind: &str) -> Result<sidereal_game::ComponentEditorValueKind, String> {
    match kind {
        "Bool" => Ok(sidereal_game::ComponentEditorValueKind::Bool),
        "SignedInteger" => Ok(sidereal_game::ComponentEditorValueKind::SignedInteger),
        "UnsignedInteger" => Ok(sidereal_game::ComponentEditorValueKind::UnsignedInteger),
        "Float" => Ok(sidereal_game::ComponentEditorValueKind::Float),
        "String" => Ok(sidereal_game::ComponentEditorValueKind::String),
        "Vec2" => Ok(sidereal_game::ComponentEditorValueKind::Vec2),
        "Vec3" => Ok(sidereal_game::ComponentEditorValueKind::Vec3),
        "Vec4" => Ok(sidereal_game::ComponentEditorValueKind::Vec4),
        "ColorRgb" => Ok(sidereal_game::ComponentEditorValueKind::ColorRgb),
        "ColorRgba" => Ok(sidereal_game::ComponentEditorValueKind::ColorRgba),
        "Enum" => Ok(sidereal_game::ComponentEditorValueKind::Enum),
        "Sequence" => Ok(sidereal_game::ComponentEditorValueKind::Sequence),
        "Struct" => Ok(sidereal_game::ComponentEditorValueKind::Struct),
        "Tuple" => Ok(sidereal_game::ComponentEditorValueKind::Tuple),
        "Unknown" => Ok(sidereal_game::ComponentEditorValueKind::Unknown),
        other => Err(format!("unsupported shader editor value kind={other}")),
    }
}

fn reload_all_scripts_from_disk_system(
    time: Res<'_, Time>,
    mut control: ResMut<'_, ScriptCatalogControlResource>,
    mut catalog: ResMut<'_, ScriptCatalogResource>,
    mut sync_state: ResMut<'_, ScriptCatalogSyncState>,
) {
    if !control.reload_all_from_disk_requested {
        return;
    }
    let root = PathBuf::from(&catalog.root_dir);
    match load_script_catalog_entries_from_disk(&root) {
        Ok(entries) => {
            catalog.entries = entries;
            catalog.revision = catalog.revision.saturating_add(1);
            sync_state.fingerprints_by_path.clear();
            sync_state.next_revision = sync_state
                .next_revision
                .max(catalog.revision.saturating_add(1));
            control.reload_all_from_disk_requested = false;
            control.last_reload_succeeded = true;
            control.last_reload_message =
                format!("reloaded {} scripts from disk", catalog.entries.len());
            control.last_reload_at_s = time.elapsed_secs_f64();
            bevy::log::info!(
                "replication script catalog reloaded from disk root={} count={} revision={}",
                catalog.root_dir,
                catalog.entries.len(),
                catalog.revision
            );
        }
        Err(err) => {
            control.reload_all_from_disk_requested = false;
            control.last_reload_succeeded = false;
            control.last_reload_message = err.clone();
            control.last_reload_at_s = time.elapsed_secs_f64();
            bevy::log::warn!(
                "replication script catalog reload from disk failed: {}",
                err
            );
        }
    }
}

fn normalize_script_catalog_resource_system(
    mut catalog: ResMut<'_, ScriptCatalogResource>,
    mut sync_state: ResMut<'_, ScriptCatalogSyncState>,
) {
    let mut changed = false;
    catalog
        .entries
        .sort_by(|a, b| a.script_path.cmp(&b.script_path));
    let mut next_fingerprints = HashMap::new();
    for entry in &mut catalog.entries {
        let fingerprint = catalog_entry_fingerprint(entry);
        next_fingerprints.insert(entry.script_path.clone(), fingerprint);
        let known = sync_state
            .fingerprints_by_path
            .get(&entry.script_path)
            .copied();
        if known != Some(fingerprint) || entry.revision == 0 {
            entry.revision = sync_state.next_revision;
            sync_state.next_revision = sync_state.next_revision.saturating_add(1);
            changed = true;
        }
    }
    if next_fingerprints.len() != sync_state.fingerprints_by_path.len() {
        changed = true;
    }
    if changed {
        catalog.revision = sync_state.next_revision;
        sync_state.next_revision = sync_state.next_revision.saturating_add(1);
        sync_state.fingerprints_by_path = next_fingerprints;
    }
}

fn persist_script_catalog_resource_system(
    time: Res<'_, Time>,
    catalog: Res<'_, ScriptCatalogResource>,
    mut control: ResMut<'_, ScriptCatalogControlResource>,
    mut persistence_state: ResMut<'_, ScriptCatalogPersistenceState>,
) {
    if persistence_state.last_persisted_catalog_revision == catalog.revision {
        return;
    }
    let database_url = replication_database_url();
    let mut client = match postgres::Client::connect(&database_url, postgres::NoTls) {
        Ok(client) => client,
        Err(err) => {
            control.last_persist_succeeded = false;
            control.last_persist_message = format!("postgres connect failed: {err}");
            control.last_persist_at_s = time.elapsed_secs_f64();
            bevy::log::warn!(
                "replication script catalog persist connect failed revision={}: {}",
                catalog.revision,
                err
            );
            return;
        }
    };
    let records = catalog
        .entries
        .iter()
        .map(persisted_record_from_entry)
        .collect::<Vec<_>>();
    match replace_active_script_catalog(&mut client, &records) {
        Ok(()) => {
            persistence_state.last_persisted_catalog_revision = catalog.revision;
            control.last_persist_succeeded = true;
            control.last_persist_message = format!(
                "persisted {} scripts to database at catalog_revision={}",
                catalog.entries.len(),
                catalog.revision
            );
            control.last_persist_at_s = time.elapsed_secs_f64();
            bevy::log::info!(
                "replication persisted script catalog to database count={} catalog_revision={}",
                catalog.entries.len(),
                catalog.revision
            );
        }
        Err(err) => {
            control.last_persist_succeeded = false;
            control.last_persist_message = err.to_string();
            control.last_persist_at_s = time.elapsed_secs_f64();
            bevy::log::warn!(
                "replication script catalog persist failed revision={}: {}",
                catalog.revision,
                err
            );
        }
    }
}

fn sync_entity_registry_resource_system(
    catalog: Res<'_, ScriptCatalogResource>,
    mut last_catalog_revision: Local<'_, u64>,
    mut registry: ResMut<'_, EntityRegistryResource>,
) {
    if *last_catalog_revision == catalog.revision {
        return;
    }
    *last_catalog_revision = catalog.revision;
    match load_entity_registry_entries_from_catalog(&catalog) {
        Ok(entries) => {
            registry.entries = entries;
            registry.revision = registry.revision.saturating_add(1);
            bevy::log::info!(
                "replication entity registry reloaded from script catalog script={} catalog_revision={} revision={} entries={}",
                ENTITY_REGISTRY_SCRIPT_REL_PATH,
                catalog.revision,
                registry.revision,
                registry.entries.len()
            );
        }
        Err(err) => {
            bevy::log::warn!(
                "replication entity registry reload failed script={}: {}",
                ENTITY_REGISTRY_SCRIPT_REL_PATH,
                err
            );
        }
    }
}

fn sync_asset_registry_resource_system(
    catalog: Res<'_, ScriptCatalogResource>,
    mut sync_state: ResMut<'_, AssetRegistrySyncState>,
    mut registry: ResMut<'_, AssetRegistryResource>,
    generated_registry: Option<ResMut<'_, sidereal_game::GeneratedComponentRegistry>>,
) {
    if sync_state.last_catalog_revision == catalog.revision {
        return;
    }
    sync_state.last_catalog_revision = catalog.revision;
    match load_asset_registry_data_from_catalog(&catalog) {
        Ok((entries, shader_entries)) => {
            registry.entries = entries;
            registry.revision = registry.revision.saturating_add(1);
            if let Some(mut generated_registry) = generated_registry {
                generated_registry.shader_entries = shader_entries;
            }
            bevy::log::info!(
                "replication asset registry reloaded from script catalog script={} catalog_revision={} revision={} entries={}",
                sync_state.registry_script_path.display(),
                catalog.revision,
                registry.revision,
                registry.entries.len()
            );
        }
        Err(err) => {
            bevy::log::warn!(
                "replication asset registry reload failed script={}: {}",
                sync_state.registry_script_path.display(),
                err
            );
        }
    }
}

fn sync_planet_registry_resource_system(
    catalog: Res<'_, ScriptCatalogResource>,
    mut sync_state: ResMut<'_, PlanetRegistrySyncState>,
    mut registry: ResMut<'_, sidereal_game::PlanetRegistry>,
) {
    if sync_state.last_catalog_revision == catalog.revision {
        return;
    }
    sync_state.last_catalog_revision = catalog.revision;
    match load_planet_registry_from_catalog(&catalog) {
        Ok(next_registry) => {
            *registry = next_registry;
            bevy::log::info!(
                "replication planet registry reloaded from script catalog script={} catalog_revision={} entries={}",
                sync_state.registry_script_path.display(),
                catalog.revision,
                registry.entries.len()
            );
        }
        Err(err) => {
            bevy::log::warn!(
                "replication planet registry reload failed script={}: {}",
                sync_state.registry_script_path.display(),
                err
            );
        }
    }
}

pub fn script_catalog_from_disk(root: &Path) -> Result<ScriptCatalogResource, String> {
    let mut entries = load_script_catalog_entries_from_disk(root)?;
    assign_initial_entry_revisions(&mut entries);
    Ok(ScriptCatalogResource {
        entries,
        revision: 1,
        root_dir: root.display().to_string(),
    })
}

#[allow(dead_code)]
pub fn load_world_init_config_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<WorldInitScriptConfig, String> {
    let policy = LuaSandboxPolicy::from_env();
    let entry = lookup_script_catalog_entry(catalog, WORLD_INIT_SCRIPT_REL_PATH)?;
    load_world_init_config_from_source(&entry.source, &policy)
        .map_err(map_script_err)
    .inspect(|config| {
        bevy::log::info!(
            "replication loaded world init config: render_layer_shader_asset_ids={:?} additional_required_asset_ids={}",
            config.render_layer_shader_asset_ids,
            config.additional_required_asset_ids.len()
        );
    })
}

#[allow(dead_code)]
pub fn load_world_init_graph_records(root: &Path) -> Result<Vec<GraphEntityRecord>, String> {
    let catalog = script_catalog_from_disk(root)?;
    let entity_entries = load_entity_registry_entries_from_catalog(&catalog)?;
    let asset_entries = load_asset_registry_entries_from_catalog(&catalog)?;
    load_world_init_graph_records_from_catalog(&catalog, &entity_entries, &asset_entries)
}

pub fn load_world_init_graph_records_from_catalog(
    catalog: &ScriptCatalogResource,
    entity_entries: &[EntityRegistryEntry],
    asset_entries: &[AssetRegistryEntry],
) -> Result<Vec<GraphEntityRecord>, String> {
    let policy = LuaSandboxPolicy::from_env();
    let entry = lookup_script_catalog_entry(catalog, WORLD_INIT_SCRIPT_REL_PATH)?;
    let module =
        load_lua_module_from_source(&entry.source, Path::new("world/world_init.lua"), &policy)
            .map_err(map_script_err)?;
    let build_graph_records = module
        .root()
        .get::<Function>("build_graph_records")
        .map_err(|err| {
            format!(
                "{}: missing build_graph_records(ctx): {err}",
                module.script_path().display()
            )
        })?;
    let ctx = module
        .root()
        .get::<Table>("context")
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    inject_world_init_context_cached(
        ctx.clone(),
        &module,
        Path::new(&catalog.root_dir),
        Arc::new(entity_entries.to_vec()),
        Arc::new(asset_entries.to_vec()),
        Arc::new(catalog.clone()),
    )?;

    let lua_value = build_graph_records
        .call::<Value>(ctx)
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    let json_value = lua_value_to_json(lua_value).map_err(map_script_err)?;
    let records =
        decode_graph_entity_records(module.script_path(), json_value).map_err(map_script_err)?;
    if records.is_empty() {
        return Err(format!(
            "{}: build_graph_records(ctx) returned empty records",
            module.script_path().display()
        ));
    }
    validate_runtime_render_graph_records(&records)
        .map_err(map_script_err)
        .map_err(|err| format!("{}: {}", module.script_path().display(), err))?;
    Ok(records)
}

#[allow(dead_code)]
pub fn spawn_bundle_graph_records(
    root: &Path,
    bundle_id: &str,
    overrides: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<GraphEntityRecord>, String> {
    let script_catalog = script_catalog_from_disk(root)?;
    let entity_entries = load_entity_registry_entries(root)?;
    let asset_entries = load_asset_registry_entries(root)?;
    spawn_bundle_graph_records_from_entries(
        root,
        &script_catalog,
        &entity_entries,
        &asset_entries,
        bundle_id,
        overrides,
    )
}

pub fn spawn_bundle_graph_records_cached(
    root: &Path,
    script_catalog: &ScriptCatalogResource,
    entity_registry: &EntityRegistryResource,
    asset_registry: &AssetRegistryResource,
    bundle_id: &str,
    overrides: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<GraphEntityRecord>, String> {
    spawn_bundle_graph_records_from_entries(
        root,
        script_catalog,
        &entity_registry.entries,
        &asset_registry.entries,
        bundle_id,
        overrides,
    )
}

fn spawn_bundle_graph_records_from_entries(
    root: &Path,
    script_catalog: &ScriptCatalogResource,
    entity_entries: &[EntityRegistryEntry],
    asset_entries: &[AssetRegistryEntry],
    bundle_id: &str,
    overrides: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<GraphEntityRecord>, String> {
    let policy = LuaSandboxPolicy::from_env();
    let script_rel_path =
        resolve_bundle_graph_records_script_from_entries(entity_entries, bundle_id)?;
    let script_entry = lookup_script_catalog_entry(script_catalog, script_rel_path.as_str())?;
    let module = load_lua_module_from_source(
        &script_entry.source,
        Path::new(script_rel_path.as_str()),
        &policy,
    )
    .map_err(map_script_err)?;
    let build_graph_records = module
        .root()
        .get::<Function>("build_graph_records")
        .map_err(|err| {
            format!(
                "{}: missing build_graph_records(ctx): {err}",
                module.script_path().display()
            )
        })?;
    let bundle_ctx = module
        .lua()
        .create_table()
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    bundle_ctx
        .set("bundle_id", bundle_id)
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    for (key, value) in overrides {
        let lua_value = json_value_to_lua(module.lua(), value)?;
        bundle_ctx
            .set(key.as_str(), lua_value)
            .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    }
    let new_uuid = module
        .lua()
        .create_function(|_, ()| Ok(Uuid::new_v4().to_string()))
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    bundle_ctx
        .set("new_uuid", new_uuid)
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    inject_script_logger(
        module.lua(),
        &bundle_ctx,
        &module.script_path().display().to_string(),
    )
    .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    inject_spawn_bundle_graph_records_fn_cached(
        bundle_ctx.clone(),
        module.lua(),
        root,
        Arc::new(entity_entries.to_vec()),
        Arc::new(asset_entries.to_vec()),
        Arc::new(script_catalog.clone()),
    )
    .map_err(|err| format!("{}: {err}", module.script_path().display()))?;

    let lua_value = build_graph_records
        .call::<Value>(bundle_ctx)
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    let json_value = lua_value_to_json(lua_value).map_err(map_script_err)?;
    let records =
        decode_graph_entity_records(module.script_path(), json_value).map_err(map_script_err)?;
    if records.is_empty() {
        return Err(format!(
            "{}: build_graph_records(ctx) returned empty records",
            module.script_path().display()
        ));
    }
    validate_runtime_render_graph_records(&records)
        .map_err(map_script_err)
        .map_err(|err| format!("{}: {}", module.script_path().display(), err))?;
    Ok(records)
}

#[allow(dead_code)]
pub fn emit_bundle_spawned_event(
    root: &Path,
    event: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let catalog = script_catalog_from_disk(root)?;
    emit_bundle_spawned_event_from_catalog(&catalog, event)
}

pub fn emit_bundle_spawned_event_from_catalog(
    catalog: &ScriptCatalogResource,
    event: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let policy = LuaSandboxPolicy::from_env();
    let entry = match lookup_script_catalog_entry(catalog, "bundles/entity_registry.lua") {
        Ok(entry) => entry,
        Err(_) => return Ok(()),
    };
    let module = load_lua_module_from_source(
        &entry.source,
        Path::new("bundles/entity_registry.lua"),
        &policy,
    )
    .map_err(map_script_err)?;
    let Ok(on_spawned) = module.root().get::<Function>("on_spawned") else {
        return Ok(());
    };
    let ctx = module
        .lua()
        .create_table()
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    let event_table = module
        .lua()
        .create_table()
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    for (key, value) in event {
        let lua_value = json_value_to_lua(module.lua(), value)?;
        event_table
            .set(key.as_str(), lua_value)
            .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    }
    on_spawned.call::<()>((ctx, event_table)).map_err(|err| {
        format!(
            "{}: on_spawned(ctx, event) failed: {err}",
            module.script_path().display()
        )
    })
}

#[allow(dead_code)]
pub fn load_known_bundle_ids(root: &Path) -> Result<HashSet<String>, String> {
    let entries = load_entity_registry_entries(root)?;
    Ok(entries.into_iter().map(|entry| entry.entity_id).collect())
}

fn map_script_err(err: ScriptError) -> String {
    err.to_string()
}

fn json_value_to_lua(lua: &Lua, value: &serde_json::Value) -> Result<Value, String> {
    match value {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(v) => Ok(Value::Boolean(*v)),
        serde_json::Value::Number(v) => {
            if let Some(i) = v.as_i64() {
                Ok(Value::Integer(i))
            } else if let Some(u) = v.as_u64() {
                if u <= i64::MAX as u64 {
                    Ok(Value::Integer(u as i64))
                } else {
                    Ok(Value::Number(u as f64))
                }
            } else {
                let Some(f) = v.as_f64() else {
                    return Err("json number could not convert to f64".to_string());
                };
                Ok(Value::Number(f))
            }
        }
        serde_json::Value::String(v) => lua
            .create_string(v.as_str())
            .map(Value::String)
            .map_err(|err| err.to_string()),
        serde_json::Value::Array(values) => {
            let table = lua.create_table().map_err(|err| err.to_string())?;
            for (idx, value) in values.iter().enumerate() {
                table
                    .set(idx + 1, json_value_to_lua(lua, value)?)
                    .map_err(|err| err.to_string())?;
            }
            Ok(Value::Table(table))
        }
        serde_json::Value::Object(values) => {
            let table = lua.create_table().map_err(|err| err.to_string())?;
            for (key, value) in values {
                table
                    .set(key.as_str(), json_value_to_lua(lua, value)?)
                    .map_err(|err| err.to_string())?;
            }
            Ok(Value::Table(table))
        }
    }
}

#[allow(dead_code)]
fn inject_world_init_context(
    ctx: Table,
    module: &sidereal_scripting::LoadedLuaModule,
    scripts_root: &Path,
) -> Result<(), String> {
    let script_catalog = Arc::new(script_catalog_from_disk(scripts_root)?);
    let entity_entries = Arc::new(load_entity_registry_entries(scripts_root)?);
    let asset_entries = Arc::new(load_asset_registry_entries(scripts_root)?);
    inject_world_init_context_cached(
        ctx,
        module,
        scripts_root,
        entity_entries,
        asset_entries,
        script_catalog,
    )
}

fn inject_world_init_context_cached(
    ctx: Table,
    module: &sidereal_scripting::LoadedLuaModule,
    scripts_root: &Path,
    entity_entries: Arc<Vec<EntityRegistryEntry>>,
    asset_entries: Arc<Vec<AssetRegistryEntry>>,
    script_catalog: Arc<ScriptCatalogResource>,
) -> Result<(), String> {
    let new_uuid = module
        .lua()
        .create_function(|_, ()| Ok(Uuid::new_v4().to_string()))
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    ctx.set("new_uuid", new_uuid)
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    inject_script_logger(
        module.lua(),
        &ctx,
        &module.script_path().display().to_string(),
    )
    .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    inject_generate_collision_outline_fn_cached(
        ctx.clone(),
        module.lua(),
        scripts_root,
        asset_entries.clone(),
    )
    .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    inject_spawn_bundle_graph_records_fn_cached(
        ctx.clone(),
        module.lua(),
        scripts_root,
        entity_entries,
        asset_entries,
        script_catalog.clone(),
    )
    .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    inject_load_planet_definitions_fn(ctx.clone(), module.lua(), script_catalog)
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    inject_render_authoring_api(module.lua(), ctx)
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    Ok(())
}

#[allow(dead_code)]
fn resolve_bundle_graph_records_script(root: &Path, bundle_id: &str) -> Result<String, String> {
    let entries = load_entity_registry_entries(root)?;
    resolve_bundle_graph_records_script_from_entries(&entries, bundle_id)
}

fn resolve_bundle_graph_records_script_from_entries(
    entries: &[EntityRegistryEntry],
    bundle_id: &str,
) -> Result<String, String> {
    entries
        .iter()
        .find(|entry| entry.entity_id == bundle_id)
        .map(|entry| entry.graph_records_script.clone())
        .ok_or_else(|| format!("{ENTITY_REGISTRY_SCRIPT_REL_PATH}: unknown bundle_id={bundle_id}"))
}

#[allow(dead_code)]
fn inject_spawn_bundle_graph_records_fn(
    ctx: Table,
    lua: &Lua,
    scripts_root: &Path,
) -> mlua::Result<()> {
    let script_catalog =
        Arc::new(script_catalog_from_disk(scripts_root).map_err(mlua::Error::runtime)?);
    let entity_entries =
        Arc::new(load_entity_registry_entries(scripts_root).map_err(mlua::Error::runtime)?);
    let asset_entries =
        Arc::new(load_asset_registry_entries(scripts_root).map_err(mlua::Error::runtime)?);
    inject_spawn_bundle_graph_records_fn_cached(
        ctx,
        lua,
        scripts_root,
        entity_entries,
        asset_entries,
        script_catalog,
    )
}

fn inject_load_planet_definitions_fn(
    ctx: Table,
    lua: &Lua,
    script_catalog: Arc<ScriptCatalogResource>,
) -> mlua::Result<()> {
    let load_planet_definitions = lua.create_function(move |lua, ()| {
        let registry = load_planet_registry_from_catalog(script_catalog.as_ref())
            .map_err(mlua::Error::runtime)?;
        let spawn_enabled_by_planet_id = registry
            .entries
            .iter()
            .map(|entry| (entry.planet_id.as_str(), entry.spawn_enabled))
            .collect::<HashMap<_, _>>();
        let definitions_json = registry
            .definitions
            .iter()
            .map(|definition| {
                let mut value = serde_json::to_value(definition).map_err(|err| err.to_string())?;
                if let Some(object) = value.as_object_mut() {
                    object.insert(
                        "spawn_enabled".to_string(),
                        serde_json::Value::Bool(
                            spawn_enabled_by_planet_id
                                .get(definition.planet_id.as_str())
                                .copied()
                                .unwrap_or(false),
                        ),
                    );
                }
                Ok(value)
            })
            .collect::<Result<Vec<_>, String>>()
            .map_err(mlua::Error::runtime)?;
        json_value_to_lua(lua, &serde_json::Value::Array(definitions_json))
            .map_err(mlua::Error::runtime)
    })?;
    ctx.set("load_planet_definitions", load_planet_definitions)?;
    Ok(())
}

fn inject_spawn_bundle_graph_records_fn_cached(
    ctx: Table,
    lua: &Lua,
    scripts_root: &Path,
    entity_entries: Arc<Vec<EntityRegistryEntry>>,
    asset_entries: Arc<Vec<AssetRegistryEntry>>,
    script_catalog: Arc<ScriptCatalogResource>,
) -> mlua::Result<()> {
    inject_generate_collision_outline_fn_cached(
        ctx.clone(),
        lua,
        scripts_root,
        asset_entries.clone(),
    )?;
    let scripts_root = scripts_root.to_path_buf();
    let entity_entries_for_spawn = entity_entries.clone();
    let asset_entries_for_spawn = asset_entries.clone();
    let script_catalog_for_spawn = script_catalog.clone();
    let spawn_bundle_graph_records =
        lua.create_function(move |lua, (bundle_id, overrides): (String, Value)| {
            let overrides_json = match overrides {
                Value::Nil => serde_json::Map::new(),
                Value::Table(_) => {
                    let json = lua_value_to_json(overrides).map_err(mlua::Error::runtime)?;
                    let Some(map) = json.as_object() else {
                        return Err(mlua::Error::runtime(
                            "spawn_bundle_graph_records override payload must decode to an object",
                        ));
                    };
                    map.clone()
                }
                _ => {
                    return Err(mlua::Error::runtime(
                        "spawn_bundle_graph_records override payload must be a table or nil",
                    ));
                }
            };
            let records = spawn_bundle_graph_records_from_entries(
                &scripts_root,
                script_catalog_for_spawn.as_ref(),
                entity_entries_for_spawn.as_slice(),
                asset_entries_for_spawn.as_slice(),
                &bundle_id,
                &overrides_json,
            )
            .map_err(mlua::Error::runtime)?;
            let records_json = serde_json::to_value(records)
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            json_value_to_lua(lua, &records_json).map_err(mlua::Error::runtime)
        })?;
    ctx.set("spawn_bundle_graph_records", spawn_bundle_graph_records)?;
    inject_render_authoring_api(lua, ctx)?;
    Ok(())
}

fn inject_render_authoring_api(lua: &Lua, ctx: Table) -> mlua::Result<()> {
    let render = lua.create_table()?;

    let define_layer = lua.create_function(|lua, (_render, layer): (Table, Value)| {
        let layer_json = lua_value_to_json(layer).map_err(mlua::Error::runtime)?;
        let Some(layer_object) = layer_json.as_object() else {
            return Err(mlua::Error::runtime(
                "render.define_layer expects a table payload",
            ));
        };
        let mut layer_object = layer_object.clone();
        remove_empty_array_like_field(&mut layer_object, "texture_bindings");
        let entity_id = layer_object
            .get("entity_id")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let display_name = layer_object
            .get("display_name")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .or_else(|| {
                layer_object
                    .get("layer_id")
                    .and_then(|value| value.as_str())
                    .map(|value| format!("RenderLayer:{value}"))
            })
            .unwrap_or_else(|| "RenderLayer".to_string());
        let record = serde_json::json!({
            "entity_id": entity_id,
            "labels": ["Entity", "RenderLayerDefinition"],
            "properties": {},
            "components": [
                {
                    "component_id": format!("{entity_id}:display_name"),
                    "component_kind": "display_name",
                    "properties": display_name,
                },
                {
                    "component_id": format!("{entity_id}:runtime_render_layer_definition"),
                    "component_kind": "runtime_render_layer_definition",
                    "properties": layer_object,
                }
            ]
        });
        json_value_to_lua(lua, &record).map_err(mlua::Error::runtime)
    })?;
    render.set("define_layer", define_layer)?;

    let define_rule = lua.create_function(|lua, (_render, rule): (Table, Value)| {
        let rule_json = lua_value_to_json(rule).map_err(mlua::Error::runtime)?;
        let Some(rule_object) = rule_json.as_object() else {
            return Err(mlua::Error::runtime(
                "render.define_rule expects a table payload",
            ));
        };
        let mut rule_object = rule_object.clone();
        remove_empty_array_like_field(&mut rule_object, "labels_any");
        remove_empty_array_like_field(&mut rule_object, "labels_all");
        remove_empty_array_like_field(&mut rule_object, "archetypes_any");
        remove_empty_array_like_field(&mut rule_object, "components_all");
        remove_empty_array_like_field(&mut rule_object, "components_any");
        let entity_id = rule_object
            .get("entity_id")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let display_name = rule_object
            .get("display_name")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .or_else(|| {
                rule_object
                    .get("rule_id")
                    .and_then(|value| value.as_str())
                    .map(|value| format!("RenderRule:{value}"))
            })
            .unwrap_or_else(|| "RenderRule".to_string());
        let record = serde_json::json!({
            "entity_id": entity_id,
            "labels": ["Entity", "RenderLayerRule"],
            "properties": {},
            "components": [
                {
                    "component_id": format!("{entity_id}:display_name"),
                    "component_kind": "display_name",
                    "properties": display_name,
                },
                {
                    "component_id": format!("{entity_id}:runtime_render_layer_rule"),
                    "component_kind": "runtime_render_layer_rule",
                    "properties": rule_object,
                }
            ]
        });
        json_value_to_lua(lua, &record).map_err(mlua::Error::runtime)
    })?;
    render.set("define_rule", define_rule)?;

    let define_post_process_stack =
        lua.create_function(|lua, (_render, stack): (Table, Value)| {
            let stack_json = lua_value_to_json(stack).map_err(mlua::Error::runtime)?;
            let Some(stack_object) = stack_json.as_object() else {
                return Err(mlua::Error::runtime(
                    "render.define_post_process_stack expects a table payload",
                ));
            };
            let mut stack_object = stack_object.clone();
            if let Some(serde_json::Value::Array(passes)) = stack_object.get_mut("passes") {
                for pass in passes {
                    if let Some(pass_object) = pass.as_object_mut() {
                        remove_empty_array_like_field(pass_object, "texture_bindings");
                    }
                }
            }
            remove_empty_array_like_field(&mut stack_object, "passes");
            let entity_id = stack_object
                .get("entity_id")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let display_name = stack_object
                .get("display_name")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "PostProcessStack".to_string());
            let record = serde_json::json!({
                "entity_id": entity_id,
                "labels": ["Entity", "RuntimePostProcessStack"],
                "properties": {},
                "components": [
                    {
                        "component_id": format!("{entity_id}:display_name"),
                        "component_kind": "display_name",
                        "properties": display_name,
                    },
                    {
                        "component_id": format!("{entity_id}:runtime_post_process_stack"),
                        "component_kind": "runtime_post_process_stack",
                        "properties": stack_object,
                    }
                ]
            });
            json_value_to_lua(lua, &record).map_err(mlua::Error::runtime)
        })?;
    render.set("define_post_process_stack", define_post_process_stack)?;

    let define_world_visual_stack =
        lua.create_function(|lua, (_render, stack): (Table, Value)| {
            let stack_json = lua_value_to_json(stack).map_err(mlua::Error::runtime)?;
            let Some(stack_object) = stack_json.as_object() else {
                return Err(mlua::Error::runtime(
                    "render.define_world_visual_stack expects a table payload",
                ));
            };
            let mut stack_object = stack_object.clone();
            if let Some(serde_json::Value::Array(passes)) = stack_object.get_mut("passes") {
                for pass in passes {
                    if let Some(pass_object) = pass.as_object_mut() {
                        remove_empty_array_like_field(pass_object, "texture_bindings");
                    }
                }
            }
            remove_empty_array_like_field(&mut stack_object, "passes");
            let entity_id = stack_object
                .get("entity_id")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let display_name = stack_object
                .get("display_name")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "WorldVisualStack".to_string());
            let record = serde_json::json!({
                "entity_id": entity_id,
                "labels": ["Entity", "RuntimeWorldVisualStack"],
                "properties": {},
                "components": [
                    {
                        "component_id": format!("{entity_id}:display_name"),
                        "component_kind": "display_name",
                        "properties": display_name,
                    },
                    {
                        "component_id": format!("{entity_id}:runtime_world_visual_stack"),
                        "component_kind": "runtime_world_visual_stack",
                        "properties": stack_object,
                    }
                ]
            });
            json_value_to_lua(lua, &record).map_err(mlua::Error::runtime)
        })?;
    render.set("define_world_visual_stack", define_world_visual_stack)?;

    ctx.set("render", render)?;
    Ok(())
}

#[allow(dead_code)]
fn inject_generate_collision_outline_fn(
    ctx: Table,
    lua: &Lua,
    scripts_root: &Path,
) -> mlua::Result<()> {
    let asset_entries =
        Arc::new(load_asset_registry_entries(scripts_root).map_err(mlua::Error::runtime)?);
    inject_generate_collision_outline_fn_cached(ctx, lua, scripts_root, asset_entries)
}

fn inject_generate_collision_outline_fn_cached(
    ctx: Table,
    lua: &Lua,
    scripts_root: &Path,
    asset_entries: Arc<Vec<AssetRegistryEntry>>,
) -> mlua::Result<()> {
    let scripts_root = scripts_root.to_path_buf();
    let scripts_root_for_half_extents = scripts_root.clone();
    let asset_entries_for_half_extents = asset_entries.clone();
    let compute_collision_half_extents_from_length =
        lua.create_function(move |lua, (visual_asset_id, length_m): (String, f32)| {
            let Some(asset) = asset_entries_for_half_extents
                .iter()
                .find(|entry| entry.asset_id == visual_asset_id)
            else {
                return Err(mlua::Error::runtime(format!(
                    "unknown visual asset id for collision half extents: {}",
                    visual_asset_id
                )));
            };
            let asset_root = std::env::var("ASSET_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    scripts_root_for_half_extents
                        .parent()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| scripts_root_for_half_extents.clone())
                });
            let sprite_path = asset_root.join(&asset.source_path);
            let sprite_png =
                std::fs::read(&sprite_path).map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let (half_x, half_y) =
                compute_collision_half_extents_from_sprite_length(&sprite_png, length_m)
                    .map_err(mlua::Error::runtime)?;
            let out = lua.create_table()?;
            out.set(1, half_x)?;
            out.set(2, half_y)?;
            Ok(out)
        })?;
    ctx.set(
        "compute_collision_half_extents_from_length",
        compute_collision_half_extents_from_length,
    )?;
    let compute_collision_half_extents_from_procedural = lua.create_function(
        move |lua, (entity_id, procedural_sprite, length_m): (String, Value, f32)| {
            let procedural_sprite_json =
                lua_value_to_json(procedural_sprite).map_err(mlua::Error::runtime)?;
            let procedural_sprite =
                serde_json::from_value::<ProceduralSprite>(procedural_sprite_json)
                    .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let (half_x, half_y) = compute_collision_half_extents_from_procedural_sprite(
                &entity_id,
                &procedural_sprite,
                length_m,
            )
            .map_err(mlua::Error::runtime)?;
            let out = lua.create_table()?;
            out.set(1, half_x)?;
            out.set(2, half_y)?;
            Ok(out)
        },
    )?;
    ctx.set(
        "compute_collision_half_extents_from_procedural",
        compute_collision_half_extents_from_procedural,
    )?;
    let asset_entries_for_outline = asset_entries.clone();
    let generate_collision_outline_rdp =
        lua.create_function(move |lua, (visual_asset_id, half_extents): (String, Value)| {
            let (half_x, half_y) = match half_extents {
                Value::Table(table) => {
                    let half_x = table.get::<f32>(1)?;
                    let half_y = table.get::<f32>(2)?;
                    (half_x, half_y)
                }
                _ => {
                    return Err(mlua::Error::runtime(
                        "generate_collision_outline_rdp expects half_extents table {half_x, half_y}",
                    ));
                }
            };
            let Some(asset) = asset_entries_for_outline
                .iter()
                .find(|entry| entry.asset_id == visual_asset_id)
            else {
                return Err(mlua::Error::runtime(format!(
                    "unknown visual asset id for collision outline: {}",
                    visual_asset_id
                )));
            };
            let asset_root = std::env::var("ASSET_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    scripts_root
                        .parent()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| scripts_root.clone())
                });
            let sprite_path = asset_root.join(&asset.source_path);
            let sprite_png =
                std::fs::read(&sprite_path).map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let outline =
                generate_rdp_collision_outline_from_sprite_png(&sprite_png, half_x, half_y)
                    .map_err(mlua::Error::runtime)?;
            let out = lua.create_table()?;
            for (idx, point) in outline.points.iter().enumerate() {
                let point_table = lua.create_table()?;
                point_table.set(1, point.x)?;
                point_table.set(2, point.y)?;
                out.set(idx + 1, point_table)?;
            }
            Ok(out)
        })?;
    ctx.set(
        "generate_collision_outline_rdp",
        generate_collision_outline_rdp,
    )?;
    let generate_collision_outline_rdp_from_procedural = lua.create_function(
        move |lua, (entity_id, procedural_sprite, half_extents): (String, Value, Value)| {
            let (half_x, half_y) = match half_extents {
                Value::Table(table) => {
                    let half_x = table.get::<f32>(1)?;
                    let half_y = table.get::<f32>(2)?;
                    (half_x, half_y)
                }
                _ => {
                    return Err(mlua::Error::runtime(
                        "generate_collision_outline_rdp_from_procedural expects half_extents table {half_x, half_y}",
                    ));
                }
            };
            let procedural_sprite_json =
                lua_value_to_json(procedural_sprite).map_err(mlua::Error::runtime)?;
            let procedural_sprite = serde_json::from_value::<ProceduralSprite>(procedural_sprite_json)
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let outline = generate_rdp_collision_outline_from_procedural_sprite(
                &entity_id,
                &procedural_sprite,
                half_x,
                half_y,
            )
            .map_err(mlua::Error::runtime)?;
            let out = lua.create_table()?;
            for (idx, point) in outline.points.iter().enumerate() {
                let point_table = lua.create_table()?;
                point_table.set(1, point.x)?;
                point_table.set(2, point.y)?;
                out.set(idx + 1, point_table)?;
            }
            Ok(out)
        },
    )?;
    ctx.set(
        "generate_collision_outline_rdp_from_procedural",
        generate_collision_outline_rdp_from_procedural,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        init_resources, load_script_catalog_from_database_or_disk_with_url, scripts_root_dir,
        spawn_bundle_graph_records,
    };
    use bevy::prelude::{App, MinimalPlugins};
    use sidereal_game::{GeneratedComponentRegistry, SiderealGameCorePlugin};

    #[test]
    fn script_catalog_falls_back_to_disk_when_database_is_unreachable() {
        let root = scripts_root_dir();
        let outcome = load_script_catalog_from_database_or_disk_with_url(
            &root,
            "postgres://sidereal:sidereal@127.0.0.1:1/sidereal",
        )
        .expect("disk fallback should succeed");
        assert!(outcome.startup_loaded_from_disk_fallback);
        assert_eq!(outcome.persisted_catalog_revision, 0);
        assert!(!outcome.catalog.entries.is_empty());
        assert!(
            outcome
                .catalog
                .entries
                .iter()
                .any(|entry| entry.script_path == "bundles/bundle_registry.lua")
        );
    }

    #[test]
    fn bundle_spawn_uses_host_provided_entity_id() {
        let root = scripts_root_dir();
        let entity_id = uuid::Uuid::new_v4().to_string();
        let owner_id = uuid::Uuid::new_v4().to_string();
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "entity_id".to_string(),
            serde_json::Value::String(entity_id.clone()),
        );
        overrides.insert(
            "owner_id".to_string(),
            serde_json::Value::String(owner_id.clone()),
        );
        let records =
            spawn_bundle_graph_records(&root, "ship.corvette", &overrides).expect("spawn");
        assert!(!records.is_empty());
        assert_eq!(records[0].entity_id, entity_id);
    }

    #[test]
    fn bundle_spawn_rejects_unknown_bundle_id() {
        let root = scripts_root_dir();
        let err = spawn_bundle_graph_records(&root, "unknown_bundle", &serde_json::Map::new())
            .expect_err("unknown bundle should fail");
        assert!(err.contains("unknown bundle_id"));
    }

    #[test]
    fn bundle_spawn_generates_nondeterministic_uuid_when_not_overridden() {
        let root = scripts_root_dir();
        let owner_id = uuid::Uuid::new_v4().to_string();
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "owner_id".to_string(),
            serde_json::Value::String(owner_id.clone()),
        );
        let first =
            spawn_bundle_graph_records(&root, "ship.corvette", &overrides).expect("spawn first");
        let second =
            spawn_bundle_graph_records(&root, "ship.corvette", &overrides).expect("spawn second");
        assert!(!first.is_empty());
        assert!(!second.is_empty());
        assert_ne!(
            first[0].entity_id, second[0].entity_id,
            "root entity IDs should be random UUIDs when no entity_id override is provided"
        );
    }

    #[test]
    fn init_resources_preserves_inferred_component_editor_schema_entries() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SiderealGameCorePlugin));

        init_resources(&mut app);

        let registry = app.world().resource::<GeneratedComponentRegistry>();
        let max_velocity = registry
            .entries
            .iter()
            .find(|entry| entry.component_kind == "max_velocity_mps")
            .expect("max_velocity_mps mapping should exist");
        assert!(
            !max_velocity.editor_schema.fields.is_empty(),
            "replication scripting init should preserve inferred editor schema fields"
        );
        assert!(
            !registry.shader_entries.is_empty(),
            "replication scripting init should still populate shader entries"
        );
    }
}

use crate::auth::error::AuthError;
use mlua::{Function, Table, Value};
use sidereal_game::generated_component_registry;
use sidereal_game::{
    ProceduralSprite, RuntimeWorldVisualStack,
    compute_collision_half_extents_from_procedural_sprite,
    compute_collision_half_extents_from_sprite_length,
    generate_rdp_collision_outline_from_procedural_sprite,
    generate_rdp_collision_outline_from_sprite_png, validate_runtime_post_process_stack,
    validate_runtime_render_layer_definition, validate_runtime_render_layer_rule,
    validate_runtime_world_visual_stack,
};
use sidereal_persistence::{
    GraphEntityRecord, ScriptCatalogDocumentDetail, ScriptCatalogDocumentSummary,
    ScriptCatalogRecord, discard_script_catalog_draft, ensure_script_catalog_schema,
    infer_script_family, list_script_catalog_documents, load_active_script_catalog,
    load_script_catalog_document, publish_script_catalog_draft, replace_active_script_catalog,
    upsert_script_catalog_draft,
};
use sidereal_scripting::{
    LuaSandboxPolicy, ScriptError, inject_script_logger, load_lua_module_from_source,
    load_lua_module_into_lua_from_source, lua_value_to_json, resolve_scripts_root,
    table_get_required_string, table_get_required_string_list, validate_component_kinds,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tracing::{info, warn};
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

fn decode_graph_entity_records(
    script_path: &Path,
    json_value: serde_json::Value,
) -> Result<Vec<GraphEntityRecord>, AuthError> {
    let Some(values) = json_value.as_array() else {
        return Err(AuthError::Internal(format!(
            "{}: build_graph_records(ctx) must return an array of graph entity records",
            script_path.display()
        )));
    };
    let mut records = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        match serde_json::from_value::<GraphEntityRecord>(value.clone()) {
            Ok(record) => records.push(record),
            Err(err) => {
                let keys = value
                    .as_object()
                    .map(|object| {
                        let mut keys = object.keys().cloned().collect::<Vec<_>>();
                        keys.sort();
                        keys.join(", ")
                    })
                    .unwrap_or_else(|| "<non-object>".to_string());
                return Err(AuthError::Internal(format!(
                    "{}: build_graph_records(ctx) record[{index}] is not GraphEntityRecord-compatible: {err}; keys={keys}; value={value}",
                    script_path.display()
                )));
            }
        }
    }
    Ok(records)
}

fn validate_runtime_render_graph_records(records: &[GraphEntityRecord]) -> Result<(), AuthError> {
    let generated_registry = sidereal_game::GeneratedComponentRegistry {
        entries: generated_component_registry(),
    };
    let known_component_kinds = sidereal_game::known_component_kinds(&generated_registry);
    let mut known_layer_ids = HashSet::<String>::from(["main_world".to_string()]);

    for record in records {
        for component in &record.components {
            if component.component_kind == "runtime_render_layer_definition" {
                let definition = serde_json::from_value::<
                    sidereal_game::RuntimeRenderLayerDefinition,
                >(component.properties.clone())
                .map_err(|err| {
                    AuthError::Internal(format!(
                        "entity {} runtime_render_layer_definition decode failed: {err}",
                        record.entity_id
                    ))
                })?;
                validate_runtime_render_layer_definition(&definition).map_err(|err| {
                    AuthError::Internal(format!(
                        "entity {} invalid runtime_render_layer_definition '{}': {}",
                        record.entity_id, definition.layer_id, err
                    ))
                })?;
                known_layer_ids.insert(definition.layer_id);
            }
        }
    }

    for record in records {
        for component in &record.components {
            match component.component_kind.as_str() {
                "runtime_render_layer_rule" => {
                    let rule = serde_json::from_value::<sidereal_game::RuntimeRenderLayerRule>(
                        component.properties.clone(),
                    )
                    .map_err(|err| {
                        AuthError::Internal(format!(
                            "entity {} runtime_render_layer_rule decode failed: {err}",
                            record.entity_id
                        ))
                    })?;
                    validate_runtime_render_layer_rule(
                        &rule,
                        &known_layer_ids,
                        &known_component_kinds,
                    )
                    .map_err(|err| {
                        AuthError::Internal(format!(
                            "entity {} invalid runtime_render_layer_rule '{}': {}",
                            record.entity_id, rule.rule_id, err
                        ))
                    })?;
                }
                "runtime_post_process_stack" => {
                    let stack = serde_json::from_value::<sidereal_game::RuntimePostProcessStack>(
                        component.properties.clone(),
                    )
                    .map_err(|err| {
                        AuthError::Internal(format!(
                            "entity {} runtime_post_process_stack decode failed: {err}",
                            record.entity_id
                        ))
                    })?;
                    validate_runtime_post_process_stack(&stack).map_err(|err| {
                        AuthError::Internal(format!(
                            "entity {} invalid runtime_post_process_stack: {}",
                            record.entity_id, err
                        ))
                    })?;
                }
                "runtime_world_visual_stack" => {
                    let stack = serde_json::from_value::<RuntimeWorldVisualStack>(
                        component.properties.clone(),
                    )
                    .map_err(|err| {
                        AuthError::Internal(format!(
                            "entity {} runtime_world_visual_stack decode failed: {err}",
                            record.entity_id
                        ))
                    })?;
                    validate_runtime_world_visual_stack(&stack).map_err(|err| {
                        AuthError::Internal(format!(
                            "entity {} invalid runtime_world_visual_stack: {}",
                            record.entity_id, err
                        ))
                    })?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldInitScriptConfig {
    pub render_layer_shader_asset_ids: Vec<String>,
    pub additional_required_asset_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInitScriptConfig {
    pub ship_bundle_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptBundleDefinition {
    pub bundle_id: String,
    pub bundle_class: String,
    pub graph_records_script: String,
    pub required_component_kinds: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptBundleRegistry {
    pub bundles: HashMap<String, ScriptBundleDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GatewayAssetRegistryEntry {
    pub asset_id: String,
    pub source_path: String,
    pub content_type: String,
    pub dependencies: Vec<String>,
    pub bootstrap_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GatewayAssetRegistry {
    pub entries: Vec<GatewayAssetRegistryEntry>,
}

#[derive(Debug, Clone)]
pub struct ScriptContext<'a> {
    pub account_id: Uuid,
    pub player_entity_id: &'a str,
    pub email: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptCatalogResource {
    pub entries: Vec<ScriptCatalogEntry>,
    pub revision: u64,
    pub root_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptCatalogEntry {
    pub script_path: String,
    pub source: String,
    pub revision: u64,
    pub origin: String,
}

#[derive(Default)]
struct ScriptCatalogCacheState {
    catalog: Option<ScriptCatalogResource>,
}

static SCRIPT_CATALOG_CACHE: OnceLock<Mutex<ScriptCatalogCacheState>> = OnceLock::new();

fn gateway_database_url() -> String {
    std::env::var("GATEWAY_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
}

pub fn scripts_root_dir() -> PathBuf {
    let resolved = resolve_scripts_root(env!("CARGO_MANIFEST_DIR"));
    info!("gateway scripting root resolved to {}", resolved.display());
    resolved
}

pub fn load_script_catalog_from_disk(root: &Path) -> Result<ScriptCatalogResource, AuthError> {
    let mut entries = Vec::new();
    load_script_catalog_entries_recursive(root, root, &mut entries)?;
    entries.sort_by(|a, b| a.script_path.cmp(&b.script_path));
    for (idx, entry) in entries.iter_mut().enumerate() {
        entry.revision = (idx as u64) + 1;
    }
    Ok(ScriptCatalogResource {
        entries,
        revision: 1,
        root_dir: root.display().to_string(),
    })
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

fn load_script_catalog_from_database_or_seed(
    root: &Path,
) -> Result<ScriptCatalogResource, AuthError> {
    let database_url = gateway_database_url();
    let fallback_catalog = || load_script_catalog_from_disk(root);
    let mut client = match postgres::Client::connect(&database_url, postgres::NoTls) {
        Ok(client) => client,
        Err(err) => {
            warn!(
                "gateway script catalog postgres unavailable; using disk fallback root={} err={}",
                root.display(),
                err
            );
            return fallback_catalog();
        }
    };
    if let Err(err) = ensure_script_catalog_schema(&mut client) {
        warn!(
            "gateway script catalog schema ensure failed; using disk fallback root={} err={}",
            root.display(),
            err
        );
        return fallback_catalog();
    }
    let persisted = match load_active_script_catalog(&mut client) {
        Ok(persisted) => persisted,
        Err(err) => {
            warn!(
                "gateway script catalog load failed; using disk fallback root={} err={}",
                root.display(),
                err
            );
            return fallback_catalog();
        }
    };
    if !persisted.is_empty() {
        let mut entries = persisted
            .into_iter()
            .map(catalog_entry_from_persisted)
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.script_path.cmp(&b.script_path));
        return Ok(ScriptCatalogResource {
            entries,
            revision: 1,
            root_dir: root.display().to_string(),
        });
    }
    let catalog = fallback_catalog()?;
    let records = catalog
        .entries
        .iter()
        .map(persisted_record_from_entry)
        .collect::<Vec<_>>();
    if let Err(err) = replace_active_script_catalog(&mut client, &records) {
        warn!(
            "gateway script catalog seed persist failed; continuing with disk catalog root={} err={}",
            root.display(),
            err
        );
    }
    Ok(catalog)
}

pub fn reload_script_catalog_from_disk(root: &Path) -> Result<ScriptCatalogResource, AuthError> {
    let catalog = load_script_catalog_from_disk(root)?;
    let database_url = gateway_database_url();
    let mut client = postgres::Client::connect(&database_url, postgres::NoTls)
        .map_err(|err| AuthError::Internal(format!("postgres connect failed: {err}")))?;
    let records = catalog
        .entries
        .iter()
        .map(persisted_record_from_entry)
        .collect::<Vec<_>>();
    replace_active_script_catalog(&mut client, &records)
        .map_err(|err| AuthError::Internal(format!("persist script catalog failed: {err}")))?;
    let cache = SCRIPT_CATALOG_CACHE.get_or_init(|| Mutex::new(ScriptCatalogCacheState::default()));
    let mut guard = cache.lock().map_err(|_| {
        AuthError::Internal("gateway script catalog cache lock poisoned".to_string())
    })?;
    guard.catalog = Some(catalog.clone());
    Ok(catalog)
}

pub fn current_script_catalog(root: &Path) -> Result<ScriptCatalogResource, AuthError> {
    let cache = SCRIPT_CATALOG_CACHE.get_or_init(|| Mutex::new(ScriptCatalogCacheState::default()));
    let mut guard = cache.lock().map_err(|_| {
        AuthError::Internal("gateway script catalog cache lock poisoned".to_string())
    })?;
    if let Some(catalog) = guard.catalog.as_ref()
        && catalog.root_dir == root.display().to_string()
    {
        return Ok(catalog.clone());
    }
    let catalog = load_script_catalog_from_database_or_seed(root)?;
    guard.catalog = Some(catalog.clone());
    Ok(catalog)
}

pub fn list_persisted_script_catalog_documents()
-> Result<Vec<ScriptCatalogDocumentSummary>, AuthError> {
    let database_url = gateway_database_url();
    let mut client = postgres::Client::connect(&database_url, postgres::NoTls)
        .map_err(|err| AuthError::Internal(format!("postgres connect failed: {err}")))?;
    list_script_catalog_documents(&mut client)
        .map_err(|err| AuthError::Internal(format!("list script catalog documents failed: {err}")))
}

pub fn load_persisted_script_catalog_document(
    script_path: &str,
) -> Result<Option<ScriptCatalogDocumentDetail>, AuthError> {
    let database_url = gateway_database_url();
    let mut client = postgres::Client::connect(&database_url, postgres::NoTls)
        .map_err(|err| AuthError::Internal(format!("postgres connect failed: {err}")))?;
    load_script_catalog_document(&mut client, script_path)
        .map_err(|err| AuthError::Internal(format!("load script catalog document failed: {err}")))
}

pub fn save_script_catalog_draft(
    script_path: &str,
    source: &str,
    origin: Option<&str>,
    family: Option<&str>,
) -> Result<(), AuthError> {
    let database_url = gateway_database_url();
    let mut client = postgres::Client::connect(&database_url, postgres::NoTls)
        .map_err(|err| AuthError::Internal(format!("postgres connect failed: {err}")))?;
    let origin = origin.unwrap_or("dashboard_draft");
    let family = family.unwrap_or("");
    upsert_script_catalog_draft(&mut client, script_path, family, source, origin)
        .map_err(|err| AuthError::Internal(format!("save script catalog draft failed: {err}")))
}

pub fn publish_persisted_script_catalog_draft(script_path: &str) -> Result<Option<u64>, AuthError> {
    let database_url = gateway_database_url();
    let mut client = postgres::Client::connect(&database_url, postgres::NoTls)
        .map_err(|err| AuthError::Internal(format!("postgres connect failed: {err}")))?;
    publish_script_catalog_draft(&mut client, script_path)
        .map_err(|err| AuthError::Internal(format!("publish script catalog draft failed: {err}")))
}

pub fn discard_persisted_script_catalog_draft(script_path: &str) -> Result<bool, AuthError> {
    let database_url = gateway_database_url();
    let mut client = postgres::Client::connect(&database_url, postgres::NoTls)
        .map_err(|err| AuthError::Internal(format!("postgres connect failed: {err}")))?;
    discard_script_catalog_draft(&mut client, script_path)
        .map_err(|err| AuthError::Internal(format!("discard script catalog draft failed: {err}")))
}

fn load_script_catalog_entries_recursive(
    root: &Path,
    current_dir: &Path,
    out: &mut Vec<ScriptCatalogEntry>,
) -> Result<(), AuthError> {
    let read_dir = std::fs::read_dir(current_dir).map_err(|err| {
        AuthError::Internal(format!("read {} failed: {err}", current_dir.display()))
    })?;
    for entry in read_dir {
        let entry = entry.map_err(|err| {
            AuthError::Internal(format!("read {} failed: {err}", current_dir.display()))
        })?;
        let path = entry.path();
        if path.is_dir() {
            load_script_catalog_entries_recursive(root, &path, out)?;
            continue;
        }
        if path.extension().and_then(|v| v.to_str()) != Some("lua") {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|err| {
                AuthError::Internal(format!("strip prefix {} failed: {err}", path.display()))
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(&path)
            .map_err(|err| AuthError::Internal(format!("read {} failed: {err}", path.display())))?;
        out.push(ScriptCatalogEntry {
            script_path: relative,
            source,
            revision: 0,
            origin: "disk".to_string(),
        });
    }
    Ok(())
}

fn lookup_script_catalog_entry<'a>(
    catalog: &'a ScriptCatalogResource,
    script_path: &str,
) -> Result<&'a ScriptCatalogEntry, AuthError> {
    catalog
        .entries
        .iter()
        .find(|entry| entry.script_path == script_path)
        .ok_or_else(|| {
            AuthError::Internal(format!(
                "gateway script catalog missing script_path={script_path}"
            ))
        })
}

pub fn load_world_init_config(root: &Path) -> Result<WorldInitScriptConfig, AuthError> {
    let catalog = current_script_catalog(root)?;
    load_world_init_config_from_catalog(&catalog)
}

pub fn load_world_init_config_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<WorldInitScriptConfig, AuthError> {
    let policy = LuaSandboxPolicy::from_env();
    let entry = lookup_script_catalog_entry(catalog, "world/world_init.lua")?;
    let module =
        load_lua_module_from_source(&entry.source, Path::new("world/world_init.lua"), &policy)
            .map_err(map_script_error)?;
    let world_defaults = module
        .root()
        .get::<Table>("world_defaults")
        .map_err(|err| AuthError::Internal(format!("world/world_init.lua: {err}")))?;
    let render_layer_shader_asset_ids = match world_defaults
        .get::<Value>("render_layer_definitions")
        .map_err(|err| AuthError::Internal(format!("world/world_init.lua: {err}")))?
    {
        Value::Nil => Vec::new(),
        Value::Table(values_table) => {
            let mut out = Vec::new();
            for value in values_table.sequence_values::<Table>() {
                let layer = value.map_err(|err| {
                    AuthError::Internal(format!(
                        "world/world_init.lua: world_defaults.render_layer_definitions entry decode failed: {err}"
                    ))
                })?;
                let shader_asset_id = layer
                    .get::<Option<String>>("shader_asset_id")
                    .map_err(|err| AuthError::Internal(format!("world/world_init.lua: {err}")))?
                    .unwrap_or_default();
                if !shader_asset_id.trim().is_empty()
                    && !out.iter().any(|value| value == &shader_asset_id)
                {
                    out.push(shader_asset_id);
                }
            }
            out
        }
        _ => {
            return Err(AuthError::Internal(
                "world/world_init.lua: world_defaults.render_layer_definitions must be an array of tables when present".to_string(),
            ));
        }
    };
    let additional_required_asset_ids = match world_defaults
        .get::<Value>("additional_required_asset_ids")
        .map_err(|err| AuthError::Internal(format!("world/world_init.lua: {err}")))?
    {
        Value::Nil => Vec::new(),
        Value::Table(values_table) => {
            let mut out = Vec::new();
            for value in values_table.sequence_values::<String>() {
                out.push(value.map_err(|err| {
                    AuthError::Internal(format!(
                        "world/world_init.lua: world_defaults.additional_required_asset_ids entry decode failed: {err}"
                    ))
                })?);
            }
            out
        }
        _ => {
            return Err(AuthError::Internal(
                "world/world_init.lua: world_defaults.additional_required_asset_ids must be an array of strings when present".to_string(),
            ));
        }
    };
    Ok(WorldInitScriptConfig {
        render_layer_shader_asset_ids,
        additional_required_asset_ids,
    })
    .inspect(|config| {
        info!(
            "gateway loaded world init config: render_layer_shader_asset_ids={:?} additional_required_asset_ids={}",
            config.render_layer_shader_asset_ids,
            config.additional_required_asset_ids.len()
        );
    })
}

#[cfg(test)]
pub fn load_world_init_graph_records(root: &Path) -> Result<Vec<GraphEntityRecord>, AuthError> {
    let catalog = current_script_catalog(root)?;
    load_world_init_graph_records_from_catalog(&catalog, root)
}

#[cfg(test)]
pub fn load_world_init_graph_records_from_catalog(
    catalog: &ScriptCatalogResource,
    root: &Path,
) -> Result<Vec<GraphEntityRecord>, AuthError> {
    let policy = LuaSandboxPolicy::from_env();
    let entry = lookup_script_catalog_entry(catalog, "world/world_init.lua")?;
    let module =
        load_lua_module_from_source(&entry.source, Path::new("world/world_init.lua"), &policy)
            .map_err(map_script_error)?;
    let build_graph_records = module
        .root()
        .get::<Function>("build_graph_records")
        .map_err(|err| {
            AuthError::Internal(format!(
                "{}: missing build_graph_records(ctx): {err}",
                module.script_path().display()
            ))
        })?;
    let ctx = module
        .root()
        .get::<Table>("context")
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    inject_script_context(ctx.clone(), &module, root, None)?;
    inject_bundle_registry_spawn_fn(ctx.clone(), &module, root)?;

    let lua_value = build_graph_records
        .call::<Value>(ctx)
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    let json_value = lua_value_to_json(lua_value).map_err(map_script_error)?;
    let records = decode_graph_entity_records(module.script_path(), json_value)?;
    if records.is_empty() {
        return Err(AuthError::Internal(format!(
            "{}: build_graph_records(ctx) returned empty records",
            module.script_path().display()
        )));
    }
    validate_runtime_render_graph_records(&records)?;
    Ok(records)
}

pub fn load_player_init_config(
    root: &Path,
    context: ScriptContext<'_>,
) -> Result<PlayerInitScriptConfig, AuthError> {
    let catalog = current_script_catalog(root)?;
    load_player_init_config_from_catalog(&catalog, root, context)
}

pub fn load_player_init_config_from_catalog(
    catalog: &ScriptCatalogResource,
    root: &Path,
    context: ScriptContext<'_>,
) -> Result<PlayerInitScriptConfig, AuthError> {
    let policy = LuaSandboxPolicy::from_env();
    let entry = lookup_script_catalog_entry(catalog, "accounts/player_init.lua")?;
    let module = load_lua_module_from_source(
        &entry.source,
        Path::new("accounts/player_init.lua"),
        &policy,
    )
    .map_err(map_script_error)?;
    let ctx = module
        .root()
        .get::<Table>("context")
        .map_err(|err| AuthError::Internal(format!("accounts/player_init.lua: {err}")))?;
    inject_script_context(ctx.clone(), &module, root, Some(&context))?;

    let player_init = module
        .root()
        .get::<Function>("player_init")
        .map_err(|err| AuthError::Internal(format!("accounts/player_init.lua: {err}")))?;
    let response = player_init
        .call::<Value>(ctx)
        .map_err(|err| AuthError::Internal(format!("accounts/player_init.lua: {err}")))?;
    let table = match response {
        Value::Table(table) => table,
        _ => {
            return Err(AuthError::Internal(
                "accounts/player_init.lua: player_init(ctx) must return a table".to_string(),
            ));
        }
    };
    let ship_bundle_id = table_get_required_string(&table, "ship_bundle_id", "player_init")
        .map_err(map_script_error)?;
    Ok(PlayerInitScriptConfig { ship_bundle_id }).inspect(|config| {
        info!(
            "gateway player-init script selected ship_bundle_id={} for account_id={} player_entity_id={}",
            config.ship_bundle_id, context.account_id, context.player_entity_id
        );
    })
}

pub fn load_bundle_registry(root: &Path) -> Result<ScriptBundleRegistry, AuthError> {
    let catalog = current_script_catalog(root)?;
    load_bundle_registry_from_catalog(&catalog)
}

pub fn load_bundle_registry_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<ScriptBundleRegistry, AuthError> {
    let policy = LuaSandboxPolicy::from_env();
    let entry = lookup_script_catalog_entry(catalog, "bundles/bundle_registry.lua")?;
    let module = load_lua_module_from_source(
        &entry.source,
        Path::new("bundles/bundle_registry.lua"),
        &policy,
    )
    .map_err(map_script_error)?;
    let bundles_table = module
        .root()
        .get::<Table>("bundles")
        .map_err(|err| AuthError::Internal(format!("bundles/bundle_registry.lua: {err}")))?;

    let mut bundles = HashMap::<String, ScriptBundleDefinition>::new();
    for pair in bundles_table.pairs::<String, Table>() {
        let (bundle_id, bundle_table) =
            pair.map_err(|err| AuthError::Internal(format!("bundle registry read failed: {err}")))?;
        let bundle_class = table_get_required_string(&bundle_table, "bundle_class", &bundle_id)
            .map_err(map_script_error)?;
        let graph_records_script =
            table_get_required_string(&bundle_table, "graph_records_script", &bundle_id)
                .map_err(map_script_error)?;
        let required_component_kinds =
            table_get_required_string_list(&bundle_table, "required_component_kinds", &bundle_id)
                .map_err(map_script_error)?;
        bundles.insert(
            bundle_id.clone(),
            ScriptBundleDefinition {
                bundle_id,
                bundle_class,
                graph_records_script,
                required_component_kinds,
            },
        );
    }

    if bundles.is_empty() {
        return Err(AuthError::Internal(
            "bundles/bundle_registry.lua: bundles table must not be empty".to_string(),
        ));
    }

    validate_bundle_registry_component_kinds(&bundles)?;
    info!(
        "gateway loaded bundle registry with {} bundle(s)",
        bundles.len()
    );
    Ok(ScriptBundleRegistry { bundles })
}

pub fn load_asset_registry_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<GatewayAssetRegistry, AuthError> {
    let policy = LuaSandboxPolicy::from_env();
    let entry = lookup_script_catalog_entry(catalog, "assets/registry.lua")?;
    let module =
        load_lua_module_from_source(&entry.source, Path::new("assets/registry.lua"), &policy)
            .map_err(map_script_error)?;
    let root = module.root();
    let schema_version_i64 = root
        .get::<i64>("schema_version")
        .map_err(|err| AuthError::Internal(format!("assets/registry.lua: {err}")))?;
    if schema_version_i64 < 1 {
        return Err(AuthError::Internal(
            "assets/registry.lua: schema_version must be >= 1".to_string(),
        ));
    }
    let assets_table = root
        .get::<Table>("assets")
        .map_err(|err| AuthError::Internal(format!("assets/registry.lua: {err}")))?;

    let mut entries = Vec::new();
    for (idx, value) in assets_table.sequence_values::<Table>().enumerate() {
        let asset = value.map_err(|err| {
            AuthError::Internal(format!(
                "assets/registry.lua: assets[{}] decode failed: {err}",
                idx + 1
            ))
        })?;
        let context = format!("assets[{}]", idx + 1);
        let asset_id =
            table_get_required_string(&asset, "asset_id", &context).map_err(map_script_error)?;
        let source_path =
            table_get_required_string(&asset, "source_path", &context).map_err(map_script_error)?;
        let content_type = table_get_required_string(&asset, "content_type", &context)
            .map_err(map_script_error)?;
        let dependencies = match asset
            .get::<Value>("dependencies")
            .map_err(|err| AuthError::Internal(format!("assets/registry.lua: {err}")))?
        {
            Value::Nil => Vec::new(),
            Value::Table(values) => {
                let mut out = Vec::new();
                for value in values.sequence_values::<String>() {
                    out.push(value.map_err(|err| {
                        AuthError::Internal(format!(
                            "assets/registry.lua: {}.dependencies decode failed: {err}",
                            context
                        ))
                    })?);
                }
                out
            }
            _ => {
                return Err(AuthError::Internal(format!(
                    "assets/registry.lua: {}.dependencies must be an array of strings when present",
                    context
                )));
            }
        };
        let bootstrap_required = match asset
            .get::<Value>("bootstrap_required")
            .map_err(|err| AuthError::Internal(format!("assets/registry.lua: {err}")))?
        {
            Value::Nil => false,
            Value::Boolean(value) => value,
            _ => {
                return Err(AuthError::Internal(format!(
                    "assets/registry.lua: {}.bootstrap_required must be boolean when present",
                    context
                )));
            }
        };
        entries.push(GatewayAssetRegistryEntry {
            asset_id,
            source_path,
            content_type,
            dependencies,
            bootstrap_required,
        });
    }
    entries.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    Ok(GatewayAssetRegistry { entries })
}

pub fn load_graph_records_for_bundle(
    root: &Path,
    bundle: &ScriptBundleDefinition,
    context: ScriptContext<'_>,
) -> Result<Vec<GraphEntityRecord>, AuthError> {
    let catalog = current_script_catalog(root)?;
    load_graph_records_for_bundle_from_catalog(&catalog, root, bundle, context)
}

pub fn load_graph_records_for_bundle_from_catalog(
    catalog: &ScriptCatalogResource,
    root: &Path,
    bundle: &ScriptBundleDefinition,
    context: ScriptContext<'_>,
) -> Result<Vec<GraphEntityRecord>, AuthError> {
    let script_rel_path = &bundle.graph_records_script;
    info!(
        "gateway loading graph records script={} for bundle_id={} account_id={} player_entity_id={}",
        script_rel_path, bundle.bundle_id, context.account_id, context.player_entity_id
    );

    let policy = LuaSandboxPolicy::from_env();
    let entry = lookup_script_catalog_entry(catalog, script_rel_path)?;
    let module = load_lua_module_from_source(&entry.source, Path::new(script_rel_path), &policy)
        .map_err(map_script_error)?;
    let build_graph_records = module
        .root()
        .get::<Function>("build_graph_records")
        .map_err(|err| {
            AuthError::Internal(format!(
                "{}: missing build_graph_records(ctx): {err}",
                module.script_path().display()
            ))
        })?;
    let ctx = module
        .root()
        .get::<Table>("context")
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    inject_script_context(ctx.clone(), &module, root, Some(&context))?;
    inject_bundle_registry_spawn_fn(ctx.clone(), &module, root)?;
    ctx.set("bundle_id", bundle.bundle_id.as_str())
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;

    let lua_value = build_graph_records
        .call::<Value>(ctx)
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    let json_value = lua_value_to_json(lua_value).map_err(map_script_error)?;
    let records = decode_graph_entity_records(module.script_path(), json_value)?;
    if records.is_empty() {
        return Err(AuthError::Internal(format!(
            "{}: build_graph_records(ctx) returned empty records",
            module.script_path().display()
        )));
    }
    validate_runtime_render_graph_records(&records)?;
    validate_graph_records_component_kinds(bundle, &records)?;
    Ok(records)
}

fn validate_bundle_registry_component_kinds(
    bundles: &HashMap<String, ScriptBundleDefinition>,
) -> Result<(), AuthError> {
    let known_component_kinds = generated_component_registry()
        .into_iter()
        .map(|entry| entry.component_kind.to_string())
        .collect::<HashSet<_>>();

    for (bundle_id, bundle) in bundles {
        validate_component_kinds(
            &known_component_kinds,
            &bundle.required_component_kinds,
            &format!("bundles/bundle_registry.lua: bundle={bundle_id}"),
        )
        .map_err(map_script_error)?;
    }
    Ok(())
}

fn validate_graph_records_component_kinds(
    bundle: &ScriptBundleDefinition,
    records: &[GraphEntityRecord],
) -> Result<(), AuthError> {
    let allowed = bundle
        .required_component_kinds
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    for record in records {
        for component in &record.components {
            if !allowed.contains(&component.component_kind) {
                return Err(AuthError::Internal(format!(
                    "bundle {} graph records contain component_kind={} not listed in required_component_kinds",
                    bundle.bundle_id, component.component_kind
                )));
            }
        }
    }
    Ok(())
}

fn inject_script_context(
    ctx: Table,
    module: &sidereal_scripting::LoadedLuaModule,
    scripts_root: &Path,
    context: Option<&ScriptContext<'_>>,
) -> Result<(), AuthError> {
    if let Some(context) = context {
        ctx.set("account_id", context.account_id.to_string())
            .map_err(|err| {
                AuthError::Internal(format!("{}: {err}", module.script_path().display()))
            })?;
        ctx.set("player_entity_id", context.player_entity_id)
            .map_err(|err| {
                AuthError::Internal(format!("{}: {err}", module.script_path().display()))
            })?;
        ctx.set("owner_id", context.player_entity_id)
            .map_err(|err| {
                AuthError::Internal(format!("{}: {err}", module.script_path().display()))
            })?;
        ctx.set("email", context.email).map_err(|err| {
            AuthError::Internal(format!("{}: {err}", module.script_path().display()))
        })?;
    }
    let new_uuid = module
        .lua()
        .create_function(|_, ()| Ok(Uuid::new_v4().to_string()))
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    ctx.set("new_uuid", new_uuid)
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    inject_script_logger(
        module.lua(),
        &ctx,
        &module.script_path().display().to_string(),
    )
    .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    inject_generate_collision_outline_fn(ctx.clone(), module.lua(), scripts_root)
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    inject_render_authoring_api(module.lua(), ctx)
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    Ok(())
}

fn inject_bundle_registry_spawn_fn(
    ctx: Table,
    module: &sidereal_scripting::LoadedLuaModule,
    scripts_root: &Path,
) -> Result<(), AuthError> {
    inject_bundle_registry_spawn_fn_inner(ctx, module.lua(), scripts_root)
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))
}

fn inject_bundle_registry_spawn_fn_inner(
    ctx: Table,
    lua: &mlua::Lua,
    scripts_root: &Path,
) -> mlua::Result<()> {
    inject_generate_collision_outline_fn(ctx.clone(), lua, scripts_root)?;
    let scripts_root = scripts_root.to_path_buf();
    let spawn_bundle_graph_records =
        lua.create_function(move |lua, (bundle_id, overrides): (String, Value)| {
            let catalog = current_script_catalog(&scripts_root)
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let bundle_registry = load_bundle_registry_from_catalog(&catalog)
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let Some(bundle_def) = bundle_registry.bundles.get(bundle_id.as_str()) else {
                return Err(mlua::Error::runtime(format!(
                    "unknown bundle_id in gateway script catalog: {}",
                    bundle_id
                )));
            };
            let script_entry =
                lookup_script_catalog_entry(&catalog, &bundle_def.graph_records_script)
                    .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let bundle_module_path =
                Path::new(bundle_def.graph_records_script.as_str()).to_path_buf();
            let bundle_module = load_lua_module_into_lua_from_source(
                lua,
                &script_entry.source,
                &bundle_module_path,
            )
            .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let build_graph_records = bundle_module
                .get::<Function>("build_graph_records")
                .map_err(|err| {
                    mlua::Error::runtime(format!("{}: {err}", bundle_module_path.display()))
                })?;
            let bundle_ctx = lua.create_table()?;
            bundle_ctx.set("bundle_id", bundle_id)?;
            match overrides {
                Value::Table(overrides_table) => {
                    for pair in overrides_table.pairs::<Value, Value>() {
                        let (key, value) = pair?;
                        bundle_ctx.set(key, value)?;
                    }
                }
                Value::Nil => {}
                _ => {
                    return Err(mlua::Error::runtime(
                        "spawn_bundle_graph_records override payload must be a table or nil",
                    ));
                }
            }
            let new_uuid = lua.create_function(|_, ()| Ok(Uuid::new_v4().to_string()))?;
            bundle_ctx.set("new_uuid", new_uuid)?;
            inject_script_logger(lua, &bundle_ctx, &bundle_module_path.display().to_string())
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            inject_bundle_registry_spawn_fn_inner(bundle_ctx.clone(), lua, &scripts_root)?;
            build_graph_records.call::<Value>(bundle_ctx)
        })?;
    ctx.set("spawn_bundle_graph_records", spawn_bundle_graph_records)?;
    inject_render_authoring_api(lua, ctx)?;
    Ok(())
}

fn json_value_to_lua(lua: &mlua::Lua, value: &serde_json::Value) -> Result<Value, String> {
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

fn inject_render_authoring_api(lua: &mlua::Lua, ctx: Table) -> mlua::Result<()> {
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

fn inject_generate_collision_outline_fn(
    ctx: Table,
    lua: &mlua::Lua,
    scripts_root: &Path,
) -> mlua::Result<()> {
    let scripts_root = scripts_root.to_path_buf();
    let scripts_root_for_half_extents = scripts_root.clone();
    let compute_collision_half_extents_from_length =
        lua.create_function(move |lua, (visual_asset_id, length_m): (String, f32)| {
            let catalog = current_script_catalog(&scripts_root_for_half_extents)
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let asset_registry = load_asset_registry_from_catalog(&catalog)
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let Some(asset) = asset_registry
                .entries
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
            let procedural_sprite_json = sidereal_scripting::lua_value_to_json(procedural_sprite)
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
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
            let catalog =
                current_script_catalog(&scripts_root).map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let asset_registry =
                load_asset_registry_from_catalog(&catalog).map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let Some(asset) = asset_registry
                .entries
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
                sidereal_scripting::lua_value_to_json(procedural_sprite)
                    .map_err(|err| mlua::Error::runtime(err.to_string()))?;
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

fn map_script_error(err: ScriptError) -> AuthError {
    AuthError::Internal(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ScriptContext, load_bundle_registry, load_graph_records_for_bundle,
        load_player_init_config, load_world_init_config, load_world_init_graph_records,
        scripts_root_dir,
    };
    use uuid::Uuid;

    #[test]
    fn default_world_init_script_loads() {
        let root = scripts_root_dir();
        let config = load_world_init_config(&root).expect("load world init");
        assert!(!config.render_layer_shader_asset_ids.is_empty());
        assert!(
            config
                .render_layer_shader_asset_ids
                .iter()
                .any(|id| id == "space_background_wgsl")
        );
        assert!(
            config
                .render_layer_shader_asset_ids
                .iter()
                .any(|id| id == "starfield_wgsl")
        );
    }

    #[test]
    fn default_player_init_script_loads() {
        let root = scripts_root_dir();
        let config = load_player_init_config(
            &root,
            ScriptContext {
                account_id: Uuid::new_v4(),
                player_entity_id: &Uuid::new_v4().to_string(),
                email: "pilot@example.com",
            },
        )
        .expect("load player init");
        assert_eq!(config.ship_bundle_id, "ship.corvette");
    }

    #[test]
    fn default_bundle_registry_loads_and_validates_component_kinds() {
        let root = scripts_root_dir();
        let registry = load_bundle_registry(&root).expect("load bundle registry");
        let corvette = registry
            .bundles
            .get("ship.corvette")
            .expect("corvette bundle");
        assert_eq!(corvette.bundle_class, "ship");
        assert_eq!(corvette.graph_records_script, "bundles/ship/corvette.lua");
        assert!(
            corvette
                .required_component_kinds
                .contains(&"display_name".to_string())
        );
        assert!(
            corvette
                .required_component_kinds
                .contains(&"visibility_range_buff_m".to_string())
        );
    }

    #[test]
    fn corvette_bundle_includes_visibility_range_buff_ms() {
        let root = scripts_root_dir();
        let registry = load_bundle_registry(&root).expect("load bundle registry");
        let ship_bundle = registry.bundles.get("ship.corvette").expect("ship bundle");
        let records = load_graph_records_for_bundle(
            &root,
            ship_bundle,
            ScriptContext {
                account_id: Uuid::new_v4(),
                player_entity_id: &Uuid::new_v4().to_string(),
                email: "pilot@example.com",
            },
        )
        .expect("load graph records for starter bundle");
        let component_kinds = records
            .iter()
            .flat_map(|record| {
                record
                    .components
                    .iter()
                    .map(|component| component.component_kind.as_str())
            })
            .collect::<std::collections::HashSet<_>>();
        assert!(component_kinds.contains("visibility_range_buff_m"));
    }

    #[test]
    fn corvette_bundle_action_capabilities_use_canonical_actions_only() {
        let root = scripts_root_dir();
        let registry = load_bundle_registry(&root).expect("load bundle registry");
        let ship_bundle = registry.bundles.get("ship.corvette").expect("ship bundle");
        let records = load_graph_records_for_bundle(
            &root,
            ship_bundle,
            ScriptContext {
                account_id: Uuid::new_v4(),
                player_entity_id: &Uuid::new_v4().to_string(),
                email: "pilot@example.com",
            },
        )
        .expect("load graph records for starter bundle");

        let allowed = [
            "Forward",
            "Backward",
            "LongitudinalNeutral",
            "Left",
            "Right",
            "LateralNeutral",
            "Brake",
            "AfterburnerOn",
            "AfterburnerOff",
            "FirePrimary",
            "FireSecondary",
        ]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();

        for record in &records {
            for component in &record.components {
                if component.component_kind != "action_capabilities" {
                    continue;
                }
                let Some(supported) = component
                    .properties
                    .get("supported")
                    .and_then(serde_json::Value::as_array)
                else {
                    panic!("action_capabilities.supported missing or not an array");
                };
                for action in supported {
                    let Some(name) = action.as_str() else {
                        panic!("action_capabilities.supported contains non-string value");
                    };
                    assert!(
                        allowed.contains(name),
                        "unexpected non-canonical action capability: {}",
                        name
                    );
                }
            }
        }
    }

    #[test]
    fn default_world_init_graph_records_script_loads() {
        let root = scripts_root_dir();
        let records = load_world_init_graph_records(&root).expect("load world init graph records");
        assert!(!records.is_empty());
        assert!(
            records
                .iter()
                .any(|record| record.entity_id == "0012ebad-0000-0000-0000-000000000001")
        );
        let background_base = records
            .iter()
            .find(|record| record.entity_id == "0012ebad-0000-0000-0000-000000000002")
            .expect("base background layer should exist");
        assert!(
            background_base
                .components
                .iter()
                .any(|component| component.component_kind == "space_background_shader_settings"),
            "base background layer should carry SpaceBackgroundShaderSettings"
        );
        let background_nebula = records
            .iter()
            .find(|record| record.entity_id == "0012ebad-0000-0000-0000-000000000014")
            .expect("nebula background layer should exist");
        assert!(
            background_nebula
                .components
                .iter()
                .any(|component| component.component_kind == "space_background_shader_settings"),
            "nebula background layer should carry SpaceBackgroundShaderSettings"
        );
        let starfield = records
            .iter()
            .find(|record| record.entity_id == "0012ebad-0000-0000-0000-000000000001")
            .expect("starfield layer should exist");
        assert!(
            starfield
                .components
                .iter()
                .any(|component| component.component_kind == "starfield_shader_settings"),
            "starfield layer should carry StarfieldShaderSettings"
        );
        let component_kinds = records
            .iter()
            .flat_map(|record| {
                record
                    .components
                    .iter()
                    .map(|component| component.component_kind.as_str())
            })
            .collect::<std::collections::HashSet<_>>();
        assert!(component_kinds.contains("visibility_range_buff_m"));
    }
}

pub fn init_resources(app: &mut App) {
    let scripts_root = scripts_root_dir();
    let asset_registry_script_path = scripts_root.join(ASSET_REGISTRY_SCRIPT_REL_PATH);
    let planet_registry_script_path = scripts_root.join(PLANET_REGISTRY_SCRIPT_REL_PATH);
    let ship_module_registry_script_path = scripts_root.join(SHIP_MODULE_REGISTRY_SCRIPT_REL_PATH);
    let ship_registry_script_path = scripts_root.join(SHIP_REGISTRY_SCRIPT_REL_PATH);
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
    let ship_module_registry = match load_ship_module_registry_from_catalog(&catalog) {
        Ok(registry) => registry,
        Err(err) => {
            bevy::log::warn!(
                "replication ship module registry initial derive failed: {}",
                err
            );
            sidereal_game::ShipModuleRegistry::default()
        }
    };
    let ship_registry = match load_ship_registry_from_catalog(&catalog) {
        Ok(registry) => registry,
        Err(err) => {
            bevy::log::warn!("replication ship registry initial derive failed: {}", err);
            sidereal_game::ShipRegistry::default()
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
    app.insert_resource(ship_module_registry);
    app.insert_resource(ship_registry);
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
    app.insert_resource(ShipModuleRegistrySyncState {
        registry_script_path: ship_module_registry_script_path,
        last_catalog_revision: 0,
    });
    app.insert_resource(ShipRegistrySyncState {
        registry_script_path: ship_registry_script_path,
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
            sync_ship_module_registry_resource_system,
            sync_ship_registry_resource_system,
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

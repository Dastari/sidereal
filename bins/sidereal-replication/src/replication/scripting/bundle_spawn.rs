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


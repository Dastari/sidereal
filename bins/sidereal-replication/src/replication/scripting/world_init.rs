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


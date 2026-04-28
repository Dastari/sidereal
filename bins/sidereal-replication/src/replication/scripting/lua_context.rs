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
    inject_load_ship_authoring_fns(ctx.clone(), module.lua(), script_catalog.clone())
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

fn inject_load_ship_authoring_fns(
    ctx: Table,
    lua: &Lua,
    script_catalog: Arc<ScriptCatalogResource>,
) -> mlua::Result<()> {
    let ship_catalog = script_catalog.clone();
    let load_ship_definition = lua.create_function(move |lua, bundle_or_ship_id: String| {
        let registry =
            load_ship_registry_from_catalog(ship_catalog.as_ref()).map_err(mlua::Error::runtime)?;
        let Some(definition) = registry.definitions.iter().find(|definition| {
            definition.bundle_id == bundle_or_ship_id || definition.ship_id == bundle_or_ship_id
        }) else {
            return Ok(Value::Nil);
        };
        let definition_json = serde_json::to_value(definition).map_err(mlua::Error::runtime)?;
        json_value_to_lua(lua, &definition_json).map_err(mlua::Error::runtime)
    })?;
    ctx.set("load_ship_definition", load_ship_definition)?;

    let module_catalog = script_catalog;
    let load_ship_module_definition = lua.create_function(move |lua, module_id: String| {
        let registry = load_ship_module_registry_from_catalog(module_catalog.as_ref())
            .map_err(mlua::Error::runtime)?;
        let Some(definition) = registry
            .definitions
            .iter()
            .find(|definition| definition.module_id == module_id)
        else {
            return Ok(Value::Nil);
        };
        let definition_json = serde_json::to_value(definition).map_err(mlua::Error::runtime)?;
        json_value_to_lua(lua, &definition_json).map_err(mlua::Error::runtime)
    })?;
    ctx.set("load_ship_module_definition", load_ship_module_definition)?;
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
    inject_load_ship_authoring_fns(ctx.clone(), lua, script_catalog.clone())?;
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

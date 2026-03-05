use mlua::{Function, Table, Value};
use sidereal_persistence::GraphEntityRecord;
use sidereal_scripting::{
    LuaSandboxPolicy, ScriptError, load_lua_module_from_root, load_lua_module_into_lua_from_root,
    lua_value_to_json, resolve_scripts_root, table_get_required_string,
};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldInitScriptConfig {
    pub space_background_shader_asset_id: String,
    pub starfield_shader_asset_id: String,
}

pub fn scripts_root_dir() -> PathBuf {
    let resolved = resolve_scripts_root(env!("CARGO_MANIFEST_DIR"));
    bevy::log::info!(
        "replication scripting root resolved to {}",
        resolved.display()
    );
    resolved
}

pub fn load_world_init_config(root: &Path) -> Result<WorldInitScriptConfig, String> {
    let policy = LuaSandboxPolicy::from_env();
    let module =
        load_lua_module_from_root(root, "world/world_init.lua", &policy).map_err(map_script_err)?;
    let world_defaults = module
        .root()
        .get::<mlua::Table>("world_defaults")
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    let space_background_shader_asset_id = table_get_required_string(
        &world_defaults,
        "space_background_shader_asset_id",
        "world_defaults",
    )
    .map_err(map_script_err)?;
    let starfield_shader_asset_id = table_get_required_string(
        &world_defaults,
        "starfield_shader_asset_id",
        "world_defaults",
    )
    .map_err(map_script_err)?;
    Ok(WorldInitScriptConfig {
        space_background_shader_asset_id,
        starfield_shader_asset_id,
    })
    .inspect(|config| {
        bevy::log::info!(
            "replication loaded world init config: space_background_shader_asset_id={} starfield_shader_asset_id={}",
            config.space_background_shader_asset_id,
            config.starfield_shader_asset_id
        );
    })
}

pub fn load_world_init_graph_records(root: &Path) -> Result<Vec<GraphEntityRecord>, String> {
    let policy = LuaSandboxPolicy::from_env();
    let module =
        load_lua_module_from_root(root, "world/world_init.lua", &policy).map_err(map_script_err)?;
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
    inject_world_init_context(ctx.clone(), &module, root)?;

    let lua_value = build_graph_records
        .call::<Value>(ctx)
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    let json_value = lua_value_to_json(lua_value).map_err(map_script_err)?;
    let records = serde_json::from_value::<Vec<GraphEntityRecord>>(json_value).map_err(|err| {
        format!(
            "{}: build_graph_records(ctx) must return Vec<GraphEntityRecord>-compatible structure: {err}",
            module.script_path().display()
        )
    })?;
    if records.is_empty() {
        return Err(format!(
            "{}: build_graph_records(ctx) returned empty records",
            module.script_path().display()
        ));
    }
    Ok(records)
}

fn map_script_err(err: ScriptError) -> String {
    err.to_string()
}

fn inject_world_init_context(
    ctx: Table,
    module: &sidereal_scripting::LoadedLuaModule,
    scripts_root: &Path,
) -> Result<(), String> {
    let new_uuid = module
        .lua()
        .create_function(|_, ()| Ok(Uuid::new_v4().to_string()))
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    ctx.set("new_uuid", new_uuid)
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;

    let (entity_registry, entity_registry_path) = load_lua_module_into_lua_from_root(
        module.lua(),
        scripts_root,
        "bundles/entity_registry.lua",
    )
    .map_err(map_script_err)?;
    let build_graph_records = entity_registry
        .get::<Function>("build_graph_records")
        .map_err(|err| format!("{}: {err}", entity_registry_path.display()))?;
    let spawn_bundle_graph_records = module
        .lua()
        .create_function(move |lua, (bundle_id, overrides): (String, Value)| {
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
            build_graph_records.call::<Value>(bundle_ctx)
        })
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    ctx.set("spawn_bundle_graph_records", spawn_bundle_graph_records)
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load_world_init_config, load_world_init_graph_records, scripts_root_dir};

    #[test]
    fn default_world_init_script_loads() {
        let root = scripts_root_dir();
        let config = load_world_init_config(&root).expect("load world init");
        assert!(!config.space_background_shader_asset_id.is_empty());
        assert!(!config.starfield_shader_asset_id.is_empty());
    }

    #[test]
    fn default_world_init_graph_records_script_loads() {
        let root = scripts_root_dir();
        let records = load_world_init_graph_records(&root).expect("load world records");
        assert!(!records.is_empty());
        assert!(
            records
                .iter()
                .any(|record| record.entity_id == "0012ebad-0000-0000-0000-000000000002")
        );
    }
}

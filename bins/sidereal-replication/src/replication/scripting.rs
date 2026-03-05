use mlua::{Function, Lua, Table, Value};
use sidereal_persistence::GraphEntityRecord;
use sidereal_scripting::{
    LuaSandboxPolicy, ScriptError, load_lua_module_from_root, load_lua_module_into_lua_from_root,
    lua_value_to_json, resolve_scripts_root, table_get_required_string,
};
use std::collections::HashSet;
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

pub fn spawn_bundle_graph_records(
    root: &Path,
    bundle_id: &str,
    overrides: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<GraphEntityRecord>, String> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(root, "bundles/entity_registry.lua", &policy)
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

    let lua_value = build_graph_records
        .call::<Value>(bundle_ctx)
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

pub fn emit_bundle_spawned_event(
    root: &Path,
    event: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(root, "bundles/entity_registry.lua", &policy)
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

pub fn load_known_bundle_ids(root: &Path) -> Result<HashSet<String>, String> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(root, "bundles/bundle_registry.lua", &policy)
        .map_err(map_script_err)?;
    let bundles = module
        .root()
        .get::<Table>("bundles")
        .map_err(|err| format!("{}: {err}", module.script_path().display()))?;
    let mut out = HashSet::new();
    for pair in bundles.pairs::<String, Table>() {
        let (bundle_id, _) =
            pair.map_err(|err| format!("{}: {err}", module.script_path().display()))?;
        out.insert(bundle_id);
    }
    if out.is_empty() {
        return Err(format!(
            "{}: bundles table must not be empty",
            module.script_path().display()
        ));
    }
    Ok(out)
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
    use super::{
        load_known_bundle_ids, load_world_init_config, load_world_init_graph_records,
        scripts_root_dir, spawn_bundle_graph_records,
    };

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

    #[test]
    fn bundle_registry_exposes_corvette_bundle() {
        let root = scripts_root_dir();
        let bundles = load_known_bundle_ids(&root).expect("bundle ids");
        assert!(bundles.contains("corvette"));
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
        let records = spawn_bundle_graph_records(&root, "corvette", &overrides).expect("spawn");
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
        let first = spawn_bundle_graph_records(&root, "corvette", &overrides).expect("spawn first");
        let second =
            spawn_bundle_graph_records(&root, "corvette", &overrides).expect("spawn second");
        assert!(!first.is_empty());
        assert!(!second.is_empty());
        assert_ne!(
            first[0].entity_id, second[0].entity_id,
            "root entity IDs should be random UUIDs when no entity_id override is provided"
        );
    }
}

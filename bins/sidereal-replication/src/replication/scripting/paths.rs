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
    LuaSandboxPolicy, PLANET_REGISTRY_SCRIPT_REL_PATH, SHIP_MODULE_REGISTRY_SCRIPT_REL_PATH,
    SHIP_REGISTRY_SCRIPT_REL_PATH, ScriptAssetRegistryEntry, ScriptError,
    WORLD_INIT_SCRIPT_REL_PATH, WorldInitScriptConfig, decode_graph_entity_records,
    inject_script_logger, load_asset_registry_from_source, load_lua_module_from_source,
    load_planet_registry_from_sources, load_ship_module_registry_from_sources,
    load_ship_registry_from_sources, load_world_init_config_from_source, lua_value_to_json,
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

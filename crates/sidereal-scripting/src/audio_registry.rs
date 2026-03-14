use crate::{
    LoadedLuaModule, LuaSandboxPolicy, ScriptError, load_lua_module_from_root,
    load_lua_module_from_source, lua_value_to_json,
};
use mlua::Value;
use serde_json::Value as JsonValue;
use sidereal_audio::{AudioRegistry, apply_clip_defaults, validate_audio_registry};
use std::path::Path;

pub fn load_audio_registry_from_root(scripts_root: &Path) -> Result<AudioRegistry, ScriptError> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(scripts_root, "audio/registry.lua", &policy)?;
    decode_audio_registry_module(&module)
}

pub fn load_audio_registry_from_source(
    source: &str,
    script_path: &Path,
) -> Result<AudioRegistry, ScriptError> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_source(source, script_path, &policy)?;
    decode_audio_registry_module(&module)
}

fn decode_audio_registry_module(module: &LoadedLuaModule) -> Result<AudioRegistry, ScriptError> {
    let mut root_json = lua_value_to_json(Value::Table(module.root().clone()))?;
    normalize_audio_registry_json(&mut root_json);
    let mut registry: AudioRegistry = serde_json::from_value(root_json).map_err(|err| {
        ScriptError::Contract(format!(
            "{}: audio registry decode failed: {err}",
            module.script_path().display()
        ))
    })?;
    apply_clip_defaults(&mut registry);
    validate_audio_registry(&registry).map_err(|err| {
        ScriptError::Contract(format!("{}: {err}", module.script_path().display()))
    })?;
    Ok(registry)
}

fn normalize_audio_registry_json(value: &mut JsonValue) {
    match value {
        JsonValue::Array(values) => {
            for entry in values {
                normalize_audio_registry_json(entry);
            }
        }
        JsonValue::Object(map) => {
            for (key, entry) in map.iter_mut() {
                match key.as_str() {
                    "buses" | "sends" | "environments" | "concurrency_groups" | "clips"
                    | "profiles" | "effects" | "variants" => normalize_empty_object_to_array(entry),
                    "bus_effect_overrides" => normalize_nested_object_values_to_arrays(entry),
                    _ => {}
                }
                normalize_audio_registry_json(entry);
            }
        }
        _ => {}
    }
}

fn normalize_empty_object_to_array(value: &mut JsonValue) {
    if value.as_object().is_some_and(|object| object.is_empty()) {
        *value = JsonValue::Array(Vec::new());
    }
}

fn normalize_nested_object_values_to_arrays(value: &mut JsonValue) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for nested in object.values_mut() {
        normalize_empty_object_to_array(nested);
    }
}

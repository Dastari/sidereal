use mlua::{HookTriggers, Lua, LuaOptions, StdLib, Table, Value, VmState};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;
use tracing::{debug, error, info};

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("script security violation: {0}")]
    Security(String),
    #[error("script io failed: {0}")]
    Io(String),
    #[error("script runtime failed: {0}")]
    Runtime(String),
    #[error("script contract violation: {0}")]
    Contract(String),
}

#[derive(Debug, Clone)]
pub struct LuaSandboxPolicy {
    pub memory_limit_bytes: usize,
    pub instruction_limit: u64,
    pub hook_instruction_interval: u32,
}

impl Default for LuaSandboxPolicy {
    fn default() -> Self {
        Self {
            memory_limit_bytes: 8 * 1024 * 1024,
            instruction_limit: 200_000,
            hook_instruction_interval: 1_000,
        }
    }
}

impl LuaSandboxPolicy {
    pub fn from_env() -> Self {
        let mut policy = Self::default();
        if let Ok(raw) = std::env::var("SIDEREAL_SCRIPT_MEMORY_LIMIT_BYTES")
            && let Ok(parsed) = raw.parse::<usize>()
            && parsed >= 1024
        {
            policy.memory_limit_bytes = parsed;
        }
        if let Ok(raw) = std::env::var("SIDEREAL_SCRIPT_INSTRUCTION_LIMIT")
            && let Ok(parsed) = raw.parse::<u64>()
            && parsed >= 1_000
        {
            policy.instruction_limit = parsed;
        }
        if let Ok(raw) = std::env::var("SIDEREAL_SCRIPT_HOOK_INTERVAL")
            && let Ok(parsed) = raw.parse::<u32>()
            && parsed > 0
        {
            policy.hook_instruction_interval = parsed;
        }
        policy
    }
}

pub struct LoadedLuaModule {
    lua: Lua,
    root: Table,
    script_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptAssetRegistryEntry {
    pub asset_id: String,
    pub shader_family: Option<String>,
    pub source_path: String,
    pub content_type: String,
    pub dependencies: Vec<String>,
    pub bootstrap_required: bool,
    pub editor_schema: Option<ScriptShaderEditorSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptAssetRegistry {
    pub schema_version: u32,
    pub assets: Vec<ScriptAssetRegistryEntry>,
}

impl ScriptAssetRegistry {
    pub fn dependencies_by_asset_id(&self) -> HashMap<String, Vec<String>> {
        let mut out = HashMap::new();
        for asset in &self.assets {
            out.insert(asset.asset_id.clone(), asset.dependencies.clone());
        }
        out
    }

    pub fn bootstrap_required_asset_ids(&self) -> HashSet<String> {
        self.assets
            .iter()
            .filter(|asset| asset.bootstrap_required)
            .map(|asset| asset.asset_id.clone())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptShaderEditorSchema {
    pub uniforms: Vec<ScriptShaderEditorFieldSchema>,
    pub presets: Vec<ScriptShaderEditorPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptShaderEditorFieldSchema {
    pub field_path: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub kind: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub options: Vec<ScriptShaderEditorOption>,
    pub default_value: Option<JsonValue>,
    pub group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScriptShaderEditorOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptShaderEditorPreset {
    pub preset_id: String,
    pub label: String,
    pub description: Option<String>,
    pub values: JsonValue,
}

impl LoadedLuaModule {
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    pub fn root(&self) -> &Table {
        &self.root
    }

    pub fn script_path(&self) -> &Path {
        &self.script_path
    }
}

pub fn resolve_scripts_root(cargo_manifest_dir: &str) -> PathBuf {
    if let Ok(value) = std::env::var("SIDEREAL_SCRIPTS_ROOT") {
        return PathBuf::from(value);
    }
    PathBuf::from(cargo_manifest_dir)
        .join("../../data/scripts")
        .components()
        .collect::<PathBuf>()
}

pub fn load_lua_module_from_root(
    scripts_root: &Path,
    relative_script_path: &str,
    policy: &LuaSandboxPolicy,
) -> Result<LoadedLuaModule, ScriptError> {
    let script_path = resolve_script_path_from_root(scripts_root, relative_script_path)?;
    info!("scripting loading lua module {}", script_path.display());
    let source = std::fs::read_to_string(&script_path)
        .map_err(|err| ScriptError::Io(format!("read {} failed: {err}", script_path.display())))?;
    load_lua_module_from_source(&source, &script_path, policy)
}

pub fn load_lua_module_from_source(
    source: &str,
    script_path: &Path,
    policy: &LuaSandboxPolicy,
) -> Result<LoadedLuaModule, ScriptError> {
    let lua = create_sandboxed_lua(policy)?;
    let module_value = lua
        .load(source)
        .set_name(script_path.to_string_lossy().as_ref())
        .eval::<Value>()
        .map_err(|err| {
            ScriptError::Runtime(format!("eval {} failed: {err}", script_path.display()))
        })?;
    let root = match module_value {
        Value::Table(table) => table,
        _ => {
            return Err(ScriptError::Contract(format!(
                "{} must return a Lua table",
                script_path.display()
            )));
        }
    };

    Ok(LoadedLuaModule {
        lua,
        root,
        script_path: script_path.to_path_buf(),
    })
}

pub fn inject_script_logger(lua: &Lua, ctx: &Table, script_label: &str) -> Result<(), ScriptError> {
    let log = lua
        .create_table()
        .map_err(|err| ScriptError::Runtime(format!("create log table failed: {err}")))?;

    let script_label = script_label.to_string();
    let debug_label = script_label.clone();
    let debug_fn = lua
        .create_function(move |_lua, (_log, message): (Table, Value)| {
            let message = script_log_value_to_string(message);
            debug!(target: "sidereal_script", script = %debug_label, "{message}");
            Ok(())
        })
        .map_err(|err| ScriptError::Runtime(format!("create log.debug failed: {err}")))?;
    log.set("debug", debug_fn)
        .map_err(|err| ScriptError::Runtime(format!("set log.debug failed: {err}")))?;

    let info_label = script_label.clone();
    let info_fn = lua
        .create_function(move |_lua, (_log, message): (Table, Value)| {
            let message = script_log_value_to_string(message);
            info!(target: "sidereal_script", script = %info_label, "{message}");
            Ok(())
        })
        .map_err(|err| ScriptError::Runtime(format!("create log.info failed: {err}")))?;
    log.set("info", info_fn)
        .map_err(|err| ScriptError::Runtime(format!("set log.info failed: {err}")))?;

    let error_fn = lua
        .create_function(move |_lua, (_log, message): (Table, Value)| {
            let message = script_log_value_to_string(message);
            error!(target: "sidereal_script", script = %script_label, "{message}");
            Ok(())
        })
        .map_err(|err| ScriptError::Runtime(format!("create log.error failed: {err}")))?;
    log.set("error", error_fn)
        .map_err(|err| ScriptError::Runtime(format!("set log.error failed: {err}")))?;

    ctx.set("log", log)
        .map_err(|err| ScriptError::Runtime(format!("set ctx.log failed: {err}")))?;
    Ok(())
}

fn script_log_value_to_string(value: Value) -> String {
    match lua_value_to_json(value) {
        Ok(JsonValue::String(value)) => value,
        Ok(other) => other.to_string(),
        Err(err) => format!("<lua-log-decode-error: {err}>"),
    }
}

pub fn load_asset_registry_from_root(
    scripts_root: &Path,
) -> Result<ScriptAssetRegistry, ScriptError> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(scripts_root, "assets/registry.lua", &policy)?;
    decode_asset_registry_module(&module)
}

pub fn load_asset_registry_from_source(
    source: &str,
    script_path: &Path,
) -> Result<ScriptAssetRegistry, ScriptError> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_source(source, script_path, &policy)?;
    decode_asset_registry_module(&module)
}

fn decode_asset_registry_module(
    module: &LoadedLuaModule,
) -> Result<ScriptAssetRegistry, ScriptError> {
    let root = module.root();
    let schema_version_i64 = root.get::<i64>("schema_version").map_err(|err| {
        ScriptError::Contract(format!(
            "{}: schema_version read failed: {err}",
            module.script_path().display()
        ))
    })?;
    if schema_version_i64 < 1 {
        return Err(ScriptError::Contract(format!(
            "{}: schema_version must be >= 1",
            module.script_path().display()
        )));
    }
    let schema_version = u32::try_from(schema_version_i64).map_err(|_| {
        ScriptError::Contract(format!(
            "{}: schema_version must fit u32",
            module.script_path().display()
        ))
    })?;
    let assets_table = root.get::<Table>("assets").map_err(|err| {
        ScriptError::Contract(format!(
            "{}: assets table read failed: {err}",
            module.script_path().display()
        ))
    })?;

    let mut assets = Vec::<ScriptAssetRegistryEntry>::new();
    for (idx, value) in assets_table.sequence_values::<Table>().enumerate() {
        let entry = value.map_err(|err| {
            ScriptError::Contract(format!(
                "{}: assets[{}] decode failed: {err}",
                module.script_path().display(),
                idx + 1
            ))
        })?;
        let context = format!("assets[{}]", idx + 1);
        let asset_id = table_get_required_string(&entry, "asset_id", &context)?;
        let shader_family = match entry.get::<Value>("shader_family").map_err(|err| {
            ScriptError::Contract(format!("{context}.shader_family read failed: {err}"))
        })? {
            Value::Nil => None,
            Value::String(value) => Some(
                value
                    .to_str()
                    .map_err(|err| {
                        ScriptError::Contract(format!(
                            "{context}.shader_family decode failed: {err}"
                        ))
                    })?
                    .to_string(),
            ),
            _ => {
                return Err(ScriptError::Contract(format!(
                    "{context}.shader_family must be a string when present"
                )));
            }
        };
        let source_path = table_get_required_string(&entry, "source_path", &context)?;
        let content_type = table_get_required_string(&entry, "content_type", &context)?;
        let dependencies = match entry.get::<Value>("dependencies").map_err(|err| {
            ScriptError::Contract(format!("{context}.dependencies read failed: {err}"))
        })? {
            Value::Nil => Vec::new(),
            Value::Table(values_table) => {
                let mut out = Vec::new();
                for value in values_table.sequence_values::<String>() {
                    out.push(value.map_err(|err| {
                        ScriptError::Contract(format!(
                            "{context}.dependencies entry decode failed: {err}"
                        ))
                    })?);
                }
                out
            }
            _ => {
                return Err(ScriptError::Contract(format!(
                    "{context}.dependencies must be an array of strings when present"
                )));
            }
        };
        let bootstrap_required = match entry.get::<Value>("bootstrap_required").map_err(|err| {
            ScriptError::Contract(format!("{context}.bootstrap_required read failed: {err}"))
        })? {
            Value::Nil => false,
            Value::Boolean(value) => value,
            _ => {
                return Err(ScriptError::Contract(format!(
                    "{context}.bootstrap_required must be a boolean when present"
                )));
            }
        };
        let editor_schema = decode_optional_shader_editor_schema(&entry, &context)?;
        assets.push(ScriptAssetRegistryEntry {
            asset_id,
            shader_family,
            source_path,
            content_type,
            dependencies,
            bootstrap_required,
            editor_schema,
        });
    }

    validate_asset_registry(&assets)?;
    Ok(ScriptAssetRegistry {
        schema_version,
        assets,
    })
}

fn validate_asset_registry(assets: &[ScriptAssetRegistryEntry]) -> Result<(), ScriptError> {
    let mut seen = HashSet::<String>::new();
    for asset in assets {
        if asset.asset_id.trim().is_empty() {
            return Err(ScriptError::Contract(
                "asset registry entry asset_id must not be empty".to_string(),
            ));
        }
        if asset.source_path.trim().is_empty() {
            return Err(ScriptError::Contract(format!(
                "asset registry entry asset_id={} source_path must not be empty",
                asset.asset_id
            )));
        }
        if asset.content_type.trim().is_empty() {
            return Err(ScriptError::Contract(format!(
                "asset registry entry asset_id={} content_type must not be empty",
                asset.asset_id
            )));
        }
        if !seen.insert(asset.asset_id.clone()) {
            return Err(ScriptError::Contract(format!(
                "asset registry duplicates asset_id={}",
                asset.asset_id
            )));
        }
        validate_shader_editor_schema(asset)?;
    }
    let known = assets
        .iter()
        .map(|asset| asset.asset_id.clone())
        .collect::<HashSet<_>>();
    for asset in assets {
        let mut dep_seen = HashSet::<String>::new();
        for dep in &asset.dependencies {
            if dep == &asset.asset_id {
                return Err(ScriptError::Contract(format!(
                    "asset registry asset_id={} depends on itself",
                    asset.asset_id
                )));
            }
            if !dep_seen.insert(dep.clone()) {
                return Err(ScriptError::Contract(format!(
                    "asset registry asset_id={} duplicates dependency={}",
                    asset.asset_id, dep
                )));
            }
            if !known.contains(dep) {
                return Err(ScriptError::Contract(format!(
                    "asset registry asset_id={} references unknown dependency={}",
                    asset.asset_id, dep
                )));
            }
        }
    }
    Ok(())
}

fn decode_optional_shader_editor_schema(
    entry: &Table,
    context: &str,
) -> Result<Option<ScriptShaderEditorSchema>, ScriptError> {
    let value = entry.get::<Value>("editor_schema").map_err(|err| {
        ScriptError::Contract(format!("{context}.editor_schema read failed: {err}"))
    })?;
    match value {
        Value::Nil => Ok(None),
        Value::Table(table) => Ok(Some(decode_shader_editor_schema(&table, context)?)),
        _ => Err(ScriptError::Contract(format!(
            "{context}.editor_schema must be a table when present"
        ))),
    }
}

fn decode_shader_editor_schema(
    table: &Table,
    context: &str,
) -> Result<ScriptShaderEditorSchema, ScriptError> {
    let uniforms = match table.get::<Value>("uniforms").map_err(|err| {
        ScriptError::Contract(format!(
            "{context}.editor_schema.uniforms read failed: {err}"
        ))
    })? {
        Value::Nil => Vec::new(),
        Value::Table(uniforms_table) => decode_shader_editor_uniforms(&uniforms_table, context)?,
        _ => {
            return Err(ScriptError::Contract(format!(
                "{context}.editor_schema.uniforms must be an object when present"
            )));
        }
    };
    let presets = match table.get::<Value>("presets").map_err(|err| {
        ScriptError::Contract(format!(
            "{context}.editor_schema.presets read failed: {err}"
        ))
    })? {
        Value::Nil => Vec::new(),
        Value::Table(presets_table) => decode_shader_editor_presets(&presets_table, context)?,
        _ => {
            return Err(ScriptError::Contract(format!(
                "{context}.editor_schema.presets must be an array when present"
            )));
        }
    };
    Ok(ScriptShaderEditorSchema { uniforms, presets })
}

fn decode_shader_editor_uniforms(
    uniforms_table: &Table,
    context: &str,
) -> Result<Vec<ScriptShaderEditorFieldSchema>, ScriptError> {
    let mut uniforms = Vec::new();
    for pair in uniforms_table.pairs::<Value, Table>() {
        let (field_key, field_table) = pair.map_err(|err| {
            ScriptError::Contract(format!(
                "{context}.editor_schema.uniforms decode failed: {err}"
            ))
        })?;
        let field_path = match field_key {
            Value::String(value) => value
                .to_str()
                .map_err(|err| {
                    ScriptError::Contract(format!(
                        "{context}.editor_schema.uniforms field key utf8 failed: {err}"
                    ))
                })?
                .to_string(),
            _ => {
                return Err(ScriptError::Contract(format!(
                    "{context}.editor_schema.uniforms keys must be strings"
                )));
            }
        };
        let field_context = format!("{context}.editor_schema.uniforms.{field_path}");
        let kind = table_get_required_string(&field_table, "kind", &field_context)?;
        let options = match field_table.get::<Value>("options").map_err(|err| {
            ScriptError::Contract(format!("{field_context}.options read failed: {err}"))
        })? {
            Value::Nil => Vec::new(),
            Value::Table(options_table) => {
                let mut out = Vec::new();
                for (index, option_value) in options_table.sequence_values::<Table>().enumerate() {
                    let option_table = option_value.map_err(|err| {
                        ScriptError::Contract(format!(
                            "{field_context}.options[{}] decode failed: {err}",
                            index + 1
                        ))
                    })?;
                    let option_context = format!("{field_context}.options[{}]", index + 1);
                    out.push(ScriptShaderEditorOption {
                        value: table_get_required_string(&option_table, "value", &option_context)?,
                        label: table_get_required_string(&option_table, "label", &option_context)?,
                    });
                }
                out
            }
            _ => {
                return Err(ScriptError::Contract(format!(
                    "{field_context}.options must be an array when present"
                )));
            }
        };
        let default_value = match field_table.get::<Value>("default").map_err(|err| {
            ScriptError::Contract(format!("{field_context}.default read failed: {err}"))
        })? {
            Value::Nil => None,
            value => Some(lua_value_to_json(value)?),
        };
        uniforms.push(ScriptShaderEditorFieldSchema {
            field_path,
            label: table_get_optional_string(&field_table, "label", &field_context)?,
            description: table_get_optional_string(&field_table, "description", &field_context)?,
            kind,
            min: table_get_optional_number(&field_table, "min", &field_context)?,
            max: table_get_optional_number(&field_table, "max", &field_context)?,
            step: table_get_optional_number(&field_table, "step", &field_context)?,
            options,
            default_value,
            group: table_get_optional_string(&field_table, "group", &field_context)?,
        });
    }
    uniforms.sort_by(|a, b| a.field_path.cmp(&b.field_path));
    Ok(uniforms)
}

fn decode_shader_editor_presets(
    presets_table: &Table,
    context: &str,
) -> Result<Vec<ScriptShaderEditorPreset>, ScriptError> {
    let mut presets = Vec::new();
    for (index, preset_value) in presets_table.sequence_values::<Table>().enumerate() {
        let preset_table = preset_value.map_err(|err| {
            ScriptError::Contract(format!(
                "{context}.editor_schema.presets[{}] decode failed: {err}",
                index + 1
            ))
        })?;
        let preset_context = format!("{context}.editor_schema.presets[{}]", index + 1);
        let values = match preset_table.get::<Value>("values").map_err(|err| {
            ScriptError::Contract(format!("{preset_context}.values read failed: {err}"))
        })? {
            Value::Nil => {
                return Err(ScriptError::Contract(format!(
                    "{preset_context}.values must be present"
                )));
            }
            value => lua_value_to_json(value)?,
        };
        presets.push(ScriptShaderEditorPreset {
            preset_id: table_get_required_string(&preset_table, "preset_id", &preset_context)?,
            label: table_get_required_string(&preset_table, "label", &preset_context)?,
            description: table_get_optional_string(&preset_table, "description", &preset_context)?,
            values,
        });
    }
    Ok(presets)
}

fn validate_shader_editor_schema(asset: &ScriptAssetRegistryEntry) -> Result<(), ScriptError> {
    let Some(schema) = &asset.editor_schema else {
        return Ok(());
    };
    let mut seen_fields = HashSet::<String>::new();
    for field in &schema.uniforms {
        if field.field_path.trim().is_empty() {
            return Err(ScriptError::Contract(format!(
                "asset registry asset_id={} editor_schema uniform field path must not be empty",
                asset.asset_id
            )));
        }
        if !seen_fields.insert(field.field_path.clone()) {
            return Err(ScriptError::Contract(format!(
                "asset registry asset_id={} duplicates editor_schema uniform={}",
                asset.asset_id, field.field_path
            )));
        }
        if let (Some(min), Some(max)) = (field.min, field.max)
            && min > max
        {
            return Err(ScriptError::Contract(format!(
                "asset registry asset_id={} uniform={} has min > max",
                asset.asset_id, field.field_path
            )));
        }
        if let Some(step) = field.step
            && step <= 0.0
        {
            return Err(ScriptError::Contract(format!(
                "asset registry asset_id={} uniform={} step must be > 0",
                asset.asset_id, field.field_path
            )));
        }
        if field.kind == "Enum" && field.options.is_empty() {
            return Err(ScriptError::Contract(format!(
                "asset registry asset_id={} uniform={} enum kind requires options",
                asset.asset_id, field.field_path
            )));
        }
    }
    let mut seen_presets = HashSet::<String>::new();
    for preset in &schema.presets {
        if !seen_presets.insert(preset.preset_id.clone()) {
            return Err(ScriptError::Contract(format!(
                "asset registry asset_id={} duplicates preset_id={}",
                asset.asset_id, preset.preset_id
            )));
        }
    }
    Ok(())
}

pub fn load_lua_module_into_lua_from_root(
    lua: &Lua,
    scripts_root: &Path,
    relative_script_path: &str,
) -> Result<(Table, PathBuf), ScriptError> {
    let script_path = resolve_script_path_from_root(scripts_root, relative_script_path)?;
    info!("scripting loading lua module {}", script_path.display());
    let source = std::fs::read_to_string(&script_path)
        .map_err(|err| ScriptError::Io(format!("read {} failed: {err}", script_path.display())))?;
    let root = load_lua_module_into_lua_from_source(lua, &source, &script_path)?;
    Ok((root, script_path))
}

pub fn load_lua_module_into_lua_from_source(
    lua: &Lua,
    source: &str,
    script_path: &Path,
) -> Result<Table, ScriptError> {
    let module_value = lua
        .load(source)
        .set_name(script_path.to_string_lossy().as_ref())
        .eval::<Value>()
        .map_err(|err| {
            ScriptError::Runtime(format!("eval {} failed: {err}", script_path.display()))
        })?;
    let root = match module_value {
        Value::Table(table) => table,
        _ => {
            return Err(ScriptError::Contract(format!(
                "{} must return a Lua table",
                script_path.display()
            )));
        }
    };
    Ok(root)
}

pub fn lua_value_to_json(value: Value) -> Result<JsonValue, ScriptError> {
    match value {
        Value::Nil => Ok(JsonValue::Null),
        Value::Boolean(v) => Ok(JsonValue::Bool(v)),
        Value::Integer(v) => Ok(JsonValue::Number(JsonNumber::from(v))),
        Value::Number(v) => JsonNumber::from_f64(v)
            .map(JsonValue::Number)
            .ok_or_else(|| {
                ScriptError::Contract("lua number cannot be represented in json".to_string())
            }),
        Value::String(v) => Ok(JsonValue::String(
            v.to_str()
                .map_err(|err| ScriptError::Contract(format!("lua string utf8 failed: {err}")))?
                .to_string(),
        )),
        Value::Table(table) => lua_table_to_json(table),
        _ => Err(ScriptError::Contract(
            "unsupported lua value in graph records payload".to_string(),
        )),
    }
}

pub fn table_get_required_string(
    table: &Table,
    key: &str,
    context: &str,
) -> Result<String, ScriptError> {
    let value = table
        .get::<Value>(key)
        .map_err(|err| ScriptError::Contract(format!("{context}.{key} read failed: {err}")))?;
    match value {
        Value::String(value) => Ok(value
            .to_str()
            .map_err(|err| ScriptError::Contract(format!("{context}.{key} utf8 failed: {err}")))?
            .to_string()),
        _ => Err(ScriptError::Contract(format!(
            "{context}.{key} must be a string"
        ))),
    }
}

pub fn table_get_optional_string(
    table: &Table,
    key: &str,
    context: &str,
) -> Result<Option<String>, ScriptError> {
    let value = table
        .get::<Value>(key)
        .map_err(|err| ScriptError::Contract(format!("{context}.{key} read failed: {err}")))?;
    match value {
        Value::Nil => Ok(None),
        Value::String(value) => Ok(Some(
            value
                .to_str()
                .map_err(|err| {
                    ScriptError::Contract(format!("{context}.{key} utf8 failed: {err}"))
                })?
                .to_string(),
        )),
        _ => Err(ScriptError::Contract(format!(
            "{context}.{key} must be a string when present"
        ))),
    }
}

pub fn table_get_optional_number(
    table: &Table,
    key: &str,
    context: &str,
) -> Result<Option<f64>, ScriptError> {
    let value = table
        .get::<Value>(key)
        .map_err(|err| ScriptError::Contract(format!("{context}.{key} read failed: {err}")))?;
    match value {
        Value::Nil => Ok(None),
        Value::Integer(value) => Ok(Some(value as f64)),
        Value::Number(value) => Ok(Some(value)),
        _ => Err(ScriptError::Contract(format!(
            "{context}.{key} must be a number when present"
        ))),
    }
}

pub fn table_get_required_string_list(
    table: &Table,
    key: &str,
    context: &str,
) -> Result<Vec<String>, ScriptError> {
    let values_table = table
        .get::<Table>(key)
        .map_err(|err| ScriptError::Contract(format!("{context}.{key} read failed: {err}")))?;
    let mut out = Vec::new();
    for value in values_table.sequence_values::<String>() {
        out.push(value.map_err(|err| {
            ScriptError::Contract(format!("{context}.{key} entry decode failed: {err}"))
        })?);
    }
    if out.is_empty() {
        return Err(ScriptError::Contract(format!(
            "{context}.{key} must include at least one entry"
        )));
    }
    Ok(out)
}

pub fn validate_component_kinds(
    known_component_kinds: &std::collections::HashSet<String>,
    component_kinds: &[String],
    context: &str,
) -> Result<(), ScriptError> {
    let mut seen = std::collections::HashSet::<String>::new();
    for kind in component_kinds {
        if !known_component_kinds.contains(kind) {
            return Err(ScriptError::Contract(format!(
                "{} references unknown component kind={}",
                context, kind
            )));
        }
        if !seen.insert(kind.clone()) {
            return Err(ScriptError::Contract(format!(
                "{} duplicates component kind={}",
                context, kind
            )));
        }
    }
    Ok(())
}

fn create_sandboxed_lua(policy: &LuaSandboxPolicy) -> Result<Lua, ScriptError> {
    let libs = StdLib::ALL_SAFE ^ StdLib::IO ^ StdLib::OS ^ StdLib::PACKAGE;
    let lua = Lua::new_with(libs, LuaOptions::default())
        .map_err(|err| ScriptError::Runtime(format!("create sandboxed lua failed: {err}")))?;
    let globals = lua.globals();
    globals
        .set("dofile", Value::Nil)
        .map_err(|err| ScriptError::Runtime(format!("disable dofile failed: {err}")))?;
    globals
        .set("loadfile", Value::Nil)
        .map_err(|err| ScriptError::Runtime(format!("disable loadfile failed: {err}")))?;
    globals
        .set("require", Value::Nil)
        .map_err(|err| ScriptError::Runtime(format!("disable require failed: {err}")))?;
    lua.set_memory_limit(policy.memory_limit_bytes)
        .map_err(|err| {
            ScriptError::Runtime(format!(
                "set lua memory limit {} failed: {err}",
                policy.memory_limit_bytes
            ))
        })?;
    let instruction_counter = Arc::new(AtomicU64::new(0));
    let _ = lua.set_app_data(Arc::clone(&instruction_counter));
    let instruction_limit = policy.instruction_limit;
    let hook_interval = policy.hook_instruction_interval as u64;
    let counter = Arc::clone(&instruction_counter);
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(policy.hook_instruction_interval),
        move |_lua, _debug| {
            let executed = counter.fetch_add(hook_interval, Ordering::Relaxed) + hook_interval;
            if executed > instruction_limit {
                return Err(mlua::Error::runtime(
                    "lua instruction budget exceeded; script aborted",
                ));
            }
            Ok(VmState::Continue)
        },
    )
    .map_err(|err| ScriptError::Runtime(format!("set lua hook failed: {err}")))?;
    Ok(lua)
}

pub fn reset_lua_instruction_budget(lua: &Lua) {
    if let Some(counter) = lua.app_data_ref::<Arc<AtomicU64>>() {
        counter.store(0, Ordering::Relaxed);
    }
}

pub fn create_sandboxed_lua_vm(policy: &LuaSandboxPolicy) -> Result<Lua, ScriptError> {
    create_sandboxed_lua(policy)
}

fn lua_table_to_json(table: Table) -> Result<JsonValue, ScriptError> {
    let mut int_entries = Vec::<(i64, JsonValue)>::new();
    let mut obj_entries = JsonMap::<String, JsonValue>::new();
    let mut has_int = false;
    let mut has_obj = false;

    for pair in table.pairs::<Value, Value>() {
        let (key, value) =
            pair.map_err(|err| ScriptError::Contract(format!("lua table decode failed: {err}")))?;
        let value = lua_value_to_json(value)?;
        match key {
            Value::Integer(i) if i >= 1 => {
                has_int = true;
                int_entries.push((i, value));
            }
            Value::String(s) => {
                has_obj = true;
                let key = s
                    .to_str()
                    .map_err(|err| ScriptError::Contract(format!("lua key utf8 failed: {err}")))?
                    .to_string();
                obj_entries.insert(key, value);
            }
            _ => {
                return Err(ScriptError::Contract(
                    "lua table keys must be positive integers or strings".to_string(),
                ));
            }
        }
    }

    if has_int && has_obj {
        return Err(ScriptError::Contract(
            "lua table cannot mix array and object keys".to_string(),
        ));
    }
    if has_int {
        int_entries.sort_by_key(|(idx, _)| *idx);
        let max = int_entries.last().map(|(idx, _)| *idx).unwrap_or(0);
        if max as usize != int_entries.len() {
            return Err(ScriptError::Contract(
                "lua array table must have contiguous integer keys starting at 1".to_string(),
            ));
        }
        let values = int_entries.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
        Ok(JsonValue::Array(values))
    } else {
        Ok(JsonValue::Object(obj_entries))
    }
}

pub fn resolve_script_path_from_root(
    scripts_root: &Path,
    relative_script_path: &str,
) -> Result<PathBuf, ScriptError> {
    let relative = Path::new(relative_script_path);
    if relative.is_absolute() {
        return Err(ScriptError::Security(format!(
            "script path must be relative: {}",
            relative_script_path
        )));
    }
    if relative.extension().and_then(|v| v.to_str()) != Some("lua") {
        return Err(ScriptError::Security(format!(
            "script path must end with .lua: {}",
            relative_script_path
        )));
    }

    let canonical_root = scripts_root.canonicalize().map_err(|err| {
        ScriptError::Io(format!(
            "canonicalize scripts root {} failed: {err}",
            scripts_root.display()
        ))
    })?;
    let candidate = canonical_root.join(relative);
    let canonical_candidate = candidate.canonicalize().map_err(|err| {
        ScriptError::Io(format!(
            "canonicalize script path {} failed: {err}",
            candidate.display()
        ))
    })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(ScriptError::Security(format!(
            "script path escapes scripts root: {}",
            relative_script_path
        )));
    }
    Ok(canonical_candidate)
}

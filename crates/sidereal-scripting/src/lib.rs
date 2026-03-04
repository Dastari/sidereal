use mlua::{HookTriggers, Lua, LuaOptions, StdLib, Table, Value, VmState};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;
use tracing::info;

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
    let lua = create_sandboxed_lua(policy)?;
    let module_value = lua
        .load(&source)
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
        script_path,
    })
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
    let module_value = lua
        .load(&source)
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
    Ok((root, script_path))
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

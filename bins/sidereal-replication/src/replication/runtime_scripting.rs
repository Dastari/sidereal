use avian2d::prelude::Position;
use bevy::prelude::*;
use mlua::{Function, Lua, RegistryKey, Table, UserData, UserDataMethods, Value};
use serde_json::Value as JsonValue;
use sidereal_game::{EntityGuid, FlightComputer, OwnerId, ScriptState, ScriptValue};
use sidereal_net::PlayerEntityId;
use sidereal_scripting::{
    LuaSandboxPolicy, ScriptError, create_sandboxed_lua_vm, inject_script_logger,
    load_lua_module_into_lua_from_source, lua_value_to_json, reset_lua_instruction_budget,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;
use uuid::Uuid;

use crate::replication::scripting::{ScriptCatalogResource, lookup_script_catalog_entry};

#[derive(Clone)]
struct ScriptEntitySnapshot {
    guid: String,
    position: Vec2,
    script_state: Option<JsonValue>,
}

impl UserData for ScriptEntitySnapshot {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("guid", |_lua, this, ()| Ok(this.guid.clone()));
        methods.add_method("position", |lua, this, ()| {
            let table = lua.create_table()?;
            table.set("x", this.position.x)?;
            table.set("y", this.position.y)?;
            Ok(table)
        });
        methods.add_method("has", |_lua, this, component_kind: String| {
            Ok(component_kind == "script_state" && this.script_state.is_some())
        });
        methods.add_method("get", |lua, this, component_kind: String| {
            if component_kind != "script_state" {
                return Ok(Value::Nil);
            }
            match &this.script_state {
                Some(value) => json_to_lua_value(lua, value)
                    .map_err(|err| mlua::Error::runtime(err.to_string())),
                None => Ok(Value::Nil),
            }
        });
    }
}

#[derive(Resource, Default)]
pub struct ScriptWorldSnapshot {
    entities_by_guid: HashMap<String, ScriptEntitySnapshot>,
}

struct ScriptHandler {
    name: String,
    on_tick_function_key: Option<RegistryKey>,
    default_tick_interval_s: f64,
    on_event_function_keys: HashMap<String, RegistryKey>,
}

#[derive(Debug, Clone)]
pub struct ScriptEvent {
    pub event_name: String,
    pub payload: JsonValue,
    pub target_entity_id: Option<String>,
}

#[derive(Resource, Default)]
pub struct ScriptEventQueue {
    pub pending: Vec<ScriptEvent>,
}

#[derive(Debug, Clone, Resource)]
pub struct ScriptRuntimeMetrics {
    pub memory_limit_bytes: u64,
    pub current_memory_bytes: Option<u64>,
    pub interval_runs: u64,
    pub event_runs: u64,
    pub error_count: u64,
    pub reload_count: u64,
    pub last_interval_run_ms: Option<f64>,
    pub last_event_run_ms: Option<f64>,
}

impl Default for ScriptRuntimeMetrics {
    fn default() -> Self {
        let policy = LuaSandboxPolicy::from_env();
        Self {
            memory_limit_bytes: policy.memory_limit_bytes as u64,
            current_memory_bytes: None,
            interval_runs: 0,
            event_runs: 0,
            error_count: 0,
            reload_count: 0,
            last_interval_run_ms: None,
            last_event_run_ms: None,
        }
    }
}

enum ScriptIntent {
    FlyTowards {
        entity_id: Uuid,
        target: Vec2,
    },
    Stop {
        entity_id: Uuid,
    },
    SetScriptState {
        entity_id: Uuid,
        key: String,
        value: JsonValue,
    },
}

pub struct ScriptRuntime {
    lua: Lua,
    handlers: HashMap<String, ScriptHandler>,
    next_tick_run_by_entity_handler: HashMap<String, f64>,
    pending_intents: Vec<ScriptIntent>,
    catalog_revision: u64,
}

impl ScriptRuntime {
    fn from_catalog(catalog: &ScriptCatalogResource) -> Result<Self, ScriptError> {
        let policy = LuaSandboxPolicy::from_env();
        let lua = create_sandboxed_lua_vm(&policy)?;
        let mut handlers = HashMap::new();

        for script_rel_path in discover_ai_script_paths(catalog) {
            let entry = lookup_script_catalog_entry(catalog, &script_rel_path)
                .map_err(ScriptError::Contract)?;
            let module_path = Path::new(&script_rel_path).to_path_buf();
            let module = load_lua_module_into_lua_from_source(&lua, &entry.source, &module_path)?;
            let default_handler_name = Path::new(&script_rel_path)
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("unknown")
                .to_string();
            let handler_name = module
                .get::<Option<String>>("handler_name")
                .map_err(|err| ScriptError::Contract(format!("{}: {err}", module_path.display())))?
                .unwrap_or(default_handler_name);

            let mut on_tick_function_key = None;
            if let Ok(on_tick) = module.get::<Function>("on_tick") {
                on_tick_function_key = Some(lua.create_registry_value(on_tick).map_err(|err| {
                    ScriptError::Runtime(format!("{}: {err}", module_path.display()))
                })?);
            }
            let default_tick_interval_s = module
                .get::<Option<f64>>("tick_interval_seconds")
                .map_err(|err| ScriptError::Contract(format!("{}: {err}", module_path.display())))?
                .unwrap_or(2.0);
            if default_tick_interval_s <= 0.0 {
                return Err(ScriptError::Contract(format!(
                    "{}: tick_interval_seconds must be > 0",
                    module_path.display()
                )));
            }

            let mut on_event_function_keys = HashMap::new();
            for pair in module.clone().pairs::<String, Value>() {
                let (key, value) = pair.map_err(|err| {
                    ScriptError::Contract(format!("{}: {err}", module_path.display()))
                })?;
                if !key.starts_with("on_") || key == "on_tick" {
                    continue;
                }
                if let Value::Function(func) = value {
                    let event_name = key.trim_start_matches("on_").to_string();
                    let key = lua.create_registry_value(func).map_err(|err| {
                        ScriptError::Runtime(format!("{}: {err}", module_path.display()))
                    })?;
                    on_event_function_keys.insert(event_name, key);
                }
            }

            handlers.insert(
                handler_name.clone(),
                ScriptHandler {
                    name: handler_name,
                    on_tick_function_key,
                    default_tick_interval_s,
                    on_event_function_keys,
                },
            );
        }

        Ok(Self {
            lua,
            handlers,
            next_tick_run_by_entity_handler: HashMap::new(),
            pending_intents: Vec::new(),
            catalog_revision: catalog.revision,
        })
    }
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(ScriptWorldSnapshot::default());
    app.insert_resource(ScriptEventQueue::default());
    app.insert_resource(ScriptRuntimeMetrics::default());
    let catalog = app.world().resource::<ScriptCatalogResource>().clone();
    match ScriptRuntime::from_catalog(&catalog) {
        Ok(runtime) => {
            debug!(
                "replication runtime scripting initialized root={} handlers={} catalog_revision={}",
                catalog.root_dir,
                runtime.handlers.len(),
                catalog.revision
            );
            app.insert_non_send_resource(runtime);
        }
        Err(err) => {
            warn!(
                "replication runtime scripting disabled: root={} error={}",
                catalog.root_dir, err
            );
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn refresh_script_world_snapshot(
    mut snapshot: ResMut<'_, ScriptWorldSnapshot>,
    query: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ Position>,
            Option<&'_ Transform>,
            Option<&'_ ScriptState>,
        ),
    >,
) {
    snapshot.entities_by_guid.clear();
    for (guid, position, transform, script_state) in &query {
        let pos = position
            .map(|v| v.0)
            .or_else(|| transform.map(|t| t.translation.truncate()))
            .unwrap_or(Vec2::ZERO);
        snapshot.entities_by_guid.insert(
            guid.0.to_string(),
            ScriptEntitySnapshot {
                guid: guid.0.to_string(),
                position: pos,
                script_state: script_state.map(script_state_to_json),
            },
        );
    }
}

pub fn run_script_intervals(
    runtime: Option<NonSendMut<'_, ScriptRuntime>>,
    catalog: Res<'_, ScriptCatalogResource>,
    snapshot: Res<'_, ScriptWorldSnapshot>,
    time: Res<'_, Time>,
    mut metrics: ResMut<'_, ScriptRuntimeMetrics>,
) {
    let Some(mut runtime) = runtime else { return };
    if runtime.catalog_revision != catalog.revision {
        match ScriptRuntime::from_catalog(&catalog) {
            Ok(new_runtime) => {
                *runtime = new_runtime;
                metrics.reload_count = metrics.reload_count.saturating_add(1);
            }
            Err(err) => {
                metrics.error_count = metrics.error_count.saturating_add(1);
                warn!(
                    "replication runtime scripting reload failed root={} catalog_revision={} error={}",
                    catalog.root_dir, catalog.revision, err
                );
                return;
            }
        }
    }
    let now_s = time.elapsed_secs_f64();
    if runtime.handlers.is_empty() {
        return;
    }
    let interval_started_at = Instant::now();
    let snapshot_map = Rc::new(snapshot.entities_by_guid.clone());
    for (entity_guid, entity) in &*snapshot_map {
        let Some((handler_name, interval_s)) = parse_tick_handler_config(entity) else {
            continue;
        };
        let Some((handler_log_name, default_tick_interval_s, has_on_tick)) =
            runtime.handlers.get(&handler_name).map(|handler| {
                (
                    handler.name.clone(),
                    handler.default_tick_interval_s,
                    handler.on_tick_function_key.is_some(),
                )
            })
        else {
            metrics.error_count = metrics.error_count.saturating_add(1);
            warn!(
                "replication runtime scripting entity={} references unknown on_tick_handler={}",
                entity_guid, handler_name
            );
            continue;
        };
        if !has_on_tick {
            continue;
        }
        let interval_s = interval_s.unwrap_or(default_tick_interval_s);
        if interval_s <= 0.0 {
            continue;
        }
        let schedule_key = format!("{}::{}", handler_log_name, entity_guid);
        if let Some(next_run_s) = runtime.next_tick_run_by_entity_handler.get(&schedule_key)
            && now_s < *next_run_s
        {
            continue;
        }
        runtime
            .next_tick_run_by_entity_handler
            .insert(schedule_key, now_s + interval_s);

        let pending_intents = Rc::new(RefCell::new(Vec::<ScriptIntent>::new()));
        let ctx = match build_script_context(
            &runtime.lua,
            Rc::clone(&snapshot_map),
            pending_intents.clone(),
            handler_log_name.as_str(),
        ) {
            Ok(v) => v,
            Err(err) => {
                metrics.error_count = metrics.error_count.saturating_add(1);
                warn!(
                    "replication runtime script on_tick handler={} entity={} context build failed: {}",
                    handler_log_name, entity_guid, err
                );
                continue;
            }
        };
        let event_payload = serde_json::json!({ "entity_id": entity_guid });
        let event = match json_to_lua_value(&runtime.lua, &event_payload) {
            Ok(v) => v,
            Err(err) => {
                metrics.error_count = metrics.error_count.saturating_add(1);
                warn!(
                    "replication runtime script on_tick handler={} entity={} event encode failed: {}",
                    handler_log_name, entity_guid, err
                );
                continue;
            }
        };
        let Some(function_key) = runtime
            .handlers
            .get(&handler_name)
            .and_then(|handler| handler.on_tick_function_key.as_ref())
        else {
            continue;
        };
        let function = match runtime.lua.registry_value::<Function>(function_key) {
            Ok(v) => v,
            Err(err) => {
                metrics.error_count = metrics.error_count.saturating_add(1);
                warn!(
                    "replication runtime script on_tick handler={} entity={} registry decode failed: {}",
                    handler_log_name, entity_guid, err
                );
                continue;
            }
        };
        reset_lua_instruction_budget(&runtime.lua);
        if let Err(err) = function.call::<()>((ctx, event)) {
            metrics.error_count = metrics.error_count.saturating_add(1);
            warn!(
                "replication runtime script on_tick handler={} entity={} execution failed: {}",
                handler_log_name, entity_guid, err
            );
            continue;
        }
        metrics.interval_runs = metrics.interval_runs.saturating_add(1);
        runtime
            .pending_intents
            .extend(pending_intents.borrow_mut().drain(..));
    }
    metrics.last_interval_run_ms = Some(interval_started_at.elapsed().as_secs_f64() * 1000.0);
    metrics.current_memory_bytes = None;
}

pub fn run_script_events(
    runtime: Option<NonSendMut<'_, ScriptRuntime>>,
    catalog: Res<'_, ScriptCatalogResource>,
    snapshot: Res<'_, ScriptWorldSnapshot>,
    mut event_queue: ResMut<'_, ScriptEventQueue>,
    mut metrics: ResMut<'_, ScriptRuntimeMetrics>,
) {
    let Some(mut runtime) = runtime else { return };
    if runtime.catalog_revision != catalog.revision {
        match ScriptRuntime::from_catalog(&catalog) {
            Ok(new_runtime) => {
                *runtime = new_runtime;
                metrics.reload_count = metrics.reload_count.saturating_add(1);
            }
            Err(err) => {
                metrics.error_count = metrics.error_count.saturating_add(1);
                warn!(
                    "replication runtime scripting reload failed root={} catalog_revision={} error={}",
                    catalog.root_dir, catalog.revision, err
                );
                return;
            }
        }
    }
    if runtime.handlers.is_empty() || event_queue.pending.is_empty() {
        return;
    }
    let events_started_at = Instant::now();
    let snapshot_map = Rc::new(snapshot.entities_by_guid.clone());
    let events = std::mem::take(&mut event_queue.pending);
    for event in events {
        let target_entities = if let Some(entity_id) = &event.target_entity_id {
            vec![entity_id.clone()]
        } else {
            snapshot_map.keys().cloned().collect::<Vec<_>>()
        };
        for entity_guid in target_entities {
            let Some(entity) = snapshot_map.get(&entity_guid) else {
                continue;
            };
            let Some(handler_name) = parse_event_handler_config(entity, &event.event_name) else {
                continue;
            };
            let Some(handler) = runtime.handlers.get(&handler_name) else {
                metrics.error_count = metrics.error_count.saturating_add(1);
                warn!(
                    "replication runtime scripting entity={} event={} references unknown handler={}",
                    entity_guid, event.event_name, handler_name
                );
                continue;
            };
            let Some(function_key) = handler.on_event_function_keys.get(&event.event_name) else {
                continue;
            };

            let pending_intents = Rc::new(RefCell::new(Vec::<ScriptIntent>::new()));
            let ctx = match build_script_context(
                &runtime.lua,
                Rc::clone(&snapshot_map),
                pending_intents.clone(),
                handler_name.as_str(),
            ) {
                Ok(v) => v,
                Err(err) => {
                    metrics.error_count = metrics.error_count.saturating_add(1);
                    warn!(
                        "replication runtime script on_{} handler={} entity={} context build failed: {}",
                        event.event_name, handler.name, entity_guid, err
                    );
                    continue;
                }
            };
            let mut event_payload = event.payload.clone();
            let has_entity_id = event_payload
                .as_object()
                .is_some_and(|obj| obj.contains_key("entity_id"));
            if !has_entity_id {
                let mut payload_map = event_payload.as_object().cloned().unwrap_or_default();
                payload_map.insert(
                    "entity_id".to_string(),
                    JsonValue::String(entity_guid.clone()),
                );
                event_payload = JsonValue::Object(payload_map);
            }
            let event_lua = match json_to_lua_value(&runtime.lua, &event_payload) {
                Ok(v) => v,
                Err(err) => {
                    metrics.error_count = metrics.error_count.saturating_add(1);
                    warn!(
                        "replication runtime script on_{} handler={} entity={} event encode failed: {}",
                        event.event_name, handler.name, entity_guid, err
                    );
                    continue;
                }
            };
            let function = match runtime.lua.registry_value::<Function>(function_key) {
                Ok(v) => v,
                Err(err) => {
                    metrics.error_count = metrics.error_count.saturating_add(1);
                    warn!(
                        "replication runtime script on_{} handler={} entity={} registry decode failed: {}",
                        event.event_name, handler.name, entity_guid, err
                    );
                    continue;
                }
            };
            reset_lua_instruction_budget(&runtime.lua);
            if let Err(err) = function.call::<()>((ctx, event_lua)) {
                metrics.error_count = metrics.error_count.saturating_add(1);
                warn!(
                    "replication runtime script on_{} handler={} entity={} execution failed: {}",
                    event.event_name, handler.name, entity_guid, err
                );
                continue;
            }
            metrics.event_runs = metrics.event_runs.saturating_add(1);
            runtime
                .pending_intents
                .extend(pending_intents.borrow_mut().drain(..));
        }
    }
    metrics.last_event_run_ms = Some(events_started_at.elapsed().as_secs_f64() * 1000.0);
    metrics.current_memory_bytes = None;
}

#[allow(clippy::type_complexity)]
pub fn apply_script_intents(
    runtime: Option<NonSendMut<'_, ScriptRuntime>>,
    mut query: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            &'_ Transform,
            Option<&'_ OwnerId>,
            Option<&'_ mut ScriptState>,
            &'_ mut FlightComputer,
        ),
    >,
) {
    let Some(mut runtime) = runtime else { return };
    if runtime.pending_intents.is_empty() {
        return;
    }
    let intents = std::mem::take(&mut runtime.pending_intents);
    for intent in intents {
        match intent {
            ScriptIntent::FlyTowards { entity_id, target } => {
                for (guid, transform, owner_id, script_state, mut computer) in &mut query {
                    if guid.0 != entity_id {
                        continue;
                    }
                    if !is_script_controllable(owner_id, script_state.as_deref()) {
                        break;
                    }
                    let position = transform.translation.truncate();
                    let to_target = target - position;
                    if to_target.length_squared() < 4.0 {
                        computer.throttle = 0.0;
                        computer.yaw_input = 0.0;
                        computer.brake_active = true;
                        break;
                    }
                    let desired = to_target.normalize();
                    let forward = transform.up().truncate();
                    let cross = forward.perp_dot(desired);
                    computer.yaw_input = if cross > 0.08 {
                        1.0
                    } else if cross < -0.08 {
                        -1.0
                    } else {
                        0.0
                    };
                    computer.throttle = 1.0;
                    computer.brake_active = false;
                    break;
                }
            }
            ScriptIntent::Stop { entity_id } => {
                for (guid, _transform, owner_id, script_state, mut computer) in &mut query {
                    if guid.0 != entity_id {
                        continue;
                    }
                    if !is_script_controllable(owner_id, script_state.as_deref()) {
                        break;
                    }
                    computer.throttle = 0.0;
                    computer.yaw_input = 0.0;
                    computer.brake_active = true;
                    break;
                }
            }
            ScriptIntent::SetScriptState {
                entity_id,
                key,
                value,
            } => {
                for (guid, _transform, owner_id, script_state, _computer) in &mut query {
                    if guid.0 != entity_id {
                        continue;
                    }
                    let Some(mut script_state) = script_state else {
                        break;
                    };
                    if !is_script_controllable(owner_id, Some(&script_state)) {
                        break;
                    }
                    script_state
                        .data
                        .insert(key.clone(), json_to_script_value(&value));
                    break;
                }
            }
        }
    }
}

fn is_script_controllable(owner_id: Option<&OwnerId>, script_state: Option<&ScriptState>) -> bool {
    if script_state.is_none() {
        return false;
    }
    owner_id.is_some_and(|owner| PlayerEntityId::parse(owner.0.as_str()).is_none())
}

fn build_script_context(
    lua: &Lua,
    snapshot_map: Rc<HashMap<String, ScriptEntitySnapshot>>,
    pending_intents: Rc<RefCell<Vec<ScriptIntent>>>,
    script_label: &str,
) -> Result<Table, ScriptError> {
    let ctx = lua
        .create_table()
        .map_err(|err| ScriptError::Runtime(format!("create script ctx failed: {err}")))?;
    inject_script_logger(lua, &ctx, script_label)?;
    let world = lua
        .create_table()
        .map_err(|err| ScriptError::Runtime(format!("create script world failed: {err}")))?;

    let entities_for_lookup = Rc::clone(&snapshot_map);
    let find_entity = lua
        .create_function(move |_lua, (_world, guid): (Table, String)| {
            Ok(entities_for_lookup.get(&guid).cloned())
        })
        .map_err(|err| ScriptError::Runtime(format!("create find_entity failed: {err}")))?;
    world
        .set("find_entity", find_entity)
        .map_err(|err| ScriptError::Runtime(format!("set find_entity failed: {err}")))?;
    ctx.set("world", world)
        .map_err(|err| ScriptError::Runtime(format!("set world failed: {err}")))?;

    let emit_intents = Rc::clone(&pending_intents);
    let emit_intent = lua
        .create_function(
            move |_lua, (_ctx, action, payload): (Table, String, Value)| {
                let payload_json = lua_value_to_json(payload).map_err(|err| {
                    mlua::Error::runtime(format!("intent payload decode failed: {err}"))
                })?;
                let intent =
                    parse_intent(action.as_str(), &payload_json).map_err(mlua::Error::runtime)?;
                emit_intents.borrow_mut().push(intent);
                Ok(())
            },
        )
        .map_err(|err| ScriptError::Runtime(format!("create emit_intent failed: {err}")))?;
    ctx.set("emit_intent", emit_intent)
        .map_err(|err| ScriptError::Runtime(format!("set emit_intent failed: {err}")))?;
    Ok(ctx)
}

fn parse_intent(action: &str, payload: &JsonValue) -> Result<ScriptIntent, String> {
    match action {
        "fly_towards" => {
            let entity_id = payload
                .get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "fly_towards requires payload.entity_id".to_string())
                .and_then(parse_uuid)?;
            let target_obj = payload
                .get("target_position")
                .and_then(|v| v.as_object())
                .ok_or_else(|| "fly_towards requires payload.target_position".to_string())?;
            let x = target_obj
                .get("x")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| "fly_towards target_position.x must be number".to_string())?
                as f32;
            let y = target_obj
                .get("y")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| "fly_towards target_position.y must be number".to_string())?
                as f32;
            Ok(ScriptIntent::FlyTowards {
                entity_id,
                target: Vec2::new(x, y),
            })
        }
        "stop" => {
            let entity_id = payload
                .get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "stop requires payload.entity_id".to_string())
                .and_then(parse_uuid)?;
            Ok(ScriptIntent::Stop { entity_id })
        }
        "set_script_state" => {
            let entity_id = payload
                .get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "set_script_state requires payload.entity_id".to_string())
                .and_then(parse_uuid)?;
            let key = payload
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "set_script_state requires payload.key".to_string())?
                .to_string();
            let value = payload
                .get("value")
                .cloned()
                .ok_or_else(|| "set_script_state requires payload.value".to_string())?;
            Ok(ScriptIntent::SetScriptState {
                entity_id,
                key,
                value,
            })
        }
        other => Err(format!("unsupported intent action={other}")),
    }
}

fn parse_tick_handler_config(entity: &ScriptEntitySnapshot) -> Option<(String, Option<f64>)> {
    let data = entity.script_state.as_ref()?.get("data")?.as_object()?;
    let handler = data.get("on_tick_handler")?.as_str()?.to_string();
    let interval_s = data.get("tick_interval_s").and_then(|v| v.as_f64());
    Some((handler, interval_s))
}

fn parse_event_handler_config(entity: &ScriptEntitySnapshot, event_name: &str) -> Option<String> {
    let data = entity.script_state.as_ref()?.get("data")?.as_object()?;
    let hooks = data.get("event_hooks")?.as_object()?;
    hooks.get(event_name)?.as_str().map(|v| v.to_string())
}

fn parse_uuid(raw: &str) -> Result<Uuid, String> {
    Uuid::parse_str(raw).map_err(|err| format!("invalid uuid {raw}: {err}"))
}

fn discover_ai_script_paths(catalog: &ScriptCatalogResource) -> Vec<String> {
    let mut rel_paths = catalog
        .entries
        .iter()
        .filter(|entry| entry.script_path.starts_with("ai/"))
        .map(|entry| entry.script_path.clone())
        .collect::<Vec<_>>();
    rel_paths.sort();
    rel_paths
}

fn json_to_lua_value(lua: &Lua, value: &JsonValue) -> Result<Value, ScriptError> {
    match value {
        JsonValue::Null => Ok(Value::Nil),
        JsonValue::Bool(v) => Ok(Value::Boolean(*v)),
        JsonValue::Number(v) => v
            .as_f64()
            .map(Value::Number)
            .ok_or_else(|| ScriptError::Contract("json number not representable".to_string())),
        JsonValue::String(v) => lua
            .create_string(v.as_str())
            .map(Value::String)
            .map_err(|err| ScriptError::Runtime(format!("json string encode failed: {err}"))),
        JsonValue::Array(values) => {
            let table = lua.create_table().map_err(|err| {
                ScriptError::Runtime(format!("json array table create failed: {err}"))
            })?;
            for (idx, item) in values.iter().enumerate() {
                table
                    .set(idx + 1, json_to_lua_value(lua, item)?)
                    .map_err(|err| ScriptError::Runtime(format!("json array set failed: {err}")))?;
            }
            Ok(Value::Table(table))
        }
        JsonValue::Object(entries) => {
            let table = lua.create_table().map_err(|err| {
                ScriptError::Runtime(format!("json object table create failed: {err}"))
            })?;
            for (key, item) in entries {
                table
                    .set(key.as_str(), json_to_lua_value(lua, item)?)
                    .map_err(|err| {
                        ScriptError::Runtime(format!("json object set key={} failed: {err}", key))
                    })?;
            }
            Ok(Value::Table(table))
        }
    }
}

fn script_state_to_json(script_state: &ScriptState) -> JsonValue {
    let mut data = serde_json::Map::new();
    for (key, value) in &script_state.data {
        data.insert(key.clone(), script_value_to_json(value));
    }
    let mut root = serde_json::Map::new();
    root.insert("data".to_string(), JsonValue::Object(data));
    JsonValue::Object(root)
}

fn script_value_to_json(value: &ScriptValue) -> JsonValue {
    match value {
        ScriptValue::Null => JsonValue::Null,
        ScriptValue::Bool(v) => JsonValue::Bool(*v),
        ScriptValue::Number(v) => serde_json::Number::from_f64(*v)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        ScriptValue::String(v) => JsonValue::String(v.clone()),
        ScriptValue::Array(values) => {
            JsonValue::Array(values.iter().map(script_value_to_json).collect::<Vec<_>>())
        }
        ScriptValue::Object(entries) => {
            let mut out = serde_json::Map::new();
            for (key, value) in entries {
                out.insert(key.clone(), script_value_to_json(value));
            }
            JsonValue::Object(out)
        }
    }
}

fn json_to_script_value(value: &JsonValue) -> ScriptValue {
    match value {
        JsonValue::Null => ScriptValue::Null,
        JsonValue::Bool(v) => ScriptValue::Bool(*v),
        JsonValue::Number(v) => ScriptValue::Number(v.as_f64().unwrap_or(0.0)),
        JsonValue::String(v) => ScriptValue::String(v.clone()),
        JsonValue::Array(values) => {
            ScriptValue::Array(values.iter().map(json_to_script_value).collect::<Vec<_>>())
        }
        JsonValue::Object(entries) => {
            let mut out = HashMap::new();
            for (key, value) in entries {
                out.insert(key.clone(), json_to_script_value(value));
            }
            ScriptValue::Object(out)
        }
    }
}

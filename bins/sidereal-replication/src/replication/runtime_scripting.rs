use avian2d::prelude::Position;
use bevy::prelude::*;
use mlua::{Function, Lua, RegistryKey, Table, UserData, UserDataMethods, Value};
use serde_json::Value as JsonValue;
use sidereal_game::{EntityGuid, FlightComputer, OwnerId, ScriptState, ScriptValue};
use sidereal_net::PlayerEntityId;
use sidereal_scripting::{
    LuaSandboxPolicy, ScriptError, create_sandboxed_lua_vm, load_lua_module_into_lua_from_root,
    lua_value_to_json,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;

use crate::replication::scripting::scripts_root_dir;

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

struct ScriptIntervalCallback {
    name: String,
    function_key: RegistryKey,
    interval_s: f64,
    next_run_s: f64,
    event_payload: JsonValue,
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
    callbacks: Vec<ScriptIntervalCallback>,
    pending_intents: Vec<ScriptIntent>,
}

impl ScriptRuntime {
    fn from_scripts_root(scripts_root: &std::path::Path) -> Result<Self, ScriptError> {
        let policy = LuaSandboxPolicy::from_env();
        let lua = create_sandboxed_lua_vm(&policy)?;
        let mut callbacks = Vec::new();

        let (module, module_path) =
            load_lua_module_into_lua_from_root(&lua, scripts_root, "ai/pirate_patrol.lua")?;
        let handler = module
            .get::<Function>("on_ai_patrol_tick")
            .map_err(|err| ScriptError::Contract(format!("{}: {err}", module_path.display())))?;
        let interval_s = module
            .get::<f64>("interval_seconds")
            .map_err(|err| ScriptError::Contract(format!("{}: {err}", module_path.display())))?;
        if interval_s <= 0.0 {
            return Err(ScriptError::Contract(format!(
                "{}: interval_seconds must be > 0",
                module_path.display()
            )));
        }
        let entity_id = module
            .get::<String>("entity_id")
            .map_err(|err| ScriptError::Contract(format!("{}: {err}", module_path.display())))?;
        if Uuid::parse_str(&entity_id).is_err() {
            return Err(ScriptError::Contract(format!(
                "{}: entity_id must be a UUID string",
                module_path.display()
            )));
        }

        callbacks.push(ScriptIntervalCallback {
            name: "ai_patrol_tick".to_string(),
            function_key: lua
                .create_registry_value(handler)
                .map_err(|err| ScriptError::Runtime(format!("{}: {err}", module_path.display())))?,
            interval_s,
            next_run_s: 0.0,
            event_payload: serde_json::json!({ "entity_id": entity_id }),
        });

        Ok(Self {
            lua,
            callbacks,
            pending_intents: Vec::new(),
        })
    }
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(ScriptWorldSnapshot::default());
    let scripts_root = scripts_root_dir();
    match ScriptRuntime::from_scripts_root(&scripts_root) {
        Ok(runtime) => {
            info!(
                "replication runtime scripting initialized root={} callbacks={}",
                scripts_root.display(),
                runtime.callbacks.len()
            );
            app.insert_non_send_resource(runtime);
        }
        Err(err) => {
            warn!(
                "replication runtime scripting disabled: root={} error={}",
                scripts_root.display(),
                err
            );
        }
    }
}

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
                script_state: script_state.map(|state| script_state_to_json(state)),
            },
        );
    }
}

pub fn run_script_intervals(
    runtime: Option<NonSendMut<'_, ScriptRuntime>>,
    snapshot: Res<'_, ScriptWorldSnapshot>,
    time: Res<'_, Time>,
) {
    let Some(mut runtime) = runtime else { return };
    let now_s = time.elapsed_secs_f64();
    if runtime.callbacks.is_empty() {
        return;
    }
    let snapshot_map = Rc::new(snapshot.entities_by_guid.clone());
    for idx in 0..runtime.callbacks.len() {
        let due = {
            let callback = &mut runtime.callbacks[idx];
            if now_s < callback.next_run_s {
                false
            } else {
                callback.next_run_s = now_s + callback.interval_s;
                true
            }
        };
        if !due {
            continue;
        }

        let callback_name = runtime.callbacks[idx].name.clone();
        let callback_payload = runtime.callbacks[idx].event_payload.clone();
        let pending_intents = Rc::new(RefCell::new(Vec::<ScriptIntent>::new()));
        let ctx = match build_script_context(
            &runtime.lua,
            Rc::clone(&snapshot_map),
            pending_intents.clone(),
        ) {
            Ok(v) => v,
            Err(err) => {
                warn!(
                    "replication runtime script interval={} context build failed: {}",
                    callback_name, err
                );
                continue;
            }
        };
        let event = match json_to_lua_value(&runtime.lua, &callback_payload) {
            Ok(v) => v,
            Err(err) => {
                warn!(
                    "replication runtime script interval={} event encode failed: {}",
                    callback_name, err
                );
                continue;
            }
        };
        let function = match runtime
            .lua
            .registry_value::<Function>(&runtime.callbacks[idx].function_key)
        {
            Ok(v) => v,
            Err(err) => {
                warn!(
                    "replication runtime script interval={} registry decode failed: {}",
                    callback_name, err
                );
                continue;
            }
        };
        if let Err(err) = function.call::<()>((ctx, event)) {
            warn!(
                "replication runtime script interval={} execution failed: {}",
                callback_name, err
            );
            continue;
        }
        runtime
            .pending_intents
            .extend(pending_intents.borrow_mut().drain(..));
    }
}

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
) -> Result<Table, ScriptError> {
    let ctx = lua
        .create_table()
        .map_err(|err| ScriptError::Runtime(format!("create script ctx failed: {err}")))?;
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

fn parse_uuid(raw: &str) -> Result<Uuid, String> {
    Uuid::parse_str(raw).map_err(|err| format!("invalid uuid {raw}: {err}"))
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

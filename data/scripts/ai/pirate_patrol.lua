local PiratePatrol = {}

PiratePatrol.handler_name = "pirate_patrol"
PiratePatrol.tick_interval_seconds = 2.0

local function read_state_value(state, key, default_value)
  if state == nil then
    return default_value
  end
  local data = state.data
  if data == nil then
    return default_value
  end
  local value = data[key]
  if value == nil then
    return default_value
  end
  return value
end

function PiratePatrol.on_tick(ctx, event)
  local npc = ctx.world:find_entity(event.entity_id)
  if npc == nil then
    return
  end
  local pos = npc:position()
  local state = npc:get("script_state")
  local patrol_points = read_state_value(state, "patrol_points", {
    { x = -2400, y = -1400 },
    { x = -1200, y = -2400 },
    { x = -100, y = -1000 },
    { x = -1600, y = -200 },
  })
  local patrol_index = read_state_value(state, "patrol_index", 1)
  local target = patrol_points[patrol_index]
  if target == nil then
    return
  end

  local dx = target.x - pos.x
  local dy = target.y - pos.y
  local distance = math.sqrt(dx * dx + dy * dy)
  if distance < 220 then
    patrol_index = (patrol_index % #patrol_points) + 1
    target = patrol_points[patrol_index]
    ctx:emit_intent("set_script_state", {
      entity_id = event.entity_id,
      key = "patrol_index",
      value = patrol_index,
    })
  end

  if target ~= nil then
    ctx:emit_intent("fly_towards", {
      entity_id = event.entity_id,
      target_position = { x = target.x, y = target.y },
    })
  else
    ctx:emit_intent("stop", {
      entity_id = event.entity_id,
    })
  end
end

return PiratePatrol

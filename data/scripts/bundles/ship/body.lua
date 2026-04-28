local ShipBodyBundle = {}

ShipBodyBundle.context = {}

local function component(entity_id, kind, properties)
  return {
    component_id = entity_id .. ":" .. kind,
    component_kind = kind,
    properties = properties,
  }
end

local function new_entity(entity_id, labels, parent_entity_id, components)
  local properties = {}
  if parent_entity_id ~= nil then
    properties.parent_entity_id = parent_entity_id
  end
  return {
    entity_id = entity_id,
    labels = labels,
    properties = properties,
    components = components,
  }
end

local function flight_actions_supported()
  return {
    "Forward",
    "Backward",
    "LongitudinalNeutral",
    "Left",
    "Right",
    "LateralNeutral",
    "Brake",
    "AfterburnerOn",
    "AfterburnerOff",
    "FirePrimary",
    "FireSecondary",
  }
end

local function copy_array(values)
  local out = {}
  if values == nil then
    return out
  end
  for index, value in ipairs(values) do
    out[index] = value
  end
  return out
end

local function shallow_merge(base, override)
  if type(base) ~= "table" then
    if override ~= nil then
      return override
    end
    return base
  end
  local out = {}
  for key, value in pairs(base) do
    out[key] = value
  end
  if type(override) == "table" then
    for key, value in pairs(override) do
      out[key] = value
    end
  elseif override ~= nil then
    return override
  end
  return out
end

local function yaw_quat_z(rad)
  local half = (rad or 0.0) * 0.5
  return { 0.0, 0.0, math.sin(half), math.cos(half) }
end

local function require_host_fn(ctx, name)
  if ctx[name] == nil then
    error("context." .. name .. " must be set by host")
  end
end

local function root_or_default(root, key, fallback)
  if root ~= nil and root[key] ~= nil then
    return root[key]
  end
  return fallback
end

local function normalized_spawn_position(ctx)
  local spawn_position = ctx.spawn_position or { 0.0, 0.0 }
  return { spawn_position[1] or spawn_position.x or 0.0, spawn_position[2] or spawn_position.y or 0.0 }
end

local function add_if_present(components, entity_id, kind, value)
  if value ~= nil then
    components[#components + 1] = component(entity_id, kind, value)
  end
end

local function module_component_properties(module_component, overrides)
  local override = nil
  if type(overrides) == "table" then
    override = overrides[module_component.kind]
  end
  return shallow_merge(module_component.properties, override)
end

function ShipBodyBundle.build_graph_records(ctx)
  require_host_fn(ctx, "load_ship_definition")
  require_host_fn(ctx, "load_ship_module_definition")
  require_host_fn(ctx, "compute_collision_half_extents_from_length")
  require_host_fn(ctx, "generate_collision_outline_rdp")

  local bundle_id = ctx.bundle_id
  local ship = ctx.load_ship_definition(bundle_id)
  if ship == nil then
    error("ship definition not found for bundle_id=" .. tostring(bundle_id))
  end

  local ship_id = ctx.entity_id or ctx.new_uuid()
  local owner_id = ctx.owner_id or "npc:unowned"
  local display_name = ctx.display_name or ship.display_name or ship.ship_id
  local entity_labels = ctx.ship_entity_labels or ship.entity_labels or { "Ship" }
  local visual = ship.visual or {}
  local dimensions = ship.dimensions or {}
  local root = ship.root or {}
  local visual_asset_id = visual.visual_asset_id or "corvette_01"
  local map_icon_asset_id = visual.map_icon_asset_id or "map_icon_ship_svg"
  local length_m = ctx.ship_length_m or dimensions.length_m or 21.2
  local height_m = dimensions.height_m or 8.0
  local base_mass_kg = root_or_default(root, "base_mass_kg", 15000.0)
  local total_mass_kg = root_or_default(root, "total_mass_kg", base_mass_kg)
  local angular_inertia = root_or_default(root, "angular_inertia", 750000.0)
  local spawn_position = normalized_spawn_position(ctx)

  local collision_half_extents = { (dimensions.width_m or length_m) * 0.5, length_m * 0.5 }
  if dimensions.collision_from_texture ~= false then
    collision_half_extents = ctx.compute_collision_half_extents_from_length(visual_asset_id, length_m)
  end
  local width_m = dimensions.width_m or (collision_half_extents[1] * 2.0)
  local generated_outline_points = {}
  if (dimensions.collision_mode or "Aabb") == "Aabb" then
    generated_outline_points = ctx.generate_collision_outline_rdp(visual_asset_id, collision_half_extents)
  end

  local ship_components = {
    component(ship_id, "display_name", display_name),
    component(ship_id, "ship_tag", {}),
    component(ship_id, "controlled_start_target", {}),
    component(ship_id, "entity_labels", entity_labels),
    component(ship_id, "flight_computer", root_or_default(root, "flight_computer", {
      profile = "basic_fly_by_wire",
      throttle = 0.0,
      yaw_input = 0.0,
      brake_active = false,
      turn_rate_deg_s = 90.0,
    })),
    component(ship_id, "afterburner_state", { active = false }),
    component(ship_id, "flight_tuning", root_or_default(root, "flight_tuning", {
      max_linear_accel_mps2 = 120.0,
      passive_brake_accel_mps2 = 16.611296,
      active_brake_accel_mps2 = 16.611296,
      drag_per_s = 0.4,
    })),
    component(ship_id, "max_velocity_mps", root_or_default(root, "max_velocity_mps", 100.0)),
    component(ship_id, "health_pool", root_or_default(root, "health_pool", { current = 1000.0, maximum = 1000.0 })),
    component(ship_id, "destructible", root_or_default(root, "destructible", {
      destruction_profile_id = "explosion_burst",
      destroy_delay_s = 0.18,
    })),
    component(ship_id, "owner_id", owner_id),
    component(ship_id, "mass_kg", base_mass_kg),
    component(ship_id, "size_m", { length = length_m, width = width_m, height = height_m }),
    component(ship_id, "collision_profile", { mode = dimensions.collision_mode or "Aabb" }),
    component(ship_id, "collision_outline_m", { points = generated_outline_points }),
    component(ship_id, "collision_aabb_m", { half_extents = { collision_half_extents[1], collision_half_extents[2], height_m * 0.5 } }),
    component(ship_id, "action_capabilities", { supported = flight_actions_supported() }),
    component(ship_id, "action_queue", { pending = { "LongitudinalNeutral", "LateralNeutral" } }),
    component(ship_id, "visual_asset_id", visual_asset_id),
    component(ship_id, "map_icon", { asset_id = map_icon_asset_id }),
    component(ship_id, "base_mass_kg", base_mass_kg),
    component(ship_id, "cargo_mass_kg", root_or_default(root, "cargo_mass_kg", 0.0)),
    component(ship_id, "module_mass_kg", root_or_default(root, "module_mass_kg", 0.0)),
    component(ship_id, "total_mass_kg", total_mass_kg),
    component(ship_id, "mass_dirty", {}),
    component(ship_id, "visibility_range_buff_m", root_or_default(root, "visibility_range_buff_m", {
      additive_m = 300.0,
      multiplier = 1.0,
    })),
    component(ship_id, "avian_position", { spawn_position[1], spawn_position[2] }),
    component(ship_id, "avian_rotation", { cos = 1.0, sin = 0.0 }),
    component(ship_id, "avian_linear_velocity", { 0.0, 0.0 }),
    component(ship_id, "avian_angular_velocity", 0.0),
    component(ship_id, "avian_rigid_body", "Dynamic"),
    component(ship_id, "avian_mass", base_mass_kg),
    component(ship_id, "avian_angular_inertia", angular_inertia),
    component(ship_id, "avian_linear_damping", root_or_default(root, "avian_linear_damping", 0.0)),
    component(ship_id, "avian_angular_damping", root_or_default(root, "avian_angular_damping", 0.0)),
  }
  add_if_present(ship_components, ship_id, "scanner_component", root.scanner_component)

  if ctx.script_state_data ~= nil then
    ship_components[#ship_components + 1] = component(ship_id, "script_state", {
      data = ctx.script_state_data,
    })
  end

  local records = {
    new_entity(ship_id, { "Entity", "Ship" }, nil, ship_components),
  }
  local hardpoint_entity_ids = {}

  for _, hardpoint in ipairs(ship.hardpoints or {}) do
    local hardpoint_entity_id = ctx.new_uuid()
    hardpoint_entity_ids[hardpoint.hardpoint_id] = hardpoint_entity_id
    records[#records + 1] = new_entity(
      hardpoint_entity_id,
      { "Entity", "Hardpoint" },
      ship_id,
      {
        component(hardpoint_entity_id, "display_name", (hardpoint.display_name or hardpoint.hardpoint_id) .. " Hardpoint"),
        component(hardpoint_entity_id, "entity_labels", { "Hardpoint" }),
        component(hardpoint_entity_id, "hardpoint", {
          hardpoint_id = hardpoint.hardpoint_id,
          offset_m = hardpoint.offset_m or { 0.0, 0.0, 0.0 },
          local_rotation = yaw_quat_z(hardpoint.local_rotation_rad or 0.0),
        }),
        component(hardpoint_entity_id, "parent_guid", ship_id),
        component(hardpoint_entity_id, "owner_id", owner_id),
      }
    )
  end

  for _, mount in ipairs(ship.mounted_modules or {}) do
    local hardpoint_entity_id = hardpoint_entity_ids[mount.hardpoint_id]
    if hardpoint_entity_id == nil then
      error("mounted module references unknown hardpoint_id=" .. tostring(mount.hardpoint_id))
    end
    local module_def = ctx.load_ship_module_definition(mount.module_id)
    if module_def == nil then
      error("ship module definition not found for module_id=" .. tostring(mount.module_id))
    end
    local module_entity_id = ctx.new_uuid()
    local module_components = {
      component(module_entity_id, "display_name", mount.display_name or module_def.display_name or module_def.module_id),
      component(module_entity_id, "entity_labels", copy_array(module_def.entity_labels or { "Module" })),
    }

    for _, module_component in ipairs(module_def.components or {}) do
      module_components[#module_components + 1] = component(
        module_entity_id,
        module_component.kind,
        module_component_properties(module_component, mount.component_overrides)
      )
    end

    module_components[#module_components + 1] = component(module_entity_id, "parent_guid", hardpoint_entity_id)
    module_components[#module_components + 1] = component(module_entity_id, "mounted_on", {
      parent_entity_id = ship_id,
      hardpoint_id = mount.hardpoint_id,
    })
    module_components[#module_components + 1] = component(module_entity_id, "owner_id", owner_id)

    local labels = copy_array(module_def.entity_labels or { "Module" })
    table.insert(labels, 1, "Entity")
    records[#records + 1] = new_entity(module_entity_id, labels, hardpoint_entity_id, module_components)
  end

  return records
end

return ShipBodyBundle

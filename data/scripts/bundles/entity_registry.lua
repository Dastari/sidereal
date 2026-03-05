local EntityRegistry = {}

EntityRegistry.context = {}

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

local function append_records(out, records)
  for i = 1, #records do
    out[#out + 1] = records[i]
  end
end

local function spawn_position_from_account(account_id)
  local hash = 2166136261
  for i = 1, #account_id do
    hash = hash ~ string.byte(account_id, i)
    hash = (hash * 16777619) % 4294967296
  end
  local x = ((hash * 1664525 + 1013904223) % 1000) - 500
  local y = ((hash * 22695477 + 1) % 1000) - 500
  return x, y
end

local function corvette_collision_outline_points()
  return {
    { -2.3, 11.6 },
    { 2.3, 11.6 },
    { 7.8, 6.2 },
    { 8.4, -1.5 },
    { 3.6, -10.8 },
    { -3.6, -10.8 },
    { -8.4, -1.5 },
    { -7.8, 6.2 },
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
    "ThrustForward",
    "ThrustReverse",
    "ThrustNeutral",
    "YawLeft",
    "YawRight",
    "YawNeutral",
    "FirePrimary",
    "FireSecondary",
  }
end

local function character_actions_supported()
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
  }
end

local function build_corvette_bundle(ctx)
  local ship_id = ctx.entity_id or ctx.new_uuid()
  local owner_id = ctx.owner_id or "npc:unowned"
  local display_name = ctx.display_name or "Corvette"
  local ship_entity_labels = ctx.ship_entity_labels or { "Ship", "Corvette" }
  local scanner_base_range_m = ctx.scanner_base_range_m or 300.0
  local spawn_position = ctx.spawn_position or { 0.0, 0.0 }

  local hardpoint_computer_core_id = ctx.new_uuid()
  local hardpoint_engine_main_aft_id = ctx.new_uuid()
  local hardpoint_fuel_left_id = ctx.new_uuid()
  local hardpoint_fuel_right_id = ctx.new_uuid()
  local hardpoint_weapon_fore_center_id = ctx.new_uuid()
  local module_flight_computer_id = ctx.new_uuid()
  local module_engine_main_id = ctx.new_uuid()
  local module_fuel_tank_left_id = ctx.new_uuid()
  local module_fuel_tank_right_id = ctx.new_uuid()
  local module_weapon_gatling_fore_id = ctx.new_uuid()

  local ship_components = {
    component(ship_id, "display_name", display_name),
    component(ship_id, "ship_tag", {}),
    component(ship_id, "entity_labels", ship_entity_labels),
    component(ship_id, "flight_computer", {
      profile = "basic_fly_by_wire",
      throttle = 0.0,
      yaw_input = 0.0,
      brake_active = false,
      turn_rate_deg_s = 90.0,
    }),
    component(ship_id, "afterburner_state", { active = false }),
    component(ship_id, "flight_tuning", {
      max_linear_accel_mps2 = 120.0,
      passive_brake_accel_mps2 = 16.611296,
      active_brake_accel_mps2 = 16.611296,
      drag_per_s = 0.4,
    }),
    component(ship_id, "max_velocity_mps", 100.0),
    component(ship_id, "health_pool", { current = 1000.0, maximum = 1000.0 }),
    component(ship_id, "owner_id", owner_id),
    component(ship_id, "mass_kg", 15000.0),
    component(ship_id, "size_m", { length = 25.0, width = 25.0, height = 8.0 }),
    component(ship_id, "collision_profile", { mode = "Aabb" }),
    component(ship_id, "collision_outline_m", {
      points = corvette_collision_outline_points(),
    }),
    component(ship_id, "collision_aabb_m", { half_extents = { 7.2, 10.6, 4.0 } }),
    component(ship_id, "action_capabilities", { supported = flight_actions_supported() }),
    component(ship_id, "action_queue", { pending = { "LongitudinalNeutral", "LateralNeutral" } }),
    component(ship_id, "visual_asset_id", "corvette_01"),
    component(ship_id, "base_mass_kg", 15000.0),
    component(ship_id, "cargo_mass_kg", 0.0),
    component(ship_id, "module_mass_kg", 0.0),
    component(ship_id, "total_mass_kg", 15000.0),
    component(ship_id, "mass_dirty", {}),
    component(ship_id, "scanner_component", {
      base_range_m = scanner_base_range_m,
      level = 1,
    }),
    component(ship_id, "avian_position", { spawn_position[1] or 0.0, spawn_position[2] or 0.0 }),
    component(ship_id, "avian_rotation", { cos = 1.0, sin = 0.0 }),
    component(ship_id, "avian_linear_velocity", { 0.0, 0.0 }),
    component(ship_id, "avian_angular_velocity", 0.0),
    component(ship_id, "avian_rigid_body", "Dynamic"),
    component(ship_id, "avian_mass", 15000.0),
    component(ship_id, "avian_angular_inertia", 750000.0),
    component(ship_id, "avian_linear_damping", 0.0),
    component(ship_id, "avian_angular_damping", 0.0),
  }

  if ctx.script_state_data ~= nil then
    ship_components[#ship_components + 1] = component(ship_id, "script_state", {
      data = ctx.script_state_data,
    })
  end

  return {
    new_entity(
      ship_id,
      { "Entity", "Ship" },
      nil,
      ship_components
    ),
    new_entity(
      hardpoint_computer_core_id,
      { "Entity", "Hardpoint" },
      ship_id,
      {
        component(hardpoint_computer_core_id, "display_name", "Computer Core Hardpoint"),
        component(hardpoint_computer_core_id, "entity_labels", { "Hardpoint" }),
        component(hardpoint_computer_core_id, "hardpoint", {
          hardpoint_id = "computer_core",
          offset_m = { 0.0, 0.0, -5.0 },
        }),
        component(hardpoint_computer_core_id, "parent_guid", ship_id),
        component(hardpoint_computer_core_id, "owner_id", owner_id),
      }
    ),
    new_entity(
      hardpoint_engine_main_aft_id,
      { "Entity", "Hardpoint" },
      ship_id,
      {
        component(hardpoint_engine_main_aft_id, "display_name", "Engine Main Aft Hardpoint"),
        component(hardpoint_engine_main_aft_id, "entity_labels", { "Hardpoint" }),
        component(hardpoint_engine_main_aft_id, "hardpoint", {
          hardpoint_id = "engine_main_aft",
          offset_m = { 0.0, -1.0, -10.0 },
        }),
        component(hardpoint_engine_main_aft_id, "parent_guid", ship_id),
        component(hardpoint_engine_main_aft_id, "owner_id", owner_id),
      }
    ),
    new_entity(
      hardpoint_fuel_left_id,
      { "Entity", "Hardpoint" },
      ship_id,
      {
        component(hardpoint_fuel_left_id, "display_name", "Fuel Tank Left Hardpoint"),
        component(hardpoint_fuel_left_id, "entity_labels", { "Hardpoint" }),
        component(hardpoint_fuel_left_id, "hardpoint", {
          hardpoint_id = "fuel_left",
          offset_m = { -5.0, 0.0, -2.0 },
        }),
        component(hardpoint_fuel_left_id, "parent_guid", ship_id),
        component(hardpoint_fuel_left_id, "owner_id", owner_id),
      }
    ),
    new_entity(
      hardpoint_fuel_right_id,
      { "Entity", "Hardpoint" },
      ship_id,
      {
        component(hardpoint_fuel_right_id, "display_name", "Fuel Tank Right Hardpoint"),
        component(hardpoint_fuel_right_id, "entity_labels", { "Hardpoint" }),
        component(hardpoint_fuel_right_id, "hardpoint", {
          hardpoint_id = "fuel_right",
          offset_m = { 5.0, 0.0, -2.0 },
        }),
        component(hardpoint_fuel_right_id, "parent_guid", ship_id),
        component(hardpoint_fuel_right_id, "owner_id", owner_id),
      }
    ),
    new_entity(
      hardpoint_weapon_fore_center_id,
      { "Entity", "Hardpoint" },
      ship_id,
      {
        component(hardpoint_weapon_fore_center_id, "display_name", "Weapon Fore Center Hardpoint"),
        component(hardpoint_weapon_fore_center_id, "entity_labels", { "Hardpoint" }),
        component(hardpoint_weapon_fore_center_id, "hardpoint", {
          hardpoint_id = "weapon_fore_center",
          offset_m = { 0.0, 0.0, 8.5 },
        }),
        component(hardpoint_weapon_fore_center_id, "parent_guid", ship_id),
        component(hardpoint_weapon_fore_center_id, "owner_id", owner_id),
      }
    ),
    new_entity(
      module_flight_computer_id,
      { "Entity", "Module" },
      hardpoint_computer_core_id,
      {
        component(module_flight_computer_id, "display_name", "Flight Computer MK1"),
        component(module_flight_computer_id, "entity_labels", { "Module" }),
        component(module_flight_computer_id, "flight_computer", {
          profile = "basic_fly_by_wire",
          throttle = 0.0,
          yaw_input = 0.0,
          brake_active = false,
          turn_rate_deg_s = 90.0,
        }),
        component(module_flight_computer_id, "mass_kg", 50.0),
        component(module_flight_computer_id, "parent_guid", hardpoint_computer_core_id),
        component(module_flight_computer_id, "mounted_on", {
          parent_entity_id = ship_id,
          hardpoint_id = "computer_core",
        }),
        component(module_flight_computer_id, "owner_id", owner_id),
      }
    ),
    new_entity(
      module_engine_main_id,
      { "Entity", "Module", "Engine" },
      hardpoint_engine_main_aft_id,
      {
        component(module_engine_main_id, "display_name", "Engine Main"),
        component(module_engine_main_id, "entity_labels", { "Module", "Engine" }),
        component(module_engine_main_id, "engine", {
          thrust = 300000.0,
          reverse_thrust = 300000.0,
          torque_thrust = 1500000.0,
          burn_rate_kg_s = 0.8,
        }),
        component(module_engine_main_id, "afterburner_capability", {
          enabled = true,
          multiplier = 1.5,
          fuel_burn_multiplier = 2.0,
          max_afterburner_velocity_mps = 250.0,
        }),
        component(module_engine_main_id, "thruster_plume_shader_settings", {
          enabled = true,
          color_rgb = { 1.5, 0.7, 0.2 },
          intensity = 2.0,
          radius = 1.0,
          length = 2.8,
          pulse_rate_hz = 16.0,
          pulse_amplitude = 0.22,
          turbulence = 0.35,
          core_softness = 0.35,
          velocity_stretch = 0.45,
          noise_scroll = 1.4,
        }),
        component(module_engine_main_id, "mass_kg", 500.0),
        component(module_engine_main_id, "parent_guid", hardpoint_engine_main_aft_id),
        component(module_engine_main_id, "mounted_on", {
          parent_entity_id = ship_id,
          hardpoint_id = "engine_main_aft",
        }),
        component(module_engine_main_id, "owner_id", owner_id),
      }
    ),
    new_entity(
      module_fuel_tank_left_id,
      { "Entity", "Module", "FuelTank" },
      hardpoint_fuel_left_id,
      {
        component(module_fuel_tank_left_id, "display_name", "Fuel Tank Port"),
        component(module_fuel_tank_left_id, "entity_labels", { "Module", "FuelTank" }),
        component(module_fuel_tank_left_id, "fuel_tank", { fuel_kg = 1000.0 }),
        component(module_fuel_tank_left_id, "mass_kg", 1100.0),
        component(module_fuel_tank_left_id, "parent_guid", hardpoint_fuel_left_id),
        component(module_fuel_tank_left_id, "mounted_on", {
          parent_entity_id = ship_id,
          hardpoint_id = "fuel_left",
        }),
        component(module_fuel_tank_left_id, "owner_id", owner_id),
      }
    ),
    new_entity(
      module_fuel_tank_right_id,
      { "Entity", "Module", "FuelTank" },
      hardpoint_fuel_right_id,
      {
        component(module_fuel_tank_right_id, "display_name", "Fuel Tank Starboard"),
        component(module_fuel_tank_right_id, "entity_labels", { "Module", "FuelTank" }),
        component(module_fuel_tank_right_id, "fuel_tank", { fuel_kg = 1000.0 }),
        component(module_fuel_tank_right_id, "mass_kg", 1100.0),
        component(module_fuel_tank_right_id, "parent_guid", hardpoint_fuel_right_id),
        component(module_fuel_tank_right_id, "mounted_on", {
          parent_entity_id = ship_id,
          hardpoint_id = "fuel_right",
        }),
        component(module_fuel_tank_right_id, "owner_id", owner_id),
      }
    ),
    new_entity(
      module_weapon_gatling_fore_id,
      { "Entity", "Module", "Weapon", "BallisticWeapon" },
      hardpoint_weapon_fore_center_id,
      {
        component(module_weapon_gatling_fore_id, "display_name", "Ballistic Gatling"),
        component(module_weapon_gatling_fore_id, "entity_labels", { "Module", "Weapon", "BallisticWeapon" }),
        component(module_weapon_gatling_fore_id, "weapon_tag", {}),
        component(module_weapon_gatling_fore_id, "ballistic_weapon", {
          weapon_name = "Ballistic Gatling",
          rpm = 750.0,
          damage_per_shot = 22.0,
          max_range_m = 2200.0,
          spread_rad = 0.0,
          damage_type = "Ballistic",
        }),
        component(module_weapon_gatling_fore_id, "ammo_count", {
          current = 500,
          maximum = 500,
        }),
        component(module_weapon_gatling_fore_id, "mass_kg", 120.0),
        component(module_weapon_gatling_fore_id, "parent_guid", hardpoint_weapon_fore_center_id),
        component(module_weapon_gatling_fore_id, "mounted_on", {
          parent_entity_id = ship_id,
          hardpoint_id = "weapon_fore_center",
        }),
        component(module_weapon_gatling_fore_id, "owner_id", owner_id),
      }
    ),
  }
end

local function build_starter_corvette(ctx)
  local ship_id = ctx.new_uuid()
  local spawn_x, spawn_y = spawn_position_from_account(ctx.account_id)

  local records = {
    new_entity(
      ctx.player_entity_id,
      { "Entity", "Player" },
      nil,
      {
        component(ctx.player_entity_id, "display_name", ctx.email),
        component(ctx.player_entity_id, "player_tag", {}),
        component(ctx.player_entity_id, "account_id", ctx.account_id),
        component(ctx.player_entity_id, "controlled_entity_guid", ship_id),
        component(ctx.player_entity_id, "entity_labels", { "Player" }),
        component(ctx.player_entity_id, "action_capabilities", { supported = character_actions_supported() }),
        component(ctx.player_entity_id, "character_movement_controller", {
          speed_mps = 220.0,
          max_accel_mps2 = 880.0,
          damping_per_s = 8.0,
        }),
        component(ctx.player_entity_id, "action_queue", { pending = { "LongitudinalNeutral", "LateralNeutral" } }),
        component(ctx.player_entity_id, "avian_position", { 0.0, 0.0 }),
        component(ctx.player_entity_id, "avian_rotation", { cos = 1.0, sin = 0.0 }),
        component(ctx.player_entity_id, "avian_linear_velocity", { 0.0, 0.0 }),
        component(ctx.player_entity_id, "avian_rigid_body", "Dynamic"),
        component(ctx.player_entity_id, "avian_mass", 1.0),
        component(ctx.player_entity_id, "avian_angular_inertia", 1.0),
        component(ctx.player_entity_id, "avian_linear_damping", 0.0),
        component(ctx.player_entity_id, "avian_angular_damping", 0.0),
      }
    ),
  }

  append_records(records, build_corvette_bundle({
    new_uuid = ctx.new_uuid,
    entity_id = ship_id,
    owner_id = ctx.player_entity_id,
    display_name = "Corvette",
    ship_entity_labels = { "Ship", "Corvette" },
    spawn_position = { spawn_x, spawn_y },
    scanner_base_range_m = 300.0,
  }))

  return records
end

local function build_debug_minimal_dynamic(ctx)
  return {
    {
      entity_id = ctx.player_entity_id,
      labels = { "Entity", "Player" },
      properties = {},
      components = {
        component(ctx.player_entity_id, "display_name", "Dynamic Player"),
        component(ctx.player_entity_id, "player_tag", {}),
        component(ctx.player_entity_id, "account_id", ctx.account_id),
        component(ctx.player_entity_id, "entity_labels", { "Player" }),
      },
    },
  }
end

local BUNDLE_BUILDERS = {
  corvette = build_corvette_bundle,
  starter_corvette = build_starter_corvette,
  debug_minimal_dynamic = build_debug_minimal_dynamic,
}

function EntityRegistry.build_graph_records(ctx)
  local bundle_id = ctx.bundle_id
  if bundle_id == nil or bundle_id == "" then
    error("context.bundle_id must be set by host")
  end
  local builder = BUNDLE_BUILDERS[bundle_id]
  if builder == nil then
    error("unknown bundle_id: " .. tostring(bundle_id))
  end
  return builder(ctx)
end

return EntityRegistry

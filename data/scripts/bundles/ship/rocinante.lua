local RocinanteBundle = {}

RocinanteBundle.context = {}

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

function RocinanteBundle.build_graph_records(ctx)
  local ship_id = ctx.entity_id or ctx.new_uuid()
  local owner_id = ctx.owner_id or "npc:unowned"
  local display_name = ctx.display_name or "Rocinante"
  local ship_entity_labels = ctx.ship_entity_labels or { "Ship", "Rocinante" }
  local scanner_base_range_m = ctx.scanner_base_range_m or 1000.0
  local ship_length_m = ctx.ship_length_m or 46.0
  local spawn_position = ctx.spawn_position or { 0.0, 0.0 }
  if ctx.compute_collision_half_extents_from_length == nil then
    error("context.compute_collision_half_extents_from_length must be set by host")
  end
  if ctx.generate_collision_outline_rdp == nil then
    error("context.generate_collision_outline_rdp must be set by host")
  end
  local collision_half_extents = ctx.compute_collision_half_extents_from_length("rocinante_01", ship_length_m)
  local ship_width_m = collision_half_extents[1] * 2.0
  local generated_outline_points = ctx.generate_collision_outline_rdp("rocinante_01", collision_half_extents)

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
    component(ship_id, "controlled_start_target", {}),
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
    component(ship_id, "destructible", {
      destruction_profile_id = "explosion_burst",
      destroy_delay_s = 0.18,
    }),
    component(ship_id, "owner_id", owner_id),
    component(ship_id, "mass_kg", 15000.0),
    component(ship_id, "size_m", { length = ship_length_m, width = ship_width_m, height = 8.0 }),
    component(ship_id, "collision_profile", { mode = "Aabb" }),
    component(ship_id, "collision_outline_m", {
      points = generated_outline_points,
    }),
    component(ship_id, "collision_aabb_m", { half_extents = { collision_half_extents[1], collision_half_extents[2], 4.0 } }),
    component(ship_id, "action_capabilities", { supported = flight_actions_supported() }),
    component(ship_id, "action_queue", { pending = { "LongitudinalNeutral", "LateralNeutral" } }),
    component(ship_id, "visual_asset_id", "rocinante_01"),
    component(ship_id, "map_icon", { asset_id = "map_icon_ship_svg" }),
    component(ship_id, "base_mass_kg", 15000.0),
    component(ship_id, "cargo_mass_kg", 0.0),
    component(ship_id, "module_mass_kg", 0.0),
    component(ship_id, "total_mass_kg", 15000.0),
    component(ship_id, "mass_dirty", {}),
    component(ship_id, "visibility_range_buff_m", {
      additive_m = scanner_base_range_m,
      multiplier = 1.0,
    }),
    component(ship_id, "scanner_component", {
      base_range_m = scanner_base_range_m,
      level = 1,
      detail_tier = "Iff",
      supports_density = true,
      supports_directional_awareness = true,
      max_contacts = 64,
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
    new_entity(ship_id, { "Entity", "Ship" }, nil, ship_components),
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
          base_length_m = 2.4,
          max_length_m = 11.2,
          base_width_m = 1.1,
          max_width_m = 3.7,
          idle_core_alpha = 0.18,
          max_alpha = 0.92,
          falloff = 1.18,
          edge_softness = 1.45,
          noise_strength = 0.32,
          flicker_hz = 16.0,
          reactive_length_scale = 1.0,
          reactive_alpha_scale = 1.0,
          afterburner_length_scale = 1.5,
          afterburner_alpha_boost = 0.18,
          base_color_rgb = { 1.0, 0.52, 0.14 },
          hot_color_rgb = { 1.0, 0.82, 0.38 },
          afterburner_color_rgb = { 1.0, 0.96, 0.78 },
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
          fire_audio_profile_id = "weapon.ballistic_gatling",
          rpm = 750.0,
          damage_per_shot = 22.0,
          max_range_m = 2200.0,
          projectile_speed_mps = 0.0,
          spread_rad = 0.0,
          damage_type = "Ballistic",
        }),
        component(module_weapon_gatling_fore_id, "ammo_count", {
          current = 500,
          capacity = 500,
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

return RocinanteBundle

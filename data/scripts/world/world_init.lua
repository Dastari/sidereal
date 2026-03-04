local WorldInit = {}

WorldInit.context = {}

WorldInit.world_defaults = {
  space_background_shader_asset_id = "space_background_wgsl",
  starfield_shader_asset_id = "starfield_wgsl",
}

local function component(entity_id, kind, properties)
  return {
    component_id = entity_id .. ":" .. kind,
    component_kind = kind,
    properties = properties,
  }
end

local function corvette_collision_outline_points(ctx)
  if ctx ~= nil and ctx.default_corvette_collision_outline_points ~= nil then
    return ctx.default_corvette_collision_outline_points()
  end
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

function WorldInit.build_graph_records(_ctx)
  local space_background_id = "0012ebad-0000-0000-0000-000000000002"
  local starfield_id = "0012ebad-0000-0000-0000-000000000001"
  local patrol_ship_id = "0012ebad-0000-0000-0000-000000000101"
  local patrol_engine_id = "0012ebad-0000-0000-0000-000000000102"
  local patrol_fuel_tank_id = "0012ebad-0000-0000-0000-000000000103"

  return {
    {
      entity_id = space_background_id,
      labels = { "Entity", "FullscreenLayer" },
      properties = {},
      components = {
        component(space_background_id, "display_name", "SpaceBackground"),
        component(space_background_id, "fullscreen_layer", {
          layer_kind = "space_background",
          shader_asset_id = WorldInit.world_defaults.space_background_shader_asset_id,
          layer_order = -200,
        }),
        component(space_background_id, "space_background_shader_settings", {
          enabled = true,
          intensity = 1.3,
          drift_scale = 1.0,
          velocity_glow = 1.0,
          nebula_strength = 1.0,
          seed = 0.0,
          background_rgb = { 0.0, 0.0, 0.0 },
          nebula_color_primary_rgb = { 0.386, 0.0, 0.023 },
          nebula_color_secondary_rgb = { 0.0, 0.0, 1.398 },
          nebula_color_accent_rgb = { 0.913, 0.16, 0.36 },
          flare_enabled = true,
          flare_tint_rgb = { 1.124, 0.0, 0.462 },
          flare_intensity = 1.58,
          flare_density = 0.42,
          flare_size = 2.89,
          flare_texture_set = 0,
          nebula_noise_mode = 0,
          nebula_octaves = 5,
          nebula_gain = 0.52,
          nebula_lacunarity = 2.0,
          nebula_power = 1.0,
          nebula_shelf = 0.42,
          nebula_ridge_offset = 1.0,
          star_mask_enabled = true,
          star_mask_mode = 0,
          star_mask_octaves = 4,
          star_mask_gain = 0.63,
          star_mask_lacunarity = 2.4,
          star_mask_threshold = 0.35,
          star_mask_power = 1.25,
          star_mask_ridge_offset = 0.99,
          star_mask_scale = 1.4,
          nebula_blend_mode = 1,
          nebula_opacity = 0.75,
          stars_blend_mode = 2,
          stars_opacity = 0.79,
          star_count = 5.0,
          star_size_min = 0.034,
          star_size_max = 0.035,
          star_color_rgb = { 1.086, 1.0, 1.487 },
          flares_blend_mode = 1,
          flares_opacity = 0.95,
          tint_rgb = { 1.0, 1.0, 1.0 },
        }),
      },
    },
    {
      entity_id = starfield_id,
      labels = { "Entity", "FullscreenLayer" },
      properties = {},
      components = {
        component(starfield_id, "display_name", "StarField"),
        component(starfield_id, "fullscreen_layer", {
          layer_kind = "starfield",
          shader_asset_id = WorldInit.world_defaults.starfield_shader_asset_id,
          layer_order = -190,
        }),
        component(starfield_id, "starfield_shader_settings", {
          enabled = true,
          density = 0.07,
          layer_count = 4,
          initial_z_offset = 0.5,
          intensity = 4.0,
          alpha = 0.32,
          tint_rgb = { 1.0, 1.0, 1.34 },
        }),
      },
    },
    {
      entity_id = patrol_ship_id,
      labels = { "Entity", "Ship" },
      properties = {},
      components = {
        component(patrol_ship_id, "display_name", "Pirate Patrol"),
        component(patrol_ship_id, "ship_tag", {}),
        component(patrol_ship_id, "entity_labels", { "Ship", "Pirate", "Npc" }),
        component(patrol_ship_id, "flight_computer", {
          profile = "basic_fly_by_wire",
          throttle = 0.0,
          yaw_input = 0.0,
          brake_active = false,
          turn_rate_deg_s = 90.0,
        }),
        component(patrol_ship_id, "afterburner_state", { active = false }),
        component(patrol_ship_id, "flight_tuning", {
          max_linear_accel_mps2 = 120.0,
          passive_brake_accel_mps2 = 16.611296,
          active_brake_accel_mps2 = 16.611296,
          drag_per_s = 0.4,
        }),
        component(patrol_ship_id, "max_velocity_mps", 100.0),
        component(patrol_ship_id, "health_pool", { current = 1000.0, maximum = 1000.0 }),
        component(patrol_ship_id, "owner_id", "npc:pirate_patrol_1"),
        component(patrol_ship_id, "mass_kg", 15000.0),
        component(patrol_ship_id, "size_m", { length = 25.0, width = 25.0, height = 8.0 }),
        component(patrol_ship_id, "collision_profile", { mode = "Aabb" }),
        component(patrol_ship_id, "collision_outline_m", {
          points = corvette_collision_outline_points(_ctx),
        }),
        component(patrol_ship_id, "collision_aabb_m", { half_extents = { 7.2, 10.6, 4.0 } }),
        component(patrol_ship_id, "scanner_range_m", 300.0),
        component(patrol_ship_id, "scanner_component", {
          base_range_m = 400.0,
          level = 1,
        }),
        component(patrol_ship_id, "action_capabilities", {
          supported = {
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
          },
        }),
        component(patrol_ship_id, "action_queue", {
          pending = { "LongitudinalNeutral", "LateralNeutral" },
        }),
        component(patrol_ship_id, "visual_asset_id", "corvette_01"),
        component(patrol_ship_id, "base_mass_kg", 15000.0),
        component(patrol_ship_id, "cargo_mass_kg", 0.0),
        component(patrol_ship_id, "module_mass_kg", 1600.0),
        component(patrol_ship_id, "total_mass_kg", 16600.0),
        component(patrol_ship_id, "mass_dirty", {}),
        component(patrol_ship_id, "script_state", {
          data = {
            patrol_index = 1,
            patrol_points = {
              { x = -2400, y = -1400 },
              { x = -1200, y = -2400 },
              { x = -100, y = -1000 },
              { x = -1600, y = -200 },
            },
          },
        }),
        component(patrol_ship_id, "avian_position", { -2400.0, -1400.0 }),
        component(patrol_ship_id, "avian_rotation", { cos = 1.0, sin = 0.0 }),
        component(patrol_ship_id, "avian_linear_velocity", { 0.0, 0.0 }),
        component(patrol_ship_id, "avian_angular_velocity", 0.0),
        component(patrol_ship_id, "avian_rigid_body", "Dynamic"),
        component(patrol_ship_id, "avian_mass", 15000.0),
        component(patrol_ship_id, "avian_angular_inertia", 750000.0),
        component(patrol_ship_id, "avian_linear_damping", 0.0),
        component(patrol_ship_id, "avian_angular_damping", 0.0),
      },
    },
    {
      entity_id = patrol_engine_id,
      labels = { "Entity", "Module" },
      properties = {
        parent_entity_id = patrol_ship_id,
      },
      components = {
        component(patrol_engine_id, "display_name", "Patrol Engine"),
        component(patrol_engine_id, "entity_labels", { "Module", "Engine" }),
        component(patrol_engine_id, "mounted_on", {
          parent_entity_id = patrol_ship_id,
          hardpoint_id = "engine_main_aft",
        }),
        component(patrol_engine_id, "mass_kg", 500.0),
        component(patrol_engine_id, "owner_id", "npc:pirate_patrol_1"),
        component(patrol_engine_id, "engine", {
          thrust = 300000.0,
          reverse_thrust = 300000.0,
          torque_thrust = 1500000.0,
          burn_rate_kg_s = 0.8,
        }),
      },
    },
    {
      entity_id = patrol_fuel_tank_id,
      labels = { "Entity", "Module" },
      properties = {
        parent_entity_id = patrol_ship_id,
      },
      components = {
        component(patrol_fuel_tank_id, "display_name", "Patrol Fuel Tank"),
        component(patrol_fuel_tank_id, "entity_labels", { "Module", "FuelTank" }),
        component(patrol_fuel_tank_id, "mounted_on", {
          parent_entity_id = patrol_ship_id,
          hardpoint_id = "fuel_main",
        }),
        component(patrol_fuel_tank_id, "mass_kg", 1100.0),
        component(patrol_fuel_tank_id, "owner_id", "npc:pirate_patrol_1"),
        component(patrol_fuel_tank_id, "fuel_tank", {
          fuel_kg = 1000.0,
        }),
      },
    },
  }
end

return WorldInit

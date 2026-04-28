return {
  ship_id = "ship.rocinante",
  bundle_id = "ship.rocinante",
  display_name = "Rocinante",
  entity_labels = { "Ship", "Rocinante" },
  tags = { "heavy", "combat", "prototype" },

  visual = {
    visual_asset_id = "rocinante_01",
    map_icon_asset_id = "map_icon_ship_svg",
  },

  dimensions = {
    length_m = 46.0,
    width_m = 14.1,
    height_m = 8.0,
    collision_mode = "Aabb",
    collision_from_texture = true,
  },

  root = {
    base_mass_kg = 15000.0,
    total_mass_kg = 15000.0,
    angular_inertia = 750000.0,
    max_velocity_mps = 100.0,
    health_pool = { current = 1000.0, maximum = 1000.0 },
    destructible = {
      destruction_profile_id = "explosion_burst",
      destroy_delay_s = 0.18,
    },
    flight_computer = {
      profile = "basic_fly_by_wire",
      throttle = 0.0,
      yaw_input = 0.0,
      brake_active = false,
      turn_rate_deg_s = 90.0,
    },
    flight_tuning = {
      max_linear_accel_mps2 = 120.0,
      passive_brake_accel_mps2 = 16.611296,
      active_brake_accel_mps2 = 16.611296,
      drag_per_s = 0.4,
    },
    visibility_range_buff_m = {
      additive_m = 1000.0,
      multiplier = 1.0,
    },
    scanner_component = {
      base_range_m = 1000.0,
      level = 1,
      detail_tier = "Iff",
      supports_density = true,
      supports_directional_awareness = true,
      max_contacts = 64,
    },
  },

  hardpoints = {
    {
      hardpoint_id = "computer_core",
      display_name = "Computer Core",
      slot_kind = "computer",
      offset_m = { 0.0, -5.0, 0.0 },
      local_rotation_rad = 0.0,
      compatible_tags = { "computer" },
    },
    {
      hardpoint_id = "engine_main_aft",
      display_name = "Engine Main Aft",
      slot_kind = "engine",
      offset_m = { 0.0, -10.0, 0.0 },
      local_rotation_rad = 0.0,
      compatible_tags = { "engine" },
    },
    {
      hardpoint_id = "fuel_left",
      display_name = "Fuel Tank Port",
      slot_kind = "fuel",
      offset_m = { -5.0, -2.0, 0.0 },
      local_rotation_rad = 0.0,
      mirror_group = "fuel",
      compatible_tags = { "fuel" },
    },
    {
      hardpoint_id = "fuel_right",
      display_name = "Fuel Tank Starboard",
      slot_kind = "fuel",
      offset_m = { 5.0, -2.0, 0.0 },
      local_rotation_rad = 0.0,
      mirror_group = "fuel",
      compatible_tags = { "fuel" },
    },
    {
      hardpoint_id = "weapon_fore_center",
      display_name = "Weapon Fore Center",
      slot_kind = "weapon",
      offset_m = { 0.0, 8.5, 0.0 },
      local_rotation_rad = 0.0,
      compatible_tags = { "weapon" },
    },
  },

  mounted_modules = {
    { hardpoint_id = "computer_core", module_id = "module.computer.flight_mk1", component_overrides = {} },
    { hardpoint_id = "engine_main_aft", module_id = "module.engine.main_mk1", component_overrides = {} },
    {
      hardpoint_id = "fuel_left",
      module_id = "module.fuel.tank_mk1",
      display_name = "Fuel Tank Port",
      component_overrides = {},
    },
    {
      hardpoint_id = "fuel_right",
      module_id = "module.fuel.tank_mk1",
      display_name = "Fuel Tank Starboard",
      component_overrides = {},
    },
    { hardpoint_id = "weapon_fore_center", module_id = "module.weapon.ballistic_gatling_mk1", component_overrides = {} },
  },
}

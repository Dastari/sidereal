local AsteroidRegistry = {}

AsteroidRegistry.schema_version = 1

AsteroidRegistry.field_profiles = {
  {
    field_profile_id = "asteroid.field.starter_belt",
    display_name = "Starter Asteroid Field",
    shape = "ClusterPatch",
    radius_m = 2600.0,
    density = 0.55,
    layout_seed = 424242,
    sprite_profile_id = "asteroid.sprite.top_down_rocky",
    fracture_profile_id = "asteroid.fracture.default",
    resource_profile_id = "asteroid.resource.common_ore",
    ambient_profile_id = "asteroid.ambient.starter_dust",
  },
}

AsteroidRegistry.sprite_profiles = {
  {
    sprite_profile_id = "asteroid.sprite.top_down_rocky",
    generator_id = "asteroid_rocky_v1",
    surface_styles = { "Rocky", "Carbonaceous", "Metallic", "GemRich" },
    pixel_step_px = 2,
    crack_intensity_range = { 0.24, 0.56 },
    mineral_vein_intensity_range = { 0.10, 0.45 },
  },
}

AsteroidRegistry.fracture_profiles = {
  {
    fracture_profile_id = "asteroid.fracture.default",
    break_massive_into_large = { 2, 3 },
    break_large_into_medium = { 2, 5 },
    break_medium_into_small = { 2, 6 },
    child_impulse_mps = { 0.4, 2.0 },
    mass_retention_ratio = 0.82,
    terminal_debris_loss_ratio = 0.65,
  },
}

AsteroidRegistry.resource_profiles = {
  {
    resource_profile_id = "asteroid.resource.common_ore",
    extraction_profile_id = "extraction.mining_laser.basic",
    depletion_pool_units = 100.0,
    yield_table = {
      { item_id = "resource.iron_ore", weight = 1.0, min_units = 4.0, max_units = 18.0 },
      { item_id = "resource.nickel_ore", weight = 0.55, min_units = 2.0, max_units = 10.0 },
      { item_id = "resource.silicate_rock", weight = 0.85, min_units = 3.0, max_units = 16.0 },
      { item_id = "resource.rare_earth_oxides", weight = 0.12, min_units = 1.0, max_units = 4.0 },
    },
  },
}

AsteroidRegistry.ambient_profiles = {
  {
    ambient_profile_id = "asteroid.ambient.starter_dust",
    trigger_radius_m = 2600.0,
    fade_band_m = 600.0,
    background_shader_asset_id = nil,
    foreground_shader_asset_id = nil,
    post_process_shader_asset_id = nil,
    max_intensity = 0.65,
  },
}

return AsteroidRegistry

local WorldInit = {}

WorldInit.context = {}

WorldInit.world_defaults = {
  additional_required_asset_ids = {},
  render_layer_definitions = {
    {
      entity_id = "0012ebad-0000-0000-0000-000000000001",
      display_name = "StarField",
      layer_id = "bg_starfield",
      phase = "fullscreen_background",
      material_domain = "fullscreen",
      shader_asset_id = "starfield_wgsl",
      order = -190,
      labels = { "Entity", "RenderLayerDefinition", "StarfieldLayer" },
      entity_labels = { "RenderLayerDefinition", "StarfieldLayer", "StarField" },
    },
    {
      entity_id = "0012ebad-0000-0000-0000-000000000002",
      display_name = "SpaceBackgroundBase",
      layer_id = "bg_space_background_base",
      phase = "fullscreen_background",
      material_domain = "fullscreen",
      shader_asset_id = "space_background_base_wgsl",
      order = -200,
      labels = { "Entity", "RenderLayerDefinition", "SpaceBackgroundLayer", "SpaceBackgroundBaseLayer" },
      entity_labels = { "RenderLayerDefinition", "SpaceBackgroundLayer", "SpaceBackgroundBaseLayer", "SpaceBackgroundBase" },
    },
    {
      entity_id = "0012ebad-0000-0000-0000-000000000014",
      display_name = "SpaceBackgroundNebula",
      layer_id = "bg_space_background_nebula",
      phase = "fullscreen_background",
      material_domain = "fullscreen",
      shader_asset_id = "space_background_nebula_wgsl",
      order = -195,
      labels = { "Entity", "RenderLayerDefinition", "SpaceBackgroundLayer", "SpaceBackgroundNebulaLayer" },
      entity_labels = { "RenderLayerDefinition", "SpaceBackgroundLayer", "SpaceBackgroundNebulaLayer", "SpaceBackgroundNebula" },
    },
    {
      entity_id = "0012ebad-0000-0000-0000-000000000003",
      display_name = "MainWorld",
      layer_id = "main_world",
      phase = "world",
      material_domain = "world_sprite",
      shader_asset_id = "",
      order = 0,
      parallax_factor = 1.0,
      depth_bias_z = 0.0,
      labels = { "Entity", "RenderLayerDefinition", "WorldLayer" },
    },
    {
      entity_id = "0012ebad-0000-0000-0000-000000000004",
      display_name = "MidgroundPlanets",
      layer_id = "midground_planets",
      phase = "world",
      material_domain = "world_polygon",
      shader_asset_id = "planet_visual_wgsl",
      order = -100,
      parallax_factor = 0.18,
      depth_bias_z = -100.0,
      labels = { "Entity", "RenderLayerDefinition", "WorldLayer" },
    },
  },
  render_layer_rules = {
    {
      entity_id = "0012ebad-0000-0000-0000-000000000005",
      display_name = "PlanetsToMidground",
      rule_id = "planets_to_midground",
      target_layer_id = "midground_planets",
      priority = 100,
      labels_any = { "Planet" },
      labels_all = {},
      archetypes_any = {},
      components_all = {},
      components_any = { "planet_body_shader_settings" },
    },
  },
  tactical_presentation_defaults = {
    entity_id = "0012ebad-0000-0000-0000-000000000013",
    display_name = "Tactical Presentation Defaults",
    entity_labels = { "TacticalPresentationDefaults", "PresentationDefaults", "TacticalPresentation" },
    default_map_icon_asset_id = "map_icon_ship_svg",
    icon_bindings_by_kind = {
      { kind = "ship", asset_id = "map_icon_ship_svg" },
      { kind = "planet", asset_id = "map_icon_planet_svg" },
      { kind = "star", asset_id = "map_icon_star_svg" },
      { kind = "asteroid", asset_id = "map_icon_planet_svg" },
    },
  },
  pirate_patrol_bundle_id = "ship.corvette",
  asteroid_bundle_id = "asteroid.field_member",
  planet_bundle_id = "planet.body",
  environment_lighting_bundle_id = "environment.lighting",
  asteroid_field_count = 120,
  asteroid_field_center = { x = 0.0, y = 0.0 },
  asteroid_field_radius_min_m = 500.0,
  asteroid_field_radius_max_m = 2600.0,
  asteroid_field_radial_jitter_m = 240.0,
  asteroid_size_min_m = 4.0,
  asteroid_size_max_m = 28.0,
  asteroid_spin_min_rad_s = -0.06,
  asteroid_spin_max_rad_s = 0.06,
  starter_planet = {
    entity_id = "0012ebad-0000-0000-0000-000000000010",
    display_name = "Aurelia",
    owner_id = "world:system:starter",
    size_m = 760.0,
    spawn_position = { x = 8000.0, y = 0.0 },
    spawn_rotation_rad = 0.0,
    seed = 424242,
    planet_type = 0,
    rotation_speed = 0.0035,
    base_radius_scale = 0.58,
    normal_strength = 0.88,
    detail_level = 0.68,
    light_wrap = 0.22,
    ambient_strength = 0.2,
    specular_strength = 0.26,
    specular_power = 28.0,
    rim_strength = 0.34,
    rim_power = 3.1,
    fresnel_strength = 0.38,
    cloud_shadow_strength = 0.25,
    night_glow_strength = 0.06,
    continent_size = 0.68,
    ocean_level = 0.5,
    mountain_height = 0.34,
    roughness = 0.36,
    terrain_octaves = 6,
    terrain_lacunarity = 2.22,
    terrain_gain = 0.54,
    crater_density = 0.05,
    crater_size = 0.12,
    volcano_density = 0.03,
    ice_cap_size = 0.12,
    storm_intensity = 0.08,
    bands_count = 5.0,
    spot_density = 0.08,
    surface_activity = 0.1,
    corona_intensity = 0.0,
    cloud_coverage = 0.58,
    cloud_scale = 1.9,
    cloud_speed = 0.08,
    cloud_alpha = 0.76,
    atmosphere_thickness = 0.18,
    atmosphere_falloff = 2.4,
    atmosphere_alpha = 0.52,
    city_lights = 0.08,
    emissive_strength = 0.0,
    surface_saturation = 1.18,
    surface_contrast = 1.12,
    light_color_mix = 0.08,
    sun_direction_xy = { 0.76, 0.58 },
    color_primary_rgb = { 0.2, 0.5, 0.24 },
    color_secondary_rgb = { 0.62, 0.56, 0.44 },
    color_tertiary_rgb = { 0.05, 0.21, 0.58 },
    color_atmosphere_rgb = { 0.42, 0.68, 1.0 },
    color_clouds_rgb = { 1.0, 1.0, 1.0 },
    color_night_lights_rgb = { 1.0, 0.82, 0.48 },
    color_emissive_rgb = { 1.0, 0.44, 0.19 },
    map_icon_asset_id = "map_icon_planet_svg",
    sprite_shader_asset_id = "planet_visual_wgsl",
  },
  starter_star = {
    entity_id = "0012ebad-0000-0000-0000-000000000012",
    display_name = "Helion",
    owner_id = "world:system:starter",
    size_m = 400.0,
    spawn_position = { x = 0.0, y = 0.0 },
    spawn_rotation_rad = 0.0,
    body_kind = 1,
    seed = 11,
    planet_type = 0,
    rotation_speed = 0.0012,
    base_radius_scale = 0.62,
    normal_strength = 0.08,
    detail_level = 0.24,
    ambient_strength = 0.0,
    specular_strength = 0.0,
    rim_strength = 1.0,
    rim_power = 2.0,
    fresnel_strength = 0.8,
    surface_activity = 0.84,
    corona_intensity = 1.18,
    atmosphere_thickness = 0.24,
    atmosphere_falloff = 1.6,
    atmosphere_alpha = 0.72,
    emissive_strength = 1.2,
    sun_intensity = 1.0,
    surface_saturation = 1.02,
    surface_contrast = 1.18,
    light_color_mix = 0.0,
    color_primary_rgb = { 1.0, 0.9, 0.52 },
    color_secondary_rgb = { 1.0, 0.72, 0.24 },
    color_tertiary_rgb = { 1.0, 0.34, 0.08 },
    color_atmosphere_rgb = { 1.0, 0.78, 0.34 },
    color_clouds_rgb = { 1.0, 0.96, 0.72 },
    color_night_lights_rgb = { 0.0, 0.0, 0.0 },
    color_emissive_rgb = { 1.0, 0.78, 0.28 },
    map_icon_asset_id = "map_icon_star_svg",
    sprite_shader_asset_id = "planet_visual_wgsl",
  },
  environment_lighting = {
    entity_id = "0012ebad-0000-0000-0000-000000000011",
    display_name = "System Lighting",
    owner_id = "world:system:starter",
    primary_direction_xy = { 0.76, 0.58 },
    primary_elevation = 0.82,
    primary_color_rgb = { 1.0, 0.97, 0.92 },
    primary_intensity = 1.0,
    ambient_color_rgb = { 0.22, 0.3, 0.42 },
    ambient_intensity = 0.18,
    backlight_color_rgb = { 0.28, 0.42, 0.62 },
    backlight_intensity = 0.16,
    event_flash_color_rgb = { 1.0, 0.95, 0.88 },
    event_flash_intensity = 0.0,
  },
}

local function component(entity_id, kind, properties)
  return {
    component_id = entity_id .. ":" .. kind,
    component_kind = kind,
    properties = properties,
  }
end

local function append_records(out, records)
  for i = 1, #records do
    out[#out + 1] = records[i]
  end
end

local function build_asteroid_field_records(ctx)
  if ctx == nil or ctx.spawn_bundle_graph_records == nil then
    return {}
  end
  local defaults = WorldInit.world_defaults
  local asteroid_bundle_id = defaults.asteroid_bundle_id or "asteroid.field_member"
  local count = math.max(math.floor(defaults.asteroid_field_count or 0), 0)
  if count == 0 then
    return {}
  end

  return ctx.spawn_bundle_graph_records(asteroid_bundle_id, {
    field_count = count,
    owner_id = "npc:asteroid_field",
    field_center = defaults.asteroid_field_center or { x = 0.0, y = 0.0 },
    field_radius_min_m = defaults.asteroid_field_radius_min_m or 500.0,
    field_radius_max_m = defaults.asteroid_field_radius_max_m or 2600.0,
    field_radial_jitter_m = defaults.asteroid_field_radial_jitter_m or 240.0,
    asteroid_size_min_m = defaults.asteroid_size_min_m or 4.0,
    asteroid_size_max_m = defaults.asteroid_size_max_m or 28.0,
    asteroid_spin_min_rad_s = defaults.asteroid_spin_min_rad_s or -0.06,
    asteroid_spin_max_rad_s = defaults.asteroid_spin_max_rad_s or 0.06,
    visual_asset_id = "asteroid_texture_red_png",
    map_icon_asset_id = "map_icon_planet_svg",
    -- Keep the sprite shader path compatible with current 2D pipeline.
    sprite_shader_asset_id = "asteroid_wgsl",
  })
end

local function build_planet_records(ctx)
  if ctx == nil or ctx.spawn_bundle_graph_records == nil then
    return {}
  end
  local defaults = WorldInit.world_defaults
  local planet_bundle_id = defaults.planet_bundle_id or "planet.body"
  local starter_planet = defaults.starter_planet
  local starter_star = defaults.starter_star
  local records = {}
  if starter_star ~= nil then
    append_records(records, ctx.spawn_bundle_graph_records(planet_bundle_id, starter_star))
  end
  if starter_planet ~= nil then
    append_records(records, ctx.spawn_bundle_graph_records(planet_bundle_id, starter_planet))
  end
  return records
end

local function build_environment_lighting_records(ctx)
  if ctx == nil or ctx.spawn_bundle_graph_records == nil then
    return {}
  end
  local defaults = WorldInit.world_defaults
  local bundle_id = defaults.environment_lighting_bundle_id or "environment.lighting"
  local lighting = defaults.environment_lighting
  if lighting == nil then
    return {}
  end
  return ctx.spawn_bundle_graph_records(bundle_id, lighting)
end

local function build_tactical_presentation_defaults_records()
  local defaults = WorldInit.world_defaults.tactical_presentation_defaults
  if defaults == nil or defaults.entity_id == nil then
    return {}
  end

  return {
    {
      entity_id = defaults.entity_id,
      labels = { "Entity", "TacticalPresentationDefaults" },
      properties = {},
      components = {
        component(defaults.entity_id, "display_name", defaults.display_name or "Tactical Presentation Defaults"),
        component(defaults.entity_id, "entity_labels", defaults.entity_labels or { "TacticalPresentationDefaults", "PresentationDefaults" }),
        component(defaults.entity_id, "owner_id", "world:system"),
        component(defaults.entity_id, "tactical_presentation_defaults", {
          default_map_icon_asset_id = defaults.default_map_icon_asset_id,
          icon_bindings_by_kind = defaults.icon_bindings_by_kind or {},
        }),
        component(defaults.entity_id, "public_visibility", {}),
      },
    },
  }
end

function WorldInit.build_graph_records(ctx)
  local records = {}
  local layer_by_id = {}
  for _, layer in ipairs(WorldInit.world_defaults.render_layer_definitions or {}) do
    local record = ctx.render:define_layer({
      entity_id = layer.entity_id,
      display_name = layer.display_name,
      layer_id = layer.layer_id,
      phase = layer.phase,
      material_domain = layer.material_domain,
      shader_asset_id = layer.shader_asset_id,
      order = layer.order,
      parallax_factor = layer.parallax_factor,
      depth_bias_z = layer.depth_bias_z,
      enabled = layer.enabled ~= false,
    })
    if layer.labels ~= nil then
      record.labels = layer.labels
    end
    if layer.entity_labels ~= nil then
      record.components[#record.components + 1] =
        component(record.entity_id, "entity_labels", layer.entity_labels)
    end
    records[#records + 1] = record
    layer_by_id[layer.layer_id] = record
  end

  for _, rule in ipairs(WorldInit.world_defaults.render_layer_rules or {}) do
    local record = ctx.render:define_rule({
      entity_id = rule.entity_id,
      display_name = rule.display_name,
      rule_id = rule.rule_id,
      target_layer_id = rule.target_layer_id,
      priority = rule.priority,
      labels_any = rule.labels_any,
      labels_all = rule.labels_all,
      archetypes_any = rule.archetypes_any,
      components_all = rule.components_all,
      components_any = rule.components_any,
      enabled = rule.enabled ~= false,
    })
    records[#records + 1] = record
  end

  do
    local record = layer_by_id["bg_space_background_base"]
    if record ~= nil then
      record.components[#record.components + 1] = component(record.entity_id, "space_background_shader_settings", {
          enabled = true,
          enable_nebula_layer = false,
          enable_stars_layer = true,
          enable_flares_layer = true,
          enable_background_gradient = false,
          intensity = 0.65,
          drift_scale = 2.8,
          zoom_rate = 1.0,
          velocity_glow = 1.0,
          nebula_strength = 0.85,
          seed = 0.0,
          background_rgb = { 0.0, 0.0, 0.0 },
          nebula_color_primary_rgb = { 0.0, 0.0, 0.196 },
          nebula_color_secondary_rgb = { 0.0, 0.073, 0.082 },
          nebula_color_accent_rgb = { 0.187, 0.16, 0.539 },
          flare_enabled = true,
          flare_tint_rgb = { 1.0, 1.0, 2.0 },
          flare_intensity = 0.42,
          flare_density = 0.54,
          flare_size = 2.29,
          flare_texture_asset_id = "space_bg_flare_white_png",
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
          star_mask_gain = 0.42,
          star_mask_lacunarity = 1.75,
          star_mask_threshold = 0.35,
          star_mask_power = 1.25,
          star_mask_ridge_offset = 0.83,
          star_mask_scale = 3.1,
          nebula_blend_mode = 1,
          nebula_opacity = 0.5,
          stars_blend_mode = 2,
          stars_opacity = 1.0,
          star_count = 5.0,
          star_size_min = 0.019,
          star_size_max = 0.022,
          star_color_rgb = { 0.698, 0.682, 2.0 },
          flares_blend_mode = 1,
          flares_opacity = 1.0,
          depth_layer_separation = 0.6,
          depth_parallax_scale = 1.3,
          depth_haze_strength = 1.69,
          depth_occlusion_strength = 1.0,
          backlight_screen_x = -0.3,
          backlight_screen_y = 0.1,
          backlight_intensity = 8.25,
          backlight_wrap = 0.5,
          backlight_edge_boost = 1.87,
          backlight_bloom_scale = 1.8,
          backlight_bloom_threshold = 0.05,
          enable_backlight = true,
          enable_light_shafts = true,
          shafts_debug_view = false,
          shaft_intensity = 11.26,
          shaft_length = 0.29,
          shaft_falloff = 3.81,
          shaft_samples = 12,
          shaft_quality = 2,
          shaft_blend_mode = 0,
          shaft_opacity = 1.0,
          shaft_color_rgb = { 0.951, 0.931, 1.188 },
          backlight_color_rgb = { 1.067, 0.815, 1.45 },
          tint_rgb = { 1.0, 1.77, 1.24 },
      })
    end
  end

  do
    local record = layer_by_id["bg_space_background_nebula"]
    if record ~= nil then
      record.components[#record.components + 1] = component(record.entity_id, "space_background_shader_settings", {
          enabled = true,
          enable_nebula_layer = true,
          enable_stars_layer = false,
          enable_flares_layer = false,
          enable_background_gradient = false,
          intensity = 0.65,
          drift_scale = 2.8,
          zoom_rate = 1.0,
          velocity_glow = 1.0,
          nebula_strength = 0.85,
          seed = 0.0,
          background_rgb = { 0.0, 0.0, 0.0 },
          nebula_color_primary_rgb = { 0.0, 0.0, 0.196 },
          nebula_color_secondary_rgb = { 0.0, 0.073, 0.082 },
          nebula_color_accent_rgb = { 0.187, 0.16, 0.539 },
          flare_enabled = false,
          flare_tint_rgb = { 1.0, 1.0, 2.0 },
          flare_intensity = 0.42,
          flare_density = 0.54,
          flare_size = 2.29,
          flare_texture_asset_id = "space_bg_flare_white_png",
          flare_texture_set = 0,
          nebula_noise_mode = 0,
          nebula_octaves = 5,
          nebula_gain = 0.52,
          nebula_lacunarity = 2.0,
          nebula_power = 1.0,
          nebula_shelf = 0.42,
          nebula_ridge_offset = 1.0,
          star_mask_enabled = false,
          star_mask_mode = 0,
          star_mask_octaves = 4,
          star_mask_gain = 0.42,
          star_mask_lacunarity = 1.75,
          star_mask_threshold = 0.35,
          star_mask_power = 1.25,
          star_mask_ridge_offset = 0.83,
          star_mask_scale = 3.1,
          nebula_blend_mode = 1,
          nebula_opacity = 0.5,
          stars_blend_mode = 2,
          stars_opacity = 1.0,
          star_count = 5.0,
          star_size_min = 0.019,
          star_size_max = 0.022,
          star_color_rgb = { 0.698, 0.682, 2.0 },
          flares_blend_mode = 1,
          flares_opacity = 1.0,
          depth_layer_separation = 0.6,
          depth_parallax_scale = 1.3,
          depth_haze_strength = 1.69,
          depth_occlusion_strength = 1.0,
          backlight_screen_x = -0.3,
          backlight_screen_y = 0.1,
          backlight_intensity = 8.25,
          backlight_wrap = 0.5,
          backlight_edge_boost = 1.87,
          backlight_bloom_scale = 1.8,
          backlight_bloom_threshold = 0.05,
          enable_backlight = true,
          enable_light_shafts = true,
          shafts_debug_view = false,
          shaft_intensity = 11.26,
          shaft_length = 0.29,
          shaft_falloff = 3.81,
          shaft_samples = 12,
          shaft_quality = 2,
          shaft_blend_mode = 0,
          shaft_opacity = 1.0,
          shaft_color_rgb = { 0.951, 0.931, 1.188 },
          backlight_color_rgb = { 1.067, 0.815, 1.45 },
          tint_rgb = { 1.0, 1.77, 1.24 },
      })
    end
  end

  do
    local record = layer_by_id["bg_starfield"]
    if record ~= nil then
      record.components[#record.components + 1] = component(record.entity_id, "starfield_shader_settings", {
          enabled = true,
          density = 0.07,
          layer_count = 4,
          initial_z_offset = 0.5,
          intensity = 4.0,
          alpha = 0.32,
          tint_rgb = { 1.0, 1.0, 1.34 },
          star_size = 0.3,
          star_intensity = 6.65,
          star_alpha = 1.0,
          star_color_rgb = { 0.33, 0.33, 1.49 },
          corona_size = 2.68,
          corona_intensity = 1.35,
          corona_alpha = 1.0,
          corona_color_rgb = { 0.42, 0.42, 1.83 },
      })
    end
  end

  if ctx ~= nil and ctx.spawn_bundle_graph_records ~= nil then
    local pirate_bundle_id = WorldInit.world_defaults.pirate_patrol_bundle_id or "ship.corvette"
    local pirate_records = ctx.spawn_bundle_graph_records(pirate_bundle_id, {
      display_name = "Pirate Patrol",
      owner_id = "npc:pirate_patrol_1",
      ship_entity_labels = { "Ship", "Corvette", "Pirate", "Npc" },
      spawn_position = { -2400.0, -1400.0 },
      scanner_base_range_m = 300.0,
      script_state_data = {
        on_tick_handler = "pirate_patrol",
        tick_interval_s = 2.0,
        event_hooks = {},
        patrol_index = 1,
        patrol_points = {
          { x = -2400, y = -1400 },
          { x = -1200, y = -2400 },
          { x = -100, y = -1000 },
          { x = -1600, y = -200 },
        },
      },
    })
    append_records(records, pirate_records)
  end

  append_records(records, build_asteroid_field_records(ctx))
  append_records(records, build_planet_records(ctx))
  append_records(records, build_environment_lighting_records(ctx))
  append_records(records, build_tactical_presentation_defaults_records())

  return records
end

return WorldInit

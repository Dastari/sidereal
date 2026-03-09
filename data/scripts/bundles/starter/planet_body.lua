local PlanetBody = {}

local function component(entity_id, kind, properties)
  return {
    component_id = entity_id .. ":" .. kind,
    component_kind = kind,
    properties = properties,
  }
end

local function vec2(value, fallback_x, fallback_y)
  if type(value) ~= "table" then
    return { fallback_x, fallback_y }
  end
  return {
    value[1] or value.x or fallback_x,
    value[2] or value.y or fallback_y,
  }
end

local function vec3(value, fallback_x, fallback_y, fallback_z)
  if type(value) ~= "table" then
    return { fallback_x, fallback_y, fallback_z }
  end
  return {
    value[1] or value.x or fallback_x,
    value[2] or value.y or fallback_y,
    value[3] or value.z or fallback_z,
  }
end

function PlanetBody.build_graph_records(ctx)
  local entity_id = ctx.entity_id or error("planet.body requires entity_id")
  local display_name = ctx.display_name or "Planet"
  local labels = ctx.entity_labels or { "Planet", "CelestialBody" }
  local owner_id = ctx.owner_id or "world:system"
  local body_size_m = ctx.size_m or 640.0
  local spawn_position = ctx.spawn_position or { 0.0, 0.0 }
  local spawn_rotation_rad = ctx.spawn_rotation_rad or 0.0
  local planet_visual_shader_asset_id = ctx.planet_visual_shader_asset_id or "planet_visual_wgsl"
  local runtime_render_layer_override = {
    layer_id = ctx.runtime_render_layer_id or "midground_planets",
  }
  local landmark_kind = ctx.landmark_kind
  if landmark_kind == nil or landmark_kind == "" then
    if (ctx.body_kind or 0) == 1 then
      landmark_kind = "Star"
    elseif (ctx.body_kind or 0) == 2 then
      landmark_kind = "BlackHole"
    else
      landmark_kind = "Planet"
    end
  end
  local static_landmark = {
    kind = landmark_kind,
    discoverable = ctx.discoverable ~= false,
    always_known = ctx.always_known == true,
    discovery_radius_m = ctx.discovery_radius_m,
    use_extent_for_discovery = ctx.use_extent_for_discovery ~= false,
  }
  local shader_settings = {
    enabled = ctx.enabled ~= false,
    enable_surface_detail = ctx.enable_surface_detail ~= false,
    enable_craters = ctx.enable_craters ~= false,
    enable_clouds = ctx.enable_clouds ~= false,
    enable_atmosphere = ctx.enable_atmosphere ~= false,
    enable_specular = ctx.enable_specular ~= false,
    enable_night_lights = ctx.enable_night_lights ~= false,
    enable_emissive = ctx.enable_emissive ~= false,
    enable_ocean_specular = ctx.enable_ocean_specular ~= false,
    body_kind = ctx.body_kind or 0,
    planet_type = ctx.planet_type or 0,
    seed = ctx.seed or 1,
    base_radius_scale = ctx.base_radius_scale or 0.5,
    normal_strength = ctx.normal_strength or 0.55,
    detail_level = ctx.detail_level or 0.3,
    rotation_speed = ctx.rotation_speed or 0.004,
    light_wrap = ctx.light_wrap or 0.2,
    ambient_strength = ctx.ambient_strength or 0.16,
    specular_strength = ctx.specular_strength or 0.12,
    specular_power = ctx.specular_power or 18.0,
    rim_strength = ctx.rim_strength or 0.28,
    rim_power = ctx.rim_power or 3.6,
    fresnel_strength = ctx.fresnel_strength or 0.4,
    cloud_shadow_strength = ctx.cloud_shadow_strength or 0.18,
    night_glow_strength = ctx.night_glow_strength or 0.05,
    continent_size = ctx.continent_size or 0.58,
    ocean_level = ctx.ocean_level or 0.46,
    mountain_height = ctx.mountain_height or 0.34,
    roughness = ctx.roughness or 0.44,
    terrain_octaves = ctx.terrain_octaves or 5,
    terrain_lacunarity = ctx.terrain_lacunarity or 2.1,
    terrain_gain = ctx.terrain_gain or 0.5,
    crater_density = ctx.crater_density or 0.18,
    crater_size = ctx.crater_size or 0.33,
    volcano_density = ctx.volcano_density or 0.04,
    ice_cap_size = ctx.ice_cap_size or 0.18,
    storm_intensity = ctx.storm_intensity or 0.1,
    bands_count = ctx.bands_count or 6.0,
    spot_density = ctx.spot_density or 0.08,
    surface_activity = ctx.surface_activity or 0.12,
    corona_intensity = ctx.corona_intensity or 0.0,
    cloud_coverage = ctx.cloud_coverage or 0.34,
    cloud_scale = ctx.cloud_scale or 1.3,
    cloud_speed = ctx.cloud_speed or 0.18,
    cloud_alpha = ctx.cloud_alpha or 0.42,
    atmosphere_thickness = ctx.atmosphere_thickness or 0.12,
    atmosphere_falloff = ctx.atmosphere_falloff or 2.8,
    atmosphere_alpha = ctx.atmosphere_alpha or 0.48,
    city_lights = ctx.city_lights or 0.04,
    emissive_strength = ctx.emissive_strength or 0.0,
    sun_intensity = ctx.sun_intensity or 1.0,
    surface_saturation = ctx.surface_saturation or 1.12,
    surface_contrast = ctx.surface_contrast or 1.08,
    light_color_mix = ctx.light_color_mix or 0.14,
    sun_direction_xy = vec2(ctx.sun_direction_xy, 0.74, 0.52),
    color_primary_rgb = vec3(ctx.color_primary_rgb, 0.24, 0.48, 0.22),
    color_secondary_rgb = vec3(ctx.color_secondary_rgb, 0.52, 0.42, 0.28),
    color_tertiary_rgb = vec3(ctx.color_tertiary_rgb, 0.08, 0.2, 0.48),
    color_atmosphere_rgb = vec3(ctx.color_atmosphere_rgb, 0.36, 0.62, 1.0),
    color_clouds_rgb = vec3(ctx.color_clouds_rgb, 0.95, 0.97, 1.0),
    color_night_lights_rgb = vec3(ctx.color_night_lights_rgb, 1.0, 0.76, 0.4),
    color_emissive_rgb = vec3(ctx.color_emissive_rgb, 1.0, 0.42, 0.18),
  }
  local visual_stack = {
    passes = {
      {
        pass_id = "planet_body",
        visual_family = "planet",
        visual_kind = "body",
        material_domain = "world_polygon",
        shader_asset_id = planet_visual_shader_asset_id,
        order = 0,
        scale_multiplier = 1.0,
        depth_bias_z = 0.0,
      },
    },
  }
  local has_clouds = shader_settings.enabled
    and shader_settings.enable_clouds
    and shader_settings.body_kind == 0
    and shader_settings.cloud_alpha > 0.01
    and shader_settings.cloud_coverage > 0.01
  if has_clouds then
    visual_stack.passes[#visual_stack.passes + 1] = {
      pass_id = "planet_cloud_back",
      visual_family = "planet",
      visual_kind = "cloud_back",
      material_domain = "world_polygon",
      shader_asset_id = planet_visual_shader_asset_id,
      order = -1,
      scale_multiplier = 1.035,
      depth_bias_z = -0.2,
    }
    visual_stack.passes[#visual_stack.passes + 1] = {
      pass_id = "planet_cloud_front",
      visual_family = "planet",
      visual_kind = "cloud_front",
      material_domain = "world_polygon",
      shader_asset_id = planet_visual_shader_asset_id,
      order = 1,
      scale_multiplier = 1.035,
      depth_bias_z = 0.5,
    }
  end
  local has_rings = shader_settings.enabled
    and (
      shader_settings.body_kind == 2 or
      (
        shader_settings.body_kind == 0
          and shader_settings.planet_type == 4
          and (shader_settings.corona_intensity > 0.05 or shader_settings.spot_density > 0.18)
      )
    )
  if has_rings then
    visual_stack.passes[#visual_stack.passes + 1] = {
      pass_id = "planet_ring_back",
      visual_family = "planet",
      visual_kind = "ring_back",
      material_domain = "world_polygon",
      shader_asset_id = planet_visual_shader_asset_id,
      order = -2,
      scale_multiplier = 1.85,
      depth_bias_z = -0.45,
    }
    visual_stack.passes[#visual_stack.passes + 1] = {
      pass_id = "planet_ring_front",
      visual_family = "planet",
      visual_kind = "ring_front",
      material_domain = "world_polygon",
      shader_asset_id = planet_visual_shader_asset_id,
      order = 2,
      scale_multiplier = 1.85,
      depth_bias_z = 0.65,
    }
  end

  return {
    {
      entity_id = entity_id,
      labels = labels,
      properties = {},
      components = {
        component(entity_id, "display_name", display_name),
        component(entity_id, "entity_labels", labels),
        component(entity_id, "owner_id", owner_id),
        component(entity_id, "size_m", {
          length = body_size_m,
          width = body_size_m,
          height = body_size_m,
        }),
        component(entity_id, "static_landmark", static_landmark),
        component(entity_id, "runtime_render_layer_override", runtime_render_layer_override),
        component(entity_id, "map_icon", {
          asset_id = ctx.map_icon_asset_id or "map_icon_planet_svg",
        }),
        component(entity_id, "sprite_shader_asset_id", planet_visual_shader_asset_id),
        component(entity_id, "world_position", {
          spawn_position[1] or spawn_position.x or 0.0,
          spawn_position[2] or spawn_position.y or 0.0,
        }),
        component(entity_id, "world_rotation", spawn_rotation_rad),
        component(entity_id, "planet_body_shader_settings", shader_settings),
        component(entity_id, "runtime_world_visual_stack", visual_stack),
      },
    },
  }
end

return PlanetBody

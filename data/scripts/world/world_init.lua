local WorldInit = {}

WorldInit.context = {}

WorldInit.world_defaults = {
  space_background_shader_asset_id = "space_background_wgsl",
  starfield_shader_asset_id = "starfield_wgsl",
  additional_required_asset_ids = {
    "sprite_pixel_effect_wgsl",
    "thruster_plume_wgsl",
    "weapon_impact_spark_wgsl",
    "tactical_map_overlay_wgsl",
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

function WorldInit.build_graph_records(ctx)
  local space_background_id = "0012ebad-0000-0000-0000-000000000002"
  local starfield_id = "0012ebad-0000-0000-0000-000000000001"

  local records = {
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
          intensity = 0.35,
          drift_scale = 2.0,
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
          flare_intensity = 4.0,
          flare_density = 0.54,
          flare_size = 2.29,
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
          depth_layer_separation = 1.03,
          depth_parallax_scale = 0.83,
          depth_haze_strength = 1.69,
          depth_occlusion_strength = 1.08,
          backlight_screen_x = -0.3,
          backlight_screen_y = 0.1,
          backlight_intensity = 4.0,
          backlight_wrap = 0.49,
          backlight_edge_boost = 2.2,
          backlight_bloom_scale = 1.35,
          backlight_bloom_threshold = 0.14,
          enable_backlight = true,
          enable_light_shafts = true,
          shafts_debug_view = false,
          shaft_intensity = 1.76,
          shaft_length = 0.47,
          shaft_falloff = 2.65,
          shaft_samples = 16,
          shaft_blend_mode = 1,
          shaft_opacity = 0.85,
          shaft_color_rgb = { 1.15, 1.0, 1.45 },
          backlight_color_rgb = { 1.15, 1.0, 1.45 },
          tint_rgb = { 1.0, 1.77, 1.24 },
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
          star_size = 0.3,
          star_intensity = 6.65,
          star_alpha = 1.0,
          star_color_rgb = { 0.33, 0.33, 1.49 },
          corona_size = 2.68,
          corona_intensity = 1.35,
          corona_alpha = 1.0,
          corona_color_rgb = { 0.42, 0.42, 1.83 },
        }),
      },
    },
  }

  if ctx ~= nil and ctx.spawn_bundle_graph_records ~= nil then
    local pirate_records = ctx.spawn_bundle_graph_records("corvette", {
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

  return records
end

return WorldInit

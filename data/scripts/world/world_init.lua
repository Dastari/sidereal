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

local PlayerBundle = {}

PlayerBundle.context = {}

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

function PlayerBundle.build_graph_records(ctx)
  local player_id = ctx.player_entity_id or ctx.entity_id or ctx.new_uuid()
  local email = ctx.email or "pilot@example.com"
  local account_id = ctx.account_id
  local controlled_entity_guid = ctx.controlled_entity_guid

  if account_id == nil or account_id == "" then
    error("player bundle requires ctx.account_id")
  end
  if controlled_entity_guid == nil or controlled_entity_guid == "" then
    error("player bundle requires ctx.controlled_entity_guid")
  end

  return {
    new_entity(player_id, { "Entity", "Player" }, nil, {
      component(player_id, "display_name", email),
      component(player_id, "player_tag", {}),
      component(player_id, "account_id", account_id),
      component(player_id, "controlled_entity_guid", controlled_entity_guid),
      component(player_id, "entity_labels", { "Player" }),
      component(player_id, "owner_id", player_id),
      component(player_id, "action_capabilities", {
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
        },
      }),
      component(player_id, "character_movement_controller", {
        speed_mps = 220.0,
        max_accel_mps2 = 880.0,
        damping_per_s = 8.0,
      }),
      component(player_id, "action_queue", {
        pending = { "LongitudinalNeutral", "LateralNeutral" },
      }),
      component(player_id, "contact_resolution_m", ctx.contact_resolution_m or 100.0),
      component(player_id, "tactical_map_ui_settings", {
        shader_asset_id = "tactical_map_overlay_wgsl",
        map_distance_m = 90.0,
        map_zoom_wheel_sensitivity = 0.12,
        overlay_takeover_alpha = 0.995,
        grid_major_color_rgb = { 0.22, 0.34, 0.48 },
        grid_minor_color_rgb = { 0.22, 0.34, 0.48 },
        grid_micro_color_rgb = { 0.22, 0.34, 0.48 },
        grid_major_alpha = 0.14,
        grid_minor_alpha = 0.126,
        grid_micro_alpha = 0.113,
        grid_major_glow_alpha = 0.02,
        grid_minor_glow_alpha = 0.018,
        grid_micro_glow_alpha = 0.016,
        background_color_rgb = { 0.005, 0.008, 0.02 },
        line_width_major_px = 1.4,
        line_width_minor_px = 0.95,
        line_width_micro_px = 0.75,
        glow_width_major_px = 2.0,
        glow_width_minor_px = 1.5,
        glow_width_micro_px = 1.2,
        fx_mode = 1,
        fx_opacity = 0.45,
        fx_noise_amount = 0.12,
        fx_scanline_density = 360.0,
        fx_scanline_speed = 0.65,
        fx_crt_distortion = 0.02,
        fx_vignette_strength = 0.24,
        fx_green_tint_mix = 0.0,
      }),
      component(player_id, "avian_position", { 0.0, 0.0 }),
      component(player_id, "avian_rotation", { cos = 1.0, sin = 0.0 }),
      component(player_id, "avian_linear_velocity", { 0.0, 0.0 }),
      component(player_id, "avian_rigid_body", "Dynamic"),
      component(player_id, "avian_mass", 1.0),
      component(player_id, "avian_angular_inertia", 1.0),
      component(player_id, "avian_linear_damping", 0.0),
      component(player_id, "avian_angular_damping", 0.0),
    }),
  }
end

return PlayerBundle

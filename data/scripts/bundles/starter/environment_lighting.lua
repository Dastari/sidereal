local EnvironmentLighting = {}

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

function EnvironmentLighting.build_graph_records(ctx)
  local entity_id = ctx.entity_id or error("environment.lighting requires entity_id")
  local display_name = ctx.display_name or "EnvironmentLighting"
  local labels = ctx.entity_labels or { "EnvironmentLighting", "Lighting" }
  local owner_id = ctx.owner_id or "world:system"
  local lighting = {
    primary_direction_xy = vec2(ctx.primary_direction_xy, 0.76, 0.58),
    primary_elevation = ctx.primary_elevation or 0.36,
    primary_color_rgb = vec3(ctx.primary_color_rgb, 1.0, 0.92, 0.78),
    primary_intensity = ctx.primary_intensity or 1.15,
    ambient_color_rgb = vec3(ctx.ambient_color_rgb, 0.16, 0.20, 0.27),
    ambient_intensity = ctx.ambient_intensity or 0.12,
    backlight_color_rgb = vec3(ctx.backlight_color_rgb, 0.28, 0.42, 0.62),
    backlight_intensity = ctx.backlight_intensity or 0.08,
    event_flash_color_rgb = vec3(ctx.event_flash_color_rgb, 1.0, 0.95, 0.88),
    event_flash_intensity = ctx.event_flash_intensity or 0.0,
  }

  return {
    {
      entity_id = entity_id,
      labels = labels,
      properties = {},
      components = {
        component(entity_id, "display_name", display_name),
        component(entity_id, "entity_labels", labels),
        component(entity_id, "owner_id", owner_id),
        component(entity_id, "public_visibility", {}),
        component(entity_id, "environment_lighting_state", lighting),
      },
    },
  }
end

return EnvironmentLighting

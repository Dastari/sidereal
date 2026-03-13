local AsteroidFieldBundle = {}

AsteroidFieldBundle.context = {}

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

local function fract(v)
  return v - math.floor(v)
end

local function hash01(index, salt)
  return fract(math.sin(index * 12.9898 + salt * 78.233) * 43758.5453)
end

local function lerp(a, b, t)
  return a + (b - a) * t
end

local function build_single_asteroid(ctx, opts)
  local asteroid_id = opts.entity_id or ctx.new_uuid()
  local owner_id = opts.owner_id or "npc:asteroid_field"
  local asteroid_labels = opts.asteroid_entity_labels or { "Asteroid", "FieldMember" }
  local display_name = opts.display_name or "Asteroid"
  local spawn_position = opts.spawn_position or { 0.0, 0.0 }
  local diameter_m = math.max(opts.diameter_m or 14.0, 2.0)
  local mass_kg = math.max(opts.mass_kg or (diameter_m * diameter_m * diameter_m * 180.0), 100.0)
  local max_hp = math.max(opts.health_points or (diameter_m * 16.0), 30.0)
  local spin_rad_s = opts.spin_rad_s or 0.0
  local rotation_rad = opts.rotation_rad or 0.0
  local visual_asset_id = opts.visual_asset_id or "asteroid_texture_red_png"
  local map_icon_asset_id = opts.map_icon_asset_id or "map_icon_planet_svg"
  local sprite_shader_asset_id = opts.sprite_shader_asset_id or "asteroid_wgsl"
  local procedural_sprite = opts.procedural_sprite or {
    generator_id = "asteroid_rocky_v1",
    resolution_px = 160,
    edge_noise = 0.03,
    lobe_amplitude = 0.12,
    crater_count = 6,
    palette_dark_rgb = { 0.18, 0.16, 0.14 },
    palette_light_rgb = { 0.54, 0.48, 0.42 },
  }
  local collision_half_extents = { diameter_m * 0.5, diameter_m * 0.5 }
  if ctx.compute_collision_half_extents_from_procedural ~= nil then
    collision_half_extents =
      ctx.compute_collision_half_extents_from_procedural(asteroid_id, procedural_sprite, diameter_m)
  end
  local half_extent_x_m = collision_half_extents[1] or (diameter_m * 0.5)
  local half_extent_y_m = collision_half_extents[2] or (diameter_m * 0.5)
  local half_extent_z_m = math.max(math.max(half_extent_x_m, half_extent_y_m) * 0.7, 0.5)
  local generated_outline_points = nil
  if ctx.generate_collision_outline_rdp_from_procedural ~= nil then
    generated_outline_points = ctx.generate_collision_outline_rdp_from_procedural(
      asteroid_id,
      procedural_sprite,
      collision_half_extents
    )
  end

  local asteroid_components = {
    component(asteroid_id, "display_name", display_name),
    component(asteroid_id, "entity_labels", asteroid_labels),
    component(asteroid_id, "health_pool", { current = max_hp, maximum = max_hp }),
    component(asteroid_id, "destructible", {
      destruction_profile_id = "destruction.asteroid.default",
      destroy_delay_s = 0.18,
    }),
    component(asteroid_id, "owner_id", owner_id),
    component(asteroid_id, "mass_kg", mass_kg),
    component(asteroid_id, "size_m", {
      length = half_extent_y_m * 2.0,
      width = half_extent_x_m * 2.0,
      height = diameter_m * 0.8,
    }),
    component(asteroid_id, "collision_profile", { mode = "Aabb" }),
    component(asteroid_id, "collision_aabb_m", {
      half_extents = { half_extent_x_m, half_extent_y_m, half_extent_z_m },
    }),
    component(asteroid_id, "visual_asset_id", visual_asset_id),
    component(asteroid_id, "map_icon", { asset_id = map_icon_asset_id }),
    component(asteroid_id, "avian_position", { spawn_position[1] or 0.0, spawn_position[2] or 0.0 }),
    component(asteroid_id, "avian_rotation", { cos = math.cos(rotation_rad), sin = math.sin(rotation_rad) }),
    component(asteroid_id, "avian_linear_velocity", { 0.0, 0.0 }),
    -- Asteroid field members currently collide via AABB/outline hulls and do not translate.
    -- Running them as continuously rotating kinematic Avian bodies forces unnecessary broadphase,
    -- replication, and persistence churn for motion that does not materially change collision.
    -- Keep the randomized initial heading, but pin the physics body static until we introduce a
    -- separate visual-only spin lane.
    component(asteroid_id, "avian_angular_velocity", 0.0),
    component(asteroid_id, "avian_rigid_body", "Static"),
    component(asteroid_id, "avian_linear_damping", 0.0),
    component(asteroid_id, "avian_angular_damping", 0.0),
  }

  if sprite_shader_asset_id ~= nil and sprite_shader_asset_id ~= "" then
    asteroid_components[#asteroid_components + 1] =
      component(asteroid_id, "sprite_shader_asset_id", sprite_shader_asset_id)
  end
  if generated_outline_points ~= nil and #generated_outline_points >= 3 then
    asteroid_components[#asteroid_components + 1] =
      component(asteroid_id, "collision_outline_m", {
        points = generated_outline_points,
      })
  end
  if procedural_sprite ~= nil then
    asteroid_components[#asteroid_components + 1] =
      component(asteroid_id, "procedural_sprite", procedural_sprite)
  end

  return new_entity(asteroid_id, { "Entity", "Asteroid" }, nil, asteroid_components)
end

function AsteroidFieldBundle.build_graph_records(ctx)
  local count = math.max(math.floor(ctx.field_count or 1), 1)
  if count <= 1 then
    return {
      build_single_asteroid(ctx, ctx),
    }
  end

  local records = {}
  local owner_id = ctx.owner_id or "npc:asteroid_field"
  local center = ctx.field_center or { x = 0.0, y = 0.0 }
  local center_x = center.x or center[1] or 0.0
  local center_y = center.y or center[2] or 0.0

  local min_radius = ctx.field_radius_min_m or 500.0
  local max_radius = ctx.field_radius_max_m or 2600.0
  if max_radius < min_radius then
    max_radius = min_radius
  end
  local radial_jitter = ctx.field_radial_jitter_m or 240.0

  local size_min = ctx.asteroid_size_min_m or 4.0
  local size_max = ctx.asteroid_size_max_m or 28.0
  if size_max < size_min then
    size_max = size_min
  end

  local spin_min = ctx.asteroid_spin_min_rad_s or -0.06
  local spin_max = ctx.asteroid_spin_max_rad_s or 0.06
  if spin_max < spin_min then
    spin_max = spin_min
  end

  local visual_asset_id = ctx.visual_asset_id or "asteroid_texture_red_png"
  local map_icon_asset_id = ctx.map_icon_asset_id or "map_icon_planet_svg"
  local sprite_shader_asset_id = ctx.sprite_shader_asset_id

  for i = 1, count do
    local u = i / count
    local angle = i * 2.399963229728653 + (hash01(i, 1.0) - 0.5) * 0.42
    local radius = lerp(min_radius, max_radius, math.sqrt(u))
    radius = radius + (hash01(i, 2.0) - 0.5) * radial_jitter

    local x = center_x + math.cos(angle) * radius
    local y = center_y + math.sin(angle) * radius

    local diameter_m = lerp(size_min, size_max, hash01(i, 3.0))
    local mass_kg = math.max(diameter_m * diameter_m * diameter_m * lerp(130.0, 260.0, hash01(i, 4.0)), 100.0)
    local health_points = math.max(diameter_m * lerp(12.0, 20.0, hash01(i, 5.0)), 30.0)
    local spin_rad_s = lerp(spin_min, spin_max, hash01(i, 6.0))
    local rotation_rad = hash01(i, 7.0) * math.pi * 2.0

    local asteroid_kind_roll = hash01(i, 8.0)
    local asteroid_kind = "Rocky"
    local palette_dark_rgb = { 0.18, 0.16, 0.14 }
    local palette_light_rgb = { 0.54, 0.48, 0.42 }
    local crater_count = 6
    if asteroid_kind_roll > 0.94 then
      asteroid_kind = "Gem-rich"
      palette_dark_rgb = { 0.15, 0.14, 0.18 }
      palette_light_rgb = { 0.46, 0.45, 0.56 }
      crater_count = 5
    elseif asteroid_kind_roll > 0.74 then
      asteroid_kind = "Metallic"
      palette_dark_rgb = { 0.16, 0.17, 0.19 }
      palette_light_rgb = { 0.58, 0.60, 0.63 }
      crater_count = 4
    elseif asteroid_kind_roll > 0.54 then
      asteroid_kind = "Carbonaceous"
      palette_dark_rgb = { 0.08, 0.08, 0.09 }
      palette_light_rgb = { 0.30, 0.29, 0.27 }
      crater_count = 7
    end

    records[#records + 1] = build_single_asteroid(ctx, {
      owner_id = owner_id,
      display_name = string.format("%s Asteroid %03d", asteroid_kind, i),
      asteroid_entity_labels = { "Asteroid", "FieldMember", asteroid_kind },
      spawn_position = { x, y },
      diameter_m = diameter_m,
      mass_kg = mass_kg,
      health_points = health_points,
      spin_rad_s = spin_rad_s,
      rotation_rad = rotation_rad,
      visual_asset_id = visual_asset_id,
      map_icon_asset_id = map_icon_asset_id,
      sprite_shader_asset_id = sprite_shader_asset_id,
      procedural_sprite = {
        generator_id = "asteroid_rocky_v1",
        resolution_px = 160,
        edge_noise = 0.018 + hash01(i, 9.0) * 0.022,
        lobe_amplitude = 0.11 + hash01(i, 10.0) * 0.06,
        crater_count = crater_count,
        palette_dark_rgb = palette_dark_rgb,
        palette_light_rgb = palette_light_rgb,
      },
    })
  end

  return records
end

return AsteroidFieldBundle

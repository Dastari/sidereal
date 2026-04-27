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
    surface_style = "Rocky",
    pixel_step_px = 2,
    crack_intensity = 0.3,
    mineral_vein_intensity = 0.16,
    mineral_accent_rgb = { 0.72, 0.52, 0.24 },
    family_seed_key = nil,
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

  if opts.field_entity_id ~= nil then
    asteroid_components[#asteroid_components + 1] =
      component(asteroid_id, "asteroid_field_member", {
        field_entity_id = opts.field_entity_id,
        cluster_key = opts.cluster_key or "core",
        member_key = opts.member_key or asteroid_id,
        parent_member_key = opts.parent_member_key,
        size_tier = opts.size_tier or "Medium",
        fracture_depth = opts.fracture_depth or 0,
        resource_profile_id = opts.resource_profile_id or "asteroid.resource.common_ore",
        fracture_profile_id = opts.fracture_profile_id or "asteroid.fracture.default",
      })
  end

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

local function asteroid_member_key(field_entity_id, cluster_key, index)
  return string.format("%s:%s:%04d", field_entity_id, cluster_key, index)
end

local function size_tier_for_diameter(diameter_m)
  if diameter_m >= 120.0 then
    return "Massive"
  elseif diameter_m >= 40.0 then
    return "Large"
  elseif diameter_m >= 12.0 then
    return "Medium"
  else
    return "Small"
  end
end

local function build_field_root(ctx, field_entity_id, center_x, center_y, count, min_radius, max_radius)
  local owner_id = ctx.owner_id or "npc:asteroid_field"
  local field_radius_m = math.max(max_radius or 2600.0, min_radius or 500.0)
  return new_entity(field_entity_id, { "Entity", "AsteroidField" }, nil, {
    component(field_entity_id, "display_name", ctx.display_name or "Starter Asteroid Field"),
    component(field_entity_id, "entity_labels", { "AsteroidField", "ResourceField", "V2" }),
    component(field_entity_id, "owner_id", owner_id),
    component(field_entity_id, "map_icon", { asset_id = ctx.map_icon_asset_id or "map_icon_planet_svg" }),
    component(field_entity_id, "world_position", { center_x, center_y }),
    component(field_entity_id, "world_rotation", 0.0),
    component(field_entity_id, "asteroid_field", {
      field_profile_id = ctx.field_profile_id or "asteroid.field.starter_belt",
      content_version = ctx.content_version or 2,
      layout_seed = ctx.layout_seed or 424242,
      activation_radius_m = ctx.activation_radius_m or (field_radius_m + 800.0),
      field_radius_m = field_radius_m,
      max_active_members = ctx.max_active_members or count,
      max_active_fragments = ctx.max_active_fragments or 96,
      max_fracture_depth = ctx.max_fracture_depth or 2,
      ambient_profile_id = ctx.ambient_profile_id or "asteroid.ambient.starter_dust",
    }),
    component(field_entity_id, "asteroid_field_layout", {
      shape = ctx.field_shape or "ClusterPatch",
      density = ctx.field_density or 0.55,
      clusters = {
        {
          cluster_key = "core",
          offset_xy_m = { 0.0, 0.0 },
          radius_m = field_radius_m,
          density_weight = 1.0,
          preferred_size_tier = "Medium",
          rarity_weight = 0.2,
        },
      },
      spawn_noise_amplitude_m = ctx.field_radial_jitter_m or 240.0,
      spawn_noise_frequency = 0.35,
    }),
    component(field_entity_id, "asteroid_field_population", {
      target_large_count = math.max(math.floor(count * 0.08), 1),
      target_medium_count = math.max(math.floor(count * 0.58), 1),
      target_small_count = math.max(count - math.floor(count * 0.66), 1),
      large_size_range_m = { min_m = 40.0, max_m = 120.0 },
      medium_size_range_m = { min_m = 12.0, max_m = 40.0 },
      small_size_range_m = { min_m = 3.0, max_m = 12.0 },
      sprite_profile_id = ctx.sprite_profile_id or "asteroid.sprite.top_down_rocky",
      resource_profile_id = ctx.resource_profile_id or "asteroid.resource.common_ore",
      fracture_profile_id = ctx.fracture_profile_id or "asteroid.fracture.default",
    }),
    component(field_entity_id, "asteroid_field_damage_state", { entries = {} }),
    component(field_entity_id, "asteroid_fracture_profile", {
      break_massive_into_large_min = 2,
      break_massive_into_large_max = 3,
      break_large_into_medium_min = 2,
      break_large_into_medium_max = 5,
      break_medium_into_small_min = 2,
      break_medium_into_small_max = 6,
      child_impulse_min_mps = 0.4,
      child_impulse_max_mps = 2.0,
      mass_retention_ratio = 0.82,
      terminal_debris_loss_ratio = 0.65,
    }),
    component(field_entity_id, "asteroid_resource_profile", {
      profile_id = ctx.resource_profile_id or "asteroid.resource.common_ore",
      extraction_profile_id = "extraction.mining_laser.basic",
      yield_table = {
        { item_id = "resource.iron_ore", weight = 1.0, min_units = 4.0, max_units = 18.0 },
        { item_id = "resource.nickel_ore", weight = 0.55, min_units = 2.0, max_units = 10.0 },
        { item_id = "resource.silicate_rock", weight = 0.85, min_units = 3.0, max_units = 16.0 },
        { item_id = "resource.rare_earth_oxides", weight = 0.12, min_units = 1.0, max_units = 4.0 },
      },
      depletion_pool_units = 100.0,
    }),
    component(field_entity_id, "asteroid_field_ambient", {
      trigger_radius_m = field_radius_m,
      fade_band_m = 600.0,
      background_shader_asset_id = nil,
      foreground_shader_asset_id = nil,
      post_process_shader_asset_id = nil,
      max_intensity = 0.65,
    }),
  })
end

function AsteroidFieldBundle.build_graph_records(ctx)
  local count = math.max(math.floor(ctx.field_count or 1), 1)
  local emit_field_root = ctx.bundle_id == "asteroid.field" or ctx.emit_field_root == true
  if count <= 1 and not emit_field_root then
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

  local field_entity_id = ctx.field_entity_id or "0012ebad-0000-0000-0000-000000000020"
  if emit_field_root then
    records[#records + 1] =
      build_field_root(ctx, field_entity_id, center_x, center_y, count, min_radius, max_radius)
  end

  for i = 1, count do
    local u = i / count
    local angle = i * 2.399963229728653 + (hash01(i, 1.0) - 0.5) * 0.42
    local radius = lerp(min_radius, max_radius, math.sqrt(u))
    radius = radius + (hash01(i, 2.0) - 0.5) * radial_jitter

    local x = center_x + math.cos(angle) * radius
    local y = center_y + math.sin(angle) * radius

    local size_roll = math.pow(hash01(i, 3.0), 0.48)
    local diameter_m = lerp(size_min, size_max, size_roll)
    local mass_kg = math.max(diameter_m * diameter_m * diameter_m * lerp(130.0, 260.0, hash01(i, 4.0)), 100.0)
    local health_points = math.max(diameter_m * lerp(12.0, 20.0, hash01(i, 5.0)), 30.0)
    local spin_rad_s = lerp(spin_min, spin_max, hash01(i, 6.0))
    local rotation_rad = hash01(i, 7.0) * math.pi * 2.0

    local asteroid_kind_roll = hash01(i, 8.0)
    local asteroid_kind = "Rocky"
    local surface_style = "Rocky"
    local palette_dark_rgb = { 0.18, 0.16, 0.14 }
    local palette_light_rgb = { 0.54, 0.48, 0.42 }
    local mineral_accent_rgb = { 0.72, 0.52, 0.24 }
    local mineral_vein_intensity = 0.16
    local crater_count = 6
    if asteroid_kind_roll > 0.94 then
      asteroid_kind = "Gem-rich"
      surface_style = "GemRich"
      palette_dark_rgb = { 0.15, 0.14, 0.18 }
      palette_light_rgb = { 0.46, 0.45, 0.56 }
      mineral_accent_rgb = { 0.25, 0.72, 0.94 }
      mineral_vein_intensity = 0.45
      crater_count = 5
    elseif asteroid_kind_roll > 0.74 then
      asteroid_kind = "Metallic"
      surface_style = "Metallic"
      palette_dark_rgb = { 0.16, 0.17, 0.19 }
      palette_light_rgb = { 0.58, 0.60, 0.63 }
      mineral_accent_rgb = { 0.90, 0.70, 0.32 }
      mineral_vein_intensity = 0.26
      crater_count = 4
    elseif asteroid_kind_roll > 0.54 then
      asteroid_kind = "Carbonaceous"
      surface_style = "Carbonaceous"
      palette_dark_rgb = { 0.08, 0.08, 0.09 }
      palette_light_rgb = { 0.30, 0.29, 0.27 }
      mineral_accent_rgb = { 0.50, 0.64, 0.54 }
      mineral_vein_intensity = 0.10
      crater_count = 7
    end
    local cluster_key = "core"
    local member_key = asteroid_member_key(field_entity_id, cluster_key, i)

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
      field_entity_id = emit_field_root and field_entity_id or nil,
      cluster_key = cluster_key,
      member_key = member_key,
      size_tier = size_tier_for_diameter(diameter_m),
      resource_profile_id = ctx.resource_profile_id or "asteroid.resource.common_ore",
      fracture_profile_id = ctx.fracture_profile_id or "asteroid.fracture.default",
      procedural_sprite = {
        generator_id = "asteroid_rocky_v1",
        resolution_px = 160,
        edge_noise = 0.018 + hash01(i, 9.0) * 0.022,
        lobe_amplitude = 0.11 + hash01(i, 10.0) * 0.06,
        crater_count = crater_count,
        palette_dark_rgb = palette_dark_rgb,
        palette_light_rgb = palette_light_rgb,
        surface_style = surface_style,
        pixel_step_px = 2,
        crack_intensity = 0.24 + hash01(i, 11.0) * 0.32,
        mineral_vein_intensity = mineral_vein_intensity,
        mineral_accent_rgb = mineral_accent_rgb,
        family_seed_key = member_key,
      },
    })
  end

  return records
end

return AsteroidFieldBundle

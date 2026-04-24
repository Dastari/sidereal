export type Vec2Tuple = [number, number]
export type Vec3Tuple = [number, number, number]

export type GenesisPlanetSpawn = {
  entity_id: string
  owner_id: string
  size_m: number
  spawn_position: Vec2Tuple
  spawn_rotation_rad: number
  map_icon_asset_id: string
  planet_visual_shader_asset_id: string
}

export type GenesisPlanetShaderSettings = {
  enabled: boolean
  enable_surface_detail: boolean
  enable_craters: boolean
  enable_clouds: boolean
  enable_atmosphere: boolean
  enable_specular: boolean
  enable_night_lights: boolean
  enable_emissive: boolean
  enable_ocean_specular: boolean
  body_kind: number
  planet_type: number
  seed: number
  base_radius_scale: number
  normal_strength: number
  detail_level: number
  rotation_speed: number
  light_wrap: number
  ambient_strength: number
  specular_strength: number
  specular_power: number
  rim_strength: number
  rim_power: number
  fresnel_strength: number
  cloud_shadow_strength: number
  night_glow_strength: number
  continent_size: number
  ocean_level: number
  mountain_height: number
  roughness: number
  terrain_octaves: number
  terrain_lacunarity: number
  terrain_gain: number
  crater_density: number
  crater_size: number
  volcano_density: number
  ice_cap_size: number
  storm_intensity: number
  bands_count: number
  spot_density: number
  surface_activity: number
  corona_intensity: number
  cloud_coverage: number
  cloud_scale: number
  cloud_speed: number
  cloud_alpha: number
  atmosphere_thickness: number
  atmosphere_falloff: number
  atmosphere_alpha: number
  city_lights: number
  emissive_strength: number
  sun_intensity: number
  surface_saturation: number
  surface_contrast: number
  light_color_mix: number
  sun_direction_xy: Vec2Tuple
  color_primary_rgb: Vec3Tuple
  color_secondary_rgb: Vec3Tuple
  color_tertiary_rgb: Vec3Tuple
  color_atmosphere_rgb: Vec3Tuple
  color_clouds_rgb: Vec3Tuple
  color_night_lights_rgb: Vec3Tuple
  color_emissive_rgb: Vec3Tuple
}

export type GenesisPlanetDefinition = {
  planet_id: string
  script_path: string
  display_name: string
  entity_labels: Array<string>
  tags: Array<string>
  spawn: GenesisPlanetSpawn
  shader_settings: GenesisPlanetShaderSettings
}

export type GenesisPlanetEntry = {
  planetId: string
  scriptPath: string
  displayName: string
  bodyKind: number | null
  planetType: number | null
  seed: number | null
  spawnEnabled: boolean
  tags: Array<string>
  hasDraft: boolean
  definition: GenesisPlanetDefinition
}

export type GenesisPlanetCatalog = {
  entries: Array<GenesisPlanetEntry>
  registryHasDraft: boolean
}

export type GenesisPlanetDraftRequest = {
  definition: GenesisPlanetDefinition
  spawnEnabled: boolean
}

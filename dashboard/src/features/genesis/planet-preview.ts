import type { ShaderPreviewUniformValues } from '@/lib/shader-preview'
import type { GenesisPlanetShaderSettings, Vec2Tuple, Vec3Tuple } from './types'

function flag(value: boolean): number {
  return value ? 1 : 0
}

function vec4(x: number, y: number, z: number, w: number): Array<number> {
  return [x, y, z, w]
}

function color(value: Vec3Tuple, alpha = 1): Array<number> {
  return [value[0], value[1], value[2], alpha]
}

function normalizedSunDirection(direction: Vec2Tuple): Array<number> {
  const x = direction[0]
  const y = direction[1]
  const z = 0.82
  const length = Math.hypot(x, y, z) || 1
  return [x / length, y / length, z / length]
}

export function planetShaderSeedUnit(seed: number): number {
  let x = Math.trunc(seed) >>> 0
  x ^= x >>> 16
  x = Math.imul(x, 0x7feb352d) >>> 0
  x ^= x >>> 15
  x = Math.imul(x, 0x846ca68b) >>> 0
  x ^= x >>> 16
  return (x >>> 0) / 0xffffffff
}

export function buildGenesisPlanetPreviewUniforms(
  settings: GenesisPlanetShaderSettings,
  timeSeconds: number,
  passFlags: Array<number> = [0, 0, 0, 0],
): ShaderPreviewUniformValues {
  const sunDir = normalizedSunDirection(settings.sun_direction_xy)
  return {
    'params.identity_a': vec4(
      settings.body_kind,
      settings.planet_type,
      planetShaderSeedUnit(settings.seed),
      timeSeconds,
    ),
    'params.identity_b': vec4(
      settings.rotation_speed,
      settings.surface_saturation,
      settings.surface_contrast,
      settings.light_color_mix,
    ),
    'params.feature_flags_a': vec4(
      flag(settings.enable_surface_detail),
      flag(settings.enable_craters),
      flag(settings.enable_clouds),
      flag(settings.enable_atmosphere),
    ),
    'params.feature_flags_b': vec4(
      flag(settings.enable_specular),
      flag(settings.enable_night_lights),
      flag(settings.enable_emissive),
      flag(settings.enable_ocean_specular),
    ),
    'params.pass_flags_a': vec4(
      passFlags[0] ?? 0,
      passFlags[1] ?? 0,
      passFlags[2] ?? 0,
      passFlags[3] ?? 0,
    ),
    'params.lighting_a': vec4(
      settings.base_radius_scale,
      settings.normal_strength,
      settings.detail_level,
      settings.light_wrap,
    ),
    'params.lighting_b': vec4(
      settings.ambient_strength,
      settings.specular_strength,
      settings.specular_power,
      settings.rim_strength,
    ),
    'params.surface_a': vec4(
      settings.rim_power,
      settings.fresnel_strength,
      settings.cloud_shadow_strength,
      settings.night_glow_strength,
    ),
    'params.surface_b': vec4(
      settings.continent_size,
      settings.ocean_level,
      settings.mountain_height,
      settings.roughness,
    ),
    'params.surface_c': vec4(
      settings.terrain_octaves,
      settings.terrain_lacunarity,
      settings.terrain_gain,
      settings.crater_density,
    ),
    'params.surface_d': vec4(
      settings.crater_size,
      settings.volcano_density,
      settings.ice_cap_size,
      settings.storm_intensity,
    ),
    'params.clouds_a': vec4(
      settings.bands_count,
      settings.spot_density,
      settings.surface_activity,
      settings.corona_intensity,
    ),
    'params.atmosphere_a': vec4(
      settings.cloud_coverage,
      settings.cloud_scale,
      settings.cloud_speed,
      settings.cloud_alpha,
    ),
    'params.emissive_a': vec4(
      settings.atmosphere_thickness,
      settings.atmosphere_falloff,
      settings.atmosphere_alpha,
      settings.city_lights,
    ),
    'params.sun_dir_a': vec4(
      settings.sun_direction_xy[0],
      settings.sun_direction_xy[1],
      0.82,
      settings.sun_intensity,
    ),
    'params.world_lighting.metadata': vec4(1, 1, 1, 0),
    'params.world_lighting.stellar_dir_intensity[0]': vec4(
      sunDir[0] ?? 0,
      sunDir[1] ?? 0,
      sunDir[2] ?? 1,
      settings.sun_intensity,
    ),
    'params.world_lighting.stellar_color_params[0]': vec4(
      1,
      0.94,
      0.82,
      sunDir[2] ?? 1,
    ),
    'params.world_lighting.ambient': vec4(0.16, 0.18, 0.22, 1),
    'params.world_lighting.backlight': vec4(0.28, 0.38, 0.62, 0.28),
    'params.world_lighting.flash': vec4(1, 1, 1, 0),
    'params.world_lighting.local_dir_intensity[0]': vec4(-0.35, 0.1, 0.93, 0),
    'params.world_lighting.local_color_radius[0]': vec4(0.45, 0.62, 1, 0),
    'params.color_primary': color(settings.color_primary_rgb),
    'params.color_secondary': color(settings.color_secondary_rgb),
    'params.color_tertiary': color(settings.color_tertiary_rgb),
    'params.color_atmosphere': color(settings.color_atmosphere_rgb),
    'params.color_clouds': color(
      settings.color_clouds_rgb,
      settings.cloud_alpha,
    ),
    'params.color_night_lights': color(settings.color_night_lights_rgb),
    'params.color_emissive': color(
      settings.color_emissive_rgb,
      settings.emissive_strength,
    ),
  }
}

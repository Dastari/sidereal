import { describe, expect, it } from 'vitest'
import {
  buildGenesisPlanetPreviewUniforms,
  planetShaderSeedUnit,
} from './planet-preview'
import type { GenesisPlanetShaderSettings } from './types'

const baseSettings: GenesisPlanetShaderSettings = {
  enabled: true,
  enable_surface_detail: true,
  enable_craters: false,
  enable_clouds: true,
  enable_atmosphere: true,
  enable_specular: true,
  enable_night_lights: false,
  enable_emissive: true,
  enable_ocean_specular: true,
  body_kind: 0,
  planet_type: 4,
  seed: 424242,
  base_radius_scale: 0.58,
  normal_strength: 0.88,
  detail_level: 0.68,
  rotation_speed: 0.0035,
  light_wrap: 0.22,
  ambient_strength: 0.2,
  specular_strength: 0.26,
  specular_power: 28,
  rim_strength: 0.34,
  rim_power: 3.1,
  fresnel_strength: 0.38,
  cloud_shadow_strength: 0.25,
  night_glow_strength: 0.06,
  continent_size: 0.68,
  ocean_level: 0.5,
  mountain_height: 0.34,
  roughness: 0.36,
  terrain_octaves: 6,
  terrain_lacunarity: 2.22,
  terrain_gain: 0.54,
  crater_density: 0.05,
  crater_size: 0.12,
  volcano_density: 0.03,
  ice_cap_size: 0.12,
  storm_intensity: 0.08,
  bands_count: 5,
  spot_density: 0.08,
  surface_activity: 0.1,
  corona_intensity: 0,
  cloud_coverage: 0.54,
  cloud_scale: 1.72,
  cloud_speed: 0.08,
  cloud_alpha: 0.7,
  atmosphere_thickness: 0.13,
  atmosphere_falloff: 2.4,
  atmosphere_alpha: 0.52,
  city_lights: 0.08,
  emissive_strength: 1.2,
  sun_intensity: 1,
  surface_saturation: 1.18,
  surface_contrast: 1.12,
  light_color_mix: 0.08,
  sun_direction_xy: [0.76, 0.58],
  color_primary_rgb: [0.2, 0.5, 0.24],
  color_secondary_rgb: [0.62, 0.56, 0.44],
  color_tertiary_rgb: [0.05, 0.21, 0.58],
  color_atmosphere_rgb: [0.42, 0.68, 1],
  color_clouds_rgb: [1, 1, 1],
  color_night_lights_rgb: [1, 0.82, 0.48],
  color_emissive_rgb: [1, 0.44, 0.19],
}

describe('buildGenesisPlanetPreviewUniforms', () => {
  it('maps Genesis shader settings onto the planet visual uniform layout', () => {
    const uniforms = buildGenesisPlanetPreviewUniforms(baseSettings, 12.5)

    expect(uniforms['params.identity_a']).toEqual([
      0,
      4,
      planetShaderSeedUnit(424242),
      12.5,
    ])
    expect(uniforms['params.feature_flags_a']).toEqual([1, 0, 1, 1])
    expect(uniforms['params.feature_flags_b']).toEqual([1, 0, 1, 1])
    expect(uniforms['params.lighting_a']).toEqual([0.58, 0.88, 0.68, 0.22])
    expect(uniforms['params.color_clouds']).toEqual([1, 1, 1, 0.7])
    expect(uniforms['params.color_emissive']).toEqual([1, 0.44, 0.19, 1.2])
  })

  it('keeps the preview primary light direction normalized', () => {
    const uniforms = buildGenesisPlanetPreviewUniforms(baseSettings, 0)
    const direction = uniforms['params.world_light_primary_dir_intensity'] ?? []
    const length = Math.hypot(
      direction[0] ?? 0,
      direction[1] ?? 0,
      direction[2] ?? 0,
    )

    expect(length).toBeCloseTo(1)
    expect(direction[3]).toBe(baseSettings.sun_intensity)
  })
})

import * as React from 'react'
import type { ComponentEditorProps } from './types'
import { Switch } from '@/components/ui/switch'
import { DebouncedNumberField } from './DebouncedNumberField'

type SpaceBackgroundShaderSettings = {
  enabled: boolean
  intensity: number
  drift_scale: number
  zoom_rate: number
  velocity_glow: number
  nebula_strength: number
  seed: number
  background_rgb: { x: number; y: number; z: number }
  nebula_color_primary_rgb: { x: number; y: number; z: number }
  nebula_color_secondary_rgb: { x: number; y: number; z: number }
  nebula_color_accent_rgb: { x: number; y: number; z: number }
  flare_enabled: boolean
  flare_tint_rgb: { x: number; y: number; z: number }
  flare_intensity: number
  flare_density: number
  flare_size: number
  flare_texture_set: number
  nebula_noise_mode: number
  nebula_octaves: number
  nebula_gain: number
  nebula_lacunarity: number
  nebula_power: number
  nebula_shelf: number
  nebula_ridge_offset: number
  star_mask_enabled: boolean
  star_mask_mode: number
  star_mask_octaves: number
  star_mask_gain: number
  star_mask_lacunarity: number
  star_mask_threshold: number
  star_mask_power: number
  star_mask_ridge_offset: number
  star_mask_scale: number
  nebula_blend_mode: number
  nebula_opacity: number
  stars_blend_mode: number
  stars_opacity: number
  star_count: number
  star_size_min: number
  star_size_max: number
  star_color_rgb: { x: number; y: number; z: number }
  flares_blend_mode: number
  flares_opacity: number
  tint_rgb: { x: number; y: number; z: number }
}

type SpaceBackgroundShaderSettingsPayload = {
  enabled: boolean
  intensity: number
  drift_scale: number
  zoom_rate: number
  velocity_glow: number
  nebula_strength: number
  seed: number
  background_rgb: [number, number, number]
  nebula_color_primary_rgb: [number, number, number]
  nebula_color_secondary_rgb: [number, number, number]
  nebula_color_accent_rgb: [number, number, number]
  flare_enabled: boolean
  flare_tint_rgb: [number, number, number]
  flare_intensity: number
  flare_density: number
  flare_size: number
  flare_texture_set: number
  nebula_noise_mode: number
  nebula_octaves: number
  nebula_gain: number
  nebula_lacunarity: number
  nebula_power: number
  nebula_shelf: number
  nebula_ridge_offset: number
  star_mask_enabled: boolean
  star_mask_mode: number
  star_mask_octaves: number
  star_mask_gain: number
  star_mask_lacunarity: number
  star_mask_threshold: number
  star_mask_power: number
  star_mask_ridge_offset: number
  star_mask_scale: number
  nebula_blend_mode: number
  nebula_opacity: number
  stars_blend_mode: number
  stars_opacity: number
  star_count: number
  star_size_min: number
  star_size_max: number
  star_color_rgb: [number, number, number]
  flares_blend_mode: number
  flares_opacity: number
  tint_rgb: [number, number, number]
}

const PRESET_OPTIONS = [
  { value: 'custom', label: 'Custom' },
  { value: 'calm', label: 'Calm' },
  { value: 'cinematic', label: 'Cinematic' },
  { value: 'dense-nebula', label: 'Dense Nebula' },
  { value: 'sparse-deep-space', label: 'Sparse Deep Space' },
] as const

type PresetKey = (typeof PRESET_OPTIONS)[number]['value']

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function roundToStep(value: number, step: number): number {
  return Math.round(value / step) * step
}

function parseVec3(value: unknown): { x: number; y: number; z: number } {
  if (Array.isArray(value)) {
    const x = Number(value[0] ?? 1)
    const y = Number(value[1] ?? 1)
    const z = Number(value[2] ?? 1)
    return {
      x: Number.isFinite(x) ? x : 1,
      y: Number.isFinite(y) ? y : 1,
      z: Number.isFinite(z) ? z : 1,
    }
  }
  if (value && typeof value === 'object') {
    const obj = value as Record<string, unknown>
    const x = Number(obj.x ?? 1)
    const y = Number(obj.y ?? 1)
    const z = Number(obj.z ?? 1)
    return {
      x: Number.isFinite(x) ? x : 1,
      y: Number.isFinite(y) ? y : 1,
      z: Number.isFinite(z) ? z : 1,
    }
  }
  return { x: 1, y: 1, z: 1 }
}

function parseSettings(value: unknown): SpaceBackgroundShaderSettings {
  if (!value || typeof value !== 'object') {
    return {
      enabled: true,
      intensity: 1,
      drift_scale: 1,
      zoom_rate: 1,
      velocity_glow: 1,
      nebula_strength: 1,
      seed: 73.421,
      background_rgb: { x: 0.004, y: 0.007, z: 0.018 },
      nebula_color_primary_rgb: { x: 0.07, y: 0.13, z: 0.28 },
      nebula_color_secondary_rgb: { x: 0.12, y: 0.24, z: 0.4 },
      nebula_color_accent_rgb: { x: 0.18, y: 0.16, z: 0.36 },
      flare_enabled: true,
      flare_tint_rgb: { x: 1, y: 1, z: 1 },
      flare_intensity: 0.18,
      flare_density: 0.22,
      flare_size: 0.85,
      flare_texture_set: 0,
      nebula_noise_mode: 0,
      nebula_octaves: 5,
      nebula_gain: 0.52,
      nebula_lacunarity: 2.0,
      nebula_power: 1.0,
      nebula_shelf: 0.42,
      nebula_ridge_offset: 1.0,
      star_mask_enabled: false,
      star_mask_mode: 0,
      star_mask_octaves: 4,
      star_mask_gain: 0.55,
      star_mask_lacunarity: 2.0,
      star_mask_threshold: 0.35,
      star_mask_power: 1.2,
      star_mask_ridge_offset: 1.0,
      star_mask_scale: 1.4,
      nebula_blend_mode: 1,
      nebula_opacity: 1.0,
      stars_blend_mode: 0,
      stars_opacity: 1.0,
      star_count: 1.0,
      star_size_min: 0.09,
      star_size_max: 0.118,
      star_color_rgb: { x: 1, y: 1, z: 1 },
      flares_blend_mode: 1,
      flares_opacity: 0.85,
      tint_rgb: { x: 1, y: 1, z: 1 },
    }
  }
  const obj = value as Record<string, unknown>
  const intensity = Number(obj.intensity ?? 1)
  const driftScale = Number(obj.drift_scale ?? 1)
  const zoomRate = Number(obj.zoom_rate ?? 1)
  const velocityGlow = Number(obj.velocity_glow ?? 1)
  const nebulaStrength = Number(obj.nebula_strength ?? 1)
  const seed = Number(obj.seed ?? 73.421)
  const backgroundRgb = parseVec3(obj.background_rgb)
  const nebulaColorPrimaryRgb = parseVec3(obj.nebula_color_primary_rgb)
  const nebulaColorSecondaryRgb = parseVec3(obj.nebula_color_secondary_rgb)
  const nebulaColorAccentRgb = parseVec3(obj.nebula_color_accent_rgb)
  const flareTintRgb = parseVec3(obj.flare_tint_rgb)
  const flareIntensity = Number(obj.flare_intensity ?? 0.18)
  const flareDensity = Number(obj.flare_density ?? 0.22)
  const flareSize = Number(obj.flare_size ?? 0.85)
  const flareTextureSet = Number(obj.flare_texture_set ?? 0)
  const nebulaNoiseMode = Number(obj.nebula_noise_mode ?? 0)
  const nebulaOctaves = Number(obj.nebula_octaves ?? 5)
  const nebulaGain = Number(obj.nebula_gain ?? 0.52)
  const nebulaLacunarity = Number(obj.nebula_lacunarity ?? 2.0)
  const nebulaPower = Number(obj.nebula_power ?? 1.0)
  const nebulaShelf = Number(obj.nebula_shelf ?? 0.42)
  const nebulaRidgeOffset = Number(obj.nebula_ridge_offset ?? 1.0)
  const starMaskMode = Number(obj.star_mask_mode ?? 0)
  const starMaskOctaves = Number(obj.star_mask_octaves ?? 4)
  const starMaskGain = Number(obj.star_mask_gain ?? 0.55)
  const starMaskLacunarity = Number(obj.star_mask_lacunarity ?? 2.0)
  const starMaskThreshold = Number(obj.star_mask_threshold ?? 0.35)
  const starMaskPower = Number(obj.star_mask_power ?? 1.2)
  const starMaskRidgeOffset = Number(obj.star_mask_ridge_offset ?? 1.0)
  const starMaskScale = Number(obj.star_mask_scale ?? 1.4)
  const nebulaBlendMode = Number(obj.nebula_blend_mode ?? 1)
  const nebulaOpacity = Number(obj.nebula_opacity ?? 1.0)
  const starsBlendMode = Number(obj.stars_blend_mode ?? 0)
  const starsOpacity = Number(obj.stars_opacity ?? 1.0)
  const starCount = Number(obj.star_count ?? 1.0)
  const starSizeMin = Number(obj.star_size_min ?? 0.09)
  const starSizeMax = Number(obj.star_size_max ?? 0.118)
  const starColorRgb = parseVec3(obj.star_color_rgb)
  const flaresBlendMode = Number(obj.flares_blend_mode ?? 1)
  const flaresOpacity = Number(obj.flares_opacity ?? 0.85)
  return {
    enabled: Boolean(obj.enabled ?? true),
    intensity: Number.isFinite(intensity) ? intensity : 1,
    drift_scale: Number.isFinite(driftScale) ? driftScale : 1,
    zoom_rate: Number.isFinite(zoomRate) ? zoomRate : 1,
    velocity_glow: Number.isFinite(velocityGlow) ? velocityGlow : 1,
    nebula_strength: Number.isFinite(nebulaStrength) ? nebulaStrength : 1,
    seed: Number.isFinite(seed) ? seed : 73.421,
    background_rgb: backgroundRgb,
    nebula_color_primary_rgb: nebulaColorPrimaryRgb,
    nebula_color_secondary_rgb: nebulaColorSecondaryRgb,
    nebula_color_accent_rgb: nebulaColorAccentRgb,
    flare_enabled: Boolean(obj.flare_enabled ?? true),
    flare_tint_rgb: flareTintRgb,
    flare_intensity: Number.isFinite(flareIntensity) ? flareIntensity : 0.18,
    flare_density: Number.isFinite(flareDensity) ? flareDensity : 0.22,
    flare_size: Number.isFinite(flareSize) ? flareSize : 0.85,
    flare_texture_set: Number.isFinite(flareTextureSet) ? flareTextureSet : 0,
    nebula_noise_mode: Number.isFinite(nebulaNoiseMode) ? nebulaNoiseMode : 0,
    nebula_octaves: Number.isFinite(nebulaOctaves) ? nebulaOctaves : 5,
    nebula_gain: Number.isFinite(nebulaGain) ? nebulaGain : 0.52,
    nebula_lacunarity: Number.isFinite(nebulaLacunarity) ? nebulaLacunarity : 2.0,
    nebula_power: Number.isFinite(nebulaPower) ? nebulaPower : 1.0,
    nebula_shelf: Number.isFinite(nebulaShelf) ? nebulaShelf : 0.42,
    nebula_ridge_offset: Number.isFinite(nebulaRidgeOffset)
      ? nebulaRidgeOffset
      : 1.0,
    star_mask_enabled: Boolean(obj.star_mask_enabled ?? false),
    star_mask_mode: Number.isFinite(starMaskMode) ? starMaskMode : 0,
    star_mask_octaves: Number.isFinite(starMaskOctaves) ? starMaskOctaves : 4,
    star_mask_gain: Number.isFinite(starMaskGain) ? starMaskGain : 0.55,
    star_mask_lacunarity: Number.isFinite(starMaskLacunarity)
      ? starMaskLacunarity
      : 2.0,
    star_mask_threshold: Number.isFinite(starMaskThreshold)
      ? starMaskThreshold
      : 0.35,
    star_mask_power: Number.isFinite(starMaskPower) ? starMaskPower : 1.2,
    star_mask_ridge_offset: Number.isFinite(starMaskRidgeOffset)
      ? starMaskRidgeOffset
      : 1.0,
    star_mask_scale: Number.isFinite(starMaskScale) ? starMaskScale : 1.4,
    nebula_blend_mode: Number.isFinite(nebulaBlendMode) ? nebulaBlendMode : 1,
    nebula_opacity: Number.isFinite(nebulaOpacity) ? nebulaOpacity : 1.0,
    stars_blend_mode: Number.isFinite(starsBlendMode) ? starsBlendMode : 0,
    stars_opacity: Number.isFinite(starsOpacity) ? starsOpacity : 1.0,
    star_count: Number.isFinite(starCount) ? starCount : 1.0,
    star_size_min: Number.isFinite(starSizeMin) ? starSizeMin : 0.09,
    star_size_max: Number.isFinite(starSizeMax) ? starSizeMax : 0.118,
    star_color_rgb: starColorRgb,
    flares_blend_mode: Number.isFinite(flaresBlendMode) ? flaresBlendMode : 1,
    flares_opacity: Number.isFinite(flaresOpacity) ? flaresOpacity : 0.85,
    tint_rgb: parseVec3(obj.tint_rgb),
  }
}

export function SpaceBackgroundShaderSettingsEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const parsed = parseSettings(value)
  const [selectedPreset, setSelectedPreset] = React.useState<PresetKey>('custom')

  const toPayload = React.useCallback(
    (next: SpaceBackgroundShaderSettings): SpaceBackgroundShaderSettingsPayload => ({
      enabled: next.enabled,
      intensity: clamp(roundToStep(next.intensity, 0.05), 0, 4),
      drift_scale: clamp(roundToStep(next.drift_scale, 0.05), 0, 4),
      zoom_rate: clamp(roundToStep(next.zoom_rate, 0.05), 0, 4),
      velocity_glow: clamp(roundToStep(next.velocity_glow, 0.05), 0, 4),
      nebula_strength: clamp(roundToStep(next.nebula_strength, 0.05), 0, 4),
      seed: clamp(roundToStep(next.seed, 0.001), 0, 100000),
      background_rgb: [
        clamp(roundToStep(next.background_rgb.x, 0.001), 0, 1),
        clamp(roundToStep(next.background_rgb.y, 0.001), 0, 1),
        clamp(roundToStep(next.background_rgb.z, 0.001), 0, 1),
      ],
      nebula_color_primary_rgb: [
        clamp(roundToStep(next.nebula_color_primary_rgb.x, 0.001), 0, 2),
        clamp(roundToStep(next.nebula_color_primary_rgb.y, 0.001), 0, 2),
        clamp(roundToStep(next.nebula_color_primary_rgb.z, 0.001), 0, 2),
      ],
      nebula_color_secondary_rgb: [
        clamp(roundToStep(next.nebula_color_secondary_rgb.x, 0.001), 0, 2),
        clamp(roundToStep(next.nebula_color_secondary_rgb.y, 0.001), 0, 2),
        clamp(roundToStep(next.nebula_color_secondary_rgb.z, 0.001), 0, 2),
      ],
      nebula_color_accent_rgb: [
        clamp(roundToStep(next.nebula_color_accent_rgb.x, 0.001), 0, 2),
        clamp(roundToStep(next.nebula_color_accent_rgb.y, 0.001), 0, 2),
        clamp(roundToStep(next.nebula_color_accent_rgb.z, 0.001), 0, 2),
      ],
      flare_enabled: next.flare_enabled,
      flare_tint_rgb: [
        clamp(roundToStep(next.flare_tint_rgb.x, 0.001), 0, 2),
        clamp(roundToStep(next.flare_tint_rgb.y, 0.001), 0, 2),
        clamp(roundToStep(next.flare_tint_rgb.z, 0.001), 0, 2),
      ],
      flare_intensity: clamp(roundToStep(next.flare_intensity, 0.01), 0, 4),
      flare_density: clamp(roundToStep(next.flare_density, 0.01), 0, 1),
      flare_size: clamp(roundToStep(next.flare_size, 0.01), 0.1, 4),
      flare_texture_set: clamp(Math.round(next.flare_texture_set), 0, 3),
      nebula_noise_mode: clamp(Math.round(next.nebula_noise_mode), 0, 1),
      nebula_octaves: clamp(Math.round(next.nebula_octaves), 1, 8),
      nebula_gain: clamp(roundToStep(next.nebula_gain, 0.01), 0.1, 0.95),
      nebula_lacunarity: clamp(roundToStep(next.nebula_lacunarity, 0.05), 1.1, 4),
      nebula_power: clamp(roundToStep(next.nebula_power, 0.05), 0.2, 4),
      nebula_shelf: clamp(roundToStep(next.nebula_shelf, 0.01), 0, 0.95),
      nebula_ridge_offset: clamp(roundToStep(next.nebula_ridge_offset, 0.01), 0.5, 2.5),
      star_mask_enabled: next.star_mask_enabled,
      star_mask_mode: clamp(Math.round(next.star_mask_mode), 0, 1),
      star_mask_octaves: clamp(Math.round(next.star_mask_octaves), 1, 8),
      star_mask_gain: clamp(roundToStep(next.star_mask_gain, 0.01), 0.1, 0.95),
      star_mask_lacunarity: clamp(roundToStep(next.star_mask_lacunarity, 0.05), 1.1, 4),
      star_mask_threshold: clamp(roundToStep(next.star_mask_threshold, 0.01), 0, 0.99),
      star_mask_power: clamp(roundToStep(next.star_mask_power, 0.05), 0.2, 4),
      star_mask_ridge_offset: clamp(
        roundToStep(next.star_mask_ridge_offset, 0.01),
        0.5,
        2.5,
      ),
      star_mask_scale: clamp(roundToStep(next.star_mask_scale, 0.05), 0.2, 8),
      nebula_blend_mode: clamp(Math.round(next.nebula_blend_mode), 0, 2),
      nebula_opacity: clamp(roundToStep(next.nebula_opacity, 0.01), 0, 1),
      stars_blend_mode: clamp(Math.round(next.stars_blend_mode), 0, 2),
      stars_opacity: clamp(roundToStep(next.stars_opacity, 0.01), 0, 1),
      star_count: clamp(roundToStep(next.star_count, 0.05), 0, 5),
      star_size_min: clamp(roundToStep(next.star_size_min, 0.001), 0.01, 0.35),
      star_size_max: clamp(roundToStep(next.star_size_max, 0.001), 0.01, 0.35),
      star_color_rgb: [
        clamp(roundToStep(next.star_color_rgb.x, 0.001), 0, 2),
        clamp(roundToStep(next.star_color_rgb.y, 0.001), 0, 2),
        clamp(roundToStep(next.star_color_rgb.z, 0.001), 0, 2),
      ],
      flares_blend_mode: clamp(Math.round(next.flares_blend_mode), 0, 2),
      flares_opacity: clamp(roundToStep(next.flares_opacity, 0.01), 0, 1),
      tint_rgb: [
        clamp(roundToStep(next.tint_rgb.x, 0.01), 0, 2),
        clamp(roundToStep(next.tint_rgb.y, 0.01), 0, 2),
        clamp(roundToStep(next.tint_rgb.z, 0.01), 0, 2),
      ],
    }),
    [],
  )

  const emit = React.useCallback(
    (next: SpaceBackgroundShaderSettings) => {
      onChange(toPayload(next))
    },
    [onChange, toPayload],
  )

  const copyCurrentAsJson = React.useCallback(async () => {
    const payload = toPayload(parsed)
    await navigator.clipboard.writeText(JSON.stringify(payload, null, 2))
  }, [parsed, toPayload])

  const updateField = <TKey extends keyof SpaceBackgroundShaderSettings>(
    key: TKey,
    next: SpaceBackgroundShaderSettings[TKey],
  ) => {
    emit({ ...parsed, [key]: next })
  }

  const updateTint = (axis: 'x' | 'y' | 'z', next: number) => {
    emit({
      ...parsed,
      tint_rgb: {
        ...parsed.tint_rgb,
        [axis]: next,
      },
    })
  }

  const updateBackground = (axis: 'x' | 'y' | 'z', next: number) => {
    emit({
      ...parsed,
      background_rgb: {
        ...parsed.background_rgb,
        [axis]: next,
      },
    })
  }

  const updateNebulaColor = (
    key:
      | 'nebula_color_primary_rgb'
      | 'nebula_color_secondary_rgb'
      | 'nebula_color_accent_rgb',
    axis: 'x' | 'y' | 'z',
    next: number,
  ) => {
    emit({
      ...parsed,
      [key]: {
        ...parsed[key],
        [axis]: next,
      },
    })
  }

  const updateFlareTint = (axis: 'x' | 'y' | 'z', next: number) => {
    emit({
      ...parsed,
      flare_tint_rgb: {
        ...parsed.flare_tint_rgb,
        [axis]: next,
      },
    })
  }

  const updateStarColor = (axis: 'x' | 'y' | 'z', next: number) => {
    emit({
      ...parsed,
      star_color_rgb: {
        ...parsed.star_color_rgb,
        [axis]: next,
      },
    })
  }

  const applyPreset = (preset: PresetKey) => {
    if (preset === 'custom') return
    const presets: Record<PresetKey, Partial<SpaceBackgroundShaderSettings>> = {
      custom: {},
      calm: {
        intensity: 0.9,
        nebula_strength: 0.7,
        flare_enabled: true,
        flare_intensity: 0.1,
        flare_density: 0.14,
        flare_size: 0.8,
        nebula_noise_mode: 0,
        nebula_octaves: 4,
        nebula_gain: 0.5,
        nebula_lacunarity: 1.9,
        nebula_power: 1.15,
        nebula_shelf: 0.46,
        nebula_blend_mode: 1,
        nebula_opacity: 0.8,
        stars_blend_mode: 0,
        stars_opacity: 0.9,
        flares_blend_mode: 1,
        flares_opacity: 0.55,
        star_mask_enabled: false,
      },
      cinematic: {
        intensity: 1.15,
        nebula_strength: 1.25,
        flare_enabled: true,
        flare_intensity: 0.26,
        flare_density: 0.26,
        flare_size: 1.05,
        nebula_noise_mode: 1,
        nebula_octaves: 6,
        nebula_gain: 0.54,
        nebula_lacunarity: 2.15,
        nebula_power: 0.95,
        nebula_shelf: 0.34,
        nebula_ridge_offset: 1.18,
        nebula_blend_mode: 1,
        nebula_opacity: 1,
        stars_blend_mode: 2,
        stars_opacity: 0.9,
        flares_blend_mode: 1,
        flares_opacity: 0.8,
        star_mask_enabled: true,
        star_mask_mode: 1,
        star_mask_octaves: 4,
        star_mask_gain: 0.55,
        star_mask_lacunarity: 2.0,
        star_mask_threshold: 0.4,
        star_mask_power: 1.2,
        star_mask_ridge_offset: 1.15,
        star_mask_scale: 1.6,
      },
      'dense-nebula': {
        intensity: 1.25,
        nebula_strength: 1.6,
        flare_enabled: true,
        flare_intensity: 0.12,
        flare_density: 0.12,
        flare_size: 0.9,
        nebula_noise_mode: 1,
        nebula_octaves: 7,
        nebula_gain: 0.56,
        nebula_lacunarity: 2.2,
        nebula_power: 0.82,
        nebula_shelf: 0.24,
        nebula_ridge_offset: 1.2,
        nebula_blend_mode: 1,
        nebula_opacity: 1,
        stars_blend_mode: 0,
        stars_opacity: 0.55,
        flares_blend_mode: 1,
        flares_opacity: 0.4,
        star_mask_enabled: true,
        star_mask_mode: 0,
        star_mask_octaves: 4,
        star_mask_gain: 0.5,
        star_mask_lacunarity: 2.0,
        star_mask_threshold: 0.48,
        star_mask_power: 1.35,
        star_mask_scale: 1.8,
      },
      'sparse-deep-space': {
        intensity: 0.75,
        nebula_strength: 0.45,
        background_rgb: { x: 0.002, y: 0.003, z: 0.01 },
        flare_enabled: true,
        flare_intensity: 0.06,
        flare_density: 0.08,
        flare_size: 0.75,
        nebula_noise_mode: 0,
        nebula_octaves: 3,
        nebula_gain: 0.5,
        nebula_lacunarity: 1.8,
        nebula_power: 1.45,
        nebula_shelf: 0.62,
        nebula_blend_mode: 0,
        nebula_opacity: 0.55,
        stars_blend_mode: 2,
        stars_opacity: 1.0,
        flares_blend_mode: 2,
        flares_opacity: 0.45,
        star_mask_enabled: true,
        star_mask_mode: 0,
        star_mask_octaves: 3,
        star_mask_gain: 0.52,
        star_mask_lacunarity: 1.9,
        star_mask_threshold: 0.6,
        star_mask_power: 1.5,
        star_mask_scale: 1.25,
      },
    }
    emit({ ...parsed, ...presets[preset] })
  }

  return (
    <div className="space-y-3">
      <SelectField
        label="Layer Preset"
        value={selectedPreset}
        options={PRESET_OPTIONS.map((option) => ({
          value: option.value,
          label: option.label,
        }))}
        readOnly={readOnly}
        onChange={(next) => {
          const preset = next as PresetKey
          setSelectedPreset(preset)
          applyPreset(preset)
        }}
      />
      <button
        type="button"
        disabled={readOnly}
        onClick={() => {
          void copyCurrentAsJson()
        }}
        className="w-full rounded-md border border-border/60 px-2 py-1.5 text-xs text-muted-foreground hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
      >
        Copy As JSON (for Rust default constant)
      </button>
      <ToggleField
        label="Enabled"
        checked={parsed.enabled}
        readOnly={readOnly}
        onChange={(next) => updateField('enabled', next)}
      />
      <Field
        label="Intensity"
        value={parsed.intensity}
        min={0}
        max={4}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('intensity', next)}
      />
      <Field
        label="Drift Scale"
        value={parsed.drift_scale}
        min={0}
        max={4}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('drift_scale', next)}
      />
      <Field
        label="Zoom Rate"
        value={parsed.zoom_rate}
        min={0}
        max={4}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('zoom_rate', next)}
      />
      <Field
        label="Velocity Glow"
        value={parsed.velocity_glow}
        min={0}
        max={4}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('velocity_glow', next)}
      />
      <Field
        label="Nebula Strength"
        value={parsed.nebula_strength}
        min={0}
        max={4}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('nebula_strength', next)}
      />
      <Field
        label="Seed"
        value={parsed.seed}
        min={0}
        max={100000}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateField('seed', next)}
      />
      <Field
        label="Background R"
        value={parsed.background_rgb.x}
        min={0}
        max={1}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateBackground('x', next)}
      />
      <Field
        label="Background G"
        value={parsed.background_rgb.y}
        min={0}
        max={1}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateBackground('y', next)}
      />
      <Field
        label="Background B"
        value={parsed.background_rgb.z}
        min={0}
        max={1}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateBackground('z', next)}
      />
      <Field
        label="Nebula Primary R"
        value={parsed.nebula_color_primary_rgb.x}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateNebulaColor('nebula_color_primary_rgb', 'x', next)}
      />
      <Field
        label="Nebula Primary G"
        value={parsed.nebula_color_primary_rgb.y}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateNebulaColor('nebula_color_primary_rgb', 'y', next)}
      />
      <Field
        label="Nebula Primary B"
        value={parsed.nebula_color_primary_rgb.z}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateNebulaColor('nebula_color_primary_rgb', 'z', next)}
      />
      <Field
        label="Nebula Secondary R"
        value={parsed.nebula_color_secondary_rgb.x}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateNebulaColor('nebula_color_secondary_rgb', 'x', next)}
      />
      <Field
        label="Nebula Secondary G"
        value={parsed.nebula_color_secondary_rgb.y}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateNebulaColor('nebula_color_secondary_rgb', 'y', next)}
      />
      <Field
        label="Nebula Secondary B"
        value={parsed.nebula_color_secondary_rgb.z}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateNebulaColor('nebula_color_secondary_rgb', 'z', next)}
      />
      <Field
        label="Nebula Accent R"
        value={parsed.nebula_color_accent_rgb.x}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateNebulaColor('nebula_color_accent_rgb', 'x', next)}
      />
      <Field
        label="Nebula Accent G"
        value={parsed.nebula_color_accent_rgb.y}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateNebulaColor('nebula_color_accent_rgb', 'y', next)}
      />
      <Field
        label="Nebula Accent B"
        value={parsed.nebula_color_accent_rgb.z}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateNebulaColor('nebula_color_accent_rgb', 'z', next)}
      />
      <ToggleField
        label="Flare Layer Enabled"
        checked={parsed.flare_enabled}
        readOnly={readOnly}
        onChange={(next) => updateField('flare_enabled', next)}
      />
      <Field
        label="Flare Intensity"
        value={parsed.flare_intensity}
        min={0}
        max={4}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('flare_intensity', next)}
      />
      <Field
        label="Flare Density"
        value={parsed.flare_density}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('flare_density', next)}
      />
      <Field
        label="Flare Size"
        value={parsed.flare_size}
        min={0.1}
        max={4}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('flare_size', next)}
      />
      <Field
        label="Flare Tint R"
        value={parsed.flare_tint_rgb.x}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateFlareTint('x', next)}
      />
      <Field
        label="Flare Tint G"
        value={parsed.flare_tint_rgb.y}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateFlareTint('y', next)}
      />
      <Field
        label="Flare Tint B"
        value={parsed.flare_tint_rgb.z}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateFlareTint('z', next)}
      />
      <SelectField
        label="Flare Image"
        value={String(parsed.flare_texture_set)}
        options={[
          { value: '0', label: 'White' },
          { value: '1', label: 'Blue / Purple' },
          { value: '2', label: 'Red / Yellow' },
          { value: '3', label: 'Sun' },
        ]}
        readOnly={readOnly}
        onChange={(next) =>
          updateField('flare_texture_set', Number.parseInt(next, 10) || 0)
        }
      />
      <SelectField
        label="Nebula Noise Mode"
        value={String(parsed.nebula_noise_mode)}
        options={[
          { value: '0', label: 'fBm' },
          { value: '1', label: 'Ridged fBm' },
        ]}
        readOnly={readOnly}
        onChange={(next) =>
          updateField('nebula_noise_mode', Number.parseInt(next, 10) || 0)
        }
      />
      <Field
        label="Nebula Octaves"
        value={parsed.nebula_octaves}
        min={1}
        max={8}
        step={1}
        readOnly={readOnly}
        onChange={(next) => updateField('nebula_octaves', next)}
      />
      <Field
        label="Nebula Gain"
        value={parsed.nebula_gain}
        min={0.1}
        max={0.95}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('nebula_gain', next)}
      />
      <Field
        label="Nebula Lacunarity"
        value={parsed.nebula_lacunarity}
        min={1.1}
        max={4}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('nebula_lacunarity', next)}
      />
      <Field
        label="Nebula Power"
        value={parsed.nebula_power}
        min={0.2}
        max={4}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('nebula_power', next)}
      />
      <Field
        label="Nebula Shelf"
        value={parsed.nebula_shelf}
        min={0}
        max={0.95}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('nebula_shelf', next)}
      />
      <Field
        label="Nebula Ridge Offset"
        value={parsed.nebula_ridge_offset}
        min={0.5}
        max={2.5}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('nebula_ridge_offset', next)}
      />
      <ToggleField
        label="Star/Flare Noise Mask Enabled"
        checked={parsed.star_mask_enabled}
        readOnly={readOnly}
        onChange={(next) => updateField('star_mask_enabled', next)}
      />
      <SelectField
        label="Star/Flare Mask Mode"
        value={String(parsed.star_mask_mode)}
        options={[
          { value: '0', label: 'fBm' },
          { value: '1', label: 'Ridged fBm' },
        ]}
        readOnly={readOnly}
        onChange={(next) => updateField('star_mask_mode', Number.parseInt(next, 10) || 0)}
      />
      <Field
        label="Star/Flare Mask Octaves"
        value={parsed.star_mask_octaves}
        min={1}
        max={8}
        step={1}
        readOnly={readOnly}
        onChange={(next) => updateField('star_mask_octaves', next)}
      />
      <Field
        label="Star/Flare Mask Gain"
        value={parsed.star_mask_gain}
        min={0.1}
        max={0.95}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('star_mask_gain', next)}
      />
      <Field
        label="Star/Flare Mask Lacunarity"
        value={parsed.star_mask_lacunarity}
        min={1.1}
        max={4}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('star_mask_lacunarity', next)}
      />
      <Field
        label="Star/Flare Mask Threshold"
        value={parsed.star_mask_threshold}
        min={0}
        max={0.99}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('star_mask_threshold', next)}
      />
      <Field
        label="Star/Flare Mask Power"
        value={parsed.star_mask_power}
        min={0.2}
        max={4}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('star_mask_power', next)}
      />
      <Field
        label="Star/Flare Mask Ridge Offset"
        value={parsed.star_mask_ridge_offset}
        min={0.5}
        max={2.5}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('star_mask_ridge_offset', next)}
      />
      <Field
        label="Star/Flare Mask Scale"
        value={parsed.star_mask_scale}
        min={0.2}
        max={8}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('star_mask_scale', next)}
      />
      <SelectField
        label="Nebula Blend Mode"
        value={String(parsed.nebula_blend_mode)}
        options={[
          { value: '0', label: 'Add' },
          { value: '1', label: 'Screen' },
          { value: '2', label: 'Lighten' },
        ]}
        readOnly={readOnly}
        onChange={(next) => updateField('nebula_blend_mode', Number.parseInt(next, 10) || 0)}
      />
      <Field
        label="Nebula Opacity"
        value={parsed.nebula_opacity}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('nebula_opacity', next)}
      />
      <SelectField
        label="Stars Blend Mode"
        value={String(parsed.stars_blend_mode)}
        options={[
          { value: '0', label: 'Add' },
          { value: '1', label: 'Screen' },
          { value: '2', label: 'Lighten' },
        ]}
        readOnly={readOnly}
        onChange={(next) => updateField('stars_blend_mode', Number.parseInt(next, 10) || 0)}
      />
      <Field
        label="Stars Opacity"
        value={parsed.stars_opacity}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('stars_opacity', next)}
      />
      <Field
        label="Star Count"
        value={parsed.star_count}
        min={0}
        max={5}
        step={0.05}
        readOnly={readOnly}
        onChange={(next) => updateField('star_count', next)}
      />
      <Field
        label="Star Size Min"
        value={parsed.star_size_min}
        min={0.01}
        max={0.35}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateField('star_size_min', next)}
      />
      <Field
        label="Star Size Max"
        value={parsed.star_size_max}
        min={0.01}
        max={0.35}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateField('star_size_max', next)}
      />
      <Field
        label="Star Color R"
        value={parsed.star_color_rgb.x}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateStarColor('x', next)}
      />
      <Field
        label="Star Color G"
        value={parsed.star_color_rgb.y}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateStarColor('y', next)}
      />
      <Field
        label="Star Color B"
        value={parsed.star_color_rgb.z}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateStarColor('z', next)}
      />
      <SelectField
        label="Flares Blend Mode"
        value={String(parsed.flares_blend_mode)}
        options={[
          { value: '0', label: 'Add' },
          { value: '1', label: 'Screen' },
          { value: '2', label: 'Lighten' },
        ]}
        readOnly={readOnly}
        onChange={(next) => updateField('flares_blend_mode', Number.parseInt(next, 10) || 0)}
      />
      <Field
        label="Flares Opacity"
        value={parsed.flares_opacity}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('flares_opacity', next)}
      />
      <Field
        label="Tint R"
        value={parsed.tint_rgb.x}
        min={0}
        max={2}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateTint('x', next)}
      />
      <Field
        label="Tint G"
        value={parsed.tint_rgb.y}
        min={0}
        max={2}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateTint('y', next)}
      />
      <Field
        label="Tint B"
        value={parsed.tint_rgb.z}
        min={0}
        max={2}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateTint('z', next)}
      />
    </div>
  )
}

function ToggleField({
  label,
  checked,
  readOnly,
  onChange,
}: {
  label: string
  checked: boolean
  readOnly: boolean
  onChange: (next: boolean) => void
}) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-md border border-border/60 px-2 py-1.5">
      <div className="text-xs text-muted-foreground">{label}</div>
      <Switch
        checked={checked}
        onCheckedChange={onChange}
        disabled={readOnly}
        aria-label={`${label} toggle`}
      />
    </div>
  )
}

function Field({
  label,
  value,
  min,
  max,
  step,
  readOnly,
  onChange,
}: {
  label: string
  value: number
  min: number
  max: number
  step: number
  readOnly: boolean
  onChange: (next: number) => void
}) {
  return (
    <DebouncedNumberField
      label={label}
      value={value}
      min={min}
      max={max}
      step={step}
      readOnly={readOnly}
      onChange={onChange}
      inputClassName="w-20 text-right font-mono text-xs"
    />
  )
}

function SelectField({
  label,
  value,
  options,
  readOnly,
  onChange,
}: {
  label: string
  value: string
  options: Array<{ value: string; label: string }>
  readOnly: boolean
  onChange: (next: string) => void
}) {
  return (
    <div className="space-y-1">
      <div className="text-xs text-muted-foreground">{label}</div>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={readOnly}
        className="flex h-8 w-full rounded-md border border-border/60 bg-background px-2 text-xs"
        aria-label={`${label} value`}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  )
}

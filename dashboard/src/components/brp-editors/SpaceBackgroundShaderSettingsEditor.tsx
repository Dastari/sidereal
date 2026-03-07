import * as React from 'react'
import { DebouncedNumberField } from './DebouncedNumberField'
import type { ComponentEditorProps } from './types'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { Input } from '@/components/ui/input'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'

type SpaceBackgroundShaderSettings = {
  enabled: boolean
  enable_nebula_layer: boolean
  enable_stars_layer: boolean
  enable_flares_layer: boolean
  enable_background_gradient: boolean
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
  depth_layer_separation: number
  depth_parallax_scale: number
  depth_haze_strength: number
  depth_occlusion_strength: number
  backlight_screen_x: number
  backlight_screen_y: number
  backlight_intensity: number
  backlight_wrap: number
  backlight_edge_boost: number
  backlight_bloom_scale: number
  backlight_bloom_threshold: number
  enable_backlight: boolean
  enable_light_shafts: boolean
  shafts_debug_view: boolean
  shaft_intensity: number
  shaft_length: number
  shaft_falloff: number
  shaft_samples: number
  shaft_quality: number
  shaft_blend_mode: number
  shaft_opacity: number
  shaft_color_rgb: { x: number; y: number; z: number }
  backlight_color_rgb: { x: number; y: number; z: number }
  tint_rgb: { x: number; y: number; z: number }
}

type SpaceBackgroundShaderSettingsPayload = {
  enabled: boolean
  enable_nebula_layer: boolean
  enable_stars_layer: boolean
  enable_flares_layer: boolean
  enable_background_gradient: boolean
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
  depth_layer_separation: number
  depth_parallax_scale: number
  depth_haze_strength: number
  depth_occlusion_strength: number
  backlight_screen_x: number
  backlight_screen_y: number
  backlight_intensity: number
  backlight_wrap: number
  backlight_edge_boost: number
  backlight_bloom_scale: number
  backlight_bloom_threshold: number
  enable_backlight: boolean
  enable_light_shafts: boolean
  shafts_debug_view: boolean
  shaft_intensity: number
  shaft_length: number
  shaft_falloff: number
  shaft_samples: number
  shaft_quality: number
  shaft_blend_mode: number
  shaft_opacity: number
  shaft_color_rgb: [number, number, number]
  backlight_color_rgb: [number, number, number]
  tint_rgb: [number, number, number]
}

const PRESET_OPTIONS = [
  { value: 'custom', label: 'Custom' },
  { value: 'calm', label: 'Calm' },
  { value: 'cinematic', label: 'Cinematic' },
  { value: 'dense-nebula', label: 'Dense Nebula' },
  { value: 'sparse-deep-space', label: 'Sparse Deep Space' },
] as const

const BLEND_MODE_OPTIONS = [
  { value: '0', label: 'Linear Dodge (Add)' },
  { value: '1', label: 'Screen' },
  { value: '2', label: 'Lighten' },
  { value: '3', label: 'Normal' },
  { value: '4', label: 'Dissolve' },
  { value: '5', label: 'Darken' },
  { value: '6', label: 'Multiply' },
  { value: '7', label: 'Color Burn' },
  { value: '8', label: 'Linear Burn' },
  { value: '9', label: 'Darker Color' },
  { value: '10', label: 'Color Dodge' },
  { value: '11', label: 'Lighter Color' },
  { value: '12', label: 'Overlay' },
  { value: '13', label: 'Soft Light' },
  { value: '14', label: 'Hard Light' },
  { value: '15', label: 'Vivid Light' },
  { value: '16', label: 'Linear Light' },
  { value: '17', label: 'Pin Light' },
  { value: '18', label: 'Hard Mix' },
  { value: '19', label: 'Difference' },
  { value: '20', label: 'Exclusion' },
  { value: '21', label: 'Subtract' },
  { value: '22', label: 'Divide' },
  { value: '23', label: 'Hue' },
  { value: '24', label: 'Saturation' },
  { value: '25', label: 'Color' },
  { value: '26', label: 'Luminosity' },
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
      enable_nebula_layer: true,
      enable_stars_layer: true,
      enable_flares_layer: true,
      enable_background_gradient: false,
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
      depth_layer_separation: 1.03,
      depth_parallax_scale: 0.83,
      depth_haze_strength: 1.69,
      depth_occlusion_strength: 1.08,
      backlight_screen_x: -0.3,
      backlight_screen_y: 0.1,
      backlight_intensity: 4,
      backlight_wrap: 0.49,
      backlight_edge_boost: 2.2,
      backlight_bloom_scale: 1.35,
      backlight_bloom_threshold: 0.14,
      enable_backlight: true,
      enable_light_shafts: true,
      shafts_debug_view: false,
      shaft_intensity: 1.76,
      shaft_length: 0.47,
      shaft_falloff: 2.65,
      shaft_samples: 16,
      shaft_quality: 1,
      shaft_blend_mode: 1,
      shaft_opacity: 0.85,
      shaft_color_rgb: { x: 1.15, y: 1.0, z: 1.45 },
      backlight_color_rgb: { x: 1.15, y: 1.0, z: 1.45 },
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
  const depthLayerSeparation = Number(obj.depth_layer_separation ?? 1.03)
  const depthParallaxScale = Number(obj.depth_parallax_scale ?? 0.83)
  const depthHazeStrength = Number(obj.depth_haze_strength ?? 1.69)
  const depthOcclusionStrength = Number(obj.depth_occlusion_strength ?? 1.08)
  const backlightScreenX = Number(obj.backlight_screen_x ?? -0.3)
  const backlightScreenY = Number(obj.backlight_screen_y ?? 0.1)
  const backlightIntensity = Number(obj.backlight_intensity ?? 4)
  const backlightWrap = Number(obj.backlight_wrap ?? 0.49)
  const backlightEdgeBoost = Number(obj.backlight_edge_boost ?? 2.2)
  const backlightBloomScale = Number(obj.backlight_bloom_scale ?? 1.35)
  const backlightBloomThreshold = Number(obj.backlight_bloom_threshold ?? 0.14)
  const shaftIntensity = Number(obj.shaft_intensity ?? 1.76)
  const shaftLength = Number(obj.shaft_length ?? 0.47)
  const shaftFalloff = Number(obj.shaft_falloff ?? 2.65)
  const shaftSamples = Number(obj.shaft_samples ?? 16)
  const shaftQuality = Number(obj.shaft_quality ?? 1)
  const shaftBlendMode = Number(obj.shaft_blend_mode ?? 1)
  const shaftOpacity = Number(obj.shaft_opacity ?? 0.85)
  const shaftColorRgb = parseVec3(obj.shaft_color_rgb)
  const backlightColorRgb = parseVec3(obj.backlight_color_rgb)
  return {
    enabled: Boolean(obj.enabled ?? true),
    enable_nebula_layer: Boolean(obj.enable_nebula_layer ?? true),
    enable_stars_layer: Boolean(obj.enable_stars_layer ?? true),
    enable_flares_layer: Boolean(obj.enable_flares_layer ?? true),
    enable_background_gradient: Boolean(
      obj.enable_background_gradient ?? false,
    ),
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
    nebula_lacunarity: Number.isFinite(nebulaLacunarity)
      ? nebulaLacunarity
      : 2.0,
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
    depth_layer_separation: Number.isFinite(depthLayerSeparation)
      ? depthLayerSeparation
      : 1.03,
    depth_parallax_scale: Number.isFinite(depthParallaxScale)
      ? depthParallaxScale
      : 0.83,
    depth_haze_strength: Number.isFinite(depthHazeStrength)
      ? depthHazeStrength
      : 1.69,
    depth_occlusion_strength: Number.isFinite(depthOcclusionStrength)
      ? depthOcclusionStrength
      : 1.08,
    backlight_screen_x: Number.isFinite(backlightScreenX)
      ? backlightScreenX
      : -0.3,
    backlight_screen_y: Number.isFinite(backlightScreenY)
      ? backlightScreenY
      : 0.1,
    backlight_intensity: Number.isFinite(backlightIntensity)
      ? backlightIntensity
      : 4,
    backlight_wrap: Number.isFinite(backlightWrap) ? backlightWrap : 0.49,
    backlight_edge_boost: Number.isFinite(backlightEdgeBoost)
      ? backlightEdgeBoost
      : 2.2,
    backlight_bloom_scale: Number.isFinite(backlightBloomScale)
      ? backlightBloomScale
      : 1.35,
    backlight_bloom_threshold: Number.isFinite(backlightBloomThreshold)
      ? backlightBloomThreshold
      : 0.14,
    enable_backlight: Boolean(obj.enable_backlight ?? true),
    enable_light_shafts: Boolean(obj.enable_light_shafts ?? true),
    shafts_debug_view: Boolean(obj.shafts_debug_view ?? false),
    shaft_intensity: Number.isFinite(shaftIntensity) ? shaftIntensity : 1.76,
    shaft_length: Number.isFinite(shaftLength) ? shaftLength : 0.47,
    shaft_falloff: Number.isFinite(shaftFalloff) ? shaftFalloff : 2.65,
    shaft_samples: Number.isFinite(shaftSamples) ? shaftSamples : 16,
    shaft_quality: Number.isFinite(shaftQuality) ? shaftQuality : 1,
    shaft_blend_mode: Number.isFinite(shaftBlendMode) ? shaftBlendMode : 1,
    shaft_opacity: Number.isFinite(shaftOpacity) ? shaftOpacity : 0.85,
    shaft_color_rgb: shaftColorRgb,
    backlight_color_rgb: backlightColorRgb,
    tint_rgb: parseVec3(obj.tint_rgb),
  }
}

export function SpaceBackgroundShaderSettingsEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const parsed = parseSettings(value)
  const [selectedPreset, setSelectedPreset] =
    React.useState<PresetKey>('custom')

  const toPayload = React.useCallback(
    (
      next: SpaceBackgroundShaderSettings,
    ): SpaceBackgroundShaderSettingsPayload => ({
      enabled: next.enabled,
      enable_nebula_layer: next.enable_nebula_layer,
      enable_stars_layer: next.enable_stars_layer,
      enable_flares_layer: next.enable_flares_layer,
      enable_background_gradient: next.enable_background_gradient,
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
      nebula_lacunarity: clamp(
        roundToStep(next.nebula_lacunarity, 0.05),
        1.1,
        4,
      ),
      nebula_power: clamp(roundToStep(next.nebula_power, 0.05), 0.2, 4),
      nebula_shelf: clamp(roundToStep(next.nebula_shelf, 0.01), 0, 0.95),
      nebula_ridge_offset: clamp(
        roundToStep(next.nebula_ridge_offset, 0.01),
        0.5,
        2.5,
      ),
      star_mask_enabled: next.star_mask_enabled,
      star_mask_mode: clamp(Math.round(next.star_mask_mode), 0, 1),
      star_mask_octaves: clamp(Math.round(next.star_mask_octaves), 1, 8),
      star_mask_gain: clamp(roundToStep(next.star_mask_gain, 0.01), 0.1, 0.95),
      star_mask_lacunarity: clamp(
        roundToStep(next.star_mask_lacunarity, 0.05),
        1.1,
        4,
      ),
      star_mask_threshold: clamp(
        roundToStep(next.star_mask_threshold, 0.01),
        0,
        0.99,
      ),
      star_mask_power: clamp(roundToStep(next.star_mask_power, 0.05), 0.2, 4),
      star_mask_ridge_offset: clamp(
        roundToStep(next.star_mask_ridge_offset, 0.01),
        0.5,
        2.5,
      ),
      star_mask_scale: clamp(roundToStep(next.star_mask_scale, 0.05), 0.2, 8),
      nebula_blend_mode: clamp(Math.round(next.nebula_blend_mode), 0, 26),
      nebula_opacity: clamp(roundToStep(next.nebula_opacity, 0.01), 0, 1),
      stars_blend_mode: clamp(Math.round(next.stars_blend_mode), 0, 26),
      stars_opacity: clamp(roundToStep(next.stars_opacity, 0.01), 0, 1),
      star_count: clamp(roundToStep(next.star_count, 0.05), 0, 5),
      star_size_min: clamp(roundToStep(next.star_size_min, 0.001), 0.01, 0.35),
      star_size_max: clamp(roundToStep(next.star_size_max, 0.001), 0.01, 0.35),
      star_color_rgb: [
        clamp(roundToStep(next.star_color_rgb.x, 0.001), 0, 2),
        clamp(roundToStep(next.star_color_rgb.y, 0.001), 0, 2),
        clamp(roundToStep(next.star_color_rgb.z, 0.001), 0, 2),
      ],
      flares_blend_mode: clamp(Math.round(next.flares_blend_mode), 0, 26),
      flares_opacity: clamp(roundToStep(next.flares_opacity, 0.01), 0, 1),
      depth_layer_separation: clamp(
        roundToStep(next.depth_layer_separation, 0.01),
        0,
        2,
      ),
      depth_parallax_scale: clamp(
        roundToStep(next.depth_parallax_scale, 0.01),
        0,
        2,
      ),
      depth_haze_strength: clamp(
        roundToStep(next.depth_haze_strength, 0.01),
        0,
        2,
      ),
      depth_occlusion_strength: clamp(
        roundToStep(next.depth_occlusion_strength, 0.01),
        0,
        3,
      ),
      backlight_screen_x: clamp(
        roundToStep(next.backlight_screen_x, 0.01),
        -1.5,
        1.5,
      ),
      backlight_screen_y: clamp(
        roundToStep(next.backlight_screen_y, 0.01),
        -1.5,
        1.5,
      ),
      backlight_intensity: clamp(
        roundToStep(next.backlight_intensity, 0.01),
        0,
        20,
      ),
      backlight_wrap: clamp(roundToStep(next.backlight_wrap, 0.01), 0, 2),
      backlight_edge_boost: clamp(
        roundToStep(next.backlight_edge_boost, 0.01),
        0,
        6,
      ),
      backlight_bloom_scale: clamp(
        roundToStep(next.backlight_bloom_scale, 0.01),
        0,
        2,
      ),
      backlight_bloom_threshold: clamp(
        roundToStep(next.backlight_bloom_threshold, 0.01),
        0,
        1,
      ),
      enable_backlight: next.enable_backlight,
      enable_light_shafts: next.enable_light_shafts,
      shafts_debug_view: next.shafts_debug_view,
      shaft_intensity: clamp(roundToStep(next.shaft_intensity, 0.01), 0, 40),
      shaft_length: clamp(roundToStep(next.shaft_length, 0.01), 0.05, 0.95),
      shaft_falloff: clamp(roundToStep(next.shaft_falloff, 0.01), 0.2, 8),
      shaft_samples: clamp(Math.round(next.shaft_samples), 4, 24),
      shaft_quality: clamp(Math.round(next.shaft_quality), 0, 2),
      shaft_blend_mode: clamp(Math.round(next.shaft_blend_mode), 0, 26),
      shaft_opacity: clamp(roundToStep(next.shaft_opacity, 0.01), 0, 1),
      shaft_color_rgb: [
        clamp(roundToStep(next.shaft_color_rgb.x, 0.001), 0, 3),
        clamp(roundToStep(next.shaft_color_rgb.y, 0.001), 0, 3),
        clamp(roundToStep(next.shaft_color_rgb.z, 0.001), 0, 3),
      ],
      backlight_color_rgb: [
        clamp(roundToStep(next.backlight_color_rgb.x, 0.001), 0, 3),
        clamp(roundToStep(next.backlight_color_rgb.y, 0.001), 0, 3),
        clamp(roundToStep(next.backlight_color_rgb.z, 0.001), 0, 3),
      ],
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
        className="w-full rounded-md border border-border/60 px-2 py-2 text-xs text-muted-foreground hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
      >
        Copy As JSON (for Rust default constant)
      </button>
      <ToggleField
        label="Enabled"
        checked={parsed.enabled}
        readOnly={readOnly}
        onChange={(next) => updateField('enabled', next)}
      />

      <Group title="Global">
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
          label="Seed"
          value={parsed.seed}
          min={0}
          max={100000}
          step={0.001}
          readOnly={readOnly}
          onChange={(next) => updateField('seed', next)}
        />
        <ColorField
          label="Background"
          value={parsed.background_rgb}
          max={1}
          readOnly={readOnly}
          onChange={(next) =>
            emit({
              ...parsed,
              background_rgb: next,
            })
          }
        />
        <ToggleField
          label="Background Gradient"
          checked={parsed.enable_background_gradient}
          readOnly={readOnly}
          onChange={(next) => updateField('enable_background_gradient', next)}
        />
        <ColorField
          label="Tint"
          value={parsed.tint_rgb}
          max={2}
          readOnly={readOnly}
          onChange={(next) =>
            emit({
              ...parsed,
              tint_rgb: next,
            })
          }
        />
      </Group>

      <Group
        title="Nebula Layer"
        enabled={parsed.enable_nebula_layer}
        readOnly={readOnly}
        onEnabledChange={(next) => updateField('enable_nebula_layer', next)}
      >
        <Field
          label="Nebula Strength"
          value={parsed.nebula_strength}
          min={0}
          max={4}
          step={0.05}
          readOnly={readOnly}
          onChange={(next) => updateField('nebula_strength', next)}
        />
        <ColorField
          label="Primary Color"
          value={parsed.nebula_color_primary_rgb}
          max={2}
          readOnly={readOnly}
          onChange={(next) =>
            emit({
              ...parsed,
              nebula_color_primary_rgb: next,
            })
          }
        />
        <ColorField
          label="Secondary Color"
          value={parsed.nebula_color_secondary_rgb}
          max={2}
          readOnly={readOnly}
          onChange={(next) =>
            emit({
              ...parsed,
              nebula_color_secondary_rgb: next,
            })
          }
        />
        <ColorField
          label="Accent Color"
          value={parsed.nebula_color_accent_rgb}
          max={2}
          readOnly={readOnly}
          onChange={(next) =>
            emit({
              ...parsed,
              nebula_color_accent_rgb: next,
            })
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
        <SelectField
          label="Nebula Blend Mode"
          value={String(parsed.nebula_blend_mode)}
          options={BLEND_MODE_OPTIONS}
          readOnly={readOnly}
          onChange={(next) =>
            updateField('nebula_blend_mode', Number.parseInt(next, 10) || 0)
          }
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
      </Group>

      <Group
        title="Stars Layer"
        enabled={parsed.enable_stars_layer}
        readOnly={readOnly}
        onEnabledChange={(next) => updateField('enable_stars_layer', next)}
      >
        <SelectField
          label="Stars Blend Mode"
          value={String(parsed.stars_blend_mode)}
          options={BLEND_MODE_OPTIONS}
          readOnly={readOnly}
          onChange={(next) =>
            updateField('stars_blend_mode', Number.parseInt(next, 10) || 0)
          }
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
        <RangeField
          label="Star Size Range"
          minValue={parsed.star_size_min}
          maxValue={parsed.star_size_max}
          min={0.01}
          max={0.35}
          step={0.001}
          readOnly={readOnly}
          onChange={(minValue, maxValue) => {
            emit({
              ...parsed,
              star_size_min: minValue,
              star_size_max: maxValue,
            })
          }}
        />
        <ColorField
          label="Star Color"
          value={parsed.star_color_rgb}
          max={2}
          alpha={parsed.stars_opacity}
          readOnly={readOnly}
          onChange={(next) =>
            emit({
              ...parsed,
              star_color_rgb: next,
            })
          }
          onAlphaChange={(next) => updateField('stars_opacity', next)}
        />
      </Group>

      <Group title="Star/Flare Mask">
        <ToggleField
          label="Enabled"
          checked={parsed.star_mask_enabled}
          readOnly={readOnly}
          onChange={(next) => updateField('star_mask_enabled', next)}
        />
        <SelectField
          label="Mask Mode"
          value={String(parsed.star_mask_mode)}
          options={[
            { value: '0', label: 'fBm' },
            { value: '1', label: 'Ridged fBm' },
          ]}
          readOnly={readOnly}
          onChange={(next) =>
            updateField('star_mask_mode', Number.parseInt(next, 10) || 0)
          }
        />
        <Field
          label="Mask Octaves"
          value={parsed.star_mask_octaves}
          min={1}
          max={8}
          step={1}
          readOnly={readOnly}
          onChange={(next) => updateField('star_mask_octaves', next)}
        />
        <Field
          label="Mask Gain"
          value={parsed.star_mask_gain}
          min={0.1}
          max={0.95}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('star_mask_gain', next)}
        />
        <Field
          label="Mask Lacunarity"
          value={parsed.star_mask_lacunarity}
          min={1.1}
          max={4}
          step={0.05}
          readOnly={readOnly}
          onChange={(next) => updateField('star_mask_lacunarity', next)}
        />
        <Field
          label="Mask Threshold"
          value={parsed.star_mask_threshold}
          min={0}
          max={0.99}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('star_mask_threshold', next)}
        />
        <Field
          label="Mask Power"
          value={parsed.star_mask_power}
          min={0.2}
          max={4}
          step={0.05}
          readOnly={readOnly}
          onChange={(next) => updateField('star_mask_power', next)}
        />
        <Field
          label="Mask Ridge Offset"
          value={parsed.star_mask_ridge_offset}
          min={0.5}
          max={2.5}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('star_mask_ridge_offset', next)}
        />
        <Field
          label="Mask Scale"
          value={parsed.star_mask_scale}
          min={0.2}
          max={8}
          step={0.05}
          readOnly={readOnly}
          onChange={(next) => updateField('star_mask_scale', next)}
        />
      </Group>

      <Group
        title="Flares Layer"
        enabled={parsed.enable_flares_layer}
        readOnly={readOnly}
        onEnabledChange={(next) => updateField('enable_flares_layer', next)}
      >
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
        <ColorField
          label="Flare Tint"
          value={parsed.flare_tint_rgb}
          max={2}
          alpha={parsed.flares_opacity}
          readOnly={readOnly}
          onChange={(next) =>
            emit({
              ...parsed,
              flare_tint_rgb: next,
            })
          }
          onAlphaChange={(next) => updateField('flares_opacity', next)}
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
          label="Flares Blend Mode"
          value={String(parsed.flares_blend_mode)}
          options={BLEND_MODE_OPTIONS}
          readOnly={readOnly}
          onChange={(next) =>
            updateField('flares_blend_mode', Number.parseInt(next, 10) || 0)
          }
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
      </Group>

      <Group title="Depth">
        <Field
          label="Depth Layer Separation"
          value={parsed.depth_layer_separation}
          min={0}
          max={2}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('depth_layer_separation', next)}
        />
        <Field
          label="Depth Parallax Scale"
          value={parsed.depth_parallax_scale}
          min={0}
          max={2}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('depth_parallax_scale', next)}
        />
        <Field
          label="Depth Haze Strength"
          value={parsed.depth_haze_strength}
          min={0}
          max={2}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('depth_haze_strength', next)}
        />
        <Field
          label="Depth Occlusion Strength"
          value={parsed.depth_occlusion_strength}
          min={0}
          max={3}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('depth_occlusion_strength', next)}
        />
      </Group>

      <Group
        title="Backlight"
        enabled={parsed.enable_backlight}
        readOnly={readOnly}
        onEnabledChange={(next) => updateField('enable_backlight', next)}
      >
        <Field
          label="Backlight Screen X"
          value={parsed.backlight_screen_x}
          min={-1.5}
          max={1.5}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('backlight_screen_x', next)}
        />
        <Field
          label="Backlight Screen Y"
          value={parsed.backlight_screen_y}
          min={-1.5}
          max={1.5}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('backlight_screen_y', next)}
        />
        <Field
          label="Backlight Intensity"
          value={parsed.backlight_intensity}
          min={0}
          max={20}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('backlight_intensity', next)}
        />
        <Field
          label="Backlight Wrap"
          value={parsed.backlight_wrap}
          min={0}
          max={2}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('backlight_wrap', next)}
        />
        <Field
          label="Backlight Edge Boost"
          value={parsed.backlight_edge_boost}
          min={0}
          max={6}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('backlight_edge_boost', next)}
        />
        <Field
          label="Backlight Bloom Scale"
          value={parsed.backlight_bloom_scale}
          min={0}
          max={2}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('backlight_bloom_scale', next)}
        />
        <Field
          label="Backlight Bloom Threshold"
          value={parsed.backlight_bloom_threshold}
          min={0}
          max={1}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('backlight_bloom_threshold', next)}
        />
        <ColorField
          label="Backlight Color"
          value={parsed.backlight_color_rgb}
          max={3}
          readOnly={readOnly}
          onChange={(next) =>
            emit({
              ...parsed,
              backlight_color_rgb: next,
            })
          }
        />
      </Group>

      <Group
        title="Light Shafts"
        enabled={parsed.enable_light_shafts}
        readOnly={readOnly}
        onEnabledChange={(next) => updateField('enable_light_shafts', next)}
      >
        <ToggleField
          label="Shafts Debug View"
          checked={parsed.shafts_debug_view}
          readOnly={readOnly}
          onChange={(next) => updateField('shafts_debug_view', next)}
        />
        <Field
          label="Shaft Intensity"
          value={parsed.shaft_intensity}
          min={0}
          max={40}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('shaft_intensity', next)}
        />
        <Field
          label="Shaft Length"
          value={parsed.shaft_length}
          min={0.05}
          max={0.95}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('shaft_length', next)}
        />
        <Field
          label="Shaft Falloff"
          value={parsed.shaft_falloff}
          min={0.2}
          max={8}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('shaft_falloff', next)}
        />
        <Field
          label="Shaft Samples"
          value={parsed.shaft_samples}
          min={4}
          max={24}
          step={1}
          readOnly={readOnly}
          onChange={(next) => updateField('shaft_samples', next)}
        />
        <SelectField
          label="Shaft Quality"
          value={String(parsed.shaft_quality)}
          options={[
            { value: '0', label: 'Low' },
            { value: '1', label: 'Medium' },
            { value: '2', label: 'High' },
          ]}
          readOnly={readOnly}
          onChange={(next) =>
            updateField('shaft_quality', Number.parseInt(next, 10) || 0)
          }
        />
        <SelectField
          label="Shaft Blend Mode"
          value={String(parsed.shaft_blend_mode)}
          options={BLEND_MODE_OPTIONS}
          readOnly={readOnly}
          onChange={(next) =>
            updateField('shaft_blend_mode', Number.parseInt(next, 10) || 0)
          }
        />
        <Field
          label="Shaft Opacity"
          value={parsed.shaft_opacity}
          min={0}
          max={1}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateField('shaft_opacity', next)}
        />
        <ColorField
          label="Shaft Color"
          value={parsed.shaft_color_rgb}
          max={3}
          alpha={parsed.shaft_opacity}
          readOnly={readOnly}
          onChange={(next) =>
            emit({
              ...parsed,
              shaft_color_rgb: next,
            })
          }
          onAlphaChange={(next) => updateField('shaft_opacity', next)}
        />
      </Group>
    </div>
  )
}

type Vec3 = { x: number; y: number; z: number }

function useDebouncedCommit<T>(
  onCommit: (next: T) => void,
  debounceMs = 180,
): (next: T) => void {
  const timerRef = React.useRef<number | null>(null)

  React.useEffect(() => {
    return () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current)
      }
    }
  }, [])

  return React.useCallback(
    (next: T) => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current)
      }
      timerRef.current = window.setTimeout(() => {
        onCommit(next)
      }, debounceMs)
    },
    [debounceMs, onCommit],
  )
}

function Group({
  title,
  children,
  enabled,
  readOnly,
  onEnabledChange,
}: {
  title: string
  children: React.ReactNode
  enabled?: boolean
  readOnly?: boolean
  onEnabledChange?: (next: boolean) => void
}) {
  return (
    <Collapsible
      defaultOpen={false}
      className="rounded-md border border-border/60"
    >
      <div className="flex items-center gap-2 px-2 py-2">
        <CollapsibleTrigger asChild>
          <button
            type="button"
            className="flex flex-1 items-center justify-between rounded-sm px-1 py-1 text-left text-xs font-medium hover:bg-accent"
          >
            <span>{title}</span>
            <span className="text-[10px] text-muted-foreground">Expand</span>
          </button>
        </CollapsibleTrigger>
        {typeof enabled === 'boolean' && onEnabledChange ? (
          <DebouncedSwitch
            label={`${title} toggle`}
            checked={enabled}
            readOnly={Boolean(readOnly)}
            onChange={onEnabledChange}
          />
        ) : null}
      </div>
      <CollapsibleContent className="space-y-2 border-t border-border/50 px-3 py-3">
        {children}
      </CollapsibleContent>
    </Collapsible>
  )
}

function DebouncedSwitch({
  checked,
  readOnly,
  label,
  onChange,
}: {
  checked: boolean
  readOnly: boolean
  label: string
  onChange: (next: boolean) => void
}) {
  const commit = useDebouncedCommit(onChange)

  return (
    <Switch
      checked={checked}
      onCheckedChange={(next) => commit(Boolean(next))}
      disabled={readOnly}
      onClick={(event) => event.stopPropagation()}
      aria-label={label}
    />
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
    <div className="flex items-center justify-between gap-3 rounded-md border border-border/60 px-2 py-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <DebouncedSwitch
        checked={checked}
        readOnly={readOnly}
        label={`${label} toggle`}
        onChange={onChange}
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
      inputClassName="h-10 w-28 [appearance:textfield] text-right font-mono text-sm [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
    />
  )
}

function toHex(value: Vec3, max: number): string {
  const r = Math.round(clamp(value.x / max, 0, 1) * 255)
  const g = Math.round(clamp(value.y / max, 0, 1) * 255)
  const b = Math.round(clamp(value.z / max, 0, 1) * 255)
  const hex = (n: number) => n.toString(16).padStart(2, '0')
  return `#${hex(r)}${hex(g)}${hex(b)}`
}

function fromHex(value: string, max: number): Vec3 | null {
  const hex = value.trim()
  const match = /^#?([0-9a-fA-F]{6})$/.exec(hex)
  if (!match) return null
  const raw = match[1]
  const r = Number.parseInt(raw.slice(0, 2), 16) / 255
  const g = Number.parseInt(raw.slice(2, 4), 16) / 255
  const b = Number.parseInt(raw.slice(4, 6), 16) / 255
  return { x: r * max, y: g * max, z: b * max }
}

function ColorField({
  label,
  value,
  max,
  alpha,
  readOnly,
  onChange,
  onAlphaChange,
}: {
  label: string
  value: Vec3
  max: number
  alpha?: number
  readOnly: boolean
  onChange: (next: Vec3) => void
  onAlphaChange?: (next: number) => void
}) {
  const commitColor = useDebouncedCommit(onChange)
  const [hexValue, setHexValue] = React.useState(toHex(value, max))

  React.useEffect(() => {
    setHexValue(toHex(value, max))
  }, [value, max])

  return (
    <div className="space-y-2 rounded-md border border-border/60 p-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="flex items-center gap-2">
        <Input
          type="color"
          value={hexValue}
          disabled={readOnly}
          onChange={(event) => {
            const nextHex = event.target.value
            setHexValue(nextHex)
            const parsed = fromHex(nextHex, max)
            if (parsed) {
              commitColor(parsed)
            }
          }}
          className="h-10 w-14 cursor-pointer p-1"
          aria-label={`${label} color`}
        />
        <Input
          value={hexValue}
          disabled={readOnly}
          onChange={(event) => {
            const nextHex = event.target.value
            setHexValue(nextHex)
            const parsed = fromHex(nextHex, max)
            if (parsed) {
              commitColor(parsed)
            }
          }}
          className="h-10 font-mono text-xs uppercase"
          aria-label={`${label} hex`}
        />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <Field
          label="R"
          value={value.x}
          min={0}
          max={max}
          step={0.001}
          readOnly={readOnly}
          onChange={(next) => onChange({ ...value, x: next })}
        />
        <Field
          label="G"
          value={value.y}
          min={0}
          max={max}
          step={0.001}
          readOnly={readOnly}
          onChange={(next) => onChange({ ...value, y: next })}
        />
        <Field
          label="B"
          value={value.z}
          min={0}
          max={max}
          step={0.001}
          readOnly={readOnly}
          onChange={(next) => onChange({ ...value, z: next })}
        />
        {onAlphaChange ? (
          <Field
            label="A"
            value={alpha ?? 1}
            min={0}
            max={1}
            step={0.01}
            readOnly={readOnly}
            onChange={onAlphaChange}
          />
        ) : null}
      </div>
    </div>
  )
}

function RangeField({
  label,
  minValue,
  maxValue,
  min,
  max,
  step,
  readOnly,
  onChange,
}: {
  label: string
  minValue: number
  maxValue: number
  min: number
  max: number
  step: number
  readOnly: boolean
  onChange: (minValue: number, maxValue: number) => void
}) {
  const [values, setValues] = React.useState<[number, number]>([
    Math.min(minValue, maxValue),
    Math.max(minValue, maxValue),
  ])
  const commit = useDebouncedCommit<[number, number]>((next) => {
    onChange(next[0], next[1])
  })

  React.useEffect(() => {
    setValues([Math.min(minValue, maxValue), Math.max(minValue, maxValue)])
  }, [minValue, maxValue])

  return (
    <div className="space-y-2 rounded-md border border-border/60 p-2">
      <div className="flex items-center justify-between">
        <div className="text-xs text-muted-foreground">{label}</div>
        <div className="text-xs font-mono text-muted-foreground">
          {values[0].toFixed(3)} to {values[1].toFixed(3)}
        </div>
      </div>
      <Slider
        value={values}
        min={min}
        max={max}
        step={step}
        disabled={readOnly}
        onValueChange={(next) => {
          if (next.length < 2) return
          const sorted: [number, number] = [
            Math.min(next[0], next[1]),
            Math.max(next[0], next[1]),
          ]
          setValues(sorted)
        }}
        onValueCommit={(next) => {
          if (next.length < 2) return
          const sorted: [number, number] = [
            Math.min(next[0], next[1]),
            Math.max(next[0], next[1]),
          ]
          commit(sorted)
        }}
      />
    </div>
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
  const commit = useDebouncedCommit(onChange)

  return (
    <div className="space-y-1">
      <div className="text-xs text-muted-foreground">{label}</div>
      <select
        value={value}
        onChange={(e) => commit(e.target.value)}
        disabled={readOnly}
        className="flex h-10 w-full rounded-md border border-border/60 bg-background px-2 text-sm"
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

import * as React from 'react'
import type { ComponentEditorProps } from './types'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { Input } from '@/components/ui/input'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'

type Vec2Value = { x: number; y: number }
type Vec3Value = { x: number; y: number; z: number }

type PlanetBodyShaderSettings = {
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
  sun_direction_xy: Vec2Value
  color_primary_rgb: Vec3Value
  color_secondary_rgb: Vec3Value
  color_tertiary_rgb: Vec3Value
  color_atmosphere_rgb: Vec3Value
  color_clouds_rgb: Vec3Value
  color_night_lights_rgb: Vec3Value
  color_emissive_rgb: Vec3Value
}

type PlanetBodyShaderSettingsPayload = {
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
  sun_direction_xy: [number, number]
  color_primary_rgb: [number, number, number]
  color_secondary_rgb: [number, number, number]
  color_tertiary_rgb: [number, number, number]
  color_atmosphere_rgb: [number, number, number]
  color_clouds_rgb: [number, number, number]
  color_night_lights_rgb: [number, number, number]
  color_emissive_rgb: [number, number, number]
}

type PresetKey =
  | 'custom'
  | 'terran'
  | 'ocean'
  | 'desert'
  | 'lava'
  | 'ice'
  | 'barren-moon'
  | 'volcanic-moon'
  | 'gas-giant'
  | 'storm-giant'
  | 'toxic'
  | 'black-hole'
  | 'star'

const PRESET_OPTIONS: Array<{ value: PresetKey; label: string }> = [
  { value: 'custom', label: 'Custom' },
  { value: 'terran', label: 'Terran' },
  { value: 'ocean', label: 'Ocean World' },
  { value: 'desert', label: 'Desert Planet' },
  { value: 'lava', label: 'Lava Planet' },
  { value: 'ice', label: 'Ice World' },
  { value: 'barren-moon', label: 'Barren Moon' },
  { value: 'volcanic-moon', label: 'Volcanic Moon' },
  { value: 'gas-giant', label: 'Gas Giant' },
  { value: 'storm-giant', label: 'Storm Giant' },
  { value: 'toxic', label: 'Toxic World' },
  { value: 'black-hole', label: 'Black Hole' },
  { value: 'star', label: 'Star / Corona' },
]

const PLANET_TYPE_OPTIONS = [
  { value: '0', label: 'Terran / Oceanic' },
  { value: '1', label: 'Desert' },
  { value: '2', label: 'Lava / Volcanic' },
  { value: '3', label: 'Ice / Frozen' },
  { value: '4', label: 'Gas Giant' },
  { value: '5', label: 'Moon / Rocky' },
]

const BODY_KIND_OPTIONS = [
  { value: '0', label: 'Planet Body' },
  { value: '1', label: 'Star' },
  { value: '2', label: 'Black Hole' },
]

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function roundToStep(value: number, step: number): number {
  return Math.round(value / step) * step
}

function decimalsFromStep(step: number): number {
  if (!Number.isFinite(step) || step <= 0) return 0
  const normalized = step.toString().toLowerCase()
  if (normalized.includes('e-')) {
    const [, exponent] = normalized.split('e-')
    return Number.parseInt(exponent ?? '0', 10) || 0
  }
  const decimal = normalized.split('.')[1]
  return decimal?.length ?? 0
}

function formatForInput(value: number, step: number): string {
  const decimals = decimalsFromStep(step)
  if (decimals === 0) {
    return String(Math.round(value))
  }
  return value.toFixed(decimals).replace(/\.?0+$/, '')
}

function parseNumber(value: unknown, fallback: number): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : fallback
}

function parseVec2(value: unknown, fallback: Vec2Value): Vec2Value {
  if (Array.isArray(value)) {
    return {
      x: parseNumber(value[0], fallback.x),
      y: parseNumber(value[1], fallback.y),
    }
  }
  if (value && typeof value === 'object') {
    const obj = value as Record<string, unknown>
    return {
      x: parseNumber(obj.x, fallback.x),
      y: parseNumber(obj.y, fallback.y),
    }
  }
  return fallback
}

function parseVec3(value: unknown, fallback: Vec3Value): Vec3Value {
  if (Array.isArray(value)) {
    return {
      x: parseNumber(value[0], fallback.x),
      y: parseNumber(value[1], fallback.y),
      z: parseNumber(value[2], fallback.z),
    }
  }
  if (value && typeof value === 'object') {
    const obj = value as Record<string, unknown>
    return {
      x: parseNumber(obj.x, fallback.x),
      y: parseNumber(obj.y, fallback.y),
      z: parseNumber(obj.z, fallback.z),
    }
  }
  return fallback
}

function parseSettings(value: unknown): PlanetBodyShaderSettings {
  const obj = value && typeof value === 'object' ? (value as Record<string, unknown>) : {}
  return {
    enabled: obj.enabled !== false,
    enable_surface_detail: obj.enable_surface_detail !== false,
    enable_craters: obj.enable_craters !== false,
    enable_clouds: obj.enable_clouds !== false,
    enable_atmosphere: obj.enable_atmosphere !== false,
    enable_specular: obj.enable_specular !== false,
    enable_night_lights: obj.enable_night_lights !== false,
    enable_emissive: obj.enable_emissive !== false,
    enable_ocean_specular: obj.enable_ocean_specular !== false,
    body_kind: parseNumber(obj.body_kind, 0),
    planet_type: parseNumber(obj.planet_type, 0),
    seed: parseNumber(obj.seed, 1),
    base_radius_scale: parseNumber(obj.base_radius_scale, 0.5),
    normal_strength: parseNumber(obj.normal_strength, 0.55),
    detail_level: parseNumber(obj.detail_level, 0.3),
    rotation_speed: parseNumber(obj.rotation_speed, 0.004),
    light_wrap: parseNumber(obj.light_wrap, 0.2),
    ambient_strength: parseNumber(obj.ambient_strength, 0.16),
    specular_strength: parseNumber(obj.specular_strength, 0.12),
    specular_power: parseNumber(obj.specular_power, 18.0),
    rim_strength: parseNumber(obj.rim_strength, 0.28),
    rim_power: parseNumber(obj.rim_power, 3.6),
    fresnel_strength: parseNumber(obj.fresnel_strength, 0.4),
    cloud_shadow_strength: parseNumber(obj.cloud_shadow_strength, 0.18),
    night_glow_strength: parseNumber(obj.night_glow_strength, 0.05),
    continent_size: parseNumber(obj.continent_size, 0.58),
    ocean_level: parseNumber(obj.ocean_level, 0.46),
    mountain_height: parseNumber(obj.mountain_height, 0.34),
    roughness: parseNumber(obj.roughness, 0.44),
    terrain_octaves: parseNumber(obj.terrain_octaves, 5),
    terrain_lacunarity: parseNumber(obj.terrain_lacunarity, 2.1),
    terrain_gain: parseNumber(obj.terrain_gain, 0.5),
    crater_density: parseNumber(obj.crater_density, 0.18),
    crater_size: parseNumber(obj.crater_size, 0.33),
    volcano_density: parseNumber(obj.volcano_density, 0.04),
    ice_cap_size: parseNumber(obj.ice_cap_size, 0.18),
    storm_intensity: parseNumber(obj.storm_intensity, 0.1),
    bands_count: parseNumber(obj.bands_count, 6.0),
    spot_density: parseNumber(obj.spot_density, 0.08),
    surface_activity: parseNumber(obj.surface_activity, 0.12),
    corona_intensity: parseNumber(obj.corona_intensity, 0.0),
    cloud_coverage: parseNumber(obj.cloud_coverage, 0.34),
    cloud_scale: parseNumber(obj.cloud_scale, 1.3),
    cloud_speed: parseNumber(obj.cloud_speed, 0.18),
    cloud_alpha: parseNumber(obj.cloud_alpha, 0.42),
    atmosphere_thickness: parseNumber(obj.atmosphere_thickness, 0.12),
    atmosphere_falloff: parseNumber(obj.atmosphere_falloff, 2.8),
    atmosphere_alpha: parseNumber(obj.atmosphere_alpha, 0.48),
    city_lights: parseNumber(obj.city_lights, 0.04),
    emissive_strength: parseNumber(obj.emissive_strength, 0.0),
    sun_intensity: parseNumber(obj.sun_intensity, 1.0),
    surface_saturation: parseNumber(obj.surface_saturation, 1.12),
    surface_contrast: parseNumber(obj.surface_contrast, 1.08),
    light_color_mix: parseNumber(obj.light_color_mix, 0.14),
    sun_direction_xy: parseVec2(obj.sun_direction_xy, { x: 0.74, y: 0.52 }),
    color_primary_rgb: parseVec3(obj.color_primary_rgb, { x: 0.24, y: 0.48, z: 0.22 }),
    color_secondary_rgb: parseVec3(obj.color_secondary_rgb, { x: 0.52, y: 0.42, z: 0.28 }),
    color_tertiary_rgb: parseVec3(obj.color_tertiary_rgb, { x: 0.08, y: 0.2, z: 0.48 }),
    color_atmosphere_rgb: parseVec3(obj.color_atmosphere_rgb, { x: 0.36, y: 0.62, z: 1.0 }),
    color_clouds_rgb: parseVec3(obj.color_clouds_rgb, { x: 0.95, y: 0.97, z: 1.0 }),
    color_night_lights_rgb: parseVec3(obj.color_night_lights_rgb, { x: 1.0, y: 0.76, z: 0.4 }),
    color_emissive_rgb: parseVec3(obj.color_emissive_rgb, { x: 1.0, y: 0.42, z: 0.18 }),
  }
}

function toPayload(settings: PlanetBodyShaderSettings): PlanetBodyShaderSettingsPayload {
  return {
    enabled: settings.enabled,
    enable_surface_detail: settings.enable_surface_detail,
    enable_craters: settings.enable_craters,
    enable_clouds: settings.enable_clouds,
    enable_atmosphere: settings.enable_atmosphere,
    enable_specular: settings.enable_specular,
    enable_night_lights: settings.enable_night_lights,
    enable_emissive: settings.enable_emissive,
    enable_ocean_specular: settings.enable_ocean_specular,
    body_kind: clamp(Math.round(settings.body_kind), 0, 2),
    planet_type: clamp(Math.round(settings.planet_type), 0, 5),
    seed: clamp(Math.round(settings.seed), 0, 999999),
    base_radius_scale: clamp(roundToStep(settings.base_radius_scale, 0.01), 0.3, 0.9),
    normal_strength: clamp(roundToStep(settings.normal_strength, 0.01), 0, 2),
    detail_level: clamp(roundToStep(settings.detail_level, 0.01), 0, 1),
    rotation_speed: clamp(roundToStep(settings.rotation_speed, 0.001), -1, 1),
    light_wrap: clamp(roundToStep(settings.light_wrap, 0.01), 0, 1),
    ambient_strength: clamp(roundToStep(settings.ambient_strength, 0.01), 0, 1),
    specular_strength: clamp(roundToStep(settings.specular_strength, 0.01), 0, 3),
    specular_power: clamp(roundToStep(settings.specular_power, 0.1), 1, 64),
    rim_strength: clamp(roundToStep(settings.rim_strength, 0.01), 0, 2),
    rim_power: clamp(roundToStep(settings.rim_power, 0.1), 0.5, 8),
    fresnel_strength: clamp(roundToStep(settings.fresnel_strength, 0.01), 0, 2),
    cloud_shadow_strength: clamp(roundToStep(settings.cloud_shadow_strength, 0.01), 0, 1),
    night_glow_strength: clamp(roundToStep(settings.night_glow_strength, 0.01), 0, 1),
    continent_size: clamp(roundToStep(settings.continent_size, 0.01), 0, 1),
    ocean_level: clamp(roundToStep(settings.ocean_level, 0.01), 0, 1),
    mountain_height: clamp(roundToStep(settings.mountain_height, 0.01), 0, 1),
    roughness: clamp(roundToStep(settings.roughness, 0.01), 0, 1),
    terrain_octaves: clamp(Math.round(settings.terrain_octaves), 1, 8),
    terrain_lacunarity: clamp(roundToStep(settings.terrain_lacunarity, 0.05), 1.1, 4),
    terrain_gain: clamp(roundToStep(settings.terrain_gain, 0.01), 0.1, 0.95),
    crater_density: clamp(roundToStep(settings.crater_density, 0.01), 0, 1),
    crater_size: clamp(roundToStep(settings.crater_size, 0.01), 0, 1),
    volcano_density: clamp(roundToStep(settings.volcano_density, 0.01), 0, 1),
    ice_cap_size: clamp(roundToStep(settings.ice_cap_size, 0.01), 0, 1),
    storm_intensity: clamp(roundToStep(settings.storm_intensity, 0.01), 0, 1),
    bands_count: clamp(roundToStep(settings.bands_count, 0.1), 0, 24),
    spot_density: clamp(roundToStep(settings.spot_density, 0.01), 0, 1),
    surface_activity: clamp(roundToStep(settings.surface_activity, 0.01), 0, 1),
    corona_intensity: clamp(roundToStep(settings.corona_intensity, 0.01), 0, 2),
    cloud_coverage: clamp(roundToStep(settings.cloud_coverage, 0.01), 0, 1),
    cloud_scale: clamp(roundToStep(settings.cloud_scale, 0.01), 0.1, 6),
    cloud_speed: clamp(roundToStep(settings.cloud_speed, 0.001), -2, 2),
    cloud_alpha: clamp(roundToStep(settings.cloud_alpha, 0.01), 0, 1),
    atmosphere_thickness: clamp(roundToStep(settings.atmosphere_thickness, 0.01), 0, 0.4),
    atmosphere_falloff: clamp(roundToStep(settings.atmosphere_falloff, 0.05), 0.5, 8),
    atmosphere_alpha: clamp(roundToStep(settings.atmosphere_alpha, 0.01), 0, 1),
    city_lights: clamp(roundToStep(settings.city_lights, 0.01), 0, 1),
    emissive_strength: clamp(roundToStep(settings.emissive_strength, 0.01), 0, 2),
    sun_intensity: clamp(roundToStep(settings.sun_intensity, 0.01), 0, 4),
    surface_saturation: clamp(roundToStep(settings.surface_saturation, 0.01), 0, 2),
    surface_contrast: clamp(roundToStep(settings.surface_contrast, 0.01), 0.2, 2),
    light_color_mix: clamp(roundToStep(settings.light_color_mix, 0.01), 0, 1),
    sun_direction_xy: [
      clamp(roundToStep(settings.sun_direction_xy.x, 0.01), -1.5, 1.5),
      clamp(roundToStep(settings.sun_direction_xy.y, 0.01), -1.5, 1.5),
    ],
    color_primary_rgb: [
      clamp(roundToStep(settings.color_primary_rgb.x, 0.001), 0, 2),
      clamp(roundToStep(settings.color_primary_rgb.y, 0.001), 0, 2),
      clamp(roundToStep(settings.color_primary_rgb.z, 0.001), 0, 2),
    ],
    color_secondary_rgb: [
      clamp(roundToStep(settings.color_secondary_rgb.x, 0.001), 0, 2),
      clamp(roundToStep(settings.color_secondary_rgb.y, 0.001), 0, 2),
      clamp(roundToStep(settings.color_secondary_rgb.z, 0.001), 0, 2),
    ],
    color_tertiary_rgb: [
      clamp(roundToStep(settings.color_tertiary_rgb.x, 0.001), 0, 2),
      clamp(roundToStep(settings.color_tertiary_rgb.y, 0.001), 0, 2),
      clamp(roundToStep(settings.color_tertiary_rgb.z, 0.001), 0, 2),
    ],
    color_atmosphere_rgb: [
      clamp(roundToStep(settings.color_atmosphere_rgb.x, 0.001), 0, 2),
      clamp(roundToStep(settings.color_atmosphere_rgb.y, 0.001), 0, 2),
      clamp(roundToStep(settings.color_atmosphere_rgb.z, 0.001), 0, 2),
    ],
    color_clouds_rgb: [
      clamp(roundToStep(settings.color_clouds_rgb.x, 0.001), 0, 2),
      clamp(roundToStep(settings.color_clouds_rgb.y, 0.001), 0, 2),
      clamp(roundToStep(settings.color_clouds_rgb.z, 0.001), 0, 2),
    ],
    color_night_lights_rgb: [
      clamp(roundToStep(settings.color_night_lights_rgb.x, 0.001), 0, 2),
      clamp(roundToStep(settings.color_night_lights_rgb.y, 0.001), 0, 2),
      clamp(roundToStep(settings.color_night_lights_rgb.z, 0.001), 0, 2),
    ],
    color_emissive_rgb: [
      clamp(roundToStep(settings.color_emissive_rgb.x, 0.001), 0, 2),
      clamp(roundToStep(settings.color_emissive_rgb.y, 0.001), 0, 2),
      clamp(roundToStep(settings.color_emissive_rgb.z, 0.001), 0, 2),
    ],
  }
}

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
  defaultOpen = false,
}: {
  title: string
  children: React.ReactNode
  defaultOpen?: boolean
}) {
  return (
    <Collapsible defaultOpen={defaultOpen} className="rounded-md border border-border/60">
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
      </div>
      <CollapsibleContent className="space-y-3 border-t border-border/50 px-3 py-3">
        {children}
      </CollapsibleContent>
    </Collapsible>
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
  const commit = useDebouncedCommit(onChange)
  return (
    <div className="flex items-center justify-between gap-3 rounded-md border border-border/60 px-2 py-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <Switch
        checked={checked}
        onCheckedChange={(next) => commit(Boolean(next))}
        disabled={readOnly}
        aria-label={label}
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
  return (
    <div className="space-y-1">
      <div className="text-xs text-muted-foreground">{label}</div>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
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

function NumericField({
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
  const safe = Number.isFinite(value) ? clamp(value, min, max) : min
  const [localValue, setLocalValue] = React.useState(safe)
  const [inputValue, setInputValue] = React.useState(formatForInput(safe, step))
  const commitSlider = useDebouncedCommit(onChange)

  React.useEffect(() => {
    setLocalValue(safe)
    setInputValue(formatForInput(safe, step))
  }, [safe, step])

  return (
    <div className="space-y-1 rounded-md border border-border/60 p-2">
      <div className="flex items-center justify-between gap-2">
        <div className="text-xs text-muted-foreground">{label}</div>
        <Input
          type="number"
          value={inputValue}
          min={min}
          max={max}
          step={step}
          readOnly={readOnly}
          onChange={(event) => {
            const raw = event.target.value
            setInputValue(raw)
            const next = Number.parseFloat(raw)
            if (!Number.isFinite(next)) return
            const clamped = clamp(next, min, max)
            setLocalValue(clamped)
            onChange(clamped)
          }}
          onBlur={() => {
            const next = Number.parseFloat(inputValue)
            if (!Number.isFinite(next)) {
              setInputValue(formatForInput(localValue, step))
              return
            }
            const clamped = clamp(next, min, max)
            setLocalValue(clamped)
            setInputValue(formatForInput(clamped, step))
            onChange(clamped)
          }}
          className="h-9 w-28 font-mono text-xs"
          aria-label={`${label} value`}
        />
      </div>
      <Slider
        value={[localValue]}
        min={min}
        max={max}
        step={step}
        disabled={readOnly}
        onValueChange={(values) => {
          const next = values[0]
          if (typeof next !== 'number') return
          const clamped = clamp(next, min, max)
          setLocalValue(clamped)
          setInputValue(String(clamped))
        }}
        onValueCommit={(values) => {
          const next = values[0]
          if (typeof next !== 'number') return
          const clamped = clamp(next, min, max)
          commitSlider(clamped)
        }}
      />
    </div>
  )
}

function toHex(value: Vec3Value, max: number): string {
  const r = Math.round(clamp(value.x / max, 0, 1) * 255)
  const g = Math.round(clamp(value.y / max, 0, 1) * 255)
  const b = Math.round(clamp(value.z / max, 0, 1) * 255)
  const hex = (n: number) => n.toString(16).padStart(2, '0')
  return `#${hex(r)}${hex(g)}${hex(b)}`
}

function fromHex(value: string, max: number): Vec3Value | null {
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
  readOnly,
  onChange,
}: {
  label: string
  value: Vec3Value
  max: number
  readOnly: boolean
  onChange: (next: Vec3Value) => void
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
            if (parsed) commitColor(parsed)
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
            if (parsed) commitColor(parsed)
          }}
          className="h-10 font-mono text-xs uppercase"
          aria-label={`${label} hex`}
        />
      </div>
      <div className="grid grid-cols-3 gap-2">
        <NumericField
          label="R"
          value={value.x}
          min={0}
          max={max}
          step={0.001}
          readOnly={readOnly}
          onChange={(next) => onChange({ ...value, x: next })}
        />
        <NumericField
          label="G"
          value={value.y}
          min={0}
          max={max}
          step={0.001}
          readOnly={readOnly}
          onChange={(next) => onChange({ ...value, y: next })}
        />
        <NumericField
          label="B"
          value={value.z}
          min={0}
          max={max}
          step={0.001}
          readOnly={readOnly}
          onChange={(next) => onChange({ ...value, z: next })}
        />
      </div>
    </div>
  )
}

function makePresetMap(): Record<PresetKey, Partial<PlanetBodyShaderSettings>> {
  return {
    custom: {},
    terran: {
      body_kind: 0,
      planet_type: 0,
      base_radius_scale: 0.58,
      normal_strength: 0.88,
      detail_level: 0.68,
      rotation_speed: 0.0035,
      ambient_strength: 0.18,
      specular_strength: 0.26,
      specular_power: 28,
      continent_size: 0.68,
      ocean_level: 0.5,
      mountain_height: 0.34,
      roughness: 0.36,
      terrain_octaves: 6,
      terrain_lacunarity: 2.22,
      terrain_gain: 0.54,
      crater_density: 0.05,
      crater_size: 0.12,
      cloud_coverage: 0.58,
      cloud_scale: 1.9,
      cloud_speed: 0.08,
      cloud_alpha: 0.76,
      atmosphere_thickness: 0.18,
      atmosphere_alpha: 0.56,
      city_lights: 0.08,
      surface_saturation: 1.18,
      surface_contrast: 1.12,
      light_color_mix: 0.08,
      color_primary_rgb: { x: 0.2, y: 0.5, z: 0.24 },
      color_secondary_rgb: { x: 0.62, y: 0.56, z: 0.44 },
      color_tertiary_rgb: { x: 0.05, y: 0.21, z: 0.58 },
      color_atmosphere_rgb: { x: 0.42, y: 0.68, z: 1.0 },
      color_clouds_rgb: { x: 1.0, y: 1.0, z: 1.0 },
      color_night_lights_rgb: { x: 1.0, y: 0.82, z: 0.48 },
    },
    ocean: {
      body_kind: 0,
      planet_type: 0,
      normal_strength: 0.8,
      detail_level: 0.62,
      ocean_level: 0.62,
      continent_size: 0.42,
      mountain_height: 0.22,
      roughness: 0.34,
      terrain_octaves: 6,
      terrain_lacunarity: 2.18,
      terrain_gain: 0.53,
      cloud_coverage: 0.58,
      cloud_scale: 1.6,
      cloud_alpha: 0.62,
      atmosphere_thickness: 0.2,
      atmosphere_alpha: 0.64,
      specular_strength: 0.34,
      surface_saturation: 1.14,
      surface_contrast: 1.08,
      light_color_mix: 0.08,
      color_primary_rgb: { x: 0.06, y: 0.24, z: 0.58 },
      color_secondary_rgb: { x: 0.14, y: 0.58, z: 0.36 },
      color_tertiary_rgb: { x: 0.01, y: 0.09, z: 0.3 },
      color_atmosphere_rgb: { x: 0.3, y: 0.72, z: 1.0 },
    },
    desert: {
      body_kind: 0,
      planet_type: 1,
      normal_strength: 0.62,
      detail_level: 0.58,
      ambient_strength: 0.24,
      continent_size: 0.2,
      ocean_level: 0.05,
      mountain_height: 0.18,
      roughness: 0.6,
      terrain_octaves: 6,
      crater_density: 0.12,
      cloud_coverage: 0.1,
      atmosphere_thickness: 0.08,
      atmosphere_alpha: 0.22,
      surface_saturation: 1.06,
      surface_contrast: 1.04,
      light_color_mix: 0.12,
      color_primary_rgb: { x: 0.72, y: 0.58, z: 0.34 },
      color_secondary_rgb: { x: 0.9, y: 0.74, z: 0.48 },
      color_tertiary_rgb: { x: 0.45, y: 0.29, z: 0.16 },
      color_atmosphere_rgb: { x: 0.97, y: 0.7, z: 0.42 },
      color_clouds_rgb: { x: 0.98, y: 0.9, z: 0.74 },
    },
    lava: {
      body_kind: 0,
      planet_type: 2,
      normal_strength: 0.84,
      detail_level: 0.7,
      rotation_speed: 0.0025,
      ambient_strength: 0.12,
      specular_strength: 0.06,
      volcano_density: 0.46,
      surface_activity: 0.74,
      storm_intensity: 0.18,
      cloud_coverage: 0.08,
      atmosphere_thickness: 0.06,
      atmosphere_alpha: 0.24,
      emissive_strength: 0.72,
      surface_saturation: 1.0,
      surface_contrast: 1.16,
      light_color_mix: 0.1,
      color_primary_rgb: { x: 0.22, y: 0.07, z: 0.05 },
      color_secondary_rgb: { x: 0.56, y: 0.12, z: 0.04 },
      color_tertiary_rgb: { x: 0.06, y: 0.02, z: 0.02 },
      color_atmosphere_rgb: { x: 1.0, y: 0.38, z: 0.15 },
      color_emissive_rgb: { x: 1.0, y: 0.38, z: 0.1 },
    },
    ice: {
      body_kind: 0,
      planet_type: 3,
      normal_strength: 0.52,
      detail_level: 0.52,
      ambient_strength: 0.28,
      specular_strength: 0.24,
      ice_cap_size: 0.72,
      cloud_coverage: 0.2,
      atmosphere_thickness: 0.1,
      atmosphere_alpha: 0.3,
      color_primary_rgb: { x: 0.82, y: 0.9, z: 0.98 },
      color_secondary_rgb: { x: 0.54, y: 0.72, z: 0.9 },
      color_tertiary_rgb: { x: 0.22, y: 0.34, z: 0.5 },
      color_atmosphere_rgb: { x: 0.7, y: 0.88, z: 1.0 },
      color_clouds_rgb: { x: 0.98, y: 0.99, z: 1.0 },
      surface_saturation: 0.96,
      surface_contrast: 1.04,
      light_color_mix: 0.1,
    },
    'barren-moon': {
      body_kind: 0,
      planet_type: 5,
      base_radius_scale: 0.5,
      normal_strength: 0.78,
      detail_level: 0.66,
      ambient_strength: 0.1,
      specular_strength: 0.03,
      crater_density: 0.44,
      crater_size: 0.56,
      cloud_coverage: 0,
      cloud_alpha: 0,
      atmosphere_thickness: 0,
      atmosphere_alpha: 0,
      color_primary_rgb: { x: 0.42, y: 0.42, z: 0.4 },
      color_secondary_rgb: { x: 0.6, y: 0.58, z: 0.54 },
      color_tertiary_rgb: { x: 0.2, y: 0.2, z: 0.22 },
      color_atmosphere_rgb: { x: 0.4, y: 0.4, z: 0.42 },
      surface_saturation: 0.9,
      surface_contrast: 1.08,
      light_color_mix: 0.16,
    },
    'volcanic-moon': {
      body_kind: 0,
      planet_type: 5,
      base_radius_scale: 0.48,
      normal_strength: 0.9,
      detail_level: 0.72,
      ambient_strength: 0.08,
      specular_strength: 0.04,
      crater_density: 0.38,
      crater_size: 0.52,
      volcano_density: 0.44,
      emissive_strength: 0.34,
      color_primary_rgb: { x: 0.18, y: 0.14, z: 0.14 },
      color_secondary_rgb: { x: 0.34, y: 0.28, z: 0.26 },
      color_tertiary_rgb: { x: 0.54, y: 0.18, z: 0.08 },
      color_emissive_rgb: { x: 1.0, y: 0.4, z: 0.12 },
      atmosphere_thickness: 0.01,
      atmosphere_alpha: 0.03,
      surface_saturation: 1.02,
      surface_contrast: 1.14,
      light_color_mix: 0.12,
    },
    'gas-giant': {
      body_kind: 0,
      planet_type: 4,
      base_radius_scale: 0.7,
      normal_strength: 0.16,
      detail_level: 0.46,
      ambient_strength: 0.22,
      specular_strength: 0.1,
      bands_count: 12,
      spot_density: 0.28,
      storm_intensity: 0.54,
      cloud_coverage: 0.74,
      cloud_scale: 2.8,
      cloud_speed: 0.22,
      cloud_alpha: 0.7,
      atmosphere_thickness: 0.18,
      atmosphere_alpha: 0.42,
      color_primary_rgb: { x: 0.72, y: 0.52, z: 0.32 },
      color_secondary_rgb: { x: 0.92, y: 0.82, z: 0.62 },
      color_tertiary_rgb: { x: 0.56, y: 0.38, z: 0.22 },
      color_atmosphere_rgb: { x: 0.9, y: 0.72, z: 0.5 },
      color_clouds_rgb: { x: 0.98, y: 0.92, z: 0.8 },
      surface_saturation: 1.04,
      surface_contrast: 1.08,
      light_color_mix: 0.16,
    },
    'storm-giant': {
      body_kind: 0,
      planet_type: 4,
      base_radius_scale: 0.72,
      normal_strength: 0.22,
      detail_level: 0.62,
      ambient_strength: 0.16,
      bands_count: 16,
      spot_density: 0.42,
      storm_intensity: 0.9,
      surface_activity: 0.62,
      cloud_coverage: 0.84,
      cloud_scale: 3.4,
      cloud_speed: 0.36,
      cloud_alpha: 0.78,
      atmosphere_thickness: 0.22,
      atmosphere_alpha: 0.5,
      corona_intensity: 0.18,
      color_primary_rgb: { x: 0.14, y: 0.28, z: 0.66 },
      color_secondary_rgb: { x: 0.28, y: 0.58, z: 0.96 },
      color_tertiary_rgb: { x: 0.04, y: 0.12, z: 0.36 },
      color_atmosphere_rgb: { x: 0.44, y: 0.76, z: 1.0 },
      color_clouds_rgb: { x: 0.8, y: 0.9, z: 1.0 },
      surface_saturation: 1.08,
      surface_contrast: 1.14,
      light_color_mix: 0.18,
    },
    toxic: {
      body_kind: 0,
      planet_type: 1,
      base_radius_scale: 0.58,
      normal_strength: 0.66,
      detail_level: 0.62,
      ambient_strength: 0.16,
      cloud_coverage: 0.46,
      cloud_scale: 1.9,
      cloud_speed: 0.14,
      cloud_alpha: 0.56,
      atmosphere_thickness: 0.18,
      atmosphere_alpha: 0.62,
      emissive_strength: 0.12,
      color_primary_rgb: { x: 0.18, y: 0.34, z: 0.08 },
      color_secondary_rgb: { x: 0.42, y: 0.58, z: 0.12 },
      color_tertiary_rgb: { x: 0.06, y: 0.12, z: 0.03 },
      color_atmosphere_rgb: { x: 0.6, y: 0.92, z: 0.18 },
      color_clouds_rgb: { x: 0.84, y: 0.98, z: 0.44 },
      color_emissive_rgb: { x: 0.72, y: 1.0, z: 0.16 },
      surface_saturation: 1.08,
      surface_contrast: 1.08,
      light_color_mix: 0.12,
    },
    'black-hole': {
      enable_surface_detail: false,
      enable_craters: false,
      enable_clouds: false,
      enable_atmosphere: true,
      enable_specular: false,
      enable_night_lights: false,
      enable_emissive: true,
      enable_ocean_specular: false,
      body_kind: 2,
      planet_type: 5,
      base_radius_scale: 0.5,
      normal_strength: 0.02,
      detail_level: 0.08,
      rotation_speed: 0.01,
      ambient_strength: 0.02,
      specular_strength: 0,
      specular_power: 4,
      rim_strength: 0.7,
      rim_power: 2.4,
      fresnel_strength: 0.9,
      cloud_shadow_strength: 0,
      night_glow_strength: 0,
      continent_size: 0,
      ocean_level: 0,
      mountain_height: 0,
      roughness: 0,
      terrain_octaves: 2,
      terrain_lacunarity: 2,
      terrain_gain: 0.4,
      crater_density: 0,
      crater_size: 0,
      volcano_density: 0,
      ice_cap_size: 0,
      storm_intensity: 0.25,
      bands_count: 0,
      spot_density: 0,
      surface_activity: 0.8,
      corona_intensity: 0.8,
      cloud_coverage: 0,
      cloud_scale: 1,
      cloud_speed: 0.2,
      cloud_alpha: 0,
      atmosphere_thickness: 0.22,
      atmosphere_falloff: 2.2,
      atmosphere_alpha: 0.45,
      city_lights: 0,
      emissive_strength: 0.9,
      sun_intensity: 0.75,
      surface_saturation: 1.0,
      surface_contrast: 1.1,
      light_color_mix: 0.12,
      color_primary_rgb: { x: 0.03, y: 0.03, z: 0.05 },
      color_secondary_rgb: { x: 0.08, y: 0.08, z: 0.12 },
      color_tertiary_rgb: { x: 0.14, y: 0.14, z: 0.18 },
      color_atmosphere_rgb: { x: 0.24, y: 0.42, z: 1.0 },
      color_clouds_rgb: { x: 0.9, y: 0.96, z: 1.0 },
      color_night_lights_rgb: { x: 0, y: 0, z: 0 },
      color_emissive_rgb: { x: 1.0, y: 0.62, z: 0.18 },
    },
    star: {
      enable_surface_detail: false,
      enable_craters: false,
      enable_clouds: false,
      enable_atmosphere: true,
      enable_specular: false,
      enable_night_lights: false,
      enable_emissive: true,
      enable_ocean_specular: false,
      body_kind: 1,
      planet_type: 0,
      base_radius_scale: 0.62,
      normal_strength: 0.1,
      detail_level: 0.24,
      rotation_speed: 0.008,
      ambient_strength: 0.55,
      specular_strength: 0.08,
      specular_power: 6,
      rim_strength: 1.0,
      rim_power: 2.0,
      fresnel_strength: 0.8,
      surface_activity: 0.7,
      corona_intensity: 1.1,
      atmosphere_thickness: 0.24,
      atmosphere_falloff: 1.6,
      atmosphere_alpha: 0.72,
      emissive_strength: 1.2,
      sun_intensity: 1.8,
      surface_saturation: 1.02,
      surface_contrast: 1.18,
      light_color_mix: 0.0,
      color_primary_rgb: { x: 1.0, y: 0.86, z: 0.42 },
      color_secondary_rgb: { x: 1.0, y: 0.62, z: 0.18 },
      color_tertiary_rgb: { x: 0.8, y: 0.24, z: 0.06 },
      color_atmosphere_rgb: { x: 1.0, y: 0.7, z: 0.3 },
      color_clouds_rgb: { x: 1.0, y: 0.92, z: 0.7 },
      color_night_lights_rgb: { x: 0, y: 0, z: 0 },
      color_emissive_rgb: { x: 1.0, y: 0.7, z: 0.24 },
    },
  }
}

export function PlanetBodyShaderSettingsEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const settings = React.useMemo(() => parseSettings(value), [value])
  const [selectedPreset, setSelectedPreset] = React.useState<PresetKey>('custom')

  const emit = React.useCallback(
    (next: PlanetBodyShaderSettings) => {
      onChange(toPayload(next))
    },
    [onChange],
  )

  const update = React.useCallback(
    (patch: Partial<PlanetBodyShaderSettings>) => {
      emit({ ...settings, ...patch })
    },
    [emit, settings],
  )

  const updateVec2 = React.useCallback(
    (key: 'sun_direction_xy', patch: Partial<Vec2Value>) => {
      update({ [key]: { ...settings[key], ...patch } } as Partial<PlanetBodyShaderSettings>)
    },
    [settings, update],
  )

  const updateVec3 = React.useCallback(
    (
      key:
        | 'color_primary_rgb'
        | 'color_secondary_rgb'
        | 'color_tertiary_rgb'
        | 'color_atmosphere_rgb'
        | 'color_clouds_rgb'
        | 'color_night_lights_rgb'
        | 'color_emissive_rgb',
      next: Vec3Value,
    ) => {
      update({ [key]: next } as Partial<PlanetBodyShaderSettings>)
    },
    [update],
  )

  const applyPreset = React.useCallback(
    (preset: PresetKey) => {
      if (preset === 'custom') return
      const presets = makePresetMap()
      emit({ ...settings, ...presets[preset] })
    },
    [emit, settings],
  )

  const copyCurrentAsJson = React.useCallback(async () => {
    await navigator.clipboard.writeText(JSON.stringify(toPayload(settings), null, 2))
  }, [settings])

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between rounded-md border border-white/10 px-3 py-2">
        <div>
          <div className="text-sm font-medium text-slate-100">Planet Body Shader</div>
          <div className="text-xs text-slate-400">
            Side-view globe tuning, palette presets, and lighting controls.
          </div>
        </div>
        <Switch
          checked={settings.enabled}
          onCheckedChange={(enabled) => update({ enabled })}
          disabled={readOnly}
        />
      </div>

      <Group title="Preset & Identity" defaultOpen>
        <SelectField
          label="Visual Preset"
          value={selectedPreset}
          options={PRESET_OPTIONS}
          readOnly={readOnly}
          onChange={(next) => {
            const preset = next as PresetKey
            setSelectedPreset(preset)
            applyPreset(preset)
          }}
        />
        <SelectField
          label="Body Kind"
          value={String(settings.body_kind)}
          options={BODY_KIND_OPTIONS}
          readOnly={readOnly}
          onChange={(next) => update({ body_kind: Number.parseInt(next, 10) || 0 })}
        />
        <SelectField
          label="Planet Type"
          value={String(settings.planet_type)}
          options={PLANET_TYPE_OPTIONS}
          readOnly={readOnly}
          onChange={(next) => update({ planet_type: Number.parseInt(next, 10) || 0 })}
        />
        <NumericField
          label="Seed"
          value={settings.seed}
          min={0}
          max={999999}
          step={1}
          readOnly={readOnly}
          onChange={(seed) => update({ seed })}
        />
        <NumericField
          label="Rotation Speed"
          value={settings.rotation_speed}
          min={-1}
          max={1}
          step={0.001}
          readOnly={readOnly}
          onChange={(rotation_speed) => update({ rotation_speed })}
        />
      </Group>

      <Group title="Shape & Lighting">
        <NumericField label="Sun Intensity" value={settings.sun_intensity} min={0} max={4} step={0.01} readOnly={readOnly} onChange={(sun_intensity) => update({ sun_intensity })} />
        <NumericField label="Surface Saturation" value={settings.surface_saturation} min={0} max={2} step={0.01} readOnly={readOnly} onChange={(surface_saturation) => update({ surface_saturation })} />
        <NumericField label="Surface Contrast" value={settings.surface_contrast} min={0.2} max={2} step={0.01} readOnly={readOnly} onChange={(surface_contrast) => update({ surface_contrast })} />
        <NumericField label="Light Color Mix" value={settings.light_color_mix} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(light_color_mix) => update({ light_color_mix })} />
        <NumericField label="Body Radius" value={settings.base_radius_scale} min={0.3} max={0.9} step={0.01} readOnly={readOnly} onChange={(base_radius_scale) => update({ base_radius_scale })} />
        <NumericField label="Normal Strength" value={settings.normal_strength} min={0} max={2} step={0.01} readOnly={readOnly} onChange={(normal_strength) => update({ normal_strength })} />
        <NumericField label="Detail Level" value={settings.detail_level} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(detail_level) => update({ detail_level })} />
        <NumericField label="Light Wrap" value={settings.light_wrap} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(light_wrap) => update({ light_wrap })} />
        <NumericField label="Ambient Strength" value={settings.ambient_strength} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(ambient_strength) => update({ ambient_strength })} />
        <NumericField label="Specular Strength" value={settings.specular_strength} min={0} max={3} step={0.01} readOnly={readOnly} onChange={(specular_strength) => update({ specular_strength })} />
        <NumericField label="Specular Power" value={settings.specular_power} min={1} max={64} step={0.1} readOnly={readOnly} onChange={(specular_power) => update({ specular_power })} />
        <NumericField label="Rim Strength" value={settings.rim_strength} min={0} max={2} step={0.01} readOnly={readOnly} onChange={(rim_strength) => update({ rim_strength })} />
        <NumericField label="Rim Power" value={settings.rim_power} min={0.5} max={8} step={0.1} readOnly={readOnly} onChange={(rim_power) => update({ rim_power })} />
        <NumericField label="Fresnel Strength" value={settings.fresnel_strength} min={0} max={2} step={0.01} readOnly={readOnly} onChange={(fresnel_strength) => update({ fresnel_strength })} />
        <NumericField label="Sun X" value={settings.sun_direction_xy.x} min={-1.5} max={1.5} step={0.01} readOnly={readOnly} onChange={(x) => updateVec2('sun_direction_xy', { x })} />
        <NumericField label="Sun Y" value={settings.sun_direction_xy.y} min={-1.5} max={1.5} step={0.01} readOnly={readOnly} onChange={(y) => updateVec2('sun_direction_xy', { y })} />
      </Group>

      <Group title="Feature Toggles">
        <ToggleField label="Surface Detail" checked={settings.enable_surface_detail} readOnly={readOnly} onChange={(enable_surface_detail) => update({ enable_surface_detail })} />
        <ToggleField label="Craters" checked={settings.enable_craters} readOnly={readOnly} onChange={(enable_craters) => update({ enable_craters })} />
        <ToggleField label="Cloud Layer" checked={settings.enable_clouds} readOnly={readOnly} onChange={(enable_clouds) => update({ enable_clouds })} />
        <ToggleField label="Atmosphere" checked={settings.enable_atmosphere} readOnly={readOnly} onChange={(enable_atmosphere) => update({ enable_atmosphere })} />
        <ToggleField label="Specular" checked={settings.enable_specular} readOnly={readOnly} onChange={(enable_specular) => update({ enable_specular })} />
        <ToggleField label="Ocean Specular" checked={settings.enable_ocean_specular} readOnly={readOnly} onChange={(enable_ocean_specular) => update({ enable_ocean_specular })} />
        <ToggleField label="Night Lights" checked={settings.enable_night_lights} readOnly={readOnly} onChange={(enable_night_lights) => update({ enable_night_lights })} />
        <ToggleField label="Emissive" checked={settings.enable_emissive} readOnly={readOnly} onChange={(enable_emissive) => update({ enable_emissive })} />
      </Group>

      <Group title="Surface">
        <NumericField label="Continent Size" value={settings.continent_size} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(continent_size) => update({ continent_size })} />
        <NumericField label="Ocean Level" value={settings.ocean_level} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(ocean_level) => update({ ocean_level })} />
        <NumericField label="Mountain Height" value={settings.mountain_height} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(mountain_height) => update({ mountain_height })} />
        <NumericField label="Roughness" value={settings.roughness} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(roughness) => update({ roughness })} />
        <NumericField label="Terrain Octaves" value={settings.terrain_octaves} min={1} max={8} step={1} readOnly={readOnly} onChange={(terrain_octaves) => update({ terrain_octaves })} />
        <NumericField label="Terrain Lacunarity" value={settings.terrain_lacunarity} min={1.1} max={4} step={0.05} readOnly={readOnly} onChange={(terrain_lacunarity) => update({ terrain_lacunarity })} />
        <NumericField label="Terrain Gain" value={settings.terrain_gain} min={0.1} max={0.95} step={0.01} readOnly={readOnly} onChange={(terrain_gain) => update({ terrain_gain })} />
        <NumericField label="Crater Density" value={settings.crater_density} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(crater_density) => update({ crater_density })} />
        <NumericField label="Crater Size" value={settings.crater_size} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(crater_size) => update({ crater_size })} />
        <NumericField label="Volcano Density" value={settings.volcano_density} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(volcano_density) => update({ volcano_density })} />
        <NumericField label="Ice Cap Size" value={settings.ice_cap_size} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(ice_cap_size) => update({ ice_cap_size })} />
        <NumericField label="Storm Intensity" value={settings.storm_intensity} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(storm_intensity) => update({ storm_intensity })} />
        <NumericField label="Bands Count" value={settings.bands_count} min={0} max={24} step={0.1} readOnly={readOnly} onChange={(bands_count) => update({ bands_count })} />
        <NumericField label="Spot Density" value={settings.spot_density} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(spot_density) => update({ spot_density })} />
        <NumericField label="Surface Activity" value={settings.surface_activity} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(surface_activity) => update({ surface_activity })} />
      </Group>

      <Group title="Atmosphere & Clouds">
        <NumericField label="Cloud Coverage" value={settings.cloud_coverage} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(cloud_coverage) => update({ cloud_coverage })} />
        <NumericField label="Cloud Scale" value={settings.cloud_scale} min={0.1} max={6} step={0.01} readOnly={readOnly} onChange={(cloud_scale) => update({ cloud_scale })} />
        <NumericField label="Cloud Speed" value={settings.cloud_speed} min={-2} max={2} step={0.001} readOnly={readOnly} onChange={(cloud_speed) => update({ cloud_speed })} />
        <NumericField label="Cloud Alpha" value={settings.cloud_alpha} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(cloud_alpha) => update({ cloud_alpha })} />
        <NumericField label="Cloud Shadow Strength" value={settings.cloud_shadow_strength} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(cloud_shadow_strength) => update({ cloud_shadow_strength })} />
        <NumericField label="Atmosphere Thickness" value={settings.atmosphere_thickness} min={0} max={0.4} step={0.01} readOnly={readOnly} onChange={(atmosphere_thickness) => update({ atmosphere_thickness })} />
        <NumericField label="Atmosphere Falloff" value={settings.atmosphere_falloff} min={0.5} max={8} step={0.05} readOnly={readOnly} onChange={(atmosphere_falloff) => update({ atmosphere_falloff })} />
        <NumericField label="Atmosphere Alpha" value={settings.atmosphere_alpha} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(atmosphere_alpha) => update({ atmosphere_alpha })} />
        <NumericField label="Corona Intensity" value={settings.corona_intensity} min={0} max={2} step={0.01} readOnly={readOnly} onChange={(corona_intensity) => update({ corona_intensity })} />
      </Group>

      <Group title="Night & Emissive">
        <NumericField label="Night Glow Strength" value={settings.night_glow_strength} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(night_glow_strength) => update({ night_glow_strength })} />
        <NumericField label="City Lights" value={settings.city_lights} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(city_lights) => update({ city_lights })} />
        <NumericField label="Emissive Strength" value={settings.emissive_strength} min={0} max={2} step={0.01} readOnly={readOnly} onChange={(emissive_strength) => update({ emissive_strength })} />
      </Group>

      <Group title="Palette">
        <ColorField label="Primary" value={settings.color_primary_rgb} max={2} readOnly={readOnly} onChange={(next) => updateVec3('color_primary_rgb', next)} />
        <ColorField label="Secondary" value={settings.color_secondary_rgb} max={2} readOnly={readOnly} onChange={(next) => updateVec3('color_secondary_rgb', next)} />
        <ColorField label="Tertiary" value={settings.color_tertiary_rgb} max={2} readOnly={readOnly} onChange={(next) => updateVec3('color_tertiary_rgb', next)} />
        <ColorField label="Atmosphere" value={settings.color_atmosphere_rgb} max={2} readOnly={readOnly} onChange={(next) => updateVec3('color_atmosphere_rgb', next)} />
        <ColorField label="Clouds" value={settings.color_clouds_rgb} max={2} readOnly={readOnly} onChange={(next) => updateVec3('color_clouds_rgb', next)} />
        <ColorField label="Night Lights" value={settings.color_night_lights_rgb} max={2} readOnly={readOnly} onChange={(next) => updateVec3('color_night_lights_rgb', next)} />
        <ColorField label="Emissive" value={settings.color_emissive_rgb} max={2} readOnly={readOnly} onChange={(next) => updateVec3('color_emissive_rgb', next)} />
      </Group>

      <div className="flex items-center justify-between rounded-md border border-white/10 p-3">
        <div>
          <div className="text-xs text-slate-300">Raw type path helper</div>
          <div className="text-xs text-slate-500">PlanetBodyShaderSettings</div>
        </div>
        <button
          type="button"
          onClick={() => void copyCurrentAsJson()}
          className="rounded-md border border-border/60 px-3 py-1 text-xs text-muted-foreground hover:bg-accent"
          disabled={readOnly}
        >
          Copy JSON
        </button>
      </div>

      <ToggleField
        label="Shader Enabled"
        checked={settings.enabled}
        readOnly={readOnly}
        onChange={(enabled) => update({ enabled })}
      />
    </div>
  )
}

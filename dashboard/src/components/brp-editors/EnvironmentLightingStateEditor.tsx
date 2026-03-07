import * as React from 'react'
import { DebouncedNumberField } from './DebouncedNumberField'
import type { ComponentEditorProps } from './types'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { Input } from '@/components/ui/input'

type Vec2Value = { x: number; y: number }
type Vec3Value = { x: number; y: number; z: number }

type EnvironmentLightingState = {
  primary_direction_xy: Vec2Value
  primary_elevation: number
  primary_color_rgb: Vec3Value
  primary_intensity: number
  ambient_color_rgb: Vec3Value
  ambient_intensity: number
  backlight_color_rgb: Vec3Value
  backlight_intensity: number
  event_flash_color_rgb: Vec3Value
  event_flash_intensity: number
}

type EnvironmentLightingStatePayload = {
  primary_direction_xy: [number, number]
  primary_elevation: number
  primary_color_rgb: [number, number, number]
  primary_intensity: number
  ambient_color_rgb: [number, number, number]
  ambient_intensity: number
  backlight_color_rgb: [number, number, number]
  backlight_intensity: number
  event_flash_color_rgb: [number, number, number]
  event_flash_intensity: number
}

type PresetKey = 'neutral-day' | 'cool-nebula' | 'red-alert' | 'dim-rim'

const PRESETS: Record<PresetKey, EnvironmentLightingState> = {
  'neutral-day': {
    primary_direction_xy: { x: 0.76, y: 0.58 },
    primary_elevation: 0.82,
    primary_color_rgb: { x: 1.0, y: 0.97, z: 0.92 },
    primary_intensity: 1.0,
    ambient_color_rgb: { x: 0.22, y: 0.3, z: 0.42 },
    ambient_intensity: 0.18,
    backlight_color_rgb: { x: 0.28, y: 0.42, z: 0.62 },
    backlight_intensity: 0.16,
    event_flash_color_rgb: { x: 1.0, y: 0.95, z: 0.88 },
    event_flash_intensity: 0.0,
  },
  'cool-nebula': {
    primary_direction_xy: { x: 0.42, y: 0.82 },
    primary_elevation: 0.74,
    primary_color_rgb: { x: 0.84, y: 0.92, z: 1.0 },
    primary_intensity: 0.86,
    ambient_color_rgb: { x: 0.12, y: 0.2, z: 0.34 },
    ambient_intensity: 0.3,
    backlight_color_rgb: { x: 0.32, y: 0.56, z: 0.88 },
    backlight_intensity: 0.34,
    event_flash_color_rgb: { x: 0.9, y: 0.98, z: 1.0 },
    event_flash_intensity: 0.0,
  },
  'red-alert': {
    primary_direction_xy: { x: 0.8, y: 0.24 },
    primary_elevation: 0.64,
    primary_color_rgb: { x: 1.0, y: 0.82, z: 0.72 },
    primary_intensity: 0.72,
    ambient_color_rgb: { x: 0.16, y: 0.08, z: 0.1 },
    ambient_intensity: 0.2,
    backlight_color_rgb: { x: 0.75, y: 0.12, z: 0.12 },
    backlight_intensity: 0.38,
    event_flash_color_rgb: { x: 1.0, y: 0.28, z: 0.22 },
    event_flash_intensity: 0.18,
  },
  'dim-rim': {
    primary_direction_xy: { x: -0.34, y: 0.94 },
    primary_elevation: 0.42,
    primary_color_rgb: { x: 0.78, y: 0.84, z: 0.92 },
    primary_intensity: 0.42,
    ambient_color_rgb: { x: 0.08, y: 0.11, z: 0.16 },
    ambient_intensity: 0.08,
    backlight_color_rgb: { x: 0.22, y: 0.32, z: 0.52 },
    backlight_intensity: 0.3,
    event_flash_color_rgb: { x: 1.0, y: 0.96, z: 0.9 },
    event_flash_intensity: 0.0,
  },
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

function parseSettings(value: unknown): EnvironmentLightingState {
  const fallback = PRESETS['neutral-day']
  const obj = value && typeof value === 'object' ? (value as Record<string, unknown>) : {}
  return {
    primary_direction_xy: parseVec2(
      obj.primary_direction_xy,
      fallback.primary_direction_xy,
    ),
    primary_elevation: parseNumber(
      obj.primary_elevation,
      fallback.primary_elevation,
    ),
    primary_color_rgb: parseVec3(obj.primary_color_rgb, fallback.primary_color_rgb),
    primary_intensity: parseNumber(
      obj.primary_intensity,
      fallback.primary_intensity,
    ),
    ambient_color_rgb: parseVec3(obj.ambient_color_rgb, fallback.ambient_color_rgb),
    ambient_intensity: parseNumber(
      obj.ambient_intensity,
      fallback.ambient_intensity,
    ),
    backlight_color_rgb: parseVec3(
      obj.backlight_color_rgb,
      fallback.backlight_color_rgb,
    ),
    backlight_intensity: parseNumber(
      obj.backlight_intensity,
      fallback.backlight_intensity,
    ),
    event_flash_color_rgb: parseVec3(
      obj.event_flash_color_rgb,
      fallback.event_flash_color_rgb,
    ),
    event_flash_intensity: parseNumber(
      obj.event_flash_intensity,
      fallback.event_flash_intensity,
    ),
  }
}

function toPayload(
  value: EnvironmentLightingState,
): EnvironmentLightingStatePayload {
  return {
    primary_direction_xy: [
      value.primary_direction_xy.x,
      value.primary_direction_xy.y,
    ],
    primary_elevation: value.primary_elevation,
    primary_color_rgb: [
      value.primary_color_rgb.x,
      value.primary_color_rgb.y,
      value.primary_color_rgb.z,
    ],
    primary_intensity: value.primary_intensity,
    ambient_color_rgb: [
      value.ambient_color_rgb.x,
      value.ambient_color_rgb.y,
      value.ambient_color_rgb.z,
    ],
    ambient_intensity: value.ambient_intensity,
    backlight_color_rgb: [
      value.backlight_color_rgb.x,
      value.backlight_color_rgb.y,
      value.backlight_color_rgb.z,
    ],
    backlight_intensity: value.backlight_intensity,
    event_flash_color_rgb: [
      value.event_flash_color_rgb.x,
      value.event_flash_color_rgb.y,
      value.event_flash_color_rgb.z,
    ],
    event_flash_intensity: value.event_flash_intensity,
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function toHex(value: Vec3Value): string {
  const r = Math.round(clamp(value.x, 0, 1) * 255)
  const g = Math.round(clamp(value.y, 0, 1) * 255)
  const b = Math.round(clamp(value.z, 0, 1) * 255)
  return `#${[r, g, b]
    .map((channel) => channel.toString(16).padStart(2, '0'))
    .join('')}`
}

function fromHex(hex: string, fallback: Vec3Value): Vec3Value {
  const match = /^#?([a-fA-F0-9]{6})$/.exec(hex.trim())
  if (!match) return fallback
  const value = match[1]
  return {
    x: Number.parseInt(value.slice(0, 2), 16) / 255,
    y: Number.parseInt(value.slice(2, 4), 16) / 255,
    z: Number.parseInt(value.slice(4, 6), 16) / 255,
  }
}

function Section({
  title,
  children,
  defaultOpen = true,
}: {
  title: string
  children: React.ReactNode
  defaultOpen?: boolean
}) {
  return (
    <Collapsible
      defaultOpen={defaultOpen}
      className="rounded-md border border-border/70 bg-muted/10"
    >
      <CollapsibleTrigger asChild>
        <button
          type="button"
          className="flex w-full items-center justify-between px-3 py-2 text-left text-sm font-medium"
        >
          <span>{title}</span>
          <span className="text-xs text-muted-foreground">Toggle</span>
        </button>
      </CollapsibleTrigger>
      <CollapsibleContent className="space-y-3 border-t border-border/60 px-3 py-3">
        {children}
      </CollapsibleContent>
    </Collapsible>
  )
}

function ColorField({
  label,
  value,
  readOnly,
  onChange,
}: {
  label: string
  value: Vec3Value
  readOnly: boolean
  onChange: (next: Vec3Value) => void
}) {
  const hex = toHex(value)
  return (
    <div className="space-y-2 rounded-md border border-border/60 bg-background/60 p-3">
      <div className="text-xs font-medium text-muted-foreground">{label}</div>
      <div className="flex items-center gap-2">
        <Input
          type="color"
          value={hex}
          disabled={readOnly}
          onChange={(event) => onChange(fromHex(event.target.value, value))}
          className="h-9 w-12 cursor-pointer p-1"
          aria-label={`${label} color`}
        />
        <Input
          type="text"
          value={hex}
          readOnly={readOnly}
          onChange={(event) => onChange(fromHex(event.target.value, value))}
          className="h-9 w-28 font-mono text-xs"
        />
      </div>
      <div className="grid grid-cols-3 gap-2">
        <DebouncedNumberField
          label="R"
          value={value.x}
          min={0}
          max={2}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => onChange({ ...value, x: next })}
          inputClassName="w-16 text-right font-mono text-xs"
          wrapperClassName="flex items-center gap-1"
        />
        <DebouncedNumberField
          label="G"
          value={value.y}
          min={0}
          max={2}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => onChange({ ...value, y: next })}
          inputClassName="w-16 text-right font-mono text-xs"
          wrapperClassName="flex items-center gap-1"
        />
        <DebouncedNumberField
          label="B"
          value={value.z}
          min={0}
          max={2}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => onChange({ ...value, z: next })}
          inputClassName="w-16 text-right font-mono text-xs"
          wrapperClassName="flex items-center gap-1"
        />
      </div>
    </div>
  )
}

export function EnvironmentLightingStateEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const settings = React.useMemo(() => parseSettings(value), [value])

  const commit = React.useCallback(
    (next: EnvironmentLightingState) => {
      onChange(toPayload(next))
    },
    [onChange],
  )

  const updateNumber = React.useCallback(
    (key: keyof EnvironmentLightingState, next: number) => {
      commit({
        ...settings,
        [key]: next,
      })
    },
    [commit, settings],
  )

  const updateVec2 = React.useCallback(
    (key: 'primary_direction_xy', next: Vec2Value) => {
      commit({
        ...settings,
        [key]: next,
      })
    },
    [commit, settings],
  )

  const updateVec3 = React.useCallback(
    (
      key:
        | 'primary_color_rgb'
        | 'ambient_color_rgb'
        | 'backlight_color_rgb'
        | 'event_flash_color_rgb',
      next: Vec3Value,
    ) => {
      commit({
        ...settings,
        [key]: next,
      })
    },
    [commit, settings],
  )

  const applyPreset = React.useCallback(
    (preset: PresetKey) => {
      commit(PRESETS[preset])
    },
    [commit],
  )

  return (
    <div className="space-y-4">
      <div className="space-y-2 rounded-md border border-border/70 bg-background/40 p-3">
        <div className="text-xs font-medium text-muted-foreground">Preset</div>
        <select
          value="custom"
          disabled={readOnly}
          onChange={(event) => {
            const preset = event.target.value as PresetKey | 'custom'
            if (preset !== 'custom') {
              applyPreset(preset)
            }
          }}
          className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm"
        >
          <option value="custom">Custom</option>
          <option value="neutral-day">Neutral Day</option>
          <option value="cool-nebula">Cool Nebula</option>
          <option value="red-alert">Red Alert</option>
          <option value="dim-rim">Dim Rim</option>
        </select>
      </div>

      <Section title="Primary Light Fallback">
        <div className="grid grid-cols-2 gap-3">
          <DebouncedNumberField
            label="Direction X"
            value={settings.primary_direction_xy.x}
            min={-1}
            max={1}
            step={0.01}
            readOnly={readOnly}
            onChange={(next) =>
              updateVec2('primary_direction_xy', {
                ...settings.primary_direction_xy,
                x: next,
              })
            }
          />
          <DebouncedNumberField
            label="Direction Y"
            value={settings.primary_direction_xy.y}
            min={-1}
            max={1}
            step={0.01}
            readOnly={readOnly}
            onChange={(next) =>
              updateVec2('primary_direction_xy', {
                ...settings.primary_direction_xy,
                y: next,
              })
            }
          />
          <DebouncedNumberField
            label="Elevation"
            value={settings.primary_elevation}
            min={0.01}
            max={1.5}
            step={0.01}
            readOnly={readOnly}
            onChange={(next) => updateNumber('primary_elevation', next)}
          />
          <DebouncedNumberField
            label="Intensity"
            value={settings.primary_intensity}
            min={0}
            max={4}
            step={0.01}
            readOnly={readOnly}
            onChange={(next) => updateNumber('primary_intensity', next)}
          />
        </div>
        <ColorField
          label="Primary Color"
          value={settings.primary_color_rgb}
          readOnly={readOnly}
          onChange={(next) => updateVec3('primary_color_rgb', next)}
        />
      </Section>

      <Section title="Ambient Fill" defaultOpen={false}>
        <DebouncedNumberField
          label="Ambient Intensity"
          value={settings.ambient_intensity}
          min={0}
          max={2}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateNumber('ambient_intensity', next)}
        />
        <ColorField
          label="Ambient Color"
          value={settings.ambient_color_rgb}
          readOnly={readOnly}
          onChange={(next) => updateVec3('ambient_color_rgb', next)}
        />
      </Section>

      <Section title="Backlight" defaultOpen={false}>
        <DebouncedNumberField
          label="Backlight Intensity"
          value={settings.backlight_intensity}
          min={0}
          max={2}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateNumber('backlight_intensity', next)}
        />
        <ColorField
          label="Backlight Color"
          value={settings.backlight_color_rgb}
          readOnly={readOnly}
          onChange={(next) => updateVec3('backlight_color_rgb', next)}
        />
      </Section>

      <Section title="Event Flash" defaultOpen={false}>
        <DebouncedNumberField
          label="Flash Intensity"
          value={settings.event_flash_intensity}
          min={0}
          max={4}
          step={0.01}
          readOnly={readOnly}
          onChange={(next) => updateNumber('event_flash_intensity', next)}
        />
        <ColorField
          label="Flash Color"
          value={settings.event_flash_color_rgb}
          readOnly={readOnly}
          onChange={(next) => updateVec3('event_flash_color_rgb', next)}
        />
      </Section>
    </div>
  )
}

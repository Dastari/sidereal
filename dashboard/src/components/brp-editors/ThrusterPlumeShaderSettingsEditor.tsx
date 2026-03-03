import * as React from 'react'
import type { ComponentEditorProps } from './types'
import { Switch } from '@/components/ui/switch'
import { DebouncedNumberField } from './DebouncedNumberField'

type ThrusterPlumeShaderSettings = {
  enabled: boolean
  debug_override_enabled: boolean
  debug_forced_thrust_alpha: number
  debug_force_afterburner: boolean
  base_length_m: number
  max_length_m: number
  base_width_m: number
  max_width_m: number
  idle_core_alpha: number
  max_alpha: number
  falloff: number
  edge_softness: number
  noise_strength: number
  flicker_hz: number
  reactive_length_scale: number
  reactive_alpha_scale: number
  afterburner_length_scale: number
  afterburner_alpha_boost: number
  base_color_rgb: { x: number; y: number; z: number }
  hot_color_rgb: { x: number; y: number; z: number }
  afterburner_color_rgb: { x: number; y: number; z: number }
}

type ThrusterPlumeShaderSettingsPayload = {
  enabled: boolean
  debug_override_enabled: boolean
  debug_forced_thrust_alpha: number
  debug_force_afterburner: boolean
  base_length_m: number
  max_length_m: number
  base_width_m: number
  max_width_m: number
  idle_core_alpha: number
  max_alpha: number
  falloff: number
  edge_softness: number
  noise_strength: number
  flicker_hz: number
  reactive_length_scale: number
  reactive_alpha_scale: number
  afterburner_length_scale: number
  afterburner_alpha_boost: number
  base_color_rgb: [number, number, number]
  hot_color_rgb: [number, number, number]
  afterburner_color_rgb: [number, number, number]
}

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

function parseSettings(value: unknown): ThrusterPlumeShaderSettings {
  if (!value || typeof value !== 'object') {
    return {
      enabled: true,
      debug_override_enabled: false,
      debug_forced_thrust_alpha: 0,
      debug_force_afterburner: false,
      base_length_m: 0,
      max_length_m: 14,
      base_width_m: 1.35,
      max_width_m: 4.1,
      idle_core_alpha: 0.2,
      max_alpha: 0.9,
      falloff: 1.25,
      edge_softness: 1.7,
      noise_strength: 0.35,
      flicker_hz: 16,
      reactive_length_scale: 1,
      reactive_alpha_scale: 1,
      afterburner_length_scale: 1.4,
      afterburner_alpha_boost: 0.2,
      base_color_rgb: { x: 0.35, y: 0.68, z: 1.2 },
      hot_color_rgb: { x: 0.7, y: 0.92, z: 1.3 },
      afterburner_color_rgb: { x: 1, y: 1, z: 1.4 },
    }
  }
  const obj = value as Record<string, unknown>
  return {
    enabled: Boolean(obj.enabled ?? true),
    debug_override_enabled: Boolean(obj.debug_override_enabled ?? false),
    debug_forced_thrust_alpha: Number(obj.debug_forced_thrust_alpha ?? 0),
    debug_force_afterburner: Boolean(obj.debug_force_afterburner ?? false),
    base_length_m: Number(obj.base_length_m ?? 0),
    max_length_m: Number(obj.max_length_m ?? 14),
    base_width_m: Number(obj.base_width_m ?? 1.35),
    max_width_m: Number(obj.max_width_m ?? 4.1),
    idle_core_alpha: Number(obj.idle_core_alpha ?? 0.2),
    max_alpha: Number(obj.max_alpha ?? 0.9),
    falloff: Number(obj.falloff ?? 1.25),
    edge_softness: Number(obj.edge_softness ?? 1.7),
    noise_strength: Number(obj.noise_strength ?? 0.35),
    flicker_hz: Number(obj.flicker_hz ?? 16),
    reactive_length_scale: Number(obj.reactive_length_scale ?? 1),
    reactive_alpha_scale: Number(obj.reactive_alpha_scale ?? 1),
    afterburner_length_scale: Number(obj.afterburner_length_scale ?? 1.4),
    afterburner_alpha_boost: Number(obj.afterburner_alpha_boost ?? 0.2),
    base_color_rgb: parseVec3(obj.base_color_rgb),
    hot_color_rgb: parseVec3(obj.hot_color_rgb),
    afterburner_color_rgb: parseVec3(obj.afterburner_color_rgb),
  }
}

export function ThrusterPlumeShaderSettingsEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const parsed = parseSettings(value)

  const toPayload = React.useCallback(
    (
      next: ThrusterPlumeShaderSettings,
    ): ThrusterPlumeShaderSettingsPayload => ({
      enabled: next.enabled,
      debug_override_enabled: next.debug_override_enabled,
      debug_forced_thrust_alpha: clamp(
        roundToStep(next.debug_forced_thrust_alpha, 0.01),
        0,
        1,
      ),
      debug_force_afterburner: next.debug_force_afterburner,
      base_length_m: clamp(roundToStep(next.base_length_m, 0.01), 0, 64),
      max_length_m: clamp(roundToStep(next.max_length_m, 0.01), 0, 96),
      base_width_m: clamp(roundToStep(next.base_width_m, 0.01), 0.01, 16),
      max_width_m: clamp(roundToStep(next.max_width_m, 0.01), 0.01, 24),
      idle_core_alpha: clamp(roundToStep(next.idle_core_alpha, 0.01), 0, 1),
      max_alpha: clamp(roundToStep(next.max_alpha, 0.01), 0, 1),
      falloff: clamp(roundToStep(next.falloff, 0.01), 0.05, 6),
      edge_softness: clamp(roundToStep(next.edge_softness, 0.01), 0.1, 6),
      noise_strength: clamp(roundToStep(next.noise_strength, 0.01), 0, 3),
      flicker_hz: clamp(roundToStep(next.flicker_hz, 0.1), 0, 80),
      reactive_length_scale: clamp(
        roundToStep(next.reactive_length_scale, 0.01),
        0,
        4,
      ),
      reactive_alpha_scale: clamp(roundToStep(next.reactive_alpha_scale, 0.01), 0, 4),
      afterburner_length_scale: clamp(
        roundToStep(next.afterburner_length_scale, 0.01),
        1,
        4,
      ),
      afterburner_alpha_boost: clamp(
        roundToStep(next.afterburner_alpha_boost, 0.01),
        0,
        1,
      ),
      base_color_rgb: [
        clamp(roundToStep(next.base_color_rgb.x, 0.001), 0, 2),
        clamp(roundToStep(next.base_color_rgb.y, 0.001), 0, 2),
        clamp(roundToStep(next.base_color_rgb.z, 0.001), 0, 2),
      ],
      hot_color_rgb: [
        clamp(roundToStep(next.hot_color_rgb.x, 0.001), 0, 2),
        clamp(roundToStep(next.hot_color_rgb.y, 0.001), 0, 2),
        clamp(roundToStep(next.hot_color_rgb.z, 0.001), 0, 2),
      ],
      afterburner_color_rgb: [
        clamp(roundToStep(next.afterburner_color_rgb.x, 0.001), 0, 2),
        clamp(roundToStep(next.afterburner_color_rgb.y, 0.001), 0, 2),
        clamp(roundToStep(next.afterburner_color_rgb.z, 0.001), 0, 2),
      ],
    }),
    [],
  )

  const emit = React.useCallback(
    (next: ThrusterPlumeShaderSettings) => {
      onChange(toPayload(next))
    },
    [onChange, toPayload],
  )

  const copyCurrentAsJson = React.useCallback(async () => {
    const payload = toPayload(parsed)
    await navigator.clipboard.writeText(JSON.stringify(payload, null, 2))
  }, [parsed, toPayload])

  const updateField = <TKey extends keyof ThrusterPlumeShaderSettings>(
    key: TKey,
    next: ThrusterPlumeShaderSettings[TKey],
  ) => {
    emit({ ...parsed, [key]: next })
  }

  const updateColor = (
    key: 'base_color_rgb' | 'hot_color_rgb' | 'afterburner_color_rgb',
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

  return (
    <div className="space-y-3">
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
      <ToggleField
        label="Debug Override"
        checked={parsed.debug_override_enabled}
        readOnly={readOnly}
        onChange={(next) => updateField('debug_override_enabled', next)}
      />
      <Field
        label="Debug Thrust Alpha"
        value={parsed.debug_forced_thrust_alpha}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('debug_forced_thrust_alpha', next)}
      />
      <ToggleField
        label="Debug Afterburner"
        checked={parsed.debug_force_afterburner}
        readOnly={readOnly}
        onChange={(next) => updateField('debug_force_afterburner', next)}
      />
      <Field
        label="Base Length (m)"
        value={parsed.base_length_m}
        min={0}
        max={64}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('base_length_m', next)}
      />
      <Field
        label="Max Length (m)"
        value={parsed.max_length_m}
        min={0}
        max={96}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('max_length_m', next)}
      />
      <Field
        label="Base Width (m)"
        value={parsed.base_width_m}
        min={0.01}
        max={16}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('base_width_m', next)}
      />
      <Field
        label="Max Width (m)"
        value={parsed.max_width_m}
        min={0.01}
        max={24}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('max_width_m', next)}
      />
      <Field
        label="Idle Core Alpha"
        value={parsed.idle_core_alpha}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('idle_core_alpha', next)}
      />
      <Field
        label="Max Alpha"
        value={parsed.max_alpha}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('max_alpha', next)}
      />
      <Field
        label="Falloff"
        value={parsed.falloff}
        min={0.05}
        max={6}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('falloff', next)}
      />
      <Field
        label="Edge Softness"
        value={parsed.edge_softness}
        min={0.1}
        max={6}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('edge_softness', next)}
      />
      <Field
        label="Noise Strength"
        value={parsed.noise_strength}
        min={0}
        max={3}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('noise_strength', next)}
      />
      <Field
        label="Flicker Hz"
        value={parsed.flicker_hz}
        min={0}
        max={80}
        step={0.1}
        readOnly={readOnly}
        onChange={(next) => updateField('flicker_hz', next)}
      />
      <Field
        label="Reactive Length Scale"
        value={parsed.reactive_length_scale}
        min={0}
        max={4}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('reactive_length_scale', next)}
      />
      <Field
        label="Reactive Alpha Scale"
        value={parsed.reactive_alpha_scale}
        min={0}
        max={4}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('reactive_alpha_scale', next)}
      />
      <Field
        label="Afterburner Length Scale"
        value={parsed.afterburner_length_scale}
        min={1}
        max={4}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('afterburner_length_scale', next)}
      />
      <Field
        label="Afterburner Alpha Boost"
        value={parsed.afterburner_alpha_boost}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('afterburner_alpha_boost', next)}
      />
      <Field
        label="Base Color R"
        value={parsed.base_color_rgb.x}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateColor('base_color_rgb', 'x', next)}
      />
      <Field
        label="Base Color G"
        value={parsed.base_color_rgb.y}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateColor('base_color_rgb', 'y', next)}
      />
      <Field
        label="Base Color B"
        value={parsed.base_color_rgb.z}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateColor('base_color_rgb', 'z', next)}
      />
      <Field
        label="Hot Color R"
        value={parsed.hot_color_rgb.x}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateColor('hot_color_rgb', 'x', next)}
      />
      <Field
        label="Hot Color G"
        value={parsed.hot_color_rgb.y}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateColor('hot_color_rgb', 'y', next)}
      />
      <Field
        label="Hot Color B"
        value={parsed.hot_color_rgb.z}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateColor('hot_color_rgb', 'z', next)}
      />
      <Field
        label="Afterburner Color R"
        value={parsed.afterburner_color_rgb.x}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateColor('afterburner_color_rgb', 'x', next)}
      />
      <Field
        label="Afterburner Color G"
        value={parsed.afterburner_color_rgb.y}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateColor('afterburner_color_rgb', 'y', next)}
      />
      <Field
        label="Afterburner Color B"
        value={parsed.afterburner_color_rgb.z}
        min={0}
        max={2}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateColor('afterburner_color_rgb', 'z', next)}
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
      inputClassName="h-8 w-24"
      wrapperClassName="grid grid-cols-[1fr_auto] gap-2"
    />
  )
}

import * as React from 'react'
import type { ComponentEditorProps } from './types'
import { Switch } from '@/components/ui/switch'
import { DebouncedNumberField } from './DebouncedNumberField'

type StarfieldShaderSettings = {
  enabled: boolean
  density: number
  layer_count: number
  initial_z_offset: number
  intensity: number
  alpha: number
  tint_rgb: { x: number; y: number; z: number }
}

type StarfieldShaderSettingsPayload = {
  enabled: boolean
  density: number
  layer_count: number
  initial_z_offset: number
  intensity: number
  alpha: number
  tint_rgb: [number, number, number]
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

function parseSettings(value: unknown): StarfieldShaderSettings {
  if (!value || typeof value !== 'object') {
    return {
      enabled: true,
      density: 0.05,
      layer_count: 3,
      initial_z_offset: 0.35,
      intensity: 1,
      alpha: 1,
      tint_rgb: { x: 1, y: 1, z: 1 },
    }
  }
  const obj = value as Record<string, unknown>
  const density = Number(obj.density ?? 0.05)
  const layerCount = Number(obj.layer_count ?? 3)
  const initialZOffset = Number(obj.initial_z_offset ?? 0.35)
  const intensity = Number(obj.intensity ?? 1)
  const alpha = Number(obj.alpha ?? 1)
  return {
    enabled: Boolean(obj.enabled ?? true),
    density: Number.isFinite(density) ? density : 0.05,
    layer_count: Number.isFinite(layerCount) ? layerCount : 3,
    initial_z_offset: Number.isFinite(initialZOffset) ? initialZOffset : 0.35,
    intensity: Number.isFinite(intensity) ? intensity : 1,
    alpha: Number.isFinite(alpha) ? alpha : 1,
    tint_rgb: parseVec3(obj.tint_rgb),
  }
}

export function StarfieldShaderSettingsEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const parsed = parseSettings(value)

  const toPayload = React.useCallback(
    (next: StarfieldShaderSettings): StarfieldShaderSettingsPayload => ({
      enabled: next.enabled,
      density: clamp(roundToStep(next.density, 0.01), 0, 1),
      layer_count: Math.round(clamp(next.layer_count, 1, 8)),
      initial_z_offset: clamp(roundToStep(next.initial_z_offset, 0.01), 0, 1),
      intensity: clamp(roundToStep(next.intensity, 0.05), 0, 4),
      alpha: clamp(roundToStep(next.alpha, 0.01), 0, 1),
      tint_rgb: [
        clamp(roundToStep(next.tint_rgb.x, 0.01), 0, 2),
        clamp(roundToStep(next.tint_rgb.y, 0.01), 0, 2),
        clamp(roundToStep(next.tint_rgb.z, 0.01), 0, 2),
      ],
    }),
    [],
  )

  const emit = React.useCallback(
    (next: StarfieldShaderSettings) => {
      onChange(toPayload(next))
    },
    [onChange, toPayload],
  )

  const copyCurrentAsJson = React.useCallback(async () => {
    const payload = toPayload(parsed)
    await navigator.clipboard.writeText(JSON.stringify(payload, null, 2))
  }, [parsed, toPayload])

  const updateField = <TKey extends keyof StarfieldShaderSettings>(
    key: TKey,
    next: StarfieldShaderSettings[TKey],
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
      <Field
        label="Density"
        value={parsed.density}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('density', next)}
      />
      <Field
        label="Layers"
        value={parsed.layer_count}
        min={1}
        max={8}
        step={1}
        readOnly={readOnly}
        onChange={(next) => updateField('layer_count', Math.round(next))}
      />
      <Field
        label="Initial Z Offset"
        value={parsed.initial_z_offset}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('initial_z_offset', next)}
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
        label="Alpha"
        value={parsed.alpha}
        min={0}
        max={1}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('alpha', next)}
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

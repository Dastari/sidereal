import * as React from 'react'
import { DebouncedNumberField } from './DebouncedNumberField'
import type { ComponentEditorProps } from './types'

type Vec3 = { x: number; y: number; z: number }

type TacticalMapUiSettings = {
  map_distance_m: number
  map_zoom_wheel_sensitivity: number
  overlay_takeover_alpha: number
  grid_major_color_rgb: Vec3
  grid_minor_color_rgb: Vec3
  grid_micro_color_rgb: Vec3
  grid_major_alpha: number
  grid_minor_alpha: number
  grid_micro_alpha: number
  grid_major_glow_alpha: number
  grid_minor_glow_alpha: number
  grid_micro_glow_alpha: number
  background_color_rgb: Vec3
  line_width_major_px: number
  line_width_minor_px: number
  line_width_micro_px: number
  glow_width_major_px: number
  glow_width_minor_px: number
  glow_width_micro_px: number
  fx_mode: number
  fx_opacity: number
  fx_noise_amount: number
  fx_scanline_density: number
  fx_scanline_speed: number
  fx_crt_distortion: number
  fx_vignette_strength: number
  fx_green_tint_mix: number
}

const PRESET_KEYS = ['clean', 'noisy', 'retro'] as const
type PresetKey = (typeof PRESET_KEYS)[number]

const DEFAULTS: TacticalMapUiSettings = {
  map_distance_m: 90,
  map_zoom_wheel_sensitivity: 0.12,
  overlay_takeover_alpha: 0.995,
  grid_major_color_rgb: { x: 0.22, y: 0.34, z: 0.48 },
  grid_minor_color_rgb: { x: 0.22, y: 0.34, z: 0.48 },
  grid_micro_color_rgb: { x: 0.22, y: 0.34, z: 0.48 },
  grid_major_alpha: 0.14,
  grid_minor_alpha: 0.126,
  grid_micro_alpha: 0.113,
  grid_major_glow_alpha: 0.02,
  grid_minor_glow_alpha: 0.018,
  grid_micro_glow_alpha: 0.016,
  background_color_rgb: { x: 0.005, y: 0.008, z: 0.02 },
  line_width_major_px: 1.4,
  line_width_minor_px: 0.95,
  line_width_micro_px: 0.75,
  glow_width_major_px: 2,
  glow_width_minor_px: 1.5,
  glow_width_micro_px: 1.2,
  fx_mode: 1,
  fx_opacity: 0.45,
  fx_noise_amount: 0.12,
  fx_scanline_density: 360,
  fx_scanline_speed: 0.65,
  fx_crt_distortion: 0.02,
  fx_vignette_strength: 0.24,
  fx_green_tint_mix: 0,
}

const PRESETS: Record<PresetKey, Partial<TacticalMapUiSettings>> = {
  clean: {
    fx_mode: 0,
    fx_opacity: 0,
    fx_noise_amount: 0,
    fx_scanline_density: 360,
    fx_scanline_speed: 0.65,
    fx_crt_distortion: 0,
    fx_vignette_strength: 0,
    fx_green_tint_mix: 0,
    grid_major_alpha: 0.12,
    grid_minor_alpha: 0.108,
    grid_micro_alpha: 0.097,
    grid_major_glow_alpha: 0.015,
    grid_minor_glow_alpha: 0.013,
    grid_micro_glow_alpha: 0.012,
    line_width_major_px: 1.2,
    line_width_minor_px: 0.85,
    line_width_micro_px: 0.65,
    glow_width_major_px: 1.6,
    glow_width_minor_px: 1.2,
    glow_width_micro_px: 1.0,
  },
  noisy: {
    fx_mode: 1,
    fx_opacity: 0.45,
    fx_noise_amount: 0.12,
    fx_scanline_density: 360,
    fx_scanline_speed: 0.65,
    fx_crt_distortion: 0.01,
    fx_vignette_strength: 0.16,
    fx_green_tint_mix: 0,
    grid_major_alpha: 0.14,
    grid_minor_alpha: 0.126,
    grid_micro_alpha: 0.113,
    grid_major_glow_alpha: 0.02,
    grid_minor_glow_alpha: 0.018,
    grid_micro_glow_alpha: 0.016,
    line_width_major_px: 1.4,
    line_width_minor_px: 0.95,
    line_width_micro_px: 0.75,
    glow_width_major_px: 2.0,
    glow_width_minor_px: 1.5,
    glow_width_micro_px: 1.2,
  },
  retro: {
    fx_mode: 2,
    fx_opacity: 0.72,
    fx_noise_amount: 0.18,
    fx_scanline_density: 520,
    fx_scanline_speed: 1.2,
    fx_crt_distortion: 0.045,
    fx_vignette_strength: 0.42,
    fx_green_tint_mix: 0.78,
    grid_major_alpha: 0.18,
    grid_minor_alpha: 0.162,
    grid_micro_alpha: 0.146,
    grid_major_glow_alpha: 0.03,
    grid_minor_glow_alpha: 0.027,
    grid_micro_glow_alpha: 0.024,
    line_width_major_px: 1.6,
    line_width_minor_px: 1.05,
    line_width_micro_px: 0.82,
    glow_width_major_px: 2.2,
    glow_width_minor_px: 1.7,
    glow_width_micro_px: 1.3,
  },
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function roundToStep(value: number, step: number): number {
  return Math.round(value / step) * step
}

function finiteOr(value: unknown, fallback: number): number {
  const n = Number(value)
  return Number.isFinite(n) ? n : fallback
}

function parseVec3(value: unknown, fallback: Vec3): Vec3 {
  if (Array.isArray(value)) {
    return {
      x: finiteOr(value[0], fallback.x),
      y: finiteOr(value[1], fallback.y),
      z: finiteOr(value[2], fallback.z),
    }
  }
  if (!value || typeof value !== 'object') {
    return fallback
  }
  const obj = value as Record<string, unknown>
  return {
    x: finiteOr(obj.x, fallback.x),
    y: finiteOr(obj.y, fallback.y),
    z: finiteOr(obj.z, fallback.z),
  }
}

function parseSettings(value: unknown): TacticalMapUiSettings {
  if (!value || typeof value !== 'object') {
    return DEFAULTS
  }
  const obj = value as Record<string, unknown>
  return {
    map_distance_m: finiteOr(obj.map_distance_m, DEFAULTS.map_distance_m),
    map_zoom_wheel_sensitivity: finiteOr(
      obj.map_zoom_wheel_sensitivity,
      DEFAULTS.map_zoom_wheel_sensitivity,
    ),
    overlay_takeover_alpha: finiteOr(
      obj.overlay_takeover_alpha,
      DEFAULTS.overlay_takeover_alpha,
    ),
    grid_major_color_rgb: parseVec3(
      obj.grid_major_color_rgb,
      DEFAULTS.grid_major_color_rgb,
    ),
    grid_minor_color_rgb: parseVec3(
      obj.grid_minor_color_rgb,
      DEFAULTS.grid_minor_color_rgb,
    ),
    grid_micro_color_rgb: parseVec3(
      obj.grid_micro_color_rgb,
      DEFAULTS.grid_micro_color_rgb,
    ),
    grid_major_alpha: finiteOr(obj.grid_major_alpha, DEFAULTS.grid_major_alpha),
    grid_minor_alpha: finiteOr(obj.grid_minor_alpha, DEFAULTS.grid_minor_alpha),
    grid_micro_alpha: finiteOr(obj.grid_micro_alpha, DEFAULTS.grid_micro_alpha),
    grid_major_glow_alpha: finiteOr(
      obj.grid_major_glow_alpha,
      DEFAULTS.grid_major_glow_alpha,
    ),
    grid_minor_glow_alpha: finiteOr(
      obj.grid_minor_glow_alpha,
      DEFAULTS.grid_minor_glow_alpha,
    ),
    grid_micro_glow_alpha: finiteOr(
      obj.grid_micro_glow_alpha,
      DEFAULTS.grid_micro_glow_alpha,
    ),
    background_color_rgb: parseVec3(
      obj.background_color_rgb,
      DEFAULTS.background_color_rgb,
    ),
    line_width_major_px: finiteOr(
      obj.line_width_major_px,
      DEFAULTS.line_width_major_px,
    ),
    line_width_minor_px: finiteOr(
      obj.line_width_minor_px,
      DEFAULTS.line_width_minor_px,
    ),
    line_width_micro_px: finiteOr(
      obj.line_width_micro_px,
      DEFAULTS.line_width_micro_px,
    ),
    glow_width_major_px: finiteOr(
      obj.glow_width_major_px,
      DEFAULTS.glow_width_major_px,
    ),
    glow_width_minor_px: finiteOr(
      obj.glow_width_minor_px,
      DEFAULTS.glow_width_minor_px,
    ),
    glow_width_micro_px: finiteOr(
      obj.glow_width_micro_px,
      DEFAULTS.glow_width_micro_px,
    ),
    fx_mode: finiteOr(obj.fx_mode, DEFAULTS.fx_mode),
    fx_opacity: finiteOr(obj.fx_opacity, DEFAULTS.fx_opacity),
    fx_noise_amount: finiteOr(obj.fx_noise_amount, DEFAULTS.fx_noise_amount),
    fx_scanline_density: finiteOr(
      obj.fx_scanline_density,
      DEFAULTS.fx_scanline_density,
    ),
    fx_scanline_speed: finiteOr(
      obj.fx_scanline_speed,
      DEFAULTS.fx_scanline_speed,
    ),
    fx_crt_distortion: finiteOr(
      obj.fx_crt_distortion,
      DEFAULTS.fx_crt_distortion,
    ),
    fx_vignette_strength: finiteOr(
      obj.fx_vignette_strength,
      DEFAULTS.fx_vignette_strength,
    ),
    fx_green_tint_mix: finiteOr(
      obj.fx_green_tint_mix,
      DEFAULTS.fx_green_tint_mix,
    ),
  }
}

function clampColor(v: number): number {
  return clamp(roundToStep(v, 0.01), 0, 1)
}

function clampAlpha(v: number): number {
  return clamp(roundToStep(v, 0.01), 0, 1)
}

export function TacticalMapUiSettingsEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const parsed = parseSettings(value)
  const [selectedPreset, setSelectedPreset] = React.useState<PresetKey>('noisy')

  const emit = React.useCallback(
    (next: TacticalMapUiSettings) => {
      onChange({
        map_distance_m: clamp(roundToStep(next.map_distance_m, 1), 30, 300),
        map_zoom_wheel_sensitivity: clamp(
          roundToStep(next.map_zoom_wheel_sensitivity, 0.01),
          0.01,
          0.5,
        ),
        overlay_takeover_alpha: clamp(
          roundToStep(next.overlay_takeover_alpha, 0.001),
          0.5,
          1,
        ),
        grid_major_color_rgb: [
          clampColor(next.grid_major_color_rgb.x),
          clampColor(next.grid_major_color_rgb.y),
          clampColor(next.grid_major_color_rgb.z),
        ],
        grid_minor_color_rgb: [
          clampColor(next.grid_minor_color_rgb.x),
          clampColor(next.grid_minor_color_rgb.y),
          clampColor(next.grid_minor_color_rgb.z),
        ],
        grid_micro_color_rgb: [
          clampColor(next.grid_micro_color_rgb.x),
          clampColor(next.grid_micro_color_rgb.y),
          clampColor(next.grid_micro_color_rgb.z),
        ],
        grid_major_alpha: clampAlpha(next.grid_major_alpha),
        grid_minor_alpha: clampAlpha(next.grid_minor_alpha),
        grid_micro_alpha: clampAlpha(next.grid_micro_alpha),
        grid_major_glow_alpha: clampAlpha(next.grid_major_glow_alpha),
        grid_minor_glow_alpha: clampAlpha(next.grid_minor_glow_alpha),
        grid_micro_glow_alpha: clampAlpha(next.grid_micro_glow_alpha),
        background_color_rgb: [
          clampColor(next.background_color_rgb.x),
          clampColor(next.background_color_rgb.y),
          clampColor(next.background_color_rgb.z),
        ],
        line_width_major_px: clamp(roundToStep(next.line_width_major_px, 0.01), 0.1, 8),
        line_width_minor_px: clamp(roundToStep(next.line_width_minor_px, 0.01), 0.1, 8),
        line_width_micro_px: clamp(roundToStep(next.line_width_micro_px, 0.01), 0.1, 8),
        glow_width_major_px: clamp(roundToStep(next.glow_width_major_px, 0.01), 0.1, 8),
        glow_width_minor_px: clamp(roundToStep(next.glow_width_minor_px, 0.01), 0.1, 8),
        glow_width_micro_px: clamp(roundToStep(next.glow_width_micro_px, 0.01), 0.1, 8),
        fx_mode: clamp(Math.round(next.fx_mode), 0, 2),
        fx_opacity: clampAlpha(next.fx_opacity),
        fx_noise_amount: clamp(roundToStep(next.fx_noise_amount, 0.01), 0, 1),
        fx_scanline_density: clamp(
          roundToStep(next.fx_scanline_density, 1),
          16,
          1024,
        ),
        fx_scanline_speed: clamp(
          roundToStep(next.fx_scanline_speed, 0.01),
          0,
          8,
        ),
        fx_crt_distortion: clamp(
          roundToStep(next.fx_crt_distortion, 0.001),
          0,
          0.2,
        ),
        fx_vignette_strength: clamp(
          roundToStep(next.fx_vignette_strength, 0.01),
          0,
          1,
        ),
        fx_green_tint_mix: clamp(
          roundToStep(next.fx_green_tint_mix, 0.01),
          0,
          1,
        ),
      } satisfies Record<string, unknown>)
    },
    [onChange],
  )

  const updateField = <TKey extends keyof TacticalMapUiSettings>(
    key: TKey,
    next: TacticalMapUiSettings[TKey],
  ) => {
    emit({ ...parsed, [key]: next })
  }

  const updateVec3 = (key: 'grid_major_color_rgb' | 'grid_minor_color_rgb' | 'grid_micro_color_rgb', channel: keyof Vec3, next: number) => {
    updateField(key, { ...parsed[key], [channel]: next })
  }

  const applyPreset = (preset: PresetKey) => {
    if (readOnly) {
      return
    }
    setSelectedPreset(preset)
    emit({
      ...parsed,
      ...PRESETS[preset],
    })
  }

  return (
    <div className="space-y-3">
      <div className="space-y-2 rounded border border-border/60 p-2">
        <div className="text-xs text-muted-foreground">Presets</div>
        <div className="flex flex-wrap gap-2">
          <PresetButton
            label="Clean"
            active={selectedPreset === 'clean'}
            readOnly={readOnly}
            onClick={() => applyPreset('clean')}
          />
          <PresetButton
            label="Noisy Screen"
            active={selectedPreset === 'noisy'}
            readOnly={readOnly}
            onClick={() => applyPreset('noisy')}
          />
          <PresetButton
            label="Retro CRT Green"
            active={selectedPreset === 'retro'}
            readOnly={readOnly}
            onClick={() => applyPreset('retro')}
          />
        </div>
      </div>
      <Field label="Map Distance (m)" value={parsed.map_distance_m} min={30} max={300} step={1} readOnly={readOnly} onChange={(next) => updateField('map_distance_m', next)} />
      <Field
        label="Map Zoom Wheel Sensitivity"
        value={parsed.map_zoom_wheel_sensitivity}
        min={0.01}
        max={0.5}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('map_zoom_wheel_sensitivity', next)}
      />
      <Field
        label="Overlay Takeover Alpha"
        value={parsed.overlay_takeover_alpha}
        min={0.5}
        max={1}
        step={0.001}
        readOnly={readOnly}
        onChange={(next) => updateField('overlay_takeover_alpha', next)}
      />
      <ColorRow
        title="Major RGB"
        value={parsed.grid_major_color_rgb}
        readOnly={readOnly}
        onChange={(channel, next) => updateVec3('grid_major_color_rgb', channel, next)}
      />
      <ColorRow
        title="Minor RGB"
        value={parsed.grid_minor_color_rgb}
        readOnly={readOnly}
        onChange={(channel, next) => updateVec3('grid_minor_color_rgb', channel, next)}
      />
      <ColorRow
        title="Micro RGB"
        value={parsed.grid_micro_color_rgb}
        readOnly={readOnly}
        onChange={(channel, next) => updateVec3('grid_micro_color_rgb', channel, next)}
      />
      <Field label="Major Alpha" value={parsed.grid_major_alpha} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => updateField('grid_major_alpha', next)} />
      <Field label="Minor Alpha" value={parsed.grid_minor_alpha} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => updateField('grid_minor_alpha', next)} />
      <Field label="Micro Alpha" value={parsed.grid_micro_alpha} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => updateField('grid_micro_alpha', next)} />
      <Field label="Major Glow Alpha" value={parsed.grid_major_glow_alpha} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => updateField('grid_major_glow_alpha', next)} />
      <Field label="Minor Glow Alpha" value={parsed.grid_minor_glow_alpha} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => updateField('grid_minor_glow_alpha', next)} />
      <Field label="Micro Glow Alpha" value={parsed.grid_micro_glow_alpha} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => updateField('grid_micro_glow_alpha', next)} />
      <ColorRow
        title="Background RGB"
        value={parsed.background_color_rgb}
        readOnly={readOnly}
        onChange={(channel, next) =>
          updateField('background_color_rgb', {
            ...parsed.background_color_rgb,
            [channel]: next,
          })
        }
      />
      <Field label="Line Width Major (px)" value={parsed.line_width_major_px} min={0.1} max={8} step={0.01} readOnly={readOnly} onChange={(next) => updateField('line_width_major_px', next)} />
      <Field label="Line Width Minor (px)" value={parsed.line_width_minor_px} min={0.1} max={8} step={0.01} readOnly={readOnly} onChange={(next) => updateField('line_width_minor_px', next)} />
      <Field label="Line Width Micro (px)" value={parsed.line_width_micro_px} min={0.1} max={8} step={0.01} readOnly={readOnly} onChange={(next) => updateField('line_width_micro_px', next)} />
      <Field label="Glow Width Major (px)" value={parsed.glow_width_major_px} min={0.1} max={8} step={0.01} readOnly={readOnly} onChange={(next) => updateField('glow_width_major_px', next)} />
      <Field label="Glow Width Minor (px)" value={parsed.glow_width_minor_px} min={0.1} max={8} step={0.01} readOnly={readOnly} onChange={(next) => updateField('glow_width_minor_px', next)} />
      <Field label="Glow Width Micro (px)" value={parsed.glow_width_micro_px} min={0.1} max={8} step={0.01} readOnly={readOnly} onChange={(next) => updateField('glow_width_micro_px', next)} />
      <Field label="FX Mode (0 none, 1 noise, 2 CRT)" value={parsed.fx_mode} min={0} max={2} step={1} readOnly={readOnly} onChange={(next) => updateField('fx_mode', next)} />
      <Field label="FX Opacity" value={parsed.fx_opacity} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => updateField('fx_opacity', next)} />
      <Field label="Noise Amount" value={parsed.fx_noise_amount} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => updateField('fx_noise_amount', next)} />
      <Field label="Scanline Density" value={parsed.fx_scanline_density} min={16} max={1024} step={1} readOnly={readOnly} onChange={(next) => updateField('fx_scanline_density', next)} />
      <Field label="Scanline Speed" value={parsed.fx_scanline_speed} min={0} max={8} step={0.01} readOnly={readOnly} onChange={(next) => updateField('fx_scanline_speed', next)} />
      <Field label="CRT Distortion" value={parsed.fx_crt_distortion} min={0} max={0.2} step={0.001} readOnly={readOnly} onChange={(next) => updateField('fx_crt_distortion', next)} />
      <Field label="Vignette Strength" value={parsed.fx_vignette_strength} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => updateField('fx_vignette_strength', next)} />
      <Field label="Green Tint Mix" value={parsed.fx_green_tint_mix} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => updateField('fx_green_tint_mix', next)} />
    </div>
  )
}

function PresetButton({
  label,
  active,
  readOnly,
  onClick,
}: {
  label: string
  active: boolean
  readOnly: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      disabled={readOnly}
      className={[
        'rounded border px-2 py-1 text-xs',
        active
          ? 'border-blue-400 bg-blue-500/20 text-blue-100'
          : 'border-border/60 bg-muted/40 text-foreground',
        readOnly ? 'cursor-not-allowed opacity-60' : 'hover:bg-muted/70',
      ].join(' ')}
      onClick={onClick}
    >
      {label}
    </button>
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
      inputClassName="w-32 text-right font-mono text-xs"
    />
  )
}

function ColorRow({
  title,
  value,
  readOnly,
  onChange,
}: {
  title: string
  value: Vec3
  readOnly: boolean
  onChange: (channel: keyof Vec3, next: number) => void
}) {
  return (
    <div className="space-y-2 rounded border border-border/60 p-2">
      <div className="text-xs text-muted-foreground">{title}</div>
      <div className="grid grid-cols-3 gap-2">
        <Field label="R" value={value.x} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => onChange('x', next)} />
        <Field label="G" value={value.y} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => onChange('y', next)} />
        <Field label="B" value={value.z} min={0} max={1} step={0.01} readOnly={readOnly} onChange={(next) => onChange('z', next)} />
      </div>
    </div>
  )
}

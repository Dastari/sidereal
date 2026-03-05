import * as React from 'react'
import { DebouncedNumberField } from './DebouncedNumberField'
import type { ComponentEditorProps } from './types'

type FlightTuning = {
  max_linear_accel_mps2: number
  passive_brake_accel_mps2: number
  active_brake_accel_mps2: number
  drag_per_s: number
}

const DEFAULTS: FlightTuning = {
  max_linear_accel_mps2: 120,
  passive_brake_accel_mps2: 16.611296,
  active_brake_accel_mps2: 16.611296,
  drag_per_s: 0.4,
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function roundToStep(value: number, step: number): number {
  return Math.round(value / step) * step
}

function finiteOr(value: unknown, fallback: number): number {
  const num = Number(value)
  return Number.isFinite(num) ? num : fallback
}

function parseFlightTuning(value: unknown): FlightTuning {
  if (!value || typeof value !== 'object') {
    return DEFAULTS
  }
  const obj = value as Record<string, unknown>
  return {
    max_linear_accel_mps2: finiteOr(
      obj.max_linear_accel_mps2,
      DEFAULTS.max_linear_accel_mps2,
    ),
    passive_brake_accel_mps2: finiteOr(
      obj.passive_brake_accel_mps2,
      DEFAULTS.passive_brake_accel_mps2,
    ),
    active_brake_accel_mps2: finiteOr(
      obj.active_brake_accel_mps2,
      DEFAULTS.active_brake_accel_mps2,
    ),
    drag_per_s: finiteOr(obj.drag_per_s, DEFAULTS.drag_per_s),
  }
}

export function FlightTuningEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const parsed = parseFlightTuning(value)

  const emit = React.useCallback(
    (next: FlightTuning) => {
      const maxLinearAccel = clamp(
        roundToStep(next.max_linear_accel_mps2, 0.1),
        0.1,
        5000,
      )
      const passiveBrake = clamp(
        roundToStep(next.passive_brake_accel_mps2, 0.1),
        0.1,
        5000,
      )
      const activeBrake = clamp(
        roundToStep(next.active_brake_accel_mps2, 0.1),
        passiveBrake,
        5000,
      )
      const drag = clamp(roundToStep(next.drag_per_s, 0.01), 0, 20)

      onChange({
        max_linear_accel_mps2: maxLinearAccel,
        passive_brake_accel_mps2: passiveBrake,
        active_brake_accel_mps2: activeBrake,
        drag_per_s: drag,
      } satisfies FlightTuning)
    },
    [onChange],
  )

  const updateField = <TKey extends keyof FlightTuning>(
    key: TKey,
    next: FlightTuning[TKey],
  ) => {
    emit({ ...parsed, [key]: next })
  }

  return (
    <div className="space-y-3">
      <Field
        label="Max Linear Accel (m/s^2)"
        value={parsed.max_linear_accel_mps2}
        min={0.1}
        max={5000}
        step={0.1}
        readOnly={readOnly}
        onChange={(next) => updateField('max_linear_accel_mps2', next)}
      />
      <Field
        label="Passive Brake Accel (m/s^2)"
        value={parsed.passive_brake_accel_mps2}
        min={0.1}
        max={5000}
        step={0.1}
        readOnly={readOnly}
        onChange={(next) => updateField('passive_brake_accel_mps2', next)}
      />
      <Field
        label="Active Brake Accel (m/s^2)"
        value={parsed.active_brake_accel_mps2}
        min={parsed.passive_brake_accel_mps2}
        max={5000}
        step={0.1}
        readOnly={readOnly}
        onChange={(next) => updateField('active_brake_accel_mps2', next)}
      />
      <Field
        label="Drag (/s)"
        value={parsed.drag_per_s}
        min={0}
        max={20}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('drag_per_s', next)}
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
      inputClassName="w-36 text-right font-mono text-xs"
    />
  )
}

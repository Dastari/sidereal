import * as React from 'react'
import { DebouncedNumberField } from './DebouncedNumberField'
import type { ComponentEditorProps } from './types'

type CharacterMovementController = {
  speed_mps: number
  max_accel_mps2: number
  damping_per_s: number
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function roundToStep(value: number, step: number): number {
  return Math.round(value / step) * step
}

function parseSettings(value: unknown): CharacterMovementController {
  if (!value || typeof value !== 'object') {
    return {
      speed_mps: 220,
      max_accel_mps2: 880,
      damping_per_s: 8,
    }
  }
  const obj = value as Record<string, unknown>
  const speed = Number(obj.speed_mps ?? 220)
  const accel = Number(obj.max_accel_mps2 ?? 880)
  const damping = Number(obj.damping_per_s ?? 8)
  return {
    speed_mps: Number.isFinite(speed) ? speed : 220,
    max_accel_mps2: Number.isFinite(accel) ? accel : 880,
    damping_per_s: Number.isFinite(damping) ? damping : 8,
  }
}

export function CharacterMovementControllerEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const parsed = parseSettings(value)

  const emit = React.useCallback(
    (next: CharacterMovementController) => {
      onChange({
        speed_mps: clamp(roundToStep(next.speed_mps, 1), 1, 1000),
        max_accel_mps2: clamp(roundToStep(next.max_accel_mps2, 5), 5, 5000),
        damping_per_s: clamp(roundToStep(next.damping_per_s, 0.1), 0, 40),
      } satisfies CharacterMovementController)
    },
    [onChange],
  )

  const updateField = <TKey extends keyof CharacterMovementController>(
    key: TKey,
    next: CharacterMovementController[TKey],
  ) => {
    emit({ ...parsed, [key]: next })
  }

  return (
    <div className="space-y-3">
      <Field
        label="Max Speed (m/s)"
        value={parsed.speed_mps}
        min={1}
        max={1000}
        step={1}
        readOnly={readOnly}
        onChange={(next) => updateField('speed_mps', next)}
      />
      <Field
        label="Max Accel (m/s^2)"
        value={parsed.max_accel_mps2}
        min={5}
        max={5000}
        step={5}
        readOnly={readOnly}
        onChange={(next) => updateField('max_accel_mps2', next)}
      />
      <Field
        label="Damping (/s)"
        value={parsed.damping_per_s}
        min={0}
        max={40}
        step={0.1}
        readOnly={readOnly}
        onChange={(next) => updateField('damping_per_s', next)}
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
      inputClassName="w-24 text-right font-mono text-xs"
    />
  )
}

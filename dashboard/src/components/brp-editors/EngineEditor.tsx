import * as React from 'react'
import type { ComponentEditorProps } from './types'
import { DebouncedNumberField } from './DebouncedNumberField'

type Engine = {
  burn_rate_kg_s: number
  reverse_thrust: number
  thrust: number
  torque_thrust: number
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function roundToStep(value: number, step: number): number {
  return Math.round(value / step) * step
}

function parseEngine(value: unknown): Engine {
  if (!value || typeof value !== 'object') {
    return {
      burn_rate_kg_s: 0.8,
      reverse_thrust: 300_000,
      thrust: 300_000,
      torque_thrust: 1_500_000,
    }
  }
  const obj = value as Record<string, unknown>
  return {
    burn_rate_kg_s: Number(obj.burn_rate_kg_s ?? 0.8),
    reverse_thrust: Number(obj.reverse_thrust ?? 300_000),
    thrust: Number(obj.thrust ?? 300_000),
    torque_thrust: Number(obj.torque_thrust ?? 1_500_000),
  }
}

export function EngineEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const parsed = parseEngine(value)

  const emit = React.useCallback(
    (next: Engine) => {
      onChange({
        burn_rate_kg_s: clamp(roundToStep(next.burn_rate_kg_s, 0.01), 0, 25),
        reverse_thrust: clamp(roundToStep(next.reverse_thrust, 100), 0, 2_500_000),
        thrust: clamp(roundToStep(next.thrust, 100), 0, 2_500_000),
        torque_thrust: clamp(roundToStep(next.torque_thrust, 500), 0, 12_000_000),
      } satisfies Engine)
    },
    [onChange],
  )

  const updateField = <TKey extends keyof Engine>(
    key: TKey,
    next: Engine[TKey],
  ) => {
    emit({ ...parsed, [key]: next })
  }

  return (
    <div className="space-y-3">
      <DebouncedNumberField
        label="Burn Rate (kg/s)"
        value={parsed.burn_rate_kg_s}
        min={0}
        max={25}
        step={0.01}
        readOnly={readOnly}
        onChange={(next) => updateField('burn_rate_kg_s', next)}
        inputClassName="w-32 text-right font-mono text-xs"
      />
      <DebouncedNumberField
        label="Reverse Thrust"
        value={parsed.reverse_thrust}
        min={0}
        max={2_500_000}
        step={100}
        readOnly={readOnly}
        onChange={(next) => updateField('reverse_thrust', next)}
        inputClassName="w-32 text-right font-mono text-xs"
      />
      <DebouncedNumberField
        label="Thrust"
        value={parsed.thrust}
        min={0}
        max={2_500_000}
        step={100}
        readOnly={readOnly}
        onChange={(next) => updateField('thrust', next)}
        inputClassName="w-32 text-right font-mono text-xs"
      />
      <DebouncedNumberField
        label="Torque Thrust"
        value={parsed.torque_thrust}
        min={0}
        max={12_000_000}
        step={500}
        readOnly={readOnly}
        onChange={(next) => updateField('torque_thrust', next)}
        inputClassName="w-32 text-right font-mono text-xs"
      />
    </div>
  )
}

import * as React from 'react'
import { DebouncedNumberField } from './DebouncedNumberField'
import type { ComponentEditorProps } from './types'

const MIN = 0.1
const MAX = 5000
const STEP = 0.1
const DEFAULT_VALUE = 100

function clamp(value: number): number {
  return Math.min(MAX, Math.max(MIN, value))
}

function roundToStep(value: number): number {
  return Math.round(value / STEP) * STEP
}

export function MaxVelocityMpsEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const num =
    typeof value === 'number' && Number.isFinite(value)
      ? clamp(value)
      : Array.isArray(value) && typeof value[0] === 'number'
        ? clamp(value[0])
        : DEFAULT_VALUE

  const commit = React.useCallback(
    (next: number) => {
      const clamped = clamp(roundToStep(next))
      // BRP insert_components expects tuple-newtype scalar payload for MaxVelocityMps(pub f32)
      onChange(clamped)
    },
    [onChange],
  )

  return (
    <div className="space-y-2">
      <DebouncedNumberField
        label="Max Velocity (m/s)"
        value={num}
        min={MIN}
        max={MAX}
        step={STEP}
        readOnly={readOnly}
        onChange={commit}
        inputClassName="w-28 shrink-0 text-right font-mono text-sm"
      />
    </div>
  )
}

import * as React from 'react'
import { DebouncedNumberField } from './DebouncedNumberField'
import type { ComponentEditorProps } from './types'

const MIN = 0
const MAX = 1
const STEP = 0.05

function clamp(value: number): number {
  return Math.min(MAX, Math.max(MIN, value))
}

function roundToStep(value: number): number {
  return Math.round(value / STEP) * STEP
}

export function DensityEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const num =
    typeof value === 'number' && Number.isFinite(value)
      ? clamp(value)
      : Array.isArray(value) && typeof value[0] === 'number'
        ? clamp(value[0])
        : 1

  const commit = React.useCallback(
    (next: number) => {
      const clamped = clamp(roundToStep(next))
      // BRP insert_components expects the component value as the inner type (f32), not a tuple array
      onChange(clamped)
    },
    [onChange],
  )

  return (
    <div className="space-y-2">
      <DebouncedNumberField
        label="Density"
        value={num}
        min={MIN}
        max={MAX}
        step={STEP}
        readOnly={readOnly}
        onChange={commit}
        inputClassName="w-16 shrink-0 text-right font-mono text-sm"
      />
      <p className="text-xs text-muted-foreground">
        Density (0–1), step 0.05
      </p>
    </div>
  )
}

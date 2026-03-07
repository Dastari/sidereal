import * as React from 'react'
import type { ComponentEditorProps } from './types'
import { DebouncedNumberField } from './DebouncedNumberField'

type Hardpoint = {
  hardpoint_id: string
  offset_m: [number, number, number]
  local_rotation: [number, number, number, number]
}

const DEFAULT_HARDPOINT: Hardpoint = {
  hardpoint_id: '',
  offset_m: [0, 0, 0],
  local_rotation: [0, 0, 0, 1],
}

function finiteOr(value: unknown, fallback: number): number {
  const num = Number(value)
  return Number.isFinite(num) ? num : fallback
}

function parseTupleN(
  value: unknown,
  expectedLength: number,
  fallback: number[],
): number[] {
  if (!Array.isArray(value)) {
    return fallback
  }
  const out = new Array<number>(expectedLength)
  for (let i = 0; i < expectedLength; i += 1) {
    out[i] = finiteOr(value[i], fallback[i] ?? 0)
  }
  return out
}

function parseHardpoint(value: unknown): Hardpoint {
  if (!value || typeof value !== 'object') {
    return DEFAULT_HARDPOINT
  }
  const obj = value as Record<string, unknown>
  return {
    hardpoint_id:
      typeof obj.hardpoint_id === 'string'
        ? obj.hardpoint_id
        : DEFAULT_HARDPOINT.hardpoint_id,
    offset_m: parseTupleN(obj.offset_m, 3, DEFAULT_HARDPOINT.offset_m) as [
      number,
      number,
      number,
    ],
    local_rotation: parseTupleN(
      obj.local_rotation,
      4,
      DEFAULT_HARDPOINT.local_rotation,
    ) as [number, number, number, number],
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function roundToStep(value: number, step: number): number {
  return Math.round(value / step) * step
}

export function HardpointEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const parsed = parseHardpoint(value)

  const emitOffset = React.useCallback(
    (nextOffset: [number, number, number]) => {
      onChange({
        hardpoint_id: parsed.hardpoint_id,
        offset_m: [
          clamp(roundToStep(nextOffset[0], 0.1), -5000, 5000),
          clamp(roundToStep(nextOffset[1], 0.1), -5000, 5000),
          clamp(roundToStep(nextOffset[2], 0.1), -5000, 5000),
        ] as [number, number, number],
        // Preserve local rotation exactly unless/until we expose explicit controls for it.
        local_rotation: parsed.local_rotation,
      } satisfies Hardpoint)
    },
    [onChange, parsed.hardpoint_id, parsed.local_rotation],
  )

  return (
    <div className="space-y-3">
      <div className="text-xs text-muted-foreground">
        hardpoint_id:{' '}
        <span className="font-mono text-foreground/90">
          {parsed.hardpoint_id || '(empty)'}
        </span>
      </div>
      <DebouncedNumberField
        label="Offset X (m)"
        value={parsed.offset_m[0]}
        min={-5000}
        max={5000}
        step={0.1}
        readOnly={readOnly}
        onChange={(next) =>
          emitOffset([next, parsed.offset_m[1], parsed.offset_m[2]])
        }
        inputClassName="w-32 text-right font-mono text-xs"
      />
      <DebouncedNumberField
        label="Offset Y (m)"
        value={parsed.offset_m[1]}
        min={-5000}
        max={5000}
        step={0.1}
        readOnly={readOnly}
        onChange={(next) =>
          emitOffset([parsed.offset_m[0], next, parsed.offset_m[2]])
        }
        inputClassName="w-32 text-right font-mono text-xs"
      />
      <DebouncedNumberField
        label="Offset Z (m)"
        value={parsed.offset_m[2]}
        min={-5000}
        max={5000}
        step={0.1}
        readOnly={readOnly}
        onChange={(next) =>
          emitOffset([parsed.offset_m[0], parsed.offset_m[1], next])
        }
        inputClassName="w-32 text-right font-mono text-xs"
      />
    </div>
  )
}

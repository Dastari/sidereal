import * as React from 'react'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import type { ComponentEditorProps } from './types'

type SizeM = {
  length: number
  width: number
  height: number
}

const DEFAULT_SIZE_M: SizeM = {
  length: 100,
  width: 100,
  height: 100,
}

const MIN_SIZE_M = 0.1
const MAX_SIZE_M = 100000
const STEP_SIZE_M = 0.1

function clamp(value: number): number {
  return Math.min(MAX_SIZE_M, Math.max(MIN_SIZE_M, value))
}

function roundToStep(value: number): number {
  return Math.round(value / STEP_SIZE_M) * STEP_SIZE_M
}

function formatForInput(value: number): string {
  return value.toFixed(1).replace(/\.0$/, '')
}

function finiteOr(value: unknown, fallback: number): number {
  const num = Number(value)
  return Number.isFinite(num) ? num : fallback
}

function parseSizeM(value: unknown): SizeM {
  if (!value || typeof value !== 'object') {
    return DEFAULT_SIZE_M
  }
  const obj = value as Record<string, unknown>
  return {
    length: finiteOr(obj.length, DEFAULT_SIZE_M.length),
    width: finiteOr(obj.width, DEFAULT_SIZE_M.width),
    height: finiteOr(obj.height, DEFAULT_SIZE_M.height),
  }
}

function sanitizeSizeM(next: SizeM): SizeM {
  return {
    length: clamp(roundToStep(next.length)),
    width: clamp(roundToStep(next.width)),
    height: clamp(roundToStep(next.height)),
  }
}

function LinkedDimensionField({
  label,
  value,
  readOnly,
  onCommit,
}: {
  label: string
  value: number
  readOnly: boolean
  onCommit: (value: number) => void
}) {
  const safeValue = Number.isFinite(value) ? clamp(value) : DEFAULT_SIZE_M.length
  const [inputValue, setInputValue] = React.useState(formatForInput(safeValue))

  React.useEffect(() => {
    setInputValue(formatForInput(safeValue))
  }, [safeValue])

  return (
    <label className="space-y-1">
      <div className="text-xs text-muted-foreground">{label}</div>
      <Input
        type="number"
        value={inputValue}
        min={MIN_SIZE_M}
        max={MAX_SIZE_M}
        step={STEP_SIZE_M}
        readOnly={readOnly}
        onChange={(event) => {
          const raw = event.target.value
          setInputValue(raw)
          const next = Number.parseFloat(raw)
          if (Number.isFinite(next)) {
            onCommit(next)
          }
        }}
        onBlur={() => {
          const next = Number.parseFloat(inputValue)
          if (!Number.isFinite(next)) {
            setInputValue(formatForInput(safeValue))
            return
          }
          const sanitized = clamp(roundToStep(next))
          setInputValue(formatForInput(sanitized))
          onCommit(sanitized)
        }}
        className="w-full text-right font-mono text-sm"
        aria-label={`${label} value`}
      />
    </label>
  )
}

export function SizeMEditor({
  value,
  onChange,
  readOnly = false,
}: ComponentEditorProps) {
  const parsed = parseSizeM(value)
  const [linked, setLinked] = React.useState(
    Math.abs(parsed.length - parsed.width) < 0.0001 &&
      Math.abs(parsed.length - parsed.height) < 0.0001,
  )

  const emit = React.useCallback(
    (next: SizeM) => {
      onChange(sanitizeSizeM(next))
    },
    [onChange],
  )

  const updateDimension = React.useCallback(
    (axis: keyof SizeM, nextValue: number) => {
      const sanitizedValue = clamp(roundToStep(nextValue))
      if (linked) {
        emit({
          length: sanitizedValue,
          width: sanitizedValue,
          height: sanitizedValue,
        })
        return
      }
      emit({
        ...parsed,
        [axis]: sanitizedValue,
      })
    },
    [emit, linked, parsed],
  )

  return (
    <div className="space-y-3 rounded-md border border-border/60 bg-card/40 p-3">
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="text-sm font-medium text-foreground">Physical Size</div>
          <div className="text-xs text-muted-foreground">
            Updates the entity `SizeM` component directly.
          </div>
        </div>
        <label className="flex items-center gap-2 text-xs text-muted-foreground">
          <span>Link</span>
          <Switch
            checked={linked}
            disabled={readOnly}
            onCheckedChange={(checked) => {
              setLinked(checked)
              if (checked) {
                const unified = clamp(roundToStep(Math.max(parsed.length, parsed.width, parsed.height)))
                emit({
                  length: unified,
                  width: unified,
                  height: unified,
                })
              }
            }}
            aria-label="Link size dimensions"
          />
        </label>
      </div>

      <div className="grid gap-3 md:grid-cols-3">
        <LinkedDimensionField
          label="Length (m)"
          value={parsed.length}
          readOnly={readOnly}
          onCommit={(next) => updateDimension('length', next)}
        />
        <LinkedDimensionField
          label="Width (m)"
          value={parsed.width}
          readOnly={readOnly}
          onCommit={(next) => updateDimension('width', next)}
        />
        <LinkedDimensionField
          label="Height (m)"
          value={parsed.height}
          readOnly={readOnly}
          onCommit={(next) => updateDimension('height', next)}
        />
      </div>
    </div>
  )
}

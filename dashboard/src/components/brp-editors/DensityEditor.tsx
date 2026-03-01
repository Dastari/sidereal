import * as React from 'react'
import type { ComponentEditorProps } from './types'
import { Slider } from '@/components/ui/slider'
import { Input } from '@/components/ui/input'

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

  const [sliderValue, setSliderValue] = React.useState(num)
  const [inputValue, setInputValue] = React.useState(num.toFixed(2))

  React.useEffect(() => {
    setSliderValue(num)
    setInputValue(num.toFixed(2))
  }, [num])

  const commit = React.useCallback(
    (next: number) => {
      const clamped = clamp(roundToStep(next))
      setSliderValue(clamped)
      setInputValue(clamped.toFixed(2))
      // BRP insert_components expects the component value as the inner type (f32), not a tuple array
      onChange(clamped)
    },
    [onChange],
  )

  const onSliderChange = (values: Array<number>) => {
    const v = values[0]
    if (typeof v !== 'number') return
    setSliderValue(v)
    setInputValue(v.toFixed(2))
    onChange(roundToStep(v))
  }

  const onInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const raw = e.target.value
    setInputValue(raw)
    const parsed = Number.parseFloat(raw)
    if (!Number.isNaN(parsed)) {
      const clamped = clamp(parsed)
      setSliderValue(clamped)
    }
  }

  const onInputBlur = () => {
    const parsed = Number.parseFloat(inputValue)
    if (Number.isNaN(parsed)) {
      setInputValue(sliderValue.toFixed(2))
      return
    }
    commit(parsed)
  }

  const onInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.currentTarget.blur()
    }
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-3">
        <Slider
          min={MIN}
          max={MAX}
          step={STEP}
          value={[sliderValue]}
          onValueChange={onSliderChange}
          disabled={readOnly}
          className="flex-1"
        />
        <Input
          type="number"
          min={MIN}
          max={MAX}
          step={STEP}
          value={inputValue}
          onChange={onInputChange}
          onBlur={onInputBlur}
          onKeyDown={onInputKeyDown}
          disabled={readOnly}
          className="w-16 shrink-0 text-right font-mono text-sm"
          aria-label="Density value"
        />
      </div>
      <p className="text-xs text-muted-foreground">
        Density (0–1), step 0.05
      </p>
    </div>
  )
}

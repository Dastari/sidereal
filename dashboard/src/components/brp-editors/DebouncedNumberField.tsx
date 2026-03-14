import * as React from 'react'
import { TheGridNumberInput } from '@/components/thegridcn/thegrid-number-input'
import { TheGridSlider } from '@/components/thegridcn/thegrid-slider'

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

type DebouncedNumberFieldProps = {
  label: string
  value: number
  min: number
  max: number
  step: number
  readOnly: boolean
  onChange: (next: number) => void
  debounceMs?: number
  inputClassName?: string
  wrapperClassName?: string
}

export function DebouncedNumberField({
  label,
  value,
  min,
  max,
  step,
  readOnly,
  onChange,
  debounceMs = 180,
  inputClassName = 'w-20 text-right font-mono text-xs',
  wrapperClassName = 'flex items-center gap-2',
}: DebouncedNumberFieldProps) {
  const safe = Number.isFinite(value) ? clamp(value, min, max) : min
  const [sliderValue, setSliderValue] = React.useState(safe)
  const timerRef = React.useRef<number | null>(null)

  React.useEffect(() => {
    setSliderValue(safe)
  }, [safe])

  React.useEffect(() => {
    return () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current)
      }
    }
  }, [])

  const scheduleCommit = React.useCallback(
    (next: number) => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current)
      }
      timerRef.current = window.setTimeout(() => {
        onChange(clamp(next, min, max))
      }, debounceMs)
    },
    [debounceMs, max, min, onChange],
  )

  const commitNow = React.useCallback(
    (next: number) => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current)
      }
      onChange(clamp(next, min, max))
    },
    [max, min, onChange],
  )

  return (
    <div className="space-y-1">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className={wrapperClassName}>
        <TheGridSlider
          value={sliderValue}
          min={min}
          max={max}
          step={step}
          disabled={readOnly}
          onChange={(next) => {
            const clamped = clamp(next, min, max)
            setSliderValue(clamped)
            scheduleCommit(clamped)
          }}
          className="flex-1"
        />
        <TheGridNumberInput
          value={sliderValue}
          min={min}
          max={max}
          step={step}
          readOnly={readOnly}
          onChange={(next) => {
            const clamped = clamp(next, min, max)
            setSliderValue(clamped)
            commitNow(clamped)
          }}
          className="shrink-0"
          inputClassName={inputClassName}
        />
      </div>
    </div>
  )
}

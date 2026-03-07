import * as React from 'react'
import { Input } from '@/components/ui/input'
import { Slider } from '@/components/ui/slider'

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function decimalsFromStep(step: number): number {
  if (!Number.isFinite(step) || step <= 0) return 0
  const normalized = step.toString().toLowerCase()
  if (normalized.includes('e-')) {
    const [, exponent] = normalized.split('e-')
    return Number.parseInt(exponent ?? '0', 10) || 0
  }
  const decimal = normalized.split('.')[1]
  return decimal?.length ?? 0
}

function formatForInput(value: number, step: number): string {
  const decimals = decimalsFromStep(step)
  if (decimals === 0) {
    return String(Math.round(value))
  }
  return value.toFixed(decimals).replace(/\.?0+$/, '')
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
  const [inputValue, setInputValue] = React.useState(formatForInput(safe, step))
  const timerRef = React.useRef<number | null>(null)

  React.useEffect(() => {
    setSliderValue(safe)
    setInputValue(formatForInput(safe, step))
  }, [safe, step])

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
        <Slider
          value={[sliderValue]}
          min={min}
          max={max}
          step={step}
          disabled={readOnly}
          onValueChange={(values) => {
            const v = values[0]
            if (typeof v !== 'number') return
            const clamped = clamp(v, min, max)
            setSliderValue(clamped)
            setInputValue(String(clamped))
            scheduleCommit(clamped)
          }}
          className="flex-1"
        />
        <Input
          type="number"
          value={inputValue}
          min={min}
          max={max}
          step={step}
          readOnly={readOnly}
          onChange={(event) => {
            const raw = event.target.value
            setInputValue(raw)
            const next = Number.parseFloat(raw)
            if (Number.isFinite(next)) {
              setSliderValue(clamp(next, min, max))
            }
          }}
          onBlur={() => {
            const next = Number.parseFloat(inputValue)
            if (Number.isFinite(next)) {
              const clamped = clamp(next, min, max)
              setSliderValue(clamped)
              setInputValue(formatForInput(clamped, step))
              commitNow(clamped)
            } else {
              setInputValue(formatForInput(sliderValue, step))
            }
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.currentTarget.blur()
            }
          }}
          className={inputClassName}
          aria-label={`${label} value`}
        />
      </div>
    </div>
  )
}

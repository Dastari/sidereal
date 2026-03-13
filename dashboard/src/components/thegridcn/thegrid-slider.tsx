import * as React from 'react'
import { cn } from '@/lib/utils'

export interface TheGridSliderProps
  extends Omit<React.HTMLAttributes<HTMLDivElement>, 'onChange'> {
  value?: number
  defaultValue?: number
  min?: number
  max?: number
  step?: number
  onChange?: (value: number) => void
  onCommit?: (value: number) => void
  label?: string
  showValue?: boolean
  disabled?: boolean
}

export function TheGridSlider({
  value: controlledValue,
  defaultValue = 0,
  min = 0,
  max = 100,
  step = 1,
  onChange,
  onCommit,
  label,
  showValue = false,
  disabled = false,
  className,
  ...props
}: TheGridSliderProps) {
  const [internalValue, setInternalValue] = React.useState(defaultValue)
  const current = controlledValue ?? internalValue
  const percent = max <= min ? 0 : ((current - min) / (max - min)) * 100

  function handleChange(event: React.ChangeEvent<HTMLInputElement>) {
    const next = Number(event.target.value)
    if (controlledValue === undefined) {
      setInternalValue(next)
    }
    onChange?.(next)
  }

  function handleCommit(event: React.SyntheticEvent<HTMLInputElement>) {
    onCommit?.(Number(event.currentTarget.value))
  }

  return (
    <div
      data-slot="tron-slider"
      className={cn(
        'thegrid-slider space-y-1.5',
        disabled && 'opacity-40',
        className,
      )}
      {...props}
    >
      {(label || showValue) && (
        <div className="flex items-center justify-between">
          {label ? (
            <span className="thegrid-slider__label font-mono text-[9px] uppercase tracking-widest">
              {label}
            </span>
          ) : null}
          {showValue ? (
            <span className="thegrid-slider__value font-mono text-[10px] tabular-nums">
              {current}
            </span>
          ) : null}
        </div>
      )}

      <div className="thegrid-slider__rail relative flex h-5 items-center">
        <div className="thegrid-slider__track absolute h-1 w-full" />
        <div
          className="thegrid-slider__range absolute h-1"
          style={{ width: `${Math.max(0, Math.min(100, percent))}%` }}
        />
        {[0, 25, 50, 75, 100].map((tick) => (
          <div
            key={tick}
            className={cn(
              'thegrid-slider__tick absolute top-1/2 h-2 w-px -translate-y-1/2',
              tick <= percent
                ? 'thegrid-slider__tick--active'
                : 'thegrid-slider__tick--idle',
            )}
            style={{ left: `${tick}%` }}
          />
        ))}
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={current}
          disabled={disabled}
          onChange={handleChange}
          onMouseUp={handleCommit}
          onTouchEnd={handleCommit}
          onKeyUp={(event) => {
            if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
              handleCommit(event)
            }
          }}
          className="absolute inset-0 cursor-pointer opacity-0"
        />
        <div
          className="thegrid-slider__thumb pointer-events-none absolute h-3.5 w-3.5 -translate-x-1/2"
          style={{ left: `${Math.max(0, Math.min(100, percent))}%` }}
        />
      </div>
    </div>
  )
}

import * as React from 'react'
import { cn } from '@/lib/utils'

export interface TheGridNumberInputProps
  extends Omit<React.HTMLAttributes<HTMLDivElement>, 'onChange'> {
  value?: number
  defaultValue?: number
  min?: number
  max?: number
  step?: number
  onChange?: (value: number) => void
  label?: string
  disabled?: boolean
  readOnly?: boolean
  inputClassName?: string
}

export function TheGridNumberInput({
  value: controlledValue,
  defaultValue = 0,
  min = Number.NEGATIVE_INFINITY,
  max = Number.POSITIVE_INFINITY,
  step = 1,
  onChange,
  label,
  disabled = false,
  readOnly = false,
  className,
  inputClassName,
  ...props
}: TheGridNumberInputProps) {
  const [internalValue, setInternalValue] = React.useState(defaultValue)
  const current = controlledValue ?? internalValue

  const update = React.useCallback(
    (next: number) => {
      const clamped = Math.min(max, Math.max(min, next))
      if (controlledValue === undefined) {
        setInternalValue(clamped)
      }
      onChange?.(clamped)
    },
    [controlledValue, max, min, onChange],
  )

  return (
    <div
      data-slot="tron-number-input"
      className={cn(
        'thegrid-number-input space-y-1',
        disabled && 'opacity-40',
        className,
      )}
      {...props}
    >
      {label ? (
        <span className="thegrid-number-input__label block font-mono text-[9px] uppercase tracking-widest">
          {label}
        </span>
      ) : null}

      <div className="thegrid-number-input__frame inline-flex items-stretch">
        <button
          type="button"
          disabled={disabled || readOnly || current <= min}
          onClick={() => update(current - step)}
          className="thegrid-number-input__button thegrid-number-input__button--decrement flex w-8 items-center justify-center"
        >
          <svg width="8" height="2" viewBox="0 0 8 2" fill="none">
            <path
              d="M0 1h8"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="square"
            />
          </svg>
        </button>

        <input
          type="number"
          inputMode="decimal"
          value={Number.isFinite(current) ? current : 0}
          min={Number.isFinite(min) ? min : undefined}
          max={Number.isFinite(max) ? max : undefined}
          step={step}
          disabled={disabled}
          readOnly={readOnly}
          onChange={(event) => {
            const next = Number(event.target.value)
            if (Number.isFinite(next)) {
              update(next)
            }
          }}
          className={cn(
            'thegrid-number-input__input w-20 bg-transparent py-1.5 text-center font-mono text-xs tabular-nums outline-none',
            inputClassName,
          )}
        />

        <button
          type="button"
          disabled={disabled || readOnly || current >= max}
          onClick={() => update(current + step)}
          className="thegrid-number-input__button thegrid-number-input__button--increment flex w-8 items-center justify-center"
        >
          <svg width="8" height="8" viewBox="0 0 8 8" fill="none">
            <path
              d="M0 4h8M4 0v8"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="square"
            />
          </svg>
        </button>
      </div>
    </div>
  )
}

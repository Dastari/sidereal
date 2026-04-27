import * as React from 'react'
import { cn } from '@/lib/utils'

type TotpCodeInputProps = {
  id?: string
  value: string
  onChange: (value: string) => void
  onComplete?: (value: string) => void
  disabled?: boolean
  className?: string
  'aria-label'?: string
}

const CODE_LENGTH = 6

export function TotpCodeInput({
  id,
  value,
  onChange,
  onComplete,
  disabled = false,
  className,
  'aria-label': ariaLabel = 'Authenticator code',
}: TotpCodeInputProps) {
  const refs = React.useRef<Array<HTMLInputElement | null>>([])
  const digits = React.useMemo(() => normalizeCode(value).split(''), [value])

  const setCode = React.useCallback(
    (next: string) => {
      onChange(normalizeCode(next))
    },
    [onChange],
  )

  const focusIndex = React.useCallback((index: number) => {
    refs.current[Math.max(0, Math.min(CODE_LENGTH - 1, index))]?.focus()
  }, [])

  const handleInput = React.useCallback(
    (index: number, rawValue: string) => {
      const nextDigits = [...digits]
      const pasted = normalizeCode(rawValue)

      if (pasted.length > 1) {
        for (let offset = 0; offset < pasted.length; offset += 1) {
          const target = index + offset
          if (target < CODE_LENGTH) nextDigits[target] = pasted[offset] ?? ''
        }
        const nextCode = normalizeCode(nextDigits.join(''))
        setCode(nextCode)
        focusIndex(Math.min(index + pasted.length, CODE_LENGTH - 1))
        if (nextCode.length === CODE_LENGTH) onComplete?.(nextCode)
        return
      }

      nextDigits[index] = pasted
      const nextCode = normalizeCode(nextDigits.join(''))
      setCode(nextCode)
      if (pasted && index < CODE_LENGTH - 1) {
        focusIndex(index + 1)
      }
      if (nextCode.length === CODE_LENGTH) onComplete?.(nextCode)
    },
    [digits, focusIndex, onComplete, setCode],
  )

  const handleKeyDown = React.useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>, index: number) => {
      if (event.key === 'Backspace' && !digits[index] && index > 0) {
        event.preventDefault()
        const nextDigits = [...digits]
        nextDigits[index - 1] = ''
        setCode(nextDigits.join(''))
        focusIndex(index - 1)
        return
      }
      if (event.key === 'ArrowLeft') {
        event.preventDefault()
        focusIndex(index - 1)
        return
      }
      if (event.key === 'ArrowRight') {
        event.preventDefault()
        focusIndex(index + 1)
        return
      }
      if (
        event.key === 'Enter' &&
        normalizeCode(value).length === CODE_LENGTH
      ) {
        event.preventDefault()
        onComplete?.(normalizeCode(value))
      }
    },
    [digits, focusIndex, onComplete, setCode, value],
  )

  return (
    <div
      id={id}
      className={cn(
        'grid grid-cols-6 gap-2 sm:gap-3',
        disabled && 'opacity-60',
        className,
      )}
      role="group"
      aria-label={ariaLabel}
    >
      {Array.from({ length: CODE_LENGTH }, (_, index) => (
        <input
          key={index}
          ref={(node) => {
            refs.current[index] = node
          }}
          type="text"
          inputMode="numeric"
          pattern="[0-9]*"
          autoComplete={index === 0 ? 'one-time-code' : 'off'}
          aria-label={`${ariaLabel} digit ${index + 1}`}
          disabled={disabled}
          value={digits[index] ?? ''}
          onChange={(event) => handleInput(index, event.target.value)}
          onKeyDown={(event) => handleKeyDown(event, index)}
          onFocus={(event) => event.currentTarget.select()}
          className={cn(
            'grid-input h-12 min-w-0 border border-input bg-background/70 text-center font-mono text-xl font-semibold text-foreground shadow-xs outline-none transition-[background-color,border-color,box-shadow,color]',
            'focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50',
            'disabled:pointer-events-none disabled:cursor-not-allowed',
          )}
        />
      ))}
    </div>
  )
}

export function normalizeCode(value: string) {
  return value.replace(/\D/g, '').slice(0, CODE_LENGTH)
}

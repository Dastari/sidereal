import * as React from 'react'
import { cn } from '@/lib/utils'

export interface ProgressBarProps
  extends React.HTMLAttributes<HTMLDivElement> {
  value?: number
  max?: number
  indeterminate?: boolean
  variant?: 'default' | 'success' | 'warning' | 'danger'
}

export function ProgressBar({
  value = 0,
  max = 100,
  indeterminate = false,
  variant = 'default',
  className,
  ...props
}: ProgressBarProps) {
  const pct = Math.min(Math.max((value / max) * 100, 0), 100)

  return (
    <div
      data-slot="grid-progress-bar"
      data-variant={variant}
      data-indeterminate={indeterminate ? 'true' : 'false'}
      className={cn('grid-progress relative overflow-hidden', className)}
      {...props}
    >
      <div className="grid-progress__track absolute inset-0" />
      <div
        className={cn(
          'grid-progress__indicator absolute inset-y-0 left-0',
          indeterminate && 'grid-progress__indicator--indeterminate',
        )}
        style={indeterminate ? undefined : { width: `${pct}%` }}
      >
        <div className="grid-progress__indicator-glow absolute inset-0" />
        <div className="grid-progress__indicator-scan absolute inset-0" />
      </div>
      <div className="grid-progress__scanline pointer-events-none absolute inset-0" />
    </div>
  )
}

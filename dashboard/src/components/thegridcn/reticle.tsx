import * as React from 'react'
import { cn } from '@/lib/utils'

export interface ReticleProps extends React.HTMLAttributes<HTMLDivElement> {
  size?: number
  animated?: boolean
  variant?: 'default' | 'locked' | 'scanning'
}

export function Reticle({
  size = 72,
  animated = true,
  variant = 'default',
  className,
  style,
  ...props
}: ReticleProps) {
  return (
    <div
      data-slot="grid-reticle"
      data-variant={variant}
      className={cn(
        'grid-reticle relative text-primary',
        animated && 'grid-reticle--animated',
        className,
      )}
      style={{ width: size, height: size, ...style }}
      {...props}
    >
      <svg viewBox="0 0 100 100" className="h-full w-full">
        <circle
          cx="50"
          cy="50"
          r="43"
          fill="none"
          className="grid-reticle__ring grid-reticle__ring--outer"
          strokeWidth="1"
        />
        <circle
          cx="50"
          cy="50"
          r="24"
          fill="none"
          className="grid-reticle__ring grid-reticle__ring--inner"
          strokeWidth="1"
        />
        <circle cx="50" cy="50" r="2.5" className="grid-reticle__dot" />
        <line
          x1="50"
          y1="4"
          x2="50"
          y2="19"
          className="grid-reticle__line"
          strokeWidth="1.5"
        />
        <line
          x1="50"
          y1="81"
          x2="50"
          y2="96"
          className="grid-reticle__line"
          strokeWidth="1.5"
        />
        <line
          x1="4"
          y1="50"
          x2="19"
          y2="50"
          className="grid-reticle__line"
          strokeWidth="1.5"
        />
        <line
          x1="81"
          y1="50"
          x2="96"
          y2="50"
          className="grid-reticle__line"
          strokeWidth="1.5"
        />
        <path
          d="M 18 26 L 18 18 L 26 18"
          fill="none"
          className="grid-reticle__bracket"
          strokeWidth="2"
        />
        <path
          d="M 74 18 L 82 18 L 82 26"
          fill="none"
          className="grid-reticle__bracket"
          strokeWidth="2"
        />
        <path
          d="M 82 74 L 82 82 L 74 82"
          fill="none"
          className="grid-reticle__bracket"
          strokeWidth="2"
        />
        <path
          d="M 26 82 L 18 82 L 18 74"
          fill="none"
          className="grid-reticle__bracket"
          strokeWidth="2"
        />
      </svg>
      {animated && variant === 'scanning' ? (
        <svg
          viewBox="0 0 100 100"
          className="grid-reticle__scanner absolute inset-0 h-full w-full"
        >
          <circle
            cx="50"
            cy="50"
            r="38"
            fill="none"
            className="grid-reticle__scanner-ring"
            strokeWidth="2"
            strokeDasharray="20 56"
          />
        </svg>
      ) : null}
    </div>
  )
}

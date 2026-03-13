import * as React from 'react'
import { cn } from '@/lib/utils'

export interface LocationDisplayProps
  extends React.HTMLAttributes<HTMLDivElement> {
  sector?: string
  grid?: string
  coordinates?: string
  status?: string
}

export function LocationDisplay({
  sector = 'SECTOR 7G',
  grid = 'GRID 12-A',
  coordinates = 'X: 847.23 Y: 129.45',
  status = 'ACTIVE',
  className,
  ...props
}: LocationDisplayProps) {
  return (
    <div
      className={cn(
        'grid-location-display font-mono text-[10px] tracking-widest',
        className,
      )}
      {...props}
    >
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
        <div className="flex items-center gap-2">
          <div className="grid-location-display__beacon h-1.5 w-1.5 animate-pulse rounded-full" />
          <span className="grid-location-display__primary">{sector}</span>
        </div>
        <span className="grid-location-display__separator">|</span>
        <span className="grid-location-display__text">{grid}</span>
        <span className="grid-location-display__separator">|</span>
        <span className="grid-location-display__text">{coordinates}</span>
        <span className="grid-location-display__separator">|</span>
        <span className="grid-location-display__status">{status}</span>
      </div>
    </div>
  )
}

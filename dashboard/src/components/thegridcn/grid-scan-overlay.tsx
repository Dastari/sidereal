import * as React from 'react'
import { cn } from '@/lib/utils'

export interface GridScanOverlayProps
  extends React.HTMLAttributes<HTMLDivElement> {
  gridSize?: number
  scanSpeed?: number
}

export function GridScanOverlay({
  gridSize = 100,
  scanSpeed = 8,
  className,
  ...props
}: GridScanOverlayProps) {
  return (
    <div
      className={cn(
        'grid-scan-overlay pointer-events-none absolute inset-0 overflow-hidden',
        className,
      )}
      {...props}
    >
      <div
        className="grid-scan-overlay__scanlines absolute inset-0"
      />
      <div
        className="grid-scan-overlay__grid absolute inset-0"
        style={{
          backgroundSize: `${gridSize}px ${gridSize}px`,
        }}
      />
      <div
        className="grid-scan-overlay__line absolute left-0 h-px w-full"
        style={{
          animationDuration: `${scanSpeed}s`,
        }}
      />
    </div>
  )
}

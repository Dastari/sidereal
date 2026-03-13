import * as React from 'react'
import { cn } from '@/lib/utils'

export const hudFrameClassName =
  'grid-hud-frame relative isolate overflow-hidden border border-border/80 bg-card/70 text-card-foreground'

interface HUDFrameProps extends React.HTMLAttributes<HTMLDivElement> {
  label?: string
}

const HUDFrame = React.forwardRef<HTMLDivElement, HUDFrameProps>(
  ({ className, label, children, ...props }, ref) => (
    <div
      ref={ref}
      data-slot="grid-hud-frame"
      className={cn(hudFrameClassName, className)}
      {...props}
    >
      <div
        aria-hidden="true"
        className="grid-hud-frame__overlay pointer-events-none absolute inset-0"
      />
      <div
        aria-hidden="true"
        className="grid-hud-frame__corner pointer-events-none absolute -left-px -top-px h-4 w-4 border-l-2 border-t-2 border-primary/80"
      />
      <div
        aria-hidden="true"
        className="grid-hud-frame__corner pointer-events-none absolute -right-px -top-px h-4 w-4 border-r-2 border-t-2 border-primary/80"
      />
      <div
        aria-hidden="true"
        className="grid-hud-frame__corner pointer-events-none absolute -bottom-px -left-px h-4 w-4 border-b-2 border-l-2 border-primary/80"
      />
      <div
        aria-hidden="true"
        className="grid-hud-frame__corner pointer-events-none absolute -bottom-px -right-px h-4 w-4 border-b-2 border-r-2 border-primary/80"
      />
      {label ? (
        <div className="grid-hud-frame__label absolute left-4 top-0 z-[1] -translate-y-1/2 bg-background px-2 text-[10px] uppercase tracking-[0.28em] text-primary/90">
          {label}
        </div>
      ) : null}
      {children}
    </div>
  ),
)

HUDFrame.displayName = 'HUDFrame'

export { HUDFrame }

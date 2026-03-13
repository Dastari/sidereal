import * as React from 'react'
import { cva } from 'class-variance-authority'
import type { VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const alertVariants = cva(
  'grid-panel relative w-full rounded-xl border px-4 py-3 text-sm',
  {
    variants: {
      variant: {
        default: 'border-border/80 bg-card/80 text-foreground',
        info:
          'border-primary/45 bg-[color:color-mix(in_oklch,var(--primary)_10%,transparent)] text-foreground',
        warning:
          'border-warning/45 bg-[color:color-mix(in_oklch,var(--color-warning)_14%,transparent)] text-warning',
        destructive:
          'border-destructive/50 bg-[color:color-mix(in_oklch,var(--destructive)_14%,transparent)] text-destructive',
        success:
          'border-success/45 bg-[color:color-mix(in_oklch,var(--color-success)_14%,transparent)] text-success',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  },
)

const Alert = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & VariantProps<typeof alertVariants>
>(({ className, variant, ...props }, ref) => (
  <div
    ref={ref}
    role="alert"
    className={cn(alertVariants({ variant }), className)}
    {...props}
  />
))
Alert.displayName = 'Alert'

const AlertTitle = React.forwardRef<
  HTMLParagraphElement,
  React.HTMLAttributes<HTMLHeadingElement>
>(({ className, ...props }, ref) => (
  <h5
    ref={ref}
    className={cn('mb-1 font-medium leading-none', className)}
    {...props}
  />
))
AlertTitle.displayName = 'AlertTitle'

const AlertDescription = React.forwardRef<
  HTMLParagraphElement,
  React.HTMLAttributes<HTMLParagraphElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn('text-sm text-current/85 [&_p]:leading-relaxed', className)}
    {...props}
  />
))
AlertDescription.displayName = 'AlertDescription'

export { Alert, AlertDescription, AlertTitle }

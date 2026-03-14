import * as React from 'react'
import { cn } from '@/lib/utils'

const SidebarNavFrame = React.forwardRef<
  HTMLElement,
  React.ComponentPropsWithoutRef<'nav'>
>(({ className, children, ...props }, ref) => (
  <nav
    ref={ref}
    data-slot="grid-sidebar-nav"
    aria-label="Sidebar navigation"
    className={cn(
      'grid-sidebar-nav relative isolate flex h-full flex-col overflow-hidden border border-sidebar-border/80 bg-sidebar/80 text-sidebar-foreground backdrop-blur-sm',
      className,
    )}
    {...props}
  >
    <div
      aria-hidden="true"
      className="grid-sidebar-nav__scanline pointer-events-none absolute inset-0"
    />
    <div
      aria-hidden="true"
      className="pointer-events-none absolute left-0 top-0 h-3 w-3 border-l-2 border-t-2 border-primary/50 z-2"
    />
    <div
      aria-hidden="true"
      className="pointer-events-none absolute right-0 top-0 h-3 w-3 border-r-2 border-t-2 border-primary/50 z-2"
    />
    <div
      aria-hidden="true"
      className="pointer-events-none absolute bottom-0 left-0 h-3 w-3 border-b-2 border-l-2 border-primary/50 z-2"
    />
    <div
      aria-hidden="true"
      className="pointer-events-none absolute bottom-0 right-0 h-3 w-3 border-b-2 border-r-2 border-primary/50 z-2"
    />
    {children}
  </nav>
))

SidebarNavFrame.displayName = 'SidebarNavFrame'

const SidebarNavHeader = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    data-slot="grid-sidebar-nav-header"
    className={cn(
      'grid-sidebar-nav__header relative z-[1] border-b border-sidebar-border/60 px-4 py-3',
      className,
    )}
    {...props}
  />
))

SidebarNavHeader.displayName = 'SidebarNavHeader'

const SidebarNavBody = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    data-slot="grid-sidebar-nav-body"
    className={cn('relative z-[1] min-h-0 flex-1 overflow-hidden', className)}
    {...props}
  />
))

SidebarNavBody.displayName = 'SidebarNavBody'

const SidebarNavFooter = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    data-slot="grid-sidebar-nav-footer"
    className={cn(
      'grid-sidebar-nav__footer relative z-[1] border-t border-sidebar-border/60',
      className,
    )}
    {...props}
  />
))

SidebarNavFooter.displayName = 'SidebarNavFooter'

export { SidebarNavFrame, SidebarNavHeader, SidebarNavBody, SidebarNavFooter }

import { Link, Outlet, useLocation } from '@tanstack/react-router'
import {
  Database,
  FileCode2,
  Gauge,
  Gamepad2,
  Orbit,
  Settings,
  Sparkles,
} from 'lucide-react'
import type { ComponentType } from 'react'
import { DashboardAdminAccess } from '@/components/layout/DashboardAdminAccess'
import { NavigationProgressBar } from '@/components/layout/NavigationProgressBar'
import { ThemeToggle } from '@/components/ThemeToggle'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'

export type ToolNavItem = {
  to: string
  label: string
  description: string
  icon: ComponentType<{ className?: string }>
}

export const toolNavItems: Array<ToolNavItem> = [
  {
    to: '/',
    label: 'Dashboard',
    description: 'Service health, counters, and endpoint status.',
    icon: Gauge,
  },
  {
    to: '/database',
    label: 'Database',
    description: 'Persisted entities, accounts, and graph-backed data tools.',
    icon: Database,
  },
  {
    to: '/game-world',
    label: 'Game World',
    description: 'Live BRP entity explorer, spawning, and diagnostics.',
    icon: Orbit,
  },
  {
    to: '/game-client',
    label: 'Game Client',
    description: 'Dashboard host surface for the browser WASM client runtime.',
    icon: Gamepad2,
  },
  {
    to: '/shader-workshop',
    label: 'Shader Workshop',
    description: 'WGSL authoring, preview, diagnostics, and asset metadata.',
    icon: Sparkles,
  },
  {
    to: '/script-editor',
    label: 'Script Editor',
    description: 'Reserved for future in-game script authoring.',
    icon: FileCode2,
  },
  {
    to: '/settings',
    label: 'Settings',
    description: 'Dashboard configuration and future environment settings.',
    icon: Settings,
  },
]

export function getActiveTool(pathname: string): ToolNavItem {
  return (
    toolNavItems.find((item) =>
      item.to === '/'
        ? pathname === '/'
        : pathname === item.to || pathname.startsWith(`${item.to}/`),
    ) ?? toolNavItems[0]
  )
}

export function DashboardShell() {
  const location = useLocation()
  const activeTool = getActiveTool(location.pathname)

  return (
    <div className="flex min-h-screen flex-col overflow-hidden bg-[radial-gradient(circle_at_top,_rgba(96,165,250,0.10),_transparent_28%),linear-gradient(180deg,_rgba(4,8,18,1),_rgba(6,10,18,1))] text-foreground">
      <header className="relative flex h-15 items-center gap-4 border-b border-border/80 bg-background px-5 backdrop-blur">
        <div className="min-w-0">
          <div className="text-[11px] font-semibold uppercase tracking-[0.22em] text-primary/85">
            Sidereal Control Surface
          </div>
        </div>
        <div className="grow" />
        <DashboardAdminAccess />
        <ThemeToggle />
        <NavigationProgressBar />
      </header>

      <div className="flex min-h-0 flex-1">
        <aside className="flex w-18 shrink-0 flex-col items-center gap-2 border-r border-border/80 bg-background px-2 py-3 backdrop-blur">
          {toolNavItems.map((item) => {
            const Icon = item.icon
            const active =
              item.to === '/'
                ? location.pathname === '/'
                : location.pathname === item.to ||
                  location.pathname.startsWith(`${item.to}/`)
            return (
              <Tooltip key={item.to}>
                <TooltipTrigger asChild>
                  <Button
                    asChild
                    variant={active ? 'secondary' : 'ghost'}
                    size="icon"
                    className={cn(
                      'h-12 w-12 rounded-xl border border-transparent',
                      active &&
                        'border-primary/40 bg-primary/15 text-primary shadow-[0_0_22px_rgba(96,165,250,0.22)]',
                    )}
                  >
                    <Link to={item.to} aria-label={item.label}>
                      <Icon className="h-5 w-5" />
                    </Link>
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="right">
                  <div className="font-medium">{item.label}</div>
                  <div className="max-w-56 text-xs text-muted-foreground">
                    {item.description}
                  </div>
                </TooltipContent>
              </Tooltip>
            )
          })}
        </aside>

        <main className="min-h-0 flex-1 overflow-hidden">
          <Outlet />
        </main>
      </div>
    </div>
  )
}

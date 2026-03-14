import { Link, Outlet, useLocation } from '@tanstack/react-router'
import {
  Database,
  FileCode2,
  Gamepad2,
  Gauge,
  Orbit,
  Settings,
  Sparkles,
  Volume2,
} from 'lucide-react'
import type { ComponentType } from 'react'
import { DashboardAdminAccess } from '@/components/layout/DashboardAdminAccess'
import { NavigationProgressBar } from '@/components/layout/NavigationProgressBar'
import { ThemeToggle } from '@/components/ThemeToggle'
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
    to: '/sound-studio',
    label: 'Sound Studio',
    description:
      'Audio registry browsing, waveform preview, and cue marker editing.',
    icon: Volume2,
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

const appVersion = import.meta.env.VITE_APP_VERSION ?? '0.0.0'

export function DashboardShell() {
  const location = useLocation()
  const activeTool = getActiveTool(location.pathname)

  return (
    <div className="grid-shell flex min-h-screen flex-col overflow-hidden text-foreground">
      <header className="grid-header relative flex h-15 items-center gap-4 border-b px-5">
        <div className="min-w-0">
          <div className="grid-title grid-text-glow text-[11px] font-semibold text-primary/90">
            Sidereal Control Surface
          </div>
          <div className="mt-0.5 text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
            {activeTool.label} / Package v{appVersion}
          </div>
        </div>
        <div className="grow" />
        <DashboardAdminAccess />
        <ThemeToggle />
        <NavigationProgressBar />
      </header>

      <div className="flex min-h-0 flex-1">
        <aside className="grid-sidebar-rail flex w-18 shrink-0 flex-col items-center gap-2 border-r px-2 py-2">
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
                      'h-10 w-10 border-none shadow-none!',
                      active &&
                        'bg-primary/18 text-primary shadow-[0_0_22px_color-mix(in_oklch,var(--glow)_42%,transparent)]',
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

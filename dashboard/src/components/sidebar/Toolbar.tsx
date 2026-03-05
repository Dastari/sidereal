import { Home, Maximize2, Minimize2, Plus, ZoomIn, ZoomOut } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { ThemeToggle } from '@/components/ThemeToggle'

export type DataSourceMode = 'database' | 'liveServer' | 'liveClient'

export type BrpTab = {
  id: string
  label: string
  port: number
  kind: 'server' | 'client'
}

interface ToolbarProps {
  onZoomIn: () => void
  onZoomOut: () => void
  onFitAll: () => void
  onResetView: () => void
  onCollapseAll?: () => void
  sourceMode: DataSourceMode
  onSourceModeChange: (mode: DataSourceMode) => void
  brpTabs: Array<BrpTab>
  activeBrpTabId: string
  onActiveBrpTabIdChange: (tabId: string) => void
  onAddClientTab: () => void
}

export function Toolbar({
  onZoomIn,
  onZoomOut,
  onFitAll,
  onResetView,
  onCollapseAll,
  sourceMode,
  onSourceModeChange,
  brpTabs,
  activeBrpTabId,
  onActiveBrpTabIdChange,
  onAddClientTab,
}: ToolbarProps) {
  const tabValue =
    sourceMode === 'database' ? 'database' : `live:${activeBrpTabId}`

  return (
    <div className="flex items-center gap-1 px-4 py-2">
      <div className="flex items-center gap-1">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" onClick={onZoomIn}>
              <ZoomIn className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Zoom in</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" onClick={onZoomOut}>
              <ZoomOut className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Zoom out</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" onClick={onFitAll}>
              <Maximize2 className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Fit all entities</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" onClick={onResetView}>
              <Home className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Reset view</TooltipContent>
        </Tooltip>

        {onCollapseAll && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon" onClick={onCollapseAll}>
                <Minimize2 className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Collapse all expanded nodes</TooltipContent>
          </Tooltip>
        )}
      </div>

      <Separator orientation="vertical" className="h-6 mx-2" />

      <Tabs
        value={tabValue}
        onValueChange={(value) => {
          if (value === 'database') {
            onSourceModeChange('database')
            return
          }
          if (!value.startsWith('live:')) return
          const tabId = value.slice('live:'.length)
          const tab = brpTabs.find((entry) => entry.id === tabId)
          if (!tab) return
          onActiveBrpTabIdChange(tabId)
          onSourceModeChange(tab.kind === 'server' ? 'liveServer' : 'liveClient')
        }}
      >
        <TabsList className="h-8 gap-1">
          <TabsTrigger value="database" className="h-6 px-3 text-xs">
            Database
          </TabsTrigger>
          {brpTabs.map((tab) => (
            <TabsTrigger
              key={tab.id}
              value={`live:${tab.id}`}
              className="h-6 px-3 text-xs"
            >
              {tab.label}
            </TabsTrigger>
          ))}
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-6 w-6"
                onClick={onAddClientTab}
                aria-label="Add client BRP tab"
              >
                <Plus className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Add client BRP tab</TooltipContent>
          </Tooltip>
        </TabsList>
      </Tabs>

      <div className="flex-1" />

      <ThemeToggle />
    </div>
  )
}

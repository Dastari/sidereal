import { Home, Maximize2, Minimize2, ZoomIn, ZoomOut } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { ThemeToggle } from '@/components/ThemeToggle'

export type DataSourceMode = 'database' | 'liveServer' | 'liveClient'

interface ToolbarProps {
  onZoomIn: () => void
  onZoomOut: () => void
  onFitAll: () => void
  onResetView: () => void
  onCollapseAll?: () => void
  sourceMode: DataSourceMode
  onSourceModeChange: (mode: DataSourceMode) => void
}

export function Toolbar({
  onZoomIn,
  onZoomOut,
  onFitAll,
  onResetView,
  onCollapseAll,
  sourceMode,
  onSourceModeChange,
}: ToolbarProps) {
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

      <div className="flex items-center rounded-md border border-border-subtle bg-muted/40 p-1 gap-1">
        <Button
          size="sm"
          variant={sourceMode === 'database' ? 'default' : 'ghost'}
          onClick={() => onSourceModeChange('database')}
          className="h-7 px-3 text-xs"
        >
          Database
        </Button>
        <Button
          size="sm"
          variant={sourceMode === 'liveServer' ? 'default' : 'ghost'}
          onClick={() => onSourceModeChange('liveServer')}
          className="h-7 px-3 text-xs"
        >
          Server BRP
        </Button>
        <Button
          size="sm"
          variant={sourceMode === 'liveClient' ? 'default' : 'ghost'}
          onClick={() => onSourceModeChange('liveClient')}
          className="h-7 px-3 text-xs"
        >
          Client BRP
        </Button>
      </div>

      <div className="flex-1" />

      <ThemeToggle />
    </div>
  )
}

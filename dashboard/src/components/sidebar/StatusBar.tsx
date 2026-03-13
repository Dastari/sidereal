import { Database, RefreshCw, Wifi, WifiOff } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'

interface StatusBarProps {
  sourceMode: 'database' | 'liveServer' | 'liveClient'
  liveSourceLabel?: string
  graphStatus: {
    connected: boolean
    nodeCount: number
    edgeCount: number
    graphName: string
  }
  worldStatus: {
    loaded: boolean
    entityCount: number
  }
  isRefreshing: boolean
  onRefresh: () => void
}

export function StatusBar({
  sourceMode,
  liveSourceLabel,
  graphStatus,
  worldStatus,
  isRefreshing,
  onRefresh,
}: StatusBarProps) {
  const liveMode = sourceMode === 'liveServer' || sourceMode === 'liveClient'
  const sourceLabel =
    sourceMode === 'liveClient'
        ? 'Client Bevy Remote'
        : 'Server Bevy Remote'
  const resolvedLiveSourceLabel = liveSourceLabel ?? sourceLabel

  return (
    <div className="grid-surface flex items-center justify-between border-t border-border/70 px-4 py-2 text-xs">
      <div className="flex items-center gap-4">
        {/* Graph status */}
        <Tooltip>
          <TooltipTrigger asChild>
            <div className="flex items-center gap-1.5 rounded-md border border-border/40 bg-background/35 px-2 py-1">
              {graphStatus.connected ? (
                <Wifi className="grid-dot h-3.5 w-3.5 text-success" />
              ) : (
                <WifiOff className="grid-dot h-3.5 w-3.5 text-destructive" />
              )}
              <span className="text-muted-foreground">
                {graphStatus.connected
                  ? graphStatus.graphName
                  : liveMode
                    ? 'BRP disconnected'
                    : 'Disconnected'}
              </span>
            </div>
          </TooltipTrigger>
          <TooltipContent>
            <p>
              {liveMode ? resolvedLiveSourceLabel : 'AGE Graph'}:{' '}
              {graphStatus.nodeCount} nodes, {graphStatus.edgeCount} edges
            </p>
          </TooltipContent>
        </Tooltip>

        {/* World status */}
        <Tooltip>
          <TooltipTrigger asChild>
            <div className="flex items-center gap-1.5 rounded-md border border-border/40 bg-background/35 px-2 py-1">
              <Database
                className={cn(
                  'grid-dot h-3.5 w-3.5',
                  worldStatus.loaded ? 'text-primary' : 'text-muted-foreground',
                )}
              />
              <span className="text-muted-foreground">
                {worldStatus.entityCount} entities
              </span>
            </div>
          </TooltipTrigger>
          <TooltipContent>
            <p>
              {liveMode
                ? `Live entities from ${resolvedLiveSourceLabel.toLowerCase()}`
                : 'Entities from AGE graph'}
            </p>
          </TooltipContent>
        </Tooltip>
      </div>

      {/* Refresh button */}
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            onClick={onRefresh}
            disabled={isRefreshing}
          >
            <RefreshCw
              className={cn('h-3.5 w-3.5', isRefreshing && 'animate-spin')}
            />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          <p>Refresh data</p>
        </TooltipContent>
      </Tooltip>
    </div>
  )
}

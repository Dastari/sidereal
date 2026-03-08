import { Plus } from 'lucide-react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'

export type DataSourceMode = 'database' | 'liveServer' | 'liveClient'

export type BrpTab = {
  id: string
  label: string
  port: number
  kind: 'server' | 'client'
}

interface ToolbarProps {
  sourceMode: DataSourceMode
  onSourceModeChange: (mode: DataSourceMode) => void
  brpTabs: Array<BrpTab>
  activeBrpTabId: string
  onActiveBrpTabIdChange: (tabId: string) => void
  onAddClientTab: () => void
  showDataSourceTabs?: boolean
  showDatabaseTab?: boolean
  children?: ReactNode
}

export function Toolbar({
  sourceMode,
  onSourceModeChange,
  brpTabs,
  activeBrpTabId,
  onActiveBrpTabIdChange,
  onAddClientTab,
  showDataSourceTabs = true,
  showDatabaseTab = true,
  children,
}: ToolbarProps) {
  if (!showDataSourceTabs && !children) {
    return null
  }

  const tabValue =
    showDatabaseTab && sourceMode === 'database'
      ? 'database'
      : `live:${activeBrpTabId}`

  return (
    <div className="flex flex-wrap items-center gap-2 bg-background px-4 py-2">
      {showDataSourceTabs ? (
        <>
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
              onSourceModeChange(
                tab.kind === 'server' ? 'liveServer' : 'liveClient',
              )
            }}
          >
            <TabsList className="h-8 gap-1">
              {showDatabaseTab ? (
                <TabsTrigger value="database" className="h-6 px-3 text-xs">
                  Database
                </TabsTrigger>
              ) : null}
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
        </>
      ) : null}
      {children}
    </div>
  )
}

import { Plus, X } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent, ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { ButtonGroup } from '@/components/ui/button-group'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
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
  host?: string
  port: number
  kind: 'server' | 'client'
}

export type NewBrpTabInput = {
  label: string
  host: string
  port: number
}

interface ToolbarProps {
  sourceMode: DataSourceMode
  onSourceModeChange: (mode: DataSourceMode) => void
  brpTabs: Array<BrpTab>
  activeBrpTabId: string
  onActiveBrpTabIdChange: (tabId: string) => void
  onAddClientTab: (tab: NewBrpTabInput) => void
  onCloseBrpTab?: (tabId: string) => void
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
  onCloseBrpTab,
  showDataSourceTabs = true,
  showDatabaseTab = true,
  children,
}: ToolbarProps) {
  const [addDialogOpen, setAddDialogOpen] = useState(false)
  const [newTabLabel, setNewTabLabel] = useState('')
  const [newTabHost, setNewTabHost] = useState('127.0.0.1')
  const [newTabPort, setNewTabPort] = useState('15714')
  const [formError, setFormError] = useState<string | null>(null)

  if (!showDataSourceTabs && !children) {
    return null
  }

  const tabValue =
    showDatabaseTab && sourceMode === 'database'
      ? 'database'
      : `live:${activeBrpTabId}`
  const handleSubmitNewTab = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const label = newTabLabel.trim()
    const host = newTabHost.trim()
    const port = Number.parseInt(newTabPort, 10)

    if (!label) {
      setFormError('Tab name is required')
      return
    }
    if (!/^[A-Za-z0-9.-]+$/.test(host)) {
      setFormError('IP address must be a host name or IP without a protocol')
      return
    }
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      setFormError('Port must be between 1 and 65535')
      return
    }

    onAddClientTab({ label, host, port })
    setFormError(null)
    setAddDialogOpen(false)
    setNewTabLabel('')
  }

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
            <ButtonGroup>
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
                    className="h-6 gap-2 px-3 text-xs"
                  >
                    {tab.label}
                    {tab.kind !== 'server' && onCloseBrpTab ? (
                      <span
                        role="button"
                        tabIndex={0}
                        className="inline-flex h-4 w-4 items-center justify-center text-muted-foreground hover:text-foreground"
                        aria-label={`Close ${tab.label}`}
                        onClick={(event) => {
                          event.preventDefault()
                          event.stopPropagation()
                          onCloseBrpTab(tab.id)
                        }}
                        onKeyDown={(event) => {
                          if (event.key !== 'Enter' && event.key !== ' ') return
                          event.preventDefault()
                          event.stopPropagation()
                          onCloseBrpTab(tab.id)
                        }}
                      >
                        <X className="h-3 w-3" />
                      </span>
                    ) : null}
                  </TabsTrigger>
                ))}
              </TabsList>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-8 w-8"
                    onClick={() => setAddDialogOpen(true)}
                    aria-label="Add client BRP tab"
                  >
                    <Plus className="h-3.5 w-3.5" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Add client BRP tab</TooltipContent>
              </Tooltip>
            </ButtonGroup>
          </Tabs>
          <Dialog open={addDialogOpen} onOpenChange={setAddDialogOpen}>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>ADD BRP TAB</DialogTitle>
                <DialogDescription>
                  Connect a live client BRP endpoint by host, port, and tab
                  name.
                </DialogDescription>
              </DialogHeader>
              <form onSubmit={handleSubmitNewTab} className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="brp-tab-name">Tab Name</Label>
                  <Input
                    id="brp-tab-name"
                    value={newTabLabel}
                    onChange={(event) => setNewTabLabel(event.target.value)}
                    placeholder="Client 1 BRP"
                  />
                </div>
                <div className="grid gap-4 sm:grid-cols-[1fr_8rem]">
                  <div className="space-y-2">
                    <Label htmlFor="brp-tab-host">IP Address</Label>
                    <Input
                      id="brp-tab-host"
                      value={newTabHost}
                      onChange={(event) => setNewTabHost(event.target.value)}
                      placeholder="127.0.0.1"
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="brp-tab-port">Port</Label>
                    <Input
                      id="brp-tab-port"
                      inputMode="numeric"
                      value={newTabPort}
                      onChange={(event) => setNewTabPort(event.target.value)}
                      placeholder="15714"
                    />
                  </div>
                </div>
                {formError ? (
                  <p className="text-sm text-destructive">{formError}</p>
                ) : null}
                <DialogFooter>
                  <Button
                    type="button"
                    variant="ghost"
                    onClick={() => setAddDialogOpen(false)}
                  >
                    Cancel
                  </Button>
                  <Button type="submit">Connect</Button>
                </DialogFooter>
              </form>
            </DialogContent>
          </Dialog>
        </>
      ) : null}
      {children}
    </div>
  )
}

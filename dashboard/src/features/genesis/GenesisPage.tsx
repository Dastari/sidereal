import * as React from 'react'
import { Dices, Globe2, Search } from 'lucide-react'
import type {
  GenesisPlanetCatalog,
  GenesisPlanetEntry,
} from '@/features/genesis/types'
import {
  AppLayout,
  Panel,
  PanelContent,
  PanelHeader,
} from '@/components/layout/AppLayout'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useSessionStorageNumber } from '@/hooks/use-session-storage-number'
import { apiGet } from '@/lib/api/client'

const DEFAULT_GENESIS_SIDEBAR_WIDTH = 320
const DEFAULT_GENESIS_DETAIL_WIDTH = 380

function bodyKindLabel(value: number | null): string {
  if (value === 1) return 'Star'
  if (value === 2) return 'Black Hole'
  return 'Planet'
}

function planetTypeLabel(value: number | null): string {
  switch (value) {
    case 1:
      return 'Desert'
    case 2:
      return 'Lava'
    case 3:
      return 'Ice'
    case 4:
      return 'Gas Giant'
    case 5:
      return 'Moon'
    default:
      return 'Terran'
  }
}

export function GenesisPage() {
  const [catalog, setCatalog] = React.useState<GenesisPlanetCatalog | null>(null)
  const [selectedPlanetId, setSelectedPlanetId] = React.useState<string | null>(null)
  const [search, setSearch] = React.useState('')
  const [errorText, setErrorText] = React.useState<string | null>(null)
  const [sidebarWidth, setSidebarWidth] = useSessionStorageNumber(
    'dashboard:genesis:sidebar-width',
    DEFAULT_GENESIS_SIDEBAR_WIDTH,
  )
  const [detailPanelWidth, setDetailPanelWidth] = useSessionStorageNumber(
    'dashboard:genesis:detail-panel-width',
    DEFAULT_GENESIS_DETAIL_WIDTH,
  )

  React.useEffect(() => {
    let cancelled = false
    apiGet<GenesisPlanetCatalog>('/api/genesis/planets')
      .then((nextCatalog) => {
        if (cancelled) return
        setCatalog(nextCatalog)
        const firstPlanetId = nextCatalog.entries.at(0)?.planetId ?? null
        setSelectedPlanetId((current) => current ?? firstPlanetId)
      })
      .catch((error: unknown) => {
        if (cancelled) return
        setErrorText(error instanceof Error ? error.message : 'Failed to load Genesis catalog.')
      })
    return () => {
      cancelled = true
    }
  }, [])

  const entries = catalog?.entries ?? []
  const filteredEntries = React.useMemo(() => {
    const query = search.trim().toLowerCase()
    if (!query) return entries
    return entries.filter((entry) =>
      `${entry.planetId} ${entry.displayName} ${entry.scriptPath} ${entry.tags.join(' ')}`
        .toLowerCase()
        .includes(query),
    )
  }, [entries, search])
  const selectedEntry =
    entries.find((entry) => entry.planetId === selectedPlanetId) ?? null

  return (
    <AppLayout
      sidebarWidth={sidebarWidth}
      detailPanelWidth={detailPanelWidth}
      onSidebarResize={setSidebarWidth}
      onDetailPanelResize={setDetailPanelWidth}
      header={
        <div className="flex items-center gap-4 px-5 py-3">
          <div className="min-w-0">
            <div className="font-display text-lg uppercase tracking-[0.22em] text-primary">
              Genesis
            </div>
            <div className="text-xs uppercase tracking-[0.16em] text-muted-foreground">
              Planet registry authoring, deterministic randomization, and Lua publishing.
            </div>
          </div>
          <div className="ml-auto flex flex-wrap items-center gap-2">
            <Badge variant="outline">{entries.length} bodies</Badge>
            {catalog?.registryHasDraft ? <Badge variant="secondary">Registry draft</Badge> : null}
          </div>
        </div>
      }
      sidebar={
        <Panel>
          <PanelHeader>
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <Globe2 className="h-4 w-4 text-primary" />
                <div className="text-[11px] uppercase tracking-[0.22em] text-primary/90">
                  Planet Library
                </div>
              </div>
              <div className="relative">
                <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder="Search planets, tags, scripts..."
                  className="pl-9"
                />
              </div>
            </div>
          </PanelHeader>
          <PanelContent>
            <ScrollArea className="h-full">
              <div className="space-y-1 p-2">
                {filteredEntries.map((entry) => (
                  <button
                    key={entry.planetId}
                    type="button"
                    onClick={() => setSelectedPlanetId(entry.planetId)}
                    className={`w-full border px-3 py-2 text-left text-sm transition ${
                      entry.planetId === selectedPlanetId
                        ? 'border-primary bg-primary/10 text-primary'
                        : 'border-border bg-background/40 text-foreground hover:border-primary/60'
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <span className="font-medium">{entry.displayName}</span>
                      {entry.hasDraft ? <Badge variant="secondary">Draft</Badge> : null}
                    </div>
                    <div className="mt-1 text-xs text-muted-foreground">
                      {entry.planetId} / {bodyKindLabel(entry.bodyKind)}
                    </div>
                  </button>
                ))}
                {filteredEntries.length === 0 ? (
                  <div className="p-4 text-sm text-muted-foreground">
                    No Genesis entries matched the current search.
                  </div>
                ) : null}
              </div>
            </ScrollArea>
          </PanelContent>
        </Panel>
      }
      detailPanel={<GenesisDetailPanel entry={selectedEntry} errorText={errorText} />}
    >
      <Panel>
        <PanelHeader>
          <div className="flex items-center gap-3">
            <Globe2 className="h-5 w-5 text-primary" />
            <div>
              <div className="text-sm font-semibold uppercase tracking-[0.18em] text-primary">
                Preview
              </div>
              <div className="text-xs text-muted-foreground">
                Shader-backed live preview and randomization controls land next.
              </div>
            </div>
          </div>
        </PanelHeader>
        <PanelContent>
          <div className="flex h-full min-h-96 items-center justify-center border border-border bg-background/40">
            <div className="max-w-md text-center">
              <Globe2 className="mx-auto h-14 w-14 text-primary/80" />
              <div className="mt-4 font-display text-xl uppercase tracking-[0.2em] text-primary">
                {selectedEntry?.displayName ?? 'Genesis'}
              </div>
              <div className="mt-2 text-sm text-muted-foreground">
                This first slice is wired to the Lua planet registry. The next slice will bind
                this surface to the existing planet shader preview controls.
              </div>
            </div>
          </div>
        </PanelContent>
      </Panel>
    </AppLayout>
  )
}

function GenesisDetailPanel({
  entry,
  errorText,
}: {
  entry: GenesisPlanetEntry | null
  errorText: string | null
}) {
  if (errorText) {
    return (
      <Panel>
        <PanelHeader>
          <div className="text-sm font-semibold text-destructive">Genesis Error</div>
        </PanelHeader>
        <PanelContent>
          <div className="text-sm text-muted-foreground">{errorText}</div>
        </PanelContent>
      </Panel>
    )
  }
  if (!entry) {
    return (
      <Panel>
        <PanelContent>
          <div className="p-4 text-sm text-muted-foreground">
            Select a planet definition to inspect registry metadata.
          </div>
        </PanelContent>
      </Panel>
    )
  }
  return (
    <Panel>
      <PanelHeader>
        <div className="flex items-center gap-2">
          <Dices className="h-4 w-4 text-primary" />
          <div className="text-[11px] uppercase tracking-[0.22em] text-primary/90">
            Definition
          </div>
        </div>
      </PanelHeader>
      <PanelContent>
        <div className="space-y-4 p-4 text-sm">
          <Readout label="Planet ID" value={entry.planetId} />
          <Readout label="Lua File" value={entry.scriptPath} />
          <Readout label="Body Kind" value={bodyKindLabel(entry.bodyKind)} />
          <Readout label="Planet Type" value={planetTypeLabel(entry.planetType)} />
          <Readout label="Seed" value={entry.seed?.toString() ?? 'Unspecified'} />
          <Readout label="Bootstrap" value={entry.spawnEnabled ? 'Enabled' : 'Library only'} />
          <div className="flex flex-wrap gap-2">
            {entry.tags.map((tag) => (
              <Badge key={tag} variant="outline">
                {tag}
              </Badge>
            ))}
          </div>
          <Button disabled className="w-full">
            Draft Save Coming Next
          </Button>
        </div>
      </PanelContent>
    </Panel>
  )
}

function Readout({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-[11px] uppercase tracking-[0.16em] text-muted-foreground">
        {label}
      </div>
      <div className="mt-1 break-words font-mono text-xs text-foreground">{value}</div>
    </div>
  )
}

import * as React from 'react'
import {
  Anchor,
  Crosshair,
  Eye,
  MousePointer2,
  Plus,
  RotateCcw,
  Save,
  Search,
  Ship,
  Trash2,
  Wrench,
  ZoomIn,
} from 'lucide-react'
import { mirrorHardpointOffset, snapHardpointOffset } from './hardpoint-overlay'
import type {
  ShipyardCatalog,
  ShipyardHardpointDefinition,
  ShipyardModuleDefinition,
  ShipyardMountedModuleDefinition,
  ShipyardShipDefinition,
  ShipyardShipEntry,
  Vec3Tuple,
} from './types'
import {
  AppLayout,
  Panel,
  PanelContent,
  PanelHeader,
} from '@/components/layout/AppLayout'
import { HorizontalSplitPanels } from '@/components/layout/ResizablePanels'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ButtonGroup } from '@/components/ui/button-group'
import { HUDFrame } from '@/components/ui/hud-frame'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { GridScanOverlay } from '@/components/thegridcn/grid-scan-overlay'
import { TheGridNumberInput } from '@/components/thegridcn/thegrid-number-input'
import { apiDelete, apiGet, apiPost } from '@/lib/api/client'
import { cn } from '@/lib/utils'

type OperationState = 'idle' | 'saving' | 'publishing' | 'discarding'
type DragState =
  | { kind: 'pan'; startX: number; startY: number; panX: number; panY: number }
  | { kind: 'hardpoint'; hardpointId: string }
  | null

const DEFAULT_SIDEBAR_WIDTH = 320
const DEFAULT_DETAIL_WIDTH = 340
const DEFAULT_INSPECTOR_WIDTH = 560

export function ShipyardPage() {
  const [catalog, setCatalog] = React.useState<ShipyardCatalog | null>(null)
  const [selectedShipId, setSelectedShipId] = React.useState<string | null>(
    null,
  )
  const [selectedModuleId, setSelectedModuleId] = React.useState<string | null>(
    null,
  )
  const [draftShip, setDraftShip] =
    React.useState<ShipyardShipDefinition | null>(null)
  const [draftSpawnEnabled, setDraftSpawnEnabled] = React.useState(false)
  const [draftModule, setDraftModule] =
    React.useState<ShipyardModuleDefinition | null>(null)
  const [selectedHardpointId, setSelectedHardpointId] = React.useState<
    string | null
  >(null)
  const [search, setSearch] = React.useState('')
  const [mode, setMode] = React.useState<'ship' | 'module'>('ship')
  const [errorText, setErrorText] = React.useState<string | null>(null)
  const [statusText, setStatusText] = React.useState<string | null>(null)
  const [operation, setOperation] = React.useState<OperationState>('idle')
  const [sidebarWidth, setSidebarWidth] = useSessionStorageNumber(
    'dashboard:shipyard:sidebar-width',
    DEFAULT_SIDEBAR_WIDTH,
  )
  const [detailPanelWidth, setDetailPanelWidth] = useSessionStorageNumber(
    'dashboard:shipyard:detail-panel-width',
    DEFAULT_DETAIL_WIDTH,
  )
  const [inspectorWidth, setInspectorWidth] = useSessionStorageNumber(
    'dashboard:shipyard:inspector-width',
    DEFAULT_INSPECTOR_WIDTH,
  )

  React.useEffect(() => {
    let cancelled = false
    apiGet<ShipyardCatalog>('/api/shipyard/catalog')
      .then((nextCatalog) => {
        if (cancelled) return
        setCatalog(nextCatalog)
        setSelectedShipId(
          (current) => current ?? nextCatalog.ships.at(0)?.shipId ?? null,
        )
        setSelectedModuleId(
          (current) => current ?? nextCatalog.modules.at(0)?.moduleId ?? null,
        )
      })
      .catch((error: unknown) => {
        if (cancelled) return
        setErrorText(
          error instanceof Error
            ? error.message
            : 'Failed to load Shipyard catalog.',
        )
      })
    return () => {
      cancelled = true
    }
  }, [])

  const ships = catalog?.ships ?? []
  const modules = catalog?.modules ?? []
  const selectedEntry =
    ships.find((entry) => entry.shipId === selectedShipId) ?? null
  const selectedModuleEntry =
    modules.find((entry) => entry.moduleId === selectedModuleId) ?? null

  React.useEffect(() => {
    if (!selectedEntry) {
      setDraftShip(null)
      setDraftSpawnEnabled(false)
      return
    }
    setDraftShip(structuredClone(selectedEntry.definition))
    setDraftSpawnEnabled(selectedEntry.spawnEnabled)
    setSelectedHardpointId(
      selectedEntry.definition.hardpoints.at(0)?.hardpoint_id ?? null,
    )
    setStatusText(null)
  }, [selectedEntry])

  React.useEffect(() => {
    if (!selectedModuleEntry) {
      setDraftModule(null)
      return
    }
    setDraftModule(structuredClone(selectedModuleEntry.definition))
  }, [selectedModuleEntry])

  const filteredShips = React.useMemo(() => {
    const query = search.trim().toLowerCase()
    if (!query) return ships
    return ships.filter((entry) =>
      `${entry.shipId} ${entry.bundleId} ${entry.displayName} ${entry.visualAssetId} ${entry.tags.join(' ')}`
        .toLowerCase()
        .includes(query),
    )
  }, [search, ships])

  const shipDirty = React.useMemo(() => {
    if (!selectedEntry || !draftShip) return false
    return (
      JSON.stringify(draftShip) !== JSON.stringify(selectedEntry.definition) ||
      draftSpawnEnabled !== selectedEntry.spawnEnabled
    )
  }, [draftShip, draftSpawnEnabled, selectedEntry])

  const moduleDirty = React.useMemo(() => {
    if (!selectedModuleEntry || !draftModule) return false
    return (
      JSON.stringify(draftModule) !==
      JSON.stringify(selectedModuleEntry.definition)
    )
  }, [draftModule, selectedModuleEntry])

  const validationErrors = React.useMemo(
    () =>
      draftShip
        ? validateShipDraft(
            draftShip,
            modules.map((entry) => entry.definition),
          )
        : [],
    [draftShip, modules],
  )

  const replaceCatalog = React.useCallback(
    (nextCatalog: ShipyardCatalog) => {
      setCatalog(nextCatalog)
      setSelectedShipId(
        nextCatalog.ships.find((entry) => entry.shipId === selectedShipId)
          ?.shipId ??
          nextCatalog.ships.at(0)?.shipId ??
          null,
      )
      setSelectedModuleId(
        nextCatalog.modules.find((entry) => entry.moduleId === selectedModuleId)
          ?.moduleId ??
          nextCatalog.modules.at(0)?.moduleId ??
          null,
      )
    },
    [selectedModuleId, selectedShipId],
  )

  const saveShipDraft = React.useCallback(async () => {
    if (!draftShip) return
    setOperation('saving')
    setErrorText(null)
    setStatusText(null)
    try {
      const nextCatalog = await apiPost<ShipyardCatalog>(
        `/api/shipyard/ships/${encodeURIComponent(draftShip.ship_id)}/draft`,
        { definition: draftShip, spawnEnabled: draftSpawnEnabled },
      )
      replaceCatalog(nextCatalog)
      setStatusText('Ship draft saved to the script catalog.')
    } catch (error) {
      setErrorText(
        error instanceof Error ? error.message : 'Failed to save ship draft.',
      )
    } finally {
      setOperation('idle')
    }
  }, [draftShip, draftSpawnEnabled, replaceCatalog])

  const publishShipDraft = React.useCallback(async () => {
    if (!selectedEntry) return
    setOperation('publishing')
    setErrorText(null)
    setStatusText(null)
    try {
      const nextCatalog = await apiPost<ShipyardCatalog>(
        `/api/shipyard/ships/${encodeURIComponent(selectedEntry.shipId)}/publish`,
      )
      replaceCatalog(nextCatalog)
      setStatusText('Ship draft published.')
    } catch (error) {
      setErrorText(
        error instanceof Error
          ? error.message
          : 'Failed to publish ship draft.',
      )
    } finally {
      setOperation('idle')
    }
  }, [replaceCatalog, selectedEntry])

  const discardShipDraft = React.useCallback(async () => {
    if (!selectedEntry) return
    setOperation('discarding')
    setErrorText(null)
    setStatusText(null)
    try {
      const nextCatalog = await apiDelete<ShipyardCatalog>(
        `/api/shipyard/ships/${encodeURIComponent(selectedEntry.shipId)}/draft`,
      )
      replaceCatalog(nextCatalog)
      setStatusText('Ship draft discarded.')
    } catch (error) {
      setErrorText(
        error instanceof Error
          ? error.message
          : 'Failed to discard ship draft.',
      )
    } finally {
      setOperation('idle')
    }
  }, [replaceCatalog, selectedEntry])

  const saveModuleDraft = React.useCallback(async () => {
    if (!draftModule) return
    setOperation('saving')
    setErrorText(null)
    setStatusText(null)
    try {
      const nextCatalog = await apiPost<ShipyardCatalog>(
        `/api/shipyard/modules/${encodeURIComponent(draftModule.module_id)}/draft`,
        { definition: draftModule },
      )
      replaceCatalog(nextCatalog)
      setStatusText('Module draft saved to the script catalog.')
    } catch (error) {
      setErrorText(
        error instanceof Error ? error.message : 'Failed to save module draft.',
      )
    } finally {
      setOperation('idle')
    }
  }, [draftModule, replaceCatalog])

  const publishModuleDraft = React.useCallback(async () => {
    if (!selectedModuleEntry) return
    setOperation('publishing')
    setErrorText(null)
    setStatusText(null)
    try {
      const nextCatalog = await apiPost<ShipyardCatalog>(
        `/api/shipyard/modules/${encodeURIComponent(selectedModuleEntry.moduleId)}/publish`,
      )
      replaceCatalog(nextCatalog)
      setStatusText('Module draft published.')
    } catch (error) {
      setErrorText(
        error instanceof Error
          ? error.message
          : 'Failed to publish module draft.',
      )
    } finally {
      setOperation('idle')
    }
  }, [replaceCatalog, selectedModuleEntry])

  const discardModuleDraft = React.useCallback(async () => {
    if (!selectedModuleEntry) return
    setOperation('discarding')
    setErrorText(null)
    setStatusText(null)
    try {
      const nextCatalog = await apiDelete<ShipyardCatalog>(
        `/api/shipyard/modules/${encodeURIComponent(selectedModuleEntry.moduleId)}/draft`,
      )
      replaceCatalog(nextCatalog)
      setStatusText('Module draft discarded.')
    } catch (error) {
      setErrorText(
        error instanceof Error
          ? error.message
          : 'Failed to discard module draft.',
      )
    } finally {
      setOperation('idle')
    }
  }, [replaceCatalog, selectedModuleEntry])

  const operationBusy = operation !== 'idle'

  return (
    <AppLayout
      sidebarWidth={sidebarWidth}
      detailPanelWidth={detailPanelWidth}
      onSidebarResize={setSidebarWidth}
      onDetailPanelResize={setDetailPanelWidth}
      sidebar={
        <ShipLibrary
          entries={filteredShips}
          selectedShipId={selectedShipId}
          search={search}
          onSearchChange={setSearch}
          onSelect={setSelectedShipId}
        />
      }
      detailPanel={
        <ShipyardActionPanel
          mode={mode}
          selectedShip={selectedEntry}
          selectedModule={selectedModuleEntry}
          shipDirty={shipDirty}
          moduleDirty={moduleDirty}
          validationErrors={validationErrors}
          errorText={errorText}
          statusText={statusText}
          operation={operation}
          onSaveShip={() => void saveShipDraft()}
          onPublishShip={() => void publishShipDraft()}
          onDiscardShip={() => void discardShipDraft()}
          onSaveModule={() => void saveModuleDraft()}
          onPublishModule={() => void publishModuleDraft()}
          onDiscardModule={() => void discardModuleDraft()}
        />
      }
    >
      <HorizontalSplitPanels
        leftWidth={inspectorWidth}
        minLeftWidth={480}
        minRightWidth={440}
        onLeftWidthChange={setInspectorWidth}
        left={
          <ShipyardInspector
            mode={mode}
            onModeChange={setMode}
            ship={draftShip}
            spawnEnabled={draftSpawnEnabled}
            modules={modules.map((entry) => entry.definition)}
            selectedModuleId={selectedModuleId}
            moduleDraft={draftModule}
            selectedHardpointId={selectedHardpointId}
            onSpawnEnabledChange={setDraftSpawnEnabled}
            onShipChange={setDraftShip}
            onModuleSelect={setSelectedModuleId}
            onModuleChange={setDraftModule}
            onHardpointSelect={setSelectedHardpointId}
          />
        }
        right={
          <ShipyardWorkbench
            ship={draftShip}
            selectedHardpointId={selectedHardpointId}
            onHardpointSelect={setSelectedHardpointId}
            onShipChange={setDraftShip}
          />
        }
      />
    </AppLayout>
  )
}

function ShipLibrary({
  entries,
  selectedShipId,
  search,
  onSearchChange,
  onSelect,
}: {
  entries: Array<ShipyardShipEntry>
  selectedShipId: string | null
  search: string
  onSearchChange: (value: string) => void
  onSelect: (value: string) => void
}) {
  return (
    <Panel>
      <PanelHeader>
        <div className="space-y-3">
          <div className="flex items-center gap-2">
            <Ship className="h-4 w-4 text-primary" />
            <div className="text-[11px] uppercase tracking-[0.22em] text-primary/90">
              Ship Library
            </div>
          </div>
          <div className="relative">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={search}
              onChange={(event) => onSearchChange(event.target.value)}
              placeholder="Search ships, tags, bundles..."
              className="pl-9"
            />
          </div>
        </div>
      </PanelHeader>
      <PanelContent>
        <ScrollArea className="h-full">
          <div className="space-y-1 p-2">
            {entries.map((entry) => (
              <button
                key={entry.shipId}
                type="button"
                onClick={() => onSelect(entry.shipId)}
                className={cn(
                  'w-full border px-3 py-2 text-left text-sm transition',
                  entry.shipId === selectedShipId
                    ? 'border-primary bg-primary/10 text-primary'
                    : 'border-border bg-background/40 text-foreground hover:border-primary/60',
                )}
              >
                <div className="flex min-w-0 items-center gap-2">
                  <span className="truncate font-medium">
                    {entry.displayName}
                  </span>
                  {entry.hasDraft ? (
                    <Badge variant="secondary">Draft</Badge>
                  ) : null}
                </div>
                <div className="mt-1 truncate text-xs text-muted-foreground">
                  {entry.shipId} / {entry.bundleId}
                </div>
                <div className="mt-2 flex flex-wrap gap-1">
                  <Badge variant="outline">{entry.visualAssetId}</Badge>
                  {entry.tags.slice(0, 3).map((tag) => (
                    <Badge key={tag} variant="outline">
                      {tag}
                    </Badge>
                  ))}
                </div>
              </button>
            ))}
            {entries.length === 0 ? (
              <div className="p-4 text-sm text-muted-foreground">
                No Shipyard entries matched the current search.
              </div>
            ) : null}
          </div>
        </ScrollArea>
      </PanelContent>
    </Panel>
  )
}

function ShipyardInspector({
  mode,
  onModeChange,
  ship,
  spawnEnabled,
  modules,
  selectedModuleId,
  moduleDraft,
  selectedHardpointId,
  onSpawnEnabledChange,
  onShipChange,
  onModuleSelect,
  onModuleChange,
  onHardpointSelect,
}: {
  mode: 'ship' | 'module'
  onModeChange: (value: 'ship' | 'module') => void
  ship: ShipyardShipDefinition | null
  spawnEnabled: boolean
  modules: Array<ShipyardModuleDefinition>
  selectedModuleId: string | null
  moduleDraft: ShipyardModuleDefinition | null
  selectedHardpointId: string | null
  onSpawnEnabledChange: (value: boolean) => void
  onShipChange: (value: ShipyardShipDefinition | null) => void
  onModuleSelect: (value: string) => void
  onModuleChange: (value: ShipyardModuleDefinition | null) => void
  onHardpointSelect: (value: string | null) => void
}) {
  return (
    <Panel>
      <PanelHeader>
        <div className="flex flex-wrap items-center gap-3">
          <Wrench className="h-5 w-5 text-primary" />
          <div>
            <div className="text-sm font-semibold uppercase tracking-[0.18em] text-primary">
              Shipyard Definition
            </div>
            <div className="text-xs text-muted-foreground">
              Registry-backed ship, hardpoint, mount, and module payloads.
            </div>
          </div>
          <Tabs
            value={mode}
            onValueChange={(value) => onModeChange(value as 'ship' | 'module')}
            className="ml-auto"
          >
            <TabsList>
              <TabsTrigger value="ship">Ship</TabsTrigger>
              <TabsTrigger value="module">Modules</TabsTrigger>
            </TabsList>
          </Tabs>
        </div>
      </PanelHeader>
      <PanelContent>
        <ScrollArea className="h-full">
          {mode === 'ship' && ship ? (
            <ShipDefinitionForm
              ship={ship}
              spawnEnabled={spawnEnabled}
              modules={modules}
              selectedHardpointId={selectedHardpointId}
              onSpawnEnabledChange={onSpawnEnabledChange}
              onChange={onShipChange}
              onHardpointSelect={onHardpointSelect}
            />
          ) : mode === 'module' ? (
            <ModuleLibraryForm
              modules={modules}
              selectedModuleId={selectedModuleId}
              moduleDraft={moduleDraft}
              onSelect={onModuleSelect}
              onChange={onModuleChange}
            />
          ) : (
            <EmptyPanel text="Select a ship to edit Shipyard registry data." />
          )}
        </ScrollArea>
      </PanelContent>
    </Panel>
  )
}

function ShipDefinitionForm({
  ship,
  spawnEnabled,
  modules,
  selectedHardpointId,
  onSpawnEnabledChange,
  onChange,
  onHardpointSelect,
}: {
  ship: ShipyardShipDefinition
  spawnEnabled: boolean
  modules: Array<ShipyardModuleDefinition>
  selectedHardpointId: string | null
  onSpawnEnabledChange: (value: boolean) => void
  onChange: (value: ShipyardShipDefinition | null) => void
  onHardpointSelect: (value: string | null) => void
}) {
  const selectedMount =
    ship.mounted_modules.find(
      (mount) => mount.hardpoint_id === selectedHardpointId,
    ) ?? null

  const update = (patch: Partial<ShipyardShipDefinition>) =>
    onChange({ ...ship, ...patch })

  const updateRoot = (
    key: keyof ShipyardShipDefinition['root'],
    value: unknown,
  ) => update({ root: { ...ship.root, [key]: value } })

  const updateDimensions = (
    key: keyof ShipyardShipDefinition['dimensions'],
    value: unknown,
  ) => update({ dimensions: { ...ship.dimensions, [key]: value } })

  const upsertMount = (hardpointId: string, moduleId: string) => {
    const nextMounts = ship.mounted_modules.filter(
      (mount) => mount.hardpoint_id !== hardpointId,
    )
    if (moduleId) {
      nextMounts.push({
        hardpoint_id: hardpointId,
        module_id: moduleId,
        component_overrides: {},
      })
    }
    update({ mounted_modules: nextMounts })
  }

  const updateMount = (
    hardpointId: string,
    patch: Partial<ShipyardMountedModuleDefinition>,
  ) =>
    update({
      mounted_modules: ship.mounted_modules.map((mount) =>
        mount.hardpoint_id === hardpointId ? { ...mount, ...patch } : mount,
      ),
    })

  return (
    <div className="grid gap-4 p-4 xl:grid-cols-2">
      <HUDFrame label="Identity" className="p-4">
        <div className="grid gap-3">
          <TextField
            label="Display Name"
            value={ship.display_name}
            onChange={(display_name) => update({ display_name })}
          />
          <TextField label="Ship ID" value={ship.ship_id} readOnly />
          <TextField label="Bundle ID" value={ship.bundle_id} readOnly />
          <TextField
            label="Tags"
            value={ship.tags.join(', ')}
            onChange={(value) => update({ tags: splitTags(value) })}
          />
          <ToggleRow
            label="Spawn Enabled"
            checked={spawnEnabled}
            onChange={onSpawnEnabledChange}
          />
        </div>
      </HUDFrame>

      <HUDFrame label="Visuals" className="p-4">
        <div className="grid gap-3">
          <TextField
            label="Texture Asset"
            value={ship.visual.visual_asset_id}
            onChange={(visual_asset_id) =>
              update({
                visual: { ...ship.visual, visual_asset_id },
              })
            }
          />
          <TextField
            label="Map Icon"
            value={ship.visual.map_icon_asset_id}
            onChange={(map_icon_asset_id) =>
              update({
                visual: { ...ship.visual, map_icon_asset_id },
              })
            }
          />
        </div>
      </HUDFrame>

      <HUDFrame label="Dimensions" className="p-4">
        <div className="grid grid-cols-2 gap-3">
          <NumberField
            label="Length M"
            value={ship.dimensions.length_m}
            step={0.1}
            onChange={(value) => updateDimensions('length_m', value)}
          />
          <NumberField
            label="Width M"
            value={ship.dimensions.width_m ?? 0}
            step={0.1}
            onChange={(value) => updateDimensions('width_m', value)}
          />
          <NumberField
            label="Height M"
            value={ship.dimensions.height_m}
            step={0.1}
            onChange={(value) => updateDimensions('height_m', value)}
          />
          <ToggleRow
            label="Texture Collision"
            checked={ship.dimensions.collision_from_texture}
            onChange={(value) =>
              updateDimensions('collision_from_texture', value)
            }
          />
        </div>
      </HUDFrame>

      <HUDFrame label="Root Components" className="p-4">
        <div className="grid grid-cols-2 gap-3">
          <NumberField
            label="Base Mass KG"
            value={ship.root.base_mass_kg}
            step={100}
            onChange={(value) => updateRoot('base_mass_kg', value)}
          />
          <NumberField
            label="Max Velocity"
            value={ship.root.max_velocity_mps}
            step={1}
            onChange={(value) => updateRoot('max_velocity_mps', value)}
          />
          <JsonPayloadEditor
            label="Health Pool"
            value={ship.root.health_pool}
            onChange={(value) => updateRoot('health_pool', value)}
          />
          <JsonPayloadEditor
            label="Flight Tuning"
            value={ship.root.flight_tuning}
            onChange={(value) => updateRoot('flight_tuning', value)}
          />
        </div>
      </HUDFrame>

      <HUDFrame label="Hardpoints" className="p-4 xl:col-span-2">
        <div className="space-y-2">
          {ship.hardpoints.map((hardpoint) => (
            <button
              key={hardpoint.hardpoint_id}
              type="button"
              onClick={() => onHardpointSelect(hardpoint.hardpoint_id)}
              className={cn(
                'grid w-full grid-cols-[1fr_auto_auto] items-center gap-3 border px-3 py-2 text-left text-sm',
                selectedHardpointId === hardpoint.hardpoint_id
                  ? 'border-primary bg-primary/10 text-primary'
                  : 'border-border bg-background/40',
              )}
            >
              <span className="min-w-0">
                <span className="block truncate font-medium">
                  {hardpoint.display_name}
                </span>
                <span className="block truncate text-xs text-muted-foreground">
                  {hardpoint.hardpoint_id}
                </span>
              </span>
              <Badge variant="outline">{hardpoint.slot_kind}</Badge>
              <span className="font-mono text-xs text-muted-foreground">
                {formatVec3(hardpoint.offset_m)}
              </span>
            </button>
          ))}
        </div>
      </HUDFrame>

      <HUDFrame label="Mounted Modules" className="p-4 xl:col-span-2">
        <div className="space-y-3">
          {ship.hardpoints.map((hardpoint) => {
            const mount =
              ship.mounted_modules.find(
                (candidate) =>
                  candidate.hardpoint_id === hardpoint.hardpoint_id,
              ) ?? null
            const compatibleModules = modules.filter((moduleDefinition) =>
              moduleDefinition.compatible_slot_kinds.includes(
                hardpoint.slot_kind,
              ),
            )
            return (
              <div
                key={hardpoint.hardpoint_id}
                className="grid gap-2 border border-border bg-background/40 p-3 md:grid-cols-[1fr_220px]"
              >
                <div>
                  <div className="text-sm font-medium">
                    {hardpoint.display_name}
                  </div>
                  <div className="text-xs text-muted-foreground">
                    {hardpoint.slot_kind} / {hardpoint.hardpoint_id}
                  </div>
                </div>
                <select
                  value={mount?.module_id ?? ''}
                  onChange={(event) =>
                    upsertMount(hardpoint.hardpoint_id, event.target.value)
                  }
                  className="grid-input border-input h-9 w-full border bg-background px-3 text-sm text-foreground [color-scheme:dark]"
                >
                  <option value="" className="bg-background text-foreground">
                    Empty
                  </option>
                  {compatibleModules.map((moduleDefinition) => (
                    <option
                      key={moduleDefinition.module_id}
                      value={moduleDefinition.module_id}
                      className="bg-background text-foreground"
                    >
                      {moduleDefinition.display_name}
                    </option>
                  ))}
                </select>
              </div>
            )
          })}
          {selectedMount ? (
            <JsonPayloadEditor
              label="Selected Mount Overrides"
              value={selectedMount.component_overrides}
              onChange={(value) =>
                updateMount(selectedMount.hardpoint_id, {
                  component_overrides:
                    value && typeof value === 'object' && !Array.isArray(value)
                      ? (value as Record<string, unknown>)
                      : {},
                })
              }
            />
          ) : null}
        </div>
      </HUDFrame>
    </div>
  )
}

function ModuleLibraryForm({
  modules,
  selectedModuleId,
  moduleDraft,
  onSelect,
  onChange,
}: {
  modules: Array<ShipyardModuleDefinition>
  selectedModuleId: string | null
  moduleDraft: ShipyardModuleDefinition | null
  onSelect: (value: string) => void
  onChange: (value: ShipyardModuleDefinition | null) => void
}) {
  return (
    <div className="grid gap-4 p-4 xl:grid-cols-[260px_1fr]">
      <HUDFrame label="Library" className="p-3">
        <div className="space-y-1">
          {modules.map((moduleDefinition) => (
            <button
              key={moduleDefinition.module_id}
              type="button"
              onClick={() => onSelect(moduleDefinition.module_id)}
              className={cn(
                'w-full border px-3 py-2 text-left text-sm',
                selectedModuleId === moduleDefinition.module_id
                  ? 'border-primary bg-primary/10 text-primary'
                  : 'border-border bg-background/40',
              )}
            >
              <span className="block truncate font-medium">
                {moduleDefinition.display_name}
              </span>
              <span className="block truncate text-xs text-muted-foreground">
                {moduleDefinition.module_id}
              </span>
            </button>
          ))}
        </div>
      </HUDFrame>
      {moduleDraft ? (
        <HUDFrame label="Module Defaults" className="p-4">
          <div className="grid gap-3">
            <TextField
              label="Display Name"
              value={moduleDraft.display_name}
              onChange={(display_name) =>
                onChange({ ...moduleDraft, display_name })
              }
            />
            <TextField
              label="Module ID"
              value={moduleDraft.module_id}
              readOnly
            />
            <TextField
              label="Category"
              value={moduleDraft.category}
              onChange={(category) => onChange({ ...moduleDraft, category })}
            />
            <TextField
              label="Compatible Slots"
              value={moduleDraft.compatible_slot_kinds.join(', ')}
              onChange={(value) =>
                onChange({
                  ...moduleDraft,
                  compatible_slot_kinds: splitTags(value),
                })
              }
            />
            <TextField
              label="Tags"
              value={moduleDraft.tags.join(', ')}
              onChange={(value) =>
                onChange({ ...moduleDraft, tags: splitTags(value) })
              }
            />
            <JsonPayloadEditor
              label="Component Payloads"
              value={moduleDraft.components}
              onChange={(value) =>
                onChange({
                  ...moduleDraft,
                  components: Array.isArray(value)
                    ? (value as ShipyardModuleDefinition['components'])
                    : moduleDraft.components,
                })
              }
            />
          </div>
        </HUDFrame>
      ) : (
        <EmptyPanel text="Select a module definition to edit library defaults." />
      )}
    </div>
  )
}

function ShipyardWorkbench({
  ship,
  selectedHardpointId,
  onHardpointSelect,
  onShipChange,
}: {
  ship: ShipyardShipDefinition | null
  selectedHardpointId: string | null
  onHardpointSelect: (value: string | null) => void
  onShipChange: (value: ShipyardShipDefinition | null) => void
}) {
  const viewportRef = React.useRef<HTMLDivElement | null>(null)
  const [viewport, setViewport] = React.useState({ width: 800, height: 600 })
  const [zoom, setZoom] = React.useState(1)
  const [pan, setPan] = React.useState({ x: 0, y: 0 })
  const [dragState, setDragState] = React.useState<DragState>(null)
  const [gridEnabled, setGridEnabled] = React.useState(true)
  const [snapEnabled, setSnapEnabled] = React.useState(true)
  const [mirrorMode, setMirrorMode] = React.useState(false)
  const [gridSpacing, setGridSpacing] = React.useState(0.5)

  React.useEffect(() => {
    const target = viewportRef.current
    if (!target) return
    const observer = new ResizeObserver(([entry]) => {
      setViewport({
        width: Math.max(1, entry.contentRect.width),
        height: Math.max(1, entry.contentRect.height),
      })
    })
    observer.observe(target)
    return () => observer.disconnect()
  }, [])

  const dimensions = ship?.dimensions
  const lengthM = dimensions?.length_m ?? 1
  const widthM = dimensions?.width_m ?? lengthM
  const planeAspect = widthM / lengthM
  const basePlane = React.useMemo(() => {
    const maxWidth = viewport.width * 0.92
    const maxHeight = viewport.height * 0.82
    let width = maxWidth
    let height = width / planeAspect
    if (height > maxHeight) {
      height = maxHeight
      width = height * planeAspect
    }
    return { width, height }
  }, [planeAspect, viewport.height, viewport.width])

  const metersToScreen = React.useCallback(
    (offset: Vec3Tuple) => {
      const pxPerMeterX = basePlane.width / widthM
      const pxPerMeterY = basePlane.height / lengthM
      return {
        x: viewport.width / 2 + pan.x + offset[0] * pxPerMeterX * zoom,
        y: viewport.height / 2 + pan.y - offset[1] * pxPerMeterY * zoom,
      }
    },
    [
      basePlane.height,
      basePlane.width,
      lengthM,
      pan.x,
      pan.y,
      viewport.height,
      viewport.width,
      widthM,
      zoom,
    ],
  )

  const screenToMeters = React.useCallback(
    (clientX: number, clientY: number): Vec3Tuple => {
      const bounds = viewportRef.current?.getBoundingClientRect()
      const localX = clientX - (bounds?.left ?? 0)
      const localY = clientY - (bounds?.top ?? 0)
      const pxPerMeterX = basePlane.width / widthM
      const pxPerMeterY = basePlane.height / lengthM
      return [
        (localX - viewport.width / 2 - pan.x) / (pxPerMeterX * zoom),
        -(localY - viewport.height / 2 - pan.y) / (pxPerMeterY * zoom),
        0,
      ]
    },
    [
      basePlane.height,
      basePlane.width,
      lengthM,
      pan.x,
      pan.y,
      viewport.height,
      viewport.width,
      widthM,
      zoom,
    ],
  )

  const updateHardpointOffset = React.useCallback(
    (hardpointId: string, offset: Vec3Tuple) => {
      if (!ship) return
      const nextOffset = snapEnabled
        ? snapHardpointOffset(offset, gridSpacing)
        : [offset[0], offset[1], 0]
      const source = ship.hardpoints.find(
        (hardpoint) => hardpoint.hardpoint_id === hardpointId,
      )
      const mirrored = source?.mirror_group
        ? mirrorHardpointOffset(nextOffset as Vec3Tuple)
        : null
      onShipChange({
        ...ship,
        hardpoints: ship.hardpoints.map((hardpoint) => {
          if (hardpoint.hardpoint_id === hardpointId) {
            return { ...hardpoint, offset_m: nextOffset as Vec3Tuple }
          }
          if (
            mirrorMode &&
            source?.mirror_group &&
            hardpoint.mirror_group === source.mirror_group &&
            hardpoint.hardpoint_id !== hardpointId &&
            mirrored
          ) {
            return { ...hardpoint, offset_m: mirrored }
          }
          return hardpoint
        }),
      })
    },
    [gridSpacing, mirrorMode, onShipChange, ship, snapEnabled],
  )

  const addHardpoint = () => {
    if (!ship) return
    const index = ship.hardpoints.length + 1
    const baseId = `hardpoint_${index}`
    const created: Array<ShipyardHardpointDefinition> = [
      {
        hardpoint_id: mirrorMode ? `${baseId}_left` : baseId,
        display_name: mirrorMode
          ? `Hardpoint ${index} Left`
          : `Hardpoint ${index}`,
        slot_kind: 'engine',
        offset_m: [-1, 0, 0],
        local_rotation_rad: 0,
        mirror_group: mirrorMode ? baseId : null,
        compatible_tags: ['engine'],
      },
    ]
    if (mirrorMode) {
      created.push({
        ...created[0],
        hardpoint_id: `${baseId}_right`,
        display_name: `Hardpoint ${index} Right`,
        offset_m: mirrorHardpointOffset(created[0].offset_m),
      })
    }
    onShipChange({ ...ship, hardpoints: [...ship.hardpoints, ...created] })
    onHardpointSelect(created[0].hardpoint_id)
  }

  const removeSelectedHardpoint = () => {
    if (!ship || !selectedHardpointId) return
    onShipChange({
      ...ship,
      hardpoints: ship.hardpoints.filter(
        (hardpoint) => hardpoint.hardpoint_id !== selectedHardpointId,
      ),
      mounted_modules: ship.mounted_modules.filter(
        (mount) => mount.hardpoint_id !== selectedHardpointId,
      ),
    })
    onHardpointSelect(ship.hardpoints.at(0)?.hardpoint_id ?? null)
  }

  const selectedHardpoint = ship?.hardpoints.find(
    (hardpoint) => hardpoint.hardpoint_id === selectedHardpointId,
  )

  return (
    <Panel>
      <PanelHeader>
        <div className="flex flex-wrap items-center gap-3">
          <Crosshair className="h-5 w-5 text-primary" />
          <div>
            <div className="text-sm font-semibold uppercase tracking-[0.18em] text-primary">
              Texture Hardpoint Workbench
            </div>
            <div className="text-xs text-muted-foreground">
              Mouse wheel zoom, drag empty texture space to pan, drag markers to
              position hardpoints.
            </div>
          </div>
          <ButtonGroup className="ml-auto">
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  onClick={addHardpoint}
                >
                  <Plus className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Add hardpoint</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  disabled={!selectedHardpointId}
                  onClick={removeSelectedHardpoint}
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Remove selected hardpoint</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  onClick={() => {
                    setZoom(1)
                    setPan({ x: 0, y: 0 })
                  }}
                >
                  <RotateCcw className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Reset view</TooltipContent>
            </Tooltip>
          </ButtonGroup>
        </div>
      </PanelHeader>
      <PanelContent>
        {ship ? (
          <div className="flex h-full min-h-0 flex-col">
            <div className="flex flex-wrap items-center gap-3 border-b border-border px-4 py-3">
              <ToggleRow
                label="Grid"
                checked={gridEnabled}
                onChange={setGridEnabled}
              />
              <ToggleRow
                label="Snap"
                checked={snapEnabled}
                onChange={setSnapEnabled}
              />
              <ToggleRow
                label="Mirror X"
                checked={mirrorMode}
                onChange={setMirrorMode}
              />
              <TheGridNumberInput
                label="Grid M"
                value={gridSpacing}
                min={0.1}
                max={10}
                step={0.1}
                onChange={setGridSpacing}
              />
              <Badge variant="outline">
                <ZoomIn className="h-3 w-3" />
                {zoom.toFixed(2)}x
              </Badge>
              {selectedHardpoint ? (
                <Badge variant="secondary">
                  {selectedHardpoint.hardpoint_id}
                </Badge>
              ) : null}
            </div>
            <div
              ref={viewportRef}
              className="relative min-h-0 flex-1 overflow-hidden bg-background"
              onWheel={(event) => {
                event.preventDefault()
                const nextZoom = Math.min(
                  8,
                  Math.max(0.35, zoom * (event.deltaY < 0 ? 1.1 : 0.9)),
                )
                setZoom(nextZoom)
              }}
              onPointerDown={(event) => {
                if (event.button !== 0) return
                setDragState({
                  kind: 'pan',
                  startX: event.clientX,
                  startY: event.clientY,
                  panX: pan.x,
                  panY: pan.y,
                })
                event.currentTarget.setPointerCapture(event.pointerId)
              }}
              onPointerMove={(event) => {
                if (!dragState) return
                if (dragState.kind === 'pan') {
                  setPan({
                    x: dragState.panX + event.clientX - dragState.startX,
                    y: dragState.panY + event.clientY - dragState.startY,
                  })
                } else {
                  updateHardpointOffset(
                    dragState.hardpointId,
                    screenToMeters(event.clientX, event.clientY),
                  )
                }
              }}
              onPointerUp={(event) => {
                setDragState(null)
                event.currentTarget.releasePointerCapture(event.pointerId)
              }}
            >
              <div
                className="absolute border border-border/80 bg-card/30"
                style={{
                  left:
                    viewport.width / 2 + pan.x - (basePlane.width * zoom) / 2,
                  top:
                    viewport.height / 2 + pan.y - (basePlane.height * zoom) / 2,
                  width: basePlane.width * zoom,
                  height: basePlane.height * zoom,
                }}
              >
                <img
                  src={`/api/shipyard/assets/${encodeURIComponent(ship.visual.visual_asset_id)}`}
                  alt=""
                  className="h-full w-full object-contain"
                  draggable={false}
                />
                {gridEnabled ? (
                  <div
                    className="pointer-events-none absolute inset-0 opacity-80"
                    style={{
                      backgroundImage:
                        'linear-gradient(to right, color-mix(in oklch, var(--primary) 28%, transparent) 1px, transparent 1px), linear-gradient(to bottom, color-mix(in oklch, var(--primary) 28%, transparent) 1px, transparent 1px)',
                      backgroundSize: `${(basePlane.width / widthM) * gridSpacing * zoom}px ${(basePlane.height / lengthM) * gridSpacing * zoom}px`,
                    }}
                  />
                ) : null}
              </div>
              <GridScanOverlay gridSize={96} scanSpeed={12} />
              {ship.hardpoints.map((hardpoint) => {
                const point = metersToScreen(hardpoint.offset_m)
                const selected = hardpoint.hardpoint_id === selectedHardpointId
                return (
                  <button
                    key={hardpoint.hardpoint_id}
                    type="button"
                    className={cn(
                      'absolute z-10 flex h-8 w-8 -translate-x-1/2 -translate-y-1/2 items-center justify-center border bg-background/80 text-primary shadow-sm',
                      selected
                        ? 'border-primary ring-2 ring-primary/30'
                        : 'border-border hover:border-primary/70',
                    )}
                    style={{ left: point.x, top: point.y }}
                    onPointerDown={(event) => {
                      event.stopPropagation()
                      onHardpointSelect(hardpoint.hardpoint_id)
                      setDragState({
                        kind: 'hardpoint',
                        hardpointId: hardpoint.hardpoint_id,
                      })
                      viewportRef.current?.setPointerCapture(event.pointerId)
                    }}
                    onPointerUp={(event) => {
                      event.stopPropagation()
                      setDragState(null)
                      if (
                        event.currentTarget.hasPointerCapture(event.pointerId)
                      ) {
                        event.currentTarget.releasePointerCapture(
                          event.pointerId,
                        )
                      }
                    }}
                  >
                    <MousePointer2 className="h-4 w-4" />
                  </button>
                )
              })}
            </div>
            {selectedHardpoint ? (
              <div className="grid gap-3 border-t border-border p-4 md:grid-cols-3">
                <TextField
                  label="Hardpoint Name"
                  value={selectedHardpoint.display_name}
                  onChange={(display_name) =>
                    onShipChange({
                      ...ship,
                      hardpoints: ship.hardpoints.map((hardpoint) =>
                        hardpoint.hardpoint_id ===
                        selectedHardpoint.hardpoint_id
                          ? { ...hardpoint, display_name }
                          : hardpoint,
                      ),
                    })
                  }
                />
                <NumberField
                  label="Yaw Rad"
                  value={selectedHardpoint.local_rotation_rad}
                  step={0.05}
                  onChange={(local_rotation_rad) =>
                    onShipChange({
                      ...ship,
                      hardpoints: ship.hardpoints.map((hardpoint) =>
                        hardpoint.hardpoint_id ===
                        selectedHardpoint.hardpoint_id
                          ? { ...hardpoint, local_rotation_rad }
                          : hardpoint,
                      ),
                    })
                  }
                />
                <TextField
                  label="Slot Kind"
                  value={selectedHardpoint.slot_kind}
                  onChange={(slot_kind) =>
                    onShipChange({
                      ...ship,
                      hardpoints: ship.hardpoints.map((hardpoint) =>
                        hardpoint.hardpoint_id ===
                        selectedHardpoint.hardpoint_id
                          ? { ...hardpoint, slot_kind }
                          : hardpoint,
                      ),
                    })
                  }
                />
              </div>
            ) : null}
          </div>
        ) : (
          <EmptyPanel text="Select a ship to position hardpoints on its texture." />
        )}
      </PanelContent>
    </Panel>
  )
}

function ShipyardActionPanel({
  mode,
  selectedShip,
  selectedModule,
  shipDirty,
  moduleDirty,
  validationErrors,
  errorText,
  statusText,
  operation,
  onSaveShip,
  onPublishShip,
  onDiscardShip,
  onSaveModule,
  onPublishModule,
  onDiscardModule,
}: {
  mode: 'ship' | 'module'
  selectedShip: ShipyardShipEntry | null
  selectedModule: { moduleId: string; hasDraft: boolean } | null
  shipDirty: boolean
  moduleDirty: boolean
  validationErrors: Array<string>
  errorText: string | null
  statusText: string | null
  operation: OperationState
  onSaveShip: () => void
  onPublishShip: () => void
  onDiscardShip: () => void
  onSaveModule: () => void
  onPublishModule: () => void
  onDiscardModule: () => void
}) {
  const busy = operation !== 'idle'
  const dirty = mode === 'ship' ? shipDirty : moduleDirty
  const hasDraft =
    mode === 'ship' ? selectedShip?.hasDraft : selectedModule?.hasDraft
  return (
    <Panel>
      <PanelHeader>
        <div className="flex items-center gap-2">
          <Anchor className="h-4 w-4 text-primary" />
          <div className="text-[11px] uppercase tracking-[0.22em] text-primary/90">
            Shipyard Actions
          </div>
        </div>
      </PanelHeader>
      <PanelContent>
        <ScrollArea className="h-full">
          <div className="space-y-4 p-4">
            <HUDFrame label="State" className="p-4">
              <div className="flex flex-wrap gap-2">
                <Badge variant={dirty ? 'secondary' : 'outline'}>
                  {dirty ? 'Dirty' : 'Clean'}
                </Badge>
                {hasDraft ? <Badge variant="secondary">Draft</Badge> : null}
                <Badge variant="outline">{mode}</Badge>
              </div>
            </HUDFrame>
            <HUDFrame label="Validation" className="p-4">
              {validationErrors.length > 0 && mode === 'ship' ? (
                <ul className="space-y-2 text-sm text-destructive">
                  {validationErrors.map((error) => (
                    <li key={error}>{error}</li>
                  ))}
                </ul>
              ) : (
                <div className="text-sm text-muted-foreground">
                  No blocking validation errors.
                </div>
              )}
            </HUDFrame>
            {errorText ? (
              <div className="border border-destructive/60 bg-destructive/10 p-3 text-sm text-destructive">
                {errorText}
              </div>
            ) : null}
            {statusText ? (
              <div className="border border-primary/50 bg-primary/10 p-3 text-sm text-primary">
                {statusText}
              </div>
            ) : null}
            <div className="grid gap-2">
              <Button
                type="button"
                disabled={
                  !dirty ||
                  busy ||
                  (mode === 'ship' && validationErrors.length > 0)
                }
                onClick={mode === 'ship' ? onSaveShip : onSaveModule}
              >
                <Save className="mr-2 h-4 w-4" />
                Save Draft
              </Button>
              <Button
                type="button"
                variant="outline"
                disabled={!hasDraft || busy}
                onClick={mode === 'ship' ? onPublishShip : onPublishModule}
              >
                <Eye className="mr-2 h-4 w-4" />
                Publish
              </Button>
              <Button
                type="button"
                variant="outline"
                disabled={!hasDraft || busy}
                onClick={mode === 'ship' ? onDiscardShip : onDiscardModule}
              >
                <RotateCcw className="mr-2 h-4 w-4" />
                Discard
              </Button>
            </div>
          </div>
        </ScrollArea>
      </PanelContent>
    </Panel>
  )
}

function TextField({
  label,
  value,
  onChange,
  readOnly = false,
}: {
  label: string
  value: string
  onChange?: (value: string) => void
  readOnly?: boolean
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </Label>
      <Input
        value={value}
        readOnly={readOnly}
        onChange={(event) => onChange?.(event.target.value)}
      />
    </div>
  )
}

function NumberField({
  label,
  value,
  step = 1,
  onChange,
}: {
  label: string
  value: number
  step?: number
  onChange: (value: number) => void
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </Label>
      <TheGridNumberInput
        value={Number.isFinite(value) ? value : 0}
        step={step}
        onChange={onChange}
        className="w-full"
        inputClassName="min-w-0 flex-1 text-xs"
      />
    </div>
  )
}

function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string
  checked: boolean
  onChange: (value: boolean) => void
}) {
  return (
    <label className="flex items-center justify-between gap-3 text-xs uppercase tracking-[0.14em] text-muted-foreground">
      <span>{label}</span>
      <Switch checked={checked} onCheckedChange={onChange} />
    </label>
  )
}

function JsonPayloadEditor({
  label,
  value,
  onChange,
}: {
  label: string
  value: unknown
  onChange: (value: unknown) => void
}) {
  const [text, setText] = React.useState(() => JSON.stringify(value, null, 2))
  const [error, setError] = React.useState<string | null>(null)

  React.useEffect(() => {
    setText(JSON.stringify(value, null, 2))
    setError(null)
  }, [value])

  return (
    <div className="space-y-1.5 md:col-span-2">
      <Label className="text-xs uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </Label>
      <textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        onBlur={() => {
          try {
            const parsed = JSON.parse(text) as unknown
            setError(null)
            onChange(parsed)
          } catch (parseError) {
            setError(
              parseError instanceof Error
                ? parseError.message
                : 'Invalid JSON payload',
            )
          }
        }}
        className="grid-input border-input min-h-36 w-full resize-y border bg-background p-3 font-mono text-xs text-foreground outline-none focus:border-primary"
      />
      {error ? <div className="text-xs text-destructive">{error}</div> : null}
    </div>
  )
}

function EmptyPanel({ text }: { text: string }) {
  return (
    <div className="flex h-full min-h-96 items-center justify-center border border-border bg-background/40 p-6">
      <div className="max-w-sm text-center text-sm text-muted-foreground">
        {text}
      </div>
    </div>
  )
}

function splitTags(value: string): Array<string> {
  return value
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean)
}

function formatVec3(value: Vec3Tuple): string {
  return value.map((entry) => Number(entry.toFixed(2))).join(', ')
}

function validateShipDraft(
  ship: ShipyardShipDefinition,
  modules: Array<ShipyardModuleDefinition>,
): Array<string> {
  const errors: Array<string> = []
  const hardpointIds = new Set<string>()
  for (const hardpoint of ship.hardpoints) {
    if (hardpointIds.has(hardpoint.hardpoint_id)) {
      errors.push(`Duplicate hardpoint ${hardpoint.hardpoint_id}`)
    }
    hardpointIds.add(hardpoint.hardpoint_id)
    if (hardpoint.offset_m[2] !== 0) {
      errors.push(`${hardpoint.hardpoint_id} must stay on z=0`)
    }
  }
  for (const mount of ship.mounted_modules) {
    const hardpoint = ship.hardpoints.find(
      (candidate) => candidate.hardpoint_id === mount.hardpoint_id,
    )
    const moduleDefinition = modules.find(
      (candidate) => candidate.module_id === mount.module_id,
    )
    if (!hardpoint) {
      errors.push(`Mount references missing hardpoint ${mount.hardpoint_id}`)
      continue
    }
    if (!moduleDefinition) {
      errors.push(`Mount references missing module ${mount.module_id}`)
      continue
    }
    if (!moduleDefinition.compatible_slot_kinds.includes(hardpoint.slot_kind)) {
      errors.push(
        `${mount.module_id} is not compatible with ${hardpoint.hardpoint_id}`,
      )
    }
  }
  return errors
}

function useSessionStorageNumber(key: string, fallback: number) {
  const [value, setValue] = React.useState(() => {
    if (typeof window === 'undefined') return fallback
    const raw = window.sessionStorage.getItem(key)
    const parsed = raw ? Number(raw) : Number.NaN
    return Number.isFinite(parsed) ? parsed : fallback
  })

  const update = React.useCallback(
    (nextValue: number) => {
      setValue(nextValue)
      if (typeof window !== 'undefined') {
        window.sessionStorage.setItem(key, String(nextValue))
      }
    },
    [key],
  )

  return [value, update] as const
}

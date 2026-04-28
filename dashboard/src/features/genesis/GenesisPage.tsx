import * as React from 'react'
import {
  Dices,
  Globe2,
  RefreshCw,
  RotateCcw,
  Save,
  Search,
  Upload,
} from 'lucide-react'
import type {
  GenesisPlanetCatalog,
  GenesisPlanetDefinition,
  GenesisPlanetEntry,
  GenesisPlanetShaderSettings,
  Vec3Tuple,
} from '@/features/genesis/types'
import { GenesisPlanetPreview } from '@/features/genesis/GenesisPlanetPreview'
import {
  AppLayout,
  Panel,
  PanelContent,
  PanelHeader,
} from '@/components/layout/AppLayout'
import { HorizontalSplitPanels } from '@/components/layout/ResizablePanels'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Switch } from '@/components/ui/switch'
import { TheGridNumberInput } from '@/components/thegridcn/thegrid-number-input'
import { useSessionStorageNumber } from '@/hooks/use-session-storage-number'
import { apiDelete, apiGet, apiPost } from '@/lib/api/client'

const DEFAULT_GENESIS_SIDEBAR_WIDTH = 320
const DEFAULT_GENESIS_DETAIL_WIDTH = 400
const DEFAULT_GENESIS_DEFINITION_WIDTH = 760
const PLANET_VISUAL_SHADER_ASSET_ID = 'planet_visual_wgsl'
const STAR_VISUAL_SHADER_ASSET_ID = 'star_visual_wgsl'

type OperationState = 'idle' | 'saving' | 'publishing' | 'discarding'

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

function hashString(value: string): number {
  let hash = 2166136261
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }
  return hash >>> 0
}

function createRng(seed: number): () => number {
  let state = seed >>> 0
  return () => {
    state = Math.imul(state ^ (state >>> 15), 1 | state)
    state ^= state + Math.imul(state ^ (state >>> 7), 61 | state)
    return ((state ^ (state >>> 14)) >>> 0) / 4294967296
  }
}

function randomRange(rng: () => number, min: number, max: number): number {
  return min + (max - min) * rng()
}

function randomColor(rng: () => number, warm: boolean): Vec3Tuple {
  if (warm) {
    return [
      Number(randomRange(rng, 0.8, 1.0).toFixed(3)),
      Number(randomRange(rng, 0.36, 0.86).toFixed(3)),
      Number(randomRange(rng, 0.05, 0.34).toFixed(3)),
    ]
  }
  return [
    Number(randomRange(rng, 0.08, 0.72).toFixed(3)),
    Number(randomRange(rng, 0.16, 0.78).toFixed(3)),
    Number(randomRange(rng, 0.18, 0.96).toFixed(3)),
  ]
}

function randomizeDefinition(
  definition: GenesisPlanetDefinition,
): GenesisPlanetDefinition {
  const settings = definition.shader_settings
  const seed = Math.max(0, Math.trunc(settings.seed))
  const rng = createRng(hashString(`${definition.planet_id}:${seed}`))
  const isStar = settings.body_kind === 1
  const nextSettings: GenesisPlanetShaderSettings = {
    ...settings,
    base_radius_scale: Number(randomRange(rng, 0.46, 0.72).toFixed(3)),
    normal_strength: Number(
      randomRange(rng, isStar ? 0.02 : 0.35, isStar ? 0.18 : 1.1).toFixed(3),
    ),
    detail_level: Number(
      randomRange(rng, isStar ? 0.1 : 0.36, isStar ? 0.34 : 0.86).toFixed(3),
    ),
    rotation_speed: Number(randomRange(rng, -0.008, 0.008).toFixed(4)),
    rim_strength: Number(
      randomRange(rng, isStar ? 0.7 : 0.16, isStar ? 1.5 : 0.64).toFixed(3),
    ),
    fresnel_strength: Number(
      randomRange(rng, isStar ? 0.55 : 0.22, isStar ? 1.2 : 0.62).toFixed(3),
    ),
    continent_size: Number(randomRange(rng, 0.38, 0.82).toFixed(3)),
    ocean_level: Number(randomRange(rng, 0.28, 0.68).toFixed(3)),
    mountain_height: Number(randomRange(rng, 0.18, 0.72).toFixed(3)),
    roughness: Number(randomRange(rng, 0.24, 0.78).toFixed(3)),
    crater_density: Number(randomRange(rng, 0.02, 0.34).toFixed(3)),
    crater_size: Number(randomRange(rng, 0.06, 0.42).toFixed(3)),
    ice_cap_size: Number(randomRange(rng, 0.02, 0.28).toFixed(3)),
    storm_intensity: Number(randomRange(rng, 0.02, 0.44).toFixed(3)),
    surface_activity: Number(
      randomRange(rng, isStar ? 0.62 : 0.02, isStar ? 1.0 : 0.28).toFixed(3),
    ),
    corona_intensity: Number(
      randomRange(rng, isStar ? 0.78 : 0.0, isStar ? 1.5 : 0.12).toFixed(3),
    ),
    cloud_coverage: Number(randomRange(rng, 0.22, 0.78).toFixed(3)),
    cloud_scale: Number(randomRange(rng, 0.9, 2.6).toFixed(3)),
    cloud_speed: Number(randomRange(rng, -0.28, 0.28).toFixed(3)),
    atmosphere_thickness: Number(
      randomRange(rng, isStar ? 0.18 : 0.08, isStar ? 0.32 : 0.22).toFixed(3),
    ),
    atmosphere_alpha: Number(randomRange(rng, 0.34, 0.82).toFixed(3)),
    surface_saturation: Number(randomRange(rng, 0.88, 1.34).toFixed(3)),
    surface_contrast: Number(randomRange(rng, 0.9, 1.28).toFixed(3)),
    color_primary_rgb: randomColor(rng, isStar),
    color_secondary_rgb: randomColor(rng, isStar),
    color_tertiary_rgb: randomColor(rng, isStar),
    color_atmosphere_rgb: randomColor(rng, isStar),
    color_clouds_rgb: isStar ? randomColor(rng, true) : [0.95, 0.97, 1.0],
    color_emissive_rgb: randomColor(rng, true),
  }
  return { ...definition, shader_settings: nextSettings }
}

function nextSeedDefinition(
  definition: GenesisPlanetDefinition,
): GenesisPlanetDefinition {
  const currentSeed = Math.max(0, Math.trunc(definition.shader_settings.seed))
  const nextSeed = (Math.imul(currentSeed + 1, 1103515245) + 12345) >>> 0
  return {
    ...definition,
    shader_settings: {
      ...definition.shader_settings,
      seed: nextSeed % 1000000000,
    },
  }
}

function colorToHex(value: Vec3Tuple): string {
  return `#${value
    .map((channel) =>
      Math.round(Math.max(0, Math.min(1, channel)) * 255)
        .toString(16)
        .padStart(2, '0'),
    )
    .join('')}`
}

function hexToColor(value: string): Vec3Tuple {
  const normalized = value.replace('#', '')
  const red = Number.parseInt(normalized.slice(0, 2), 16) / 255
  const green = Number.parseInt(normalized.slice(2, 4), 16) / 255
  const blue = Number.parseInt(normalized.slice(4, 6), 16) / 255
  return [
    Number(red.toFixed(3)),
    Number(green.toFixed(3)),
    Number(blue.toFixed(3)),
  ]
}

function joinList(values: Array<string>): string {
  return values.join(', ')
}

function parseList(value: string): Array<string> {
  return value
    .split(',')
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0)
}

export function GenesisPage() {
  const [catalog, setCatalog] = React.useState<GenesisPlanetCatalog | null>(
    null,
  )
  const [selectedPlanetId, setSelectedPlanetId] = React.useState<string | null>(
    null,
  )
  const [draftDefinition, setDraftDefinition] =
    React.useState<GenesisPlanetDefinition | null>(null)
  const [draftSpawnEnabled, setDraftSpawnEnabled] = React.useState(false)
  const [search, setSearch] = React.useState('')
  const [errorText, setErrorText] = React.useState<string | null>(null)
  const [statusText, setStatusText] = React.useState<string | null>(null)
  const [operation, setOperation] = React.useState<OperationState>('idle')
  const [sidebarWidth, setSidebarWidth] = useSessionStorageNumber(
    'dashboard:genesis:sidebar-width',
    DEFAULT_GENESIS_SIDEBAR_WIDTH,
  )
  const [detailPanelWidth, setDetailPanelWidth] = useSessionStorageNumber(
    'dashboard:genesis:detail-panel-width',
    DEFAULT_GENESIS_DETAIL_WIDTH,
  )
  const [definitionPanelWidth, setDefinitionPanelWidth] =
    useSessionStorageNumber(
      'dashboard:genesis:definition-panel-width',
      DEFAULT_GENESIS_DEFINITION_WIDTH,
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
        setErrorText(
          error instanceof Error
            ? error.message
            : 'Failed to load Genesis catalog.',
        )
      })
    return () => {
      cancelled = true
    }
  }, [])

  const entries = catalog?.entries ?? []
  const selectedEntry =
    entries.find((entry) => entry.planetId === selectedPlanetId) ?? null

  React.useEffect(() => {
    if (!selectedEntry) {
      setDraftDefinition(null)
      setDraftSpawnEnabled(false)
      return
    }
    setDraftDefinition(selectedEntry.definition)
    setDraftSpawnEnabled(selectedEntry.spawnEnabled)
    setStatusText(null)
  }, [selectedEntry])

  const filteredEntries = React.useMemo(() => {
    const query = search.trim().toLowerCase()
    if (!query) return entries
    return entries.filter((entry) =>
      `${entry.planetId} ${entry.displayName} ${entry.scriptPath} ${entry.tags.join(' ')}`
        .toLowerCase()
        .includes(query),
    )
  }, [entries, search])

  const dirty = React.useMemo(() => {
    if (!selectedEntry || !draftDefinition) return false
    return (
      JSON.stringify(draftDefinition) !==
        JSON.stringify(selectedEntry.definition) ||
      draftSpawnEnabled !== selectedEntry.spawnEnabled
    )
  }, [draftDefinition, draftSpawnEnabled, selectedEntry])

  const updateDefinition = React.useCallback(
    (patch: Partial<GenesisPlanetDefinition>) => {
      setDraftDefinition((current) =>
        current ? { ...current, ...patch } : current,
      )
    },
    [],
  )

  const updateSettings = React.useCallback(
    (patch: Partial<GenesisPlanetShaderSettings>) => {
      setDraftDefinition((current) => {
        if (!current) return current
        const nextSettings = { ...current.shader_settings, ...patch }
        const currentBodyKind = current.shader_settings.body_kind
        const nextBodyKind = patch.body_kind ?? currentBodyKind
        const shouldSwitchShader =
          patch.body_kind !== undefined && nextBodyKind !== currentBodyKind
        const nextShaderAssetId =
          shouldSwitchShader && nextBodyKind === 1
            ? STAR_VISUAL_SHADER_ASSET_ID
            : shouldSwitchShader && current.spawn.planet_visual_shader_asset_id === STAR_VISUAL_SHADER_ASSET_ID
              ? PLANET_VISUAL_SHADER_ASSET_ID
              : current.spawn.planet_visual_shader_asset_id
        return {
          ...current,
          spawn: {
            ...current.spawn,
            planet_visual_shader_asset_id: nextShaderAssetId,
          },
          shader_settings: nextSettings,
        }
      })
    },
    [],
  )

  const replaceCatalog = React.useCallback(
    (nextCatalog: GenesisPlanetCatalog) => {
      setCatalog(nextCatalog)
      const nextSelected =
        nextCatalog.entries.find(
          (entry) => entry.planetId === selectedPlanetId,
        ) ??
        nextCatalog.entries.at(0) ??
        null
      setSelectedPlanetId(nextSelected?.planetId ?? null)
    },
    [selectedPlanetId],
  )

  const saveDraft = React.useCallback(async () => {
    if (!draftDefinition) return
    setOperation('saving')
    setErrorText(null)
    setStatusText(null)
    try {
      const nextCatalog = await apiPost<GenesisPlanetCatalog>(
        `/api/genesis/planets/${encodeURIComponent(draftDefinition.planet_id)}/draft`,
        {
          definition: draftDefinition,
          spawnEnabled: draftSpawnEnabled,
        },
      )
      replaceCatalog(nextCatalog)
      setStatusText('Draft saved to the Lua script catalog.')
    } catch (error) {
      setErrorText(
        error instanceof Error ? error.message : 'Failed to save draft.',
      )
    } finally {
      setOperation('idle')
    }
  }, [draftDefinition, draftSpawnEnabled, replaceCatalog])

  const publishDraft = React.useCallback(async () => {
    if (!selectedEntry) return
    setOperation('publishing')
    setErrorText(null)
    setStatusText(null)
    try {
      const nextCatalog = await apiPost<GenesisPlanetCatalog>(
        `/api/genesis/planets/${encodeURIComponent(selectedEntry.planetId)}/publish`,
      )
      replaceCatalog(nextCatalog)
      setStatusText('Genesis draft published.')
    } catch (error) {
      setErrorText(
        error instanceof Error ? error.message : 'Failed to publish draft.',
      )
    } finally {
      setOperation('idle')
    }
  }, [replaceCatalog, selectedEntry])

  const discardDraft = React.useCallback(async () => {
    if (!selectedEntry) return
    setOperation('discarding')
    setErrorText(null)
    setStatusText(null)
    try {
      const nextCatalog = await apiDelete<GenesisPlanetCatalog>(
        `/api/genesis/planets/${encodeURIComponent(selectedEntry.planetId)}/draft`,
      )
      replaceCatalog(nextCatalog)
      setStatusText('Genesis draft discarded.')
    } catch (error) {
      setErrorText(
        error instanceof Error ? error.message : 'Failed to discard draft.',
      )
    } finally {
      setOperation('idle')
    }
  }, [replaceCatalog, selectedEntry])

  const operationBusy = operation !== 'idle'

  return (
    <AppLayout
      sidebarWidth={sidebarWidth}
      detailPanelWidth={detailPanelWidth}
      onSidebarResize={setSidebarWidth}
      onDetailPanelResize={setDetailPanelWidth}
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
                      {entry.hasDraft ? (
                        <Badge variant="secondary">Draft</Badge>
                      ) : null}
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
      detailPanel={
        <GenesisActionPanel
          entry={selectedEntry}
          dirty={dirty}
          errorText={errorText}
          statusText={statusText}
          operation={operation}
          onSave={() => void saveDraft()}
          onPublish={() => void publishDraft()}
          onDiscard={() => void discardDraft()}
        />
      }
    >
      <HorizontalSplitPanels
        leftWidth={definitionPanelWidth}
        minLeftWidth={520}
        minRightWidth={360}
        onLeftWidthChange={setDefinitionPanelWidth}
        left={
          <Panel>
            <PanelHeader>
              <div className="flex flex-wrap items-center gap-3">
                <Globe2 className="h-5 w-5 text-primary" />
                <div>
                  <div className="text-sm font-semibold uppercase tracking-[0.18em] text-primary">
                    Planet Definition
                  </div>
                  <div className="text-xs text-muted-foreground">
                    Edits save to planet Lua drafts and update the registry
                    draft together.
                  </div>
                </div>
                <div className="ml-auto flex flex-wrap items-center gap-2">
                  <Badge variant="outline">{entries.length} bodies</Badge>
                  {dirty ? (
                    <Badge variant="secondary">Unsaved edits</Badge>
                  ) : null}
                  {catalog?.registryHasDraft ? (
                    <Badge variant="secondary">Registry draft</Badge>
                  ) : null}
                </div>
              </div>
            </PanelHeader>
            <PanelContent>
              {draftDefinition ? (
                <ScrollArea className="h-full">
                  <div className="grid gap-4 p-4 xl:grid-cols-2">
                    <GenesisIdentityForm
                      definition={draftDefinition}
                      spawnEnabled={draftSpawnEnabled}
                      onSpawnEnabledChange={setDraftSpawnEnabled}
                      onUpdate={updateDefinition}
                    />
                    <GenesisSpawnForm
                      definition={draftDefinition}
                      onUpdate={updateDefinition}
                    />
                    <GenesisShaderForm
                      definition={draftDefinition}
                      onUpdateSettings={updateSettings}
                    />
                  </div>
                </ScrollArea>
              ) : (
                <div className="flex h-full min-h-96 items-center justify-center border border-border bg-background/40">
                  <div className="max-w-md text-center text-sm text-muted-foreground">
                    Select a planet definition to edit Genesis registry data.
                  </div>
                </div>
              )}
            </PanelContent>
          </Panel>
        }
        right={
          <Panel>
            <PanelHeader>
              <div className="flex flex-wrap items-center gap-3">
                <Dices className="h-5 w-5 text-primary" />
                <div>
                  <div className="text-sm font-semibold uppercase tracking-[0.18em] text-primary">
                    Planet Preview
                  </div>
                  <div className="text-xs text-muted-foreground">
                    Live shader preview from the selected Genesis definition.
                  </div>
                </div>
                <div className="ml-auto flex flex-wrap gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!draftDefinition || operationBusy}
                    onClick={() =>
                      setDraftDefinition((current) =>
                        current ? randomizeDefinition(current) : current,
                      )
                    }
                  >
                    <Dices className="mr-2 h-4 w-4" />
                    Randomize
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!draftDefinition || operationBusy}
                    onClick={() =>
                      setDraftDefinition((current) =>
                        current
                          ? randomizeDefinition(nextSeedDefinition(current))
                          : current,
                      )
                    }
                  >
                    <RefreshCw className="mr-2 h-4 w-4" />
                    New Seed
                  </Button>
                </div>
              </div>
            </PanelHeader>
            <PanelContent>
              {draftDefinition ? (
                <ScrollArea className="h-full">
                  <div className="p-4">
                    <GenesisPlanetPreview definition={draftDefinition} />
                  </div>
                </ScrollArea>
              ) : (
                <div className="flex h-full min-h-96 items-center justify-center border border-border bg-background/40">
                  <div className="max-w-md text-center text-sm text-muted-foreground">
                    Select a planet definition to preview its shader.
                  </div>
                </div>
              )}
            </PanelContent>
          </Panel>
        }
      />
    </AppLayout>
  )
}

function GenesisIdentityForm({
  definition,
  spawnEnabled,
  onSpawnEnabledChange,
  onUpdate,
}: {
  definition: GenesisPlanetDefinition
  spawnEnabled: boolean
  onSpawnEnabledChange: (value: boolean) => void
  onUpdate: (patch: Partial<GenesisPlanetDefinition>) => void
}) {
  return (
    <FormSection title="Identity">
      <TextField label="Planet ID" value={definition.planet_id} readOnly />
      <TextField label="Lua File" value={definition.script_path} readOnly />
      <TextField
        label="Display Name"
        value={definition.display_name}
        onChange={(display_name) => onUpdate({ display_name })}
      />
      <TextField
        label="Tags"
        value={joinList(definition.tags)}
        onChange={(value) => onUpdate({ tags: parseList(value) })}
      />
      <TextField
        label="Entity Labels"
        value={joinList(definition.entity_labels)}
        onChange={(value) => onUpdate({ entity_labels: parseList(value) })}
      />
      <ToggleRow
        label="Spawn In Bootstrap"
        checked={spawnEnabled}
        onChange={onSpawnEnabledChange}
      />
    </FormSection>
  )
}

function GenesisSpawnForm({
  definition,
  onUpdate,
}: {
  definition: GenesisPlanetDefinition
  onUpdate: (patch: Partial<GenesisPlanetDefinition>) => void
}) {
  const spawn = definition.spawn
  const updateSpawn = (patch: Partial<typeof spawn>) =>
    onUpdate({ spawn: { ...spawn, ...patch } })
  return (
    <FormSection title="Spawn">
      <TextField label="Entity UUID" value={spawn.entity_id} readOnly />
      <TextField
        label="Owner ID"
        value={spawn.owner_id}
        onChange={(owner_id) => updateSpawn({ owner_id })}
      />
      <NumberField
        label="Size M"
        value={spawn.size_m}
        onChange={(size_m) => updateSpawn({ size_m })}
      />
      <NumberField
        label="Position X"
        value={spawn.spawn_position[0]}
        onChange={(x) =>
          updateSpawn({ spawn_position: [x, spawn.spawn_position[1]] })
        }
      />
      <NumberField
        label="Position Y"
        value={spawn.spawn_position[1]}
        onChange={(y) =>
          updateSpawn({ spawn_position: [spawn.spawn_position[0], y] })
        }
      />
      <NumberField
        label="Rotation Rad"
        value={spawn.spawn_rotation_rad}
        onChange={(spawn_rotation_rad) => updateSpawn({ spawn_rotation_rad })}
      />
      <TextField
        label="Map Icon Asset"
        value={spawn.map_icon_asset_id}
        onChange={(map_icon_asset_id) => updateSpawn({ map_icon_asset_id })}
      />
      <TextField
        label="Shader Asset"
        value={spawn.planet_visual_shader_asset_id}
        onChange={(planet_visual_shader_asset_id) =>
          updateSpawn({ planet_visual_shader_asset_id })
        }
      />
    </FormSection>
  )
}

function GenesisShaderForm({
  definition,
  onUpdateSettings,
}: {
  definition: GenesisPlanetDefinition
  onUpdateSettings: (patch: Partial<GenesisPlanetShaderSettings>) => void
}) {
  const settings = definition.shader_settings
  const isStar = settings.body_kind === 1
  return (
    <div className="xl:col-span-2">
      <FormSection title="Shader">
        <div className="grid gap-3 md:grid-cols-3">
          <ToggleRow
            label="Shader Enabled"
            checked={settings.enabled}
            onChange={(enabled) => onUpdateSettings({ enabled })}
          />
          <SelectField
            label="Body Kind"
            value={settings.body_kind}
            options={[
              [0, 'Planet'],
              [1, 'Star'],
              [2, 'Black Hole'],
            ]}
            onChange={(body_kind) => onUpdateSettings({ body_kind })}
          />
          <SelectField
            label="Planet Type"
            value={settings.planet_type}
            options={[
              [0, 'Terran'],
              [1, 'Desert'],
              [2, 'Lava'],
              [3, 'Ice'],
              [4, 'Gas Giant'],
              [5, 'Moon'],
            ]}
            onChange={(planet_type) => onUpdateSettings({ planet_type })}
          />
          <NumberField
            label="Seed"
            value={settings.seed}
            onChange={(seed) => onUpdateSettings({ seed: Math.trunc(seed) })}
          />
          <NumberField
            label="Radius"
            value={settings.base_radius_scale}
            step={0.01}
            onChange={(base_radius_scale) =>
              onUpdateSettings({ base_radius_scale })
            }
          />
          <NumberField
            label="Rotation"
            value={settings.rotation_speed}
            step={0.001}
            onChange={(rotation_speed) => onUpdateSettings({ rotation_speed })}
          />
          <NumberField
            label="Detail"
            value={settings.detail_level}
            step={0.01}
            onChange={(detail_level) => onUpdateSettings({ detail_level })}
          />
          <NumberField
            label="Normal"
            value={settings.normal_strength}
            step={0.01}
            onChange={(normal_strength) =>
              onUpdateSettings({ normal_strength })
            }
          />
          <NumberField
            label="Ocean Level"
            value={settings.ocean_level}
            step={0.01}
            onChange={(ocean_level) => onUpdateSettings({ ocean_level })}
          />
          <NumberField
            label={isStar ? 'Surface Coverage' : 'Cloud Coverage'}
            value={settings.cloud_coverage}
            step={0.01}
            onChange={(cloud_coverage) => onUpdateSettings({ cloud_coverage })}
          />
          <NumberField
            label={isStar ? 'Surface Alpha' : 'Cloud Alpha'}
            value={settings.cloud_alpha}
            step={0.01}
            onChange={(cloud_alpha) => onUpdateSettings({ cloud_alpha })}
          />
          <NumberField
            label={isStar ? 'Surface Scale' : 'Cloud Scale'}
            value={settings.cloud_scale}
            step={0.01}
            onChange={(cloud_scale) => onUpdateSettings({ cloud_scale })}
          />
          <NumberField
            label={isStar ? 'Flare Rate' : 'Cloud Speed'}
            value={settings.cloud_speed}
            step={0.01}
            onChange={(cloud_speed) => onUpdateSettings({ cloud_speed })}
          />
          <NumberField
            label={isStar ? 'Glow Size' : 'Atmosphere'}
            value={settings.atmosphere_thickness}
            step={0.01}
            onChange={(atmosphere_thickness) =>
              onUpdateSettings({ atmosphere_thickness })
            }
          />
          <NumberField
            label={isStar ? 'Glow Alpha' : 'Atmosphere Alpha'}
            value={settings.atmosphere_alpha}
            step={0.01}
            onChange={(atmosphere_alpha) =>
              onUpdateSettings({ atmosphere_alpha })
            }
          />
          <NumberField
            label={isStar ? 'Glow Falloff' : 'Atmosphere Falloff'}
            value={settings.atmosphere_falloff}
            step={0.01}
            onChange={(atmosphere_falloff) =>
              onUpdateSettings({ atmosphere_falloff })
            }
          />
          <NumberField
            label={isStar ? 'Flare Reach' : 'Corona Size'}
            value={settings.corona_intensity}
            step={0.01}
            onChange={(corona_intensity) =>
              onUpdateSettings({ corona_intensity })
            }
          />
          <NumberField
            label="Emissive"
            value={settings.emissive_strength}
            step={0.01}
            onChange={(emissive_strength) =>
              onUpdateSettings({ emissive_strength })
            }
          />
        </div>
        <div className="mt-4 grid gap-3 md:grid-cols-4">
          <NumberField
            label="Light Wrap"
            value={settings.light_wrap}
            step={0.01}
            onChange={(light_wrap) => onUpdateSettings({ light_wrap })}
          />
          <NumberField
            label="Ambient"
            value={settings.ambient_strength}
            step={0.01}
            onChange={(ambient_strength) =>
              onUpdateSettings({ ambient_strength })
            }
          />
          <NumberField
            label="Rim"
            value={settings.rim_strength}
            step={0.01}
            onChange={(rim_strength) => onUpdateSettings({ rim_strength })}
          />
          <NumberField
            label="Fresnel"
            value={settings.fresnel_strength}
            step={0.01}
            onChange={(fresnel_strength) =>
              onUpdateSettings({ fresnel_strength })
            }
          />
          <NumberField
            label={isStar ? 'Flare Density' : 'Surface Activity'}
            value={settings.surface_activity}
            step={0.01}
            onChange={(surface_activity) =>
              onUpdateSettings({ surface_activity })
            }
          />
          <NumberField
            label={isStar ? 'Flare Count' : 'Spot Density'}
            value={settings.spot_density}
            step={0.01}
            onChange={(spot_density) => onUpdateSettings({ spot_density })}
          />
          <NumberField
            label={isStar ? 'Arc Events' : 'Bands'}
            value={settings.bands_count}
            step={0.01}
            onChange={(bands_count) => onUpdateSettings({ bands_count })}
          />
          <NumberField
            label="Storm"
            value={settings.storm_intensity}
            step={0.01}
            onChange={(storm_intensity) =>
              onUpdateSettings({ storm_intensity })
            }
          />
          <NumberField
            label="Cloud Shadow"
            value={settings.cloud_shadow_strength}
            step={0.01}
            onChange={(cloud_shadow_strength) =>
              onUpdateSettings({ cloud_shadow_strength })
            }
          />
        </div>
        <div className="mt-4 grid gap-3 md:grid-cols-4">
          <ColorField
            label="Primary"
            value={settings.color_primary_rgb}
            onChange={(color_primary_rgb) =>
              onUpdateSettings({ color_primary_rgb })
            }
          />
          <ColorField
            label="Secondary"
            value={settings.color_secondary_rgb}
            onChange={(color_secondary_rgb) =>
              onUpdateSettings({ color_secondary_rgb })
            }
          />
          <ColorField
            label="Tertiary"
            value={settings.color_tertiary_rgb}
            onChange={(color_tertiary_rgb) =>
              onUpdateSettings({ color_tertiary_rgb })
            }
          />
          <ColorField
            label={isStar ? 'Corona / Glow' : 'Atmosphere'}
            value={settings.color_atmosphere_rgb}
            onChange={(color_atmosphere_rgb) =>
              onUpdateSettings({ color_atmosphere_rgb })
            }
          />
          <ColorField
            label="Clouds"
            value={settings.color_clouds_rgb}
            onChange={(color_clouds_rgb) =>
              onUpdateSettings({ color_clouds_rgb })
            }
          />
          <ColorField
            label={isStar ? 'Back / Shadow' : 'Night Lights'}
            value={settings.color_night_lights_rgb}
            onChange={(color_night_lights_rgb) =>
              onUpdateSettings({ color_night_lights_rgb })
            }
          />
          <ColorField
            label="Emissive"
            value={settings.color_emissive_rgb}
            onChange={(color_emissive_rgb) =>
              onUpdateSettings({ color_emissive_rgb })
            }
          />
        </div>
        <div className="mt-4 grid gap-3 md:grid-cols-4">
          <ToggleRow
            label="Surface Detail"
            checked={settings.enable_surface_detail}
            onChange={(enable_surface_detail) =>
              onUpdateSettings({ enable_surface_detail })
            }
          />
          <ToggleRow
            label="Clouds"
            checked={settings.enable_clouds}
            onChange={(enable_clouds) => onUpdateSettings({ enable_clouds })}
          />
          <ToggleRow
            label="Atmosphere"
            checked={settings.enable_atmosphere}
            onChange={(enable_atmosphere) =>
              onUpdateSettings({ enable_atmosphere })
            }
          />
          <ToggleRow
            label="Emissive"
            checked={settings.enable_emissive}
            onChange={(enable_emissive) =>
              onUpdateSettings({ enable_emissive })
            }
          />
        </div>
      </FormSection>
    </div>
  )
}

function GenesisActionPanel({
  entry,
  dirty,
  errorText,
  statusText,
  operation,
  onSave,
  onPublish,
  onDiscard,
}: {
  entry: GenesisPlanetEntry | null
  dirty: boolean
  errorText: string | null
  statusText: string | null
  operation: OperationState
  onSave: () => void
  onPublish: () => void
  onDiscard: () => void
}) {
  if (!entry) {
    return (
      <Panel>
        <PanelContent>
          <div className="p-4 text-sm text-muted-foreground">
            Select a planet definition to edit registry metadata.
          </div>
        </PanelContent>
      </Panel>
    )
  }
  const busy = operation !== 'idle'
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
          <Readout
            label="Planet Type"
            value={planetTypeLabel(entry.planetType)}
          />
          <Readout
            label="Seed"
            value={entry.seed?.toString() ?? 'Unspecified'}
          />
          <Readout
            label="Bootstrap"
            value={entry.spawnEnabled ? 'Enabled' : 'Library only'}
          />
          <div className="flex flex-wrap gap-2">
            {entry.tags.map((tag) => (
              <Badge key={tag} variant="outline">
                {tag}
              </Badge>
            ))}
          </div>
          {errorText ? (
            <div className="border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              {errorText}
            </div>
          ) : null}
          {statusText ? (
            <div className="border border-primary/40 bg-primary/10 p-3 text-sm text-primary">
              {statusText}
            </div>
          ) : null}
          <div className="grid gap-2">
            <Button disabled={!dirty || busy} onClick={onSave}>
              <Save className="mr-2 h-4 w-4" />
              {operation === 'saving' ? 'Saving...' : 'Save Draft'}
            </Button>
            <Button
              variant="outline"
              disabled={busy || dirty || !entry.hasDraft}
              onClick={onPublish}
            >
              <Upload className="mr-2 h-4 w-4" />
              {operation === 'publishing' ? 'Publishing...' : 'Publish Draft'}
            </Button>
            <Button
              variant="outline"
              disabled={busy || !entry.hasDraft}
              onClick={onDiscard}
            >
              <RotateCcw className="mr-2 h-4 w-4" />
              {operation === 'discarding' ? 'Discarding...' : 'Discard Draft'}
            </Button>
          </div>
        </div>
      </PanelContent>
    </Panel>
  )
}

function FormSection({
  title,
  children,
}: {
  title: string
  children: React.ReactNode
}) {
  return (
    <section className="border border-border bg-background/40 p-4">
      <div className="mb-4 text-[11px] uppercase tracking-[0.22em] text-primary/90">
        {title}
      </div>
      <div className="space-y-3">{children}</div>
    </section>
  )
}

function TextField({
  label,
  value,
  readOnly = false,
  onChange,
}: {
  label: string
  value: string
  readOnly?: boolean
  onChange?: (value: string) => void
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

function SelectField({
  label,
  value,
  options,
  onChange,
}: {
  label: string
  value: number
  options: Array<[number, string]>
  onChange: (value: number) => void
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </Label>
      <select
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
        className="grid-input border-input h-9 w-full border bg-background px-3 text-sm text-foreground [color-scheme:dark]"
      >
        {options.map(([optionValue, optionLabel]) => (
          <option
            key={optionValue}
            value={optionValue}
            className="bg-background text-foreground"
          >
            {optionLabel}
          </option>
        ))}
      </select>
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
    <div className="flex min-h-9 items-center justify-between gap-3 border border-border bg-background/40 px-3 py-2">
      <Label className="text-xs uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </Label>
      <Switch checked={checked} onCheckedChange={onChange} />
    </div>
  )
}

function ColorField({
  label,
  value,
  onChange,
}: {
  label: string
  value: Vec3Tuple
  onChange: (value: Vec3Tuple) => void
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </Label>
      <div className="flex items-center gap-2">
        <Input
          type="color"
          value={colorToHex(value)}
          onChange={(event) => onChange(hexToColor(event.target.value))}
          className="h-9 w-12 p-1"
        />
        <div className="font-mono text-xs text-muted-foreground">
          {value.map((channel) => channel.toFixed(2)).join(', ')}
        </div>
      </div>
    </div>
  )
}

function Readout({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-[11px] uppercase tracking-[0.16em] text-muted-foreground">
        {label}
      </div>
      <div className="mt-1 break-words font-mono text-xs text-foreground">
        {value}
      </div>
    </div>
  )
}

import * as React from 'react'
import {
  Box,
  ChevronRight,
  Copy,
  Gauge,
  Layers,
  MapPin,
  Puzzle,
  Users,
  X,
} from 'lucide-react'
import { Badge } from '../thegridcn/badge'
import type {
  ExpandedNode,
  GraphEdge,
  GraphNode,
  WorldEntity,
} from '@/components/grid/types'
import type { DataSourceMode } from '@/components/sidebar/Toolbar'
import type { GeneratedComponentRegistryResource } from '@/features/component-schema/types'
import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import { HUDFrame } from '@/components/ui/hud-frame'
import {
  ComponentEditorRenderer,
  getComponentValue,
  getEditableComponentTypeKey,
  getEditorForNode,
} from '@/components/brp-editors'
import {
  findGeneratedComponentRegistryResource,
  isGeneratedComponentRegistryTypePath,
  parseGeneratedComponentRegistryResource,
  resolveComponentRegistryEntry,
} from '@/features/component-schema/registry'

interface DetailPanelProps {
  selectedId: string | null
  entities: Array<WorldEntity>
  resources?: Array<{ typePath: string; value?: unknown; error?: string }>
  expandedNodes: Map<string, ExpandedNode>
  graphNodes: Map<string, GraphNode>
  graphEdges: Array<GraphEdge>
  onSelect: (id: string) => void
  onExpand: (id: string) => void
  onCollapse: (id: string) => void
  /** When set, BRP editable component editors are shown and updates are allowed. */
  sourceMode?: DataSourceMode
  /** Called after a component value is updated (BRP only). Caller should refresh data. */
  onComponentUpdate?: (
    entityId: string,
    typePath: string,
    componentKind: string,
    value: unknown,
  ) => Promise<void> | void
  /** Called when the close button is clicked. Caller should clear selection. */
  onClose?: () => void
}

export function DetailPanel({
  selectedId,
  entities,
  resources = [],
  expandedNodes,
  graphNodes,
  graphEdges,
  onSelect,
  onExpand: _onExpand,
  onCollapse: _onCollapse,
  sourceMode,
  onComponentUpdate,
  onClose,
}: DetailPanelProps) {
  const detailScrollAreaRef = React.useRef<HTMLDivElement | null>(null)
  const detailScrollTopRef = React.useRef(0)
  const lastSelectedIdRef = React.useRef<string | null>(null)
  const copyResetTimerRef = React.useRef<number | null>(null)
  const [copyJsonState, setCopyJsonState] = React.useState<
    'idle' | 'copied' | 'error'
  >('idle')
  const generatedComponentRegistry = React.useMemo(
    () => findGeneratedComponentRegistryResource(resources),
    [resources],
  )
  const selectedResourceTypePath = selectedId?.startsWith('resource:')
    ? selectedId.slice('resource:'.length)
    : null

  // Resolve entity details unconditionally so hook dependencies remain stable
  // across resource/entity/empty states.
  const worldEntity = selectedId
    ? entities.find((e) => e.id === selectedId)
    : undefined
  const expandedNode = selectedId ? expandedNodes.get(selectedId) : undefined
  const graphNode = selectedId ? graphNodes.get(selectedId) : undefined
  const kind =
    worldEntity?.kind || expandedNode?.kind || graphNode?.kind || 'unknown'
  const entityLabels = worldEntity?.entity_labels

  React.useLayoutEffect(() => {
    const root = detailScrollAreaRef.current
    const viewport = root?.querySelector(
      '[data-radix-scroll-area-viewport]',
    ) as HTMLDivElement | null
    if (!viewport) return

    if (lastSelectedIdRef.current !== selectedId) {
      detailScrollTopRef.current = 0
      viewport.scrollTop = 0
      lastSelectedIdRef.current = selectedId
      return
    }

    viewport.scrollTop = detailScrollTopRef.current
  }, [
    selectedId,
    graphEdges,
    graphNodes,
    expandedNodes,
    resources,
    worldEntity?.componentCount,
  ])

  React.useEffect(() => {
    const root = detailScrollAreaRef.current
    const viewport = root?.querySelector(
      '[data-radix-scroll-area-viewport]',
    ) as HTMLDivElement | null
    if (!viewport) return

    const handleScroll = () => {
      detailScrollTopRef.current = viewport.scrollTop
    }

    viewport.addEventListener('scroll', handleScroll)
    return () => {
      viewport.removeEventListener('scroll', handleScroll)
    }
  }, [selectedId])

  React.useEffect(() => {
    return () => {
      if (copyResetTimerRef.current !== null) {
        window.clearTimeout(copyResetTimerRef.current)
      }
    }
  }, [])

  const handleCopyEntityJson = React.useCallback(() => {
    if (!selectedId) {
      return
    }

    const exportPayload = buildEntityClipboardExport(
      selectedId,
      entities,
      expandedNodes,
      graphNodes,
      graphEdges,
    )
    if (!exportPayload) {
      setCopyJsonState('error')
      return
    }

    void navigator.clipboard
      .writeText(JSON.stringify(exportPayload, null, 2))
      .then(() => {
        setCopyJsonState('copied')
        if (copyResetTimerRef.current !== null) {
          window.clearTimeout(copyResetTimerRef.current)
        }
        copyResetTimerRef.current = window.setTimeout(() => {
          setCopyJsonState('idle')
          copyResetTimerRef.current = null
        }, 1400)
      })
      .catch(() => {
        setCopyJsonState('error')
      })
  }, [entities, expandedNodes, graphEdges, graphNodes, selectedId])

  if (selectedResourceTypePath) {
    const selectedResource =
      resources.find(
        (resource) => resource.typePath === selectedResourceTypePath,
      ) ?? null
    const resourceValue = selectedResource?.value
    const displayValue = formatDisplayValue(resourceValue ?? null)
    const renderEntityRegistryResource = selectedResourceTypePath.includes(
      'EntityRegistryResource',
    )
    const renderGeneratedRegistryResource =
      isGeneratedComponentRegistryTypePath(selectedResourceTypePath)
    return (
      <div className="flex flex-col h-full">
        <div className="flex-none p-5 border-b border-border-subtle">
          <div className="flex items-start justify-between gap-2">
            <div className="min-w-0 flex-1">
              <h2 className="font-semibold text-foreground truncate">
                {selectedResourceTypePath}
              </h2>
              <div className="flex items-center gap-2 mt-1 flex-wrap">
                <Badge className="capitalize text-xs">Resource</Badge>
                {selectedResource?.error ? (
                  <span className="text-xs text-destructive truncate">
                    {selectedResource.error}
                  </span>
                ) : null}
              </div>
            </div>
            {onClose ? (
              <Button
                variant="ghost"
                size="icon"
                onClick={onClose}
                className="h-8 w-8 shrink-0 text-muted-foreground hover:text-foreground"
                aria-label="Close panel"
              >
                <X className="h-4 w-4" />
              </Button>
            ) : null}
          </div>
        </div>
        <ScrollArea className="flex-1 ml-1">
          <div className="p-5 space-y-4">
            <PropertySection title="Type Path" icon={Layers}>
              <PropertyRow
                label="type_path"
                value={selectedResourceTypePath}
                mono
              />
            </PropertySection>
            <PropertySection title="Value" icon={Box}>
              {selectedResource ? (
                renderEntityRegistryResource ? (
                  <EntityRegistryResourceView value={resourceValue} />
                ) : renderGeneratedRegistryResource ? (
                  <GeneratedComponentRegistryView value={resourceValue} />
                ) : (
                  <ValueField value={displayValue} mono className="text-xs" />
                )
              ) : (
                <span className="text-sm text-muted-foreground">
                  Resource not found in current BRP snapshot
                </span>
              )}
            </PropertySection>
          </div>
        </ScrollArea>
      </div>
    )
  }

  if (!selectedId) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-muted-foreground p-8">
        <Box className="h-12 w-12 mb-4 opacity-40" />
        <p className="text-sm text-center">
          Select an entity on the grid or from the tree to view details
        </p>
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex-none p-5 border-b border-border-subtle">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2 mt-1 flex-wrap">
              {entityLabels && entityLabels.length > 0 ? (
                entityLabels.map((label) => (
                  <Badge key={label} className="capitalize text-xs">
                    {label}
                  </Badge>
                ))
              ) : (
                <Badge className="capitalize text-xs">{kind}</Badge>
              )}
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <Button
              variant="ghost"
              size="sm"
              onClick={handleCopyEntityJson}
              className="h-8 gap-2 px-2 text-muted-foreground hover:text-foreground"
              aria-label="Copy entity subtree as structured JSON"
              title="Copy entity subtree as structured JSON"
            >
              <Copy className="h-3.5 w-3.5" />
              <span className="text-[11px] uppercase tracking-[0.16em]">
                {copyJsonState === 'copied'
                  ? 'Copied'
                  : copyJsonState === 'error'
                    ? 'Retry'
                    : 'Copy JSON'}
              </span>
            </Button>
            {onClose && (
              <Button
                variant="ghost"
                size="icon"
                onClick={onClose}
                className="h-8 w-8 shrink-0 text-muted-foreground hover:text-foreground"
                aria-label="Close panel"
              >
                <X className="h-4 w-4" />
              </Button>
            )}
          </div>
        </div>
      </div>

      {/* Content - single scrollable properties view */}
      <ScrollArea ref={detailScrollAreaRef} className="flex-1 ml-1">
        <div className="p-5 space-y-4">
          {/* Position: X/Y on same line as heading, no unit. Speed: magnitude with m/s. */}
          {worldEntity && (
            <>
              <div>
                <div className="flex items-center justify-between gap-2 mb-2 w-full">
                  <div className="flex items-center gap-2 min-w-0">
                    <MapPin className="h-4 w-4 shrink-0 text-muted-foreground" />
                    <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                      Position
                    </h3>
                  </div>
                  <span className="text-xs text-foreground text-right shrink-0 tabular-nums">
                    <span className="text-muted-foreground">X:</span>{' '}
                    {worldEntity.x.toFixed(2)} ·{' '}
                    <span className="text-muted-foreground">Y:</span>{' '}
                    {worldEntity.y.toFixed(2)}
                  </span>
                </div>
              </div>
              <div>
                <div className="flex items-center justify-between gap-2 mb-2 w-full">
                  <div className="flex items-center gap-2 min-w-0">
                    <Gauge className="h-4 w-4 shrink-0 text-muted-foreground" />
                    <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                      Speed
                    </h3>
                  </div>
                  <span className="text-xs text-foreground text-right shrink-0 tabular-nums">
                    {Math.sqrt(
                      worldEntity.vx * worldEntity.vx +
                        worldEntity.vy * worldEntity.vy,
                    ).toFixed(2)}{' '}
                    <span className="text-muted-foreground">m/s</span>
                  </span>
                </div>
              </div>
            </>
          )}

          <div>
            <div className="flex items-left justify-between gap-2 mb-2 w-full flex-col">
              <div className="flex items-center gap-2 min-w-0">
                <Layers className="h-4 w-4 shrink-0 text-muted-foreground" />
                <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                  Entity Guid
                </h3>
              </div>
              <div className="flex items-center gap-1 flex-1 min-w-0 justify-end">
                <span className="text-xs text-foreground font-mono tabular-nums min-w-0 break-all text-right">
                  {worldEntity?.entityGuid ?? '—'}
                </span>
                {worldEntity?.entityGuid ? (
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7 text-muted-foreground hover:text-foreground"
                    aria-label="Copy Entity Guid"
                    onClick={() => {
                      if (!worldEntity.entityGuid) return
                      void navigator.clipboard.writeText(worldEntity.entityGuid)
                    }}
                  >
                    <Copy className="h-3.5 w-3.5" />
                  </Button>
                ) : null}
              </div>
            </div>
          </div>

          {/* Parent: link to parent entity like children */}
          {worldEntity?.parentEntityId && (
            <div>
              <div className="flex items-center justify-between gap-2 mb-2 w-full">
                <div className="flex items-center gap-2 min-w-0">
                  <Box className="h-4 w-4 shrink-0 text-muted-foreground" />
                  <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                    Parent
                  </h3>
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() => onSelect(worldEntity.parentEntityId!)}
                  className="grid-sidebar-nav__item h-auto justify-start px-2 py-1 text-left text-sm text-foreground"
                >
                  <span className="truncate max-w-[140px]">
                    {entities.find((e) => e.id === worldEntity.parentEntityId)
                      ?.name ?? worldEntity.parentEntityId}
                  </span>
                  <ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0 rotate-180" />
                </Button>
              </div>
            </div>
          )}

          {/* Child Entities Section */}
          <ChildEntitiesSection
            entityId={selectedId}
            entities={entities}
            onSelect={onSelect}
          />

          {/* Graph properties (exclude entity_labels) */}
          {/* {graphNode?.properties &&
            Object.keys(graphNode.properties).length > 0 && (
              <PropertySection title="Graph Properties" icon={Box}>
                {Object.entries(graphNode.properties)
                  .filter(([key]) => key !== 'entity_labels')
                  .map(([key, value]) => (
                    <PropertyRow key={key} label={key} value={value} mono />
                  ))}
              </PropertySection>
            )} */}

          {/* Expanded node properties */}
          {expandedNode?.properties &&
            Object.keys(expandedNode.properties).length > 0 && (
              <PropertySection title="Node Properties" icon={Box}>
                {Object.entries(expandedNode.properties).map(([key, value]) => (
                  <PropertyRow key={key} label={key} value={value} mono />
                ))}
              </PropertySection>
            )}

          {/* Components - same style heading as Position, Speed, Children; count in header */}
          {(() => {
            const componentCount =
              worldEntity?.componentCount ??
              graphEdges.filter(
                (e) => e.from === selectedId && e.label === 'HAS_COMPONENT',
              ).length
            return (
              <div>
                <div className="flex items-center gap-2 mb-2">
                  <Puzzle className="h-4 w-4 text-muted-foreground" />
                  <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                    Components ({componentCount})
                  </h3>
                </div>
                <div className="pl-6">
                  <ComponentsList
                    entityId={selectedId}
                    graphNodes={graphNodes}
                    graphEdges={graphEdges}
                    generatedComponentRegistry={generatedComponentRegistry}
                    sourceMode={sourceMode}
                    onComponentUpdate={onComponentUpdate}
                  />
                </div>
              </div>
            )
          })()}
        </div>
      </ScrollArea>
    </div>
  )
}

type EntityClipboardComponentExport = {
  nodeId: string
  label: string
  kind: string
  typePath: string | null
  properties: Record<string, unknown>
  value: unknown
}

type EntityClipboardNodeExport = {
  entity: WorldEntity | null
  graphNode: GraphNode | null
  expandedNode: ExpandedNode | null
  components: Array<EntityClipboardComponentExport>
  children: Array<EntityClipboardNodeExport>
}

type EntityClipboardExport = {
  rootEntityId: string
  entityTree: EntityClipboardNodeExport
}

function buildEntityClipboardExport(
  rootEntityId: string,
  entities: Array<WorldEntity>,
  expandedNodes: Map<string, ExpandedNode>,
  graphNodes: Map<string, GraphNode>,
  graphEdges: Array<GraphEdge>,
): EntityClipboardExport | null {
  const entitiesById = new Map(entities.map((entity) => [entity.id, entity]))
  const childrenByParent = new Map<string, Array<string>>()
  for (const entity of entities) {
    if (!entity.parentEntityId) continue
    const list = childrenByParent.get(entity.parentEntityId)
    if (list) {
      list.push(entity.id)
    } else {
      childrenByParent.set(entity.parentEntityId, [entity.id])
    }
  }

  const componentIdsByEntityId = new Map<string, Array<string>>()
  for (const edge of graphEdges) {
    if (edge.label !== 'HAS_COMPONENT') continue
    const list = componentIdsByEntityId.get(edge.from)
    if (list) {
      list.push(edge.to)
    } else {
      componentIdsByEntityId.set(edge.from, [edge.to])
    }
  }

  const seen = new Set<string>()
  const buildNode = (entityId: string): EntityClipboardNodeExport | null => {
    if (seen.has(entityId)) {
      return null
    }
    seen.add(entityId)

    const entity = entitiesById.get(entityId) ?? null
    const graphNode = graphNodes.get(entityId) ?? null
    const expandedNode = expandedNodes.get(entityId) ?? null
    const components = (componentIdsByEntityId.get(entityId) ?? [])
      .map((componentId) => {
        const componentNode = graphNodes.get(componentId)
        if (!componentNode) {
          return null
        }
        const typePath = componentNode.properties.typePath
        const value = componentNode.properties.value
        return {
          nodeId: componentId,
          label: componentNode.label,
          kind: componentNode.kind,
          typePath: typeof typePath === 'string' ? typePath : null,
          properties: componentNode.properties,
          value,
        } satisfies EntityClipboardComponentExport
      })
      .filter(
        (
          component,
        ): component is EntityClipboardComponentExport => component !== null,
      )
      .sort((a, b) => {
        const left = a.typePath ?? a.label
        const right = b.typePath ?? b.label
        return left.localeCompare(right)
      })

    const children = (childrenByParent.get(entityId) ?? [])
      .map((childId) => buildNode(childId))
      .filter((child): child is EntityClipboardNodeExport => child !== null)
      .sort((a, b) => {
        const left = a.entity?.name ?? a.graphNode?.label ?? a.entity?.id ?? ''
        const right =
          b.entity?.name ?? b.graphNode?.label ?? b.entity?.id ?? ''
        return left.localeCompare(right)
      })

    return {
      entity,
      graphNode,
      expandedNode,
      components,
      children,
    }
  }

  const entityTree = buildNode(rootEntityId)
  if (!entityTree) {
    return null
  }

  return {
    rootEntityId,
    entityTree,
  }
}

type EntityRegistryResourceEntry = {
  entity_id: string
  entity_class?: string
  graph_records_script?: string
  required_component_kinds?: Array<string>
}

function parseEntityRegistryResourceEntries(
  value: unknown,
): Array<EntityRegistryResourceEntry> {
  if (typeof value !== 'object' || value === null) return []
  const root = value as Record<string, unknown>
  const wrapped =
    typeof root.value === 'object' && root.value !== null
      ? (root.value as Record<string, unknown>)
      : root
  const entries = wrapped.entries
  if (!Array.isArray(entries)) return []
  const parsed: Array<EntityRegistryResourceEntry> = []
  for (const rawEntry of entries) {
    if (typeof rawEntry !== 'object' || rawEntry === null) continue
    const entry = rawEntry as Record<string, unknown>
    const entityId = entry.entity_id
    if (typeof entityId !== 'string') continue
    parsed.push({
      entity_id: entityId,
      entity_class:
        typeof entry.entity_class === 'string' ? entry.entity_class : undefined,
      graph_records_script:
        typeof entry.graph_records_script === 'string'
          ? entry.graph_records_script
          : undefined,
      required_component_kinds: Array.isArray(entry.required_component_kinds)
        ? entry.required_component_kinds.filter(
            (kind): kind is string => typeof kind === 'string',
          )
        : undefined,
    })
  }
  return parsed.sort((a, b) => a.entity_id.localeCompare(b.entity_id))
}

function EntityRegistryResourceView({ value }: { value: unknown }) {
  const entries = React.useMemo(
    () => parseEntityRegistryResourceEntries(value),
    [value],
  )
  if (entries.length === 0) {
    return (
      <div className="text-sm text-muted-foreground">
        EntityRegistryResource has no entries in this snapshot.
      </div>
    )
  }
  return (
    <div className="space-y-1">
      {entries.map((entry) => (
        <EntityRegistryEntryCard key={entry.entity_id} entry={entry} />
      ))}
    </div>
  )
}

function EntityRegistryEntryCard({
  entry,
}: {
  entry: EntityRegistryResourceEntry
}) {
  const [expanded, setExpanded] = React.useState(false)
  return (
    <HUDFrame>
      <Button
        type="button"
        variant="ghost"
        onClick={() => setExpanded((prev) => !prev)}
        className="grid-sidebar-nav__item h-auto w-full justify-start rounded-none px-3 py-2 text-left"
      >
        <span className="flex-1 text-sm font-medium min-w-0 flex items-baseline gap-2 w-0">
          <span className="font-mono truncate">{entry.entity_id}</span>
          {entry.entity_class ? (
            <span className="text-muted-foreground font-normal text-xs truncate">
              {entry.entity_class}
            </span>
          ) : null}
        </span>
        <ChevronRight
          className={cn(
            'h-4 w-4 text-muted-foreground transition-transform flex-none',
            expanded && 'rotate-90',
          )}
        />
      </Button>
      {expanded ? (
        <div className="px-3 py-2 bg-secondary/20 border-t border-border space-y-1">
          {entry.entity_class ? (
            <div className="flex items-start gap-2 py-0.5 min-w-0">
              <span className="text-xs text-muted-foreground shrink-0">
                class
              </span>
              <ValueField
                value={formatDisplayValue(entry.entity_class)}
                mono
                className="text-xs text-foreground/90 min-w-0 flex-1 truncate grow w-0"
              />
            </div>
          ) : null}
          {entry.graph_records_script ? (
            <div className="flex items-start gap-2 py-0.5 min-w-0">
              <span className="text-xs text-muted-foreground shrink-0">
                script
              </span>
              <ValueField
                value={formatDisplayValue(entry.graph_records_script)}
                mono
                className="text-xs text-foreground/90 min-w-0 flex-1 truncate grow w-0"
              />
            </div>
          ) : null}
          {entry.required_component_kinds &&
          entry.required_component_kinds.length > 0 ? (
            <div className="flex items-start gap-2 py-0.5 min-w-0">
              <span className="text-xs text-muted-foreground shrink-0">
                components
              </span>
              <ValueField
                value={formatDisplayValue(entry.required_component_kinds)}
                mono
                className="text-xs text-foreground/90 min-w-0 flex-1 truncate grow w-0"
              />
            </div>
          ) : null}
        </div>
      ) : null}
    </HUDFrame>
  )
}

function GeneratedComponentRegistryView({ value }: { value: unknown }) {
  const registry = React.useMemo(
    () => parseGeneratedComponentRegistryResource(value),
    [value],
  )

  if (
    !registry ||
    (registry.entries.length === 0 && registry.shader_entries.length === 0)
  ) {
    return (
      <div className="text-sm text-muted-foreground">
        GeneratedComponentRegistry has no entries in this snapshot.
      </div>
    )
  }

  return (
    <div className="space-y-1">
      {registry.shader_entries.length > 0 ? (
        <HUDFrame className="px-3 py-2 text-xs">
          <div className="font-medium text-foreground">
            Shader editor entries
          </div>
          <div className="mt-1 text-muted-foreground">
            {registry.shader_entries.length} shader assets
          </div>
        </HUDFrame>
      ) : null}
      {registry.entries.map((entry) => (
        <HUDFrame key={entry.type_path} className="px-3 py-2 text-xs">
          <div className="font-medium text-foreground">
            {entry.component_kind}
          </div>
          <div className="font-mono text-muted-foreground">
            {entry.type_path}
          </div>
          <div className="mt-1 text-muted-foreground">
            {entry.editor_schema.fields.length} fields
          </div>
        </HUDFrame>
      ))}
    </div>
  )
}

interface PropertySectionProps {
  title: string
  icon: React.ComponentType<{ className?: string }>
  children: React.ReactNode
}

function PropertySection({
  title,
  icon: Icon,
  children,
}: PropertySectionProps) {
  return (
    <div>
      <div className="flex items-center gap-2 mb-2">
        <Icon className="h-4 w-4 text-muted-foreground" />
        <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
          {title}
        </h3>
      </div>
      <div className="space-y-1 pl-6">{children}</div>
    </div>
  )
}

interface PropertyRowProps {
  label: string
  value: unknown
  unit?: string
  mono?: boolean
}

function PropertyRow({ label, value, unit, mono }: PropertyRowProps) {
  const formattedValue = formatDisplayValue(value)

  return (
    <div className="flex items-start gap-2 py-0.5 min-w-0">
      <span className="text-sm text-muted-foreground shrink-0">{label}</span>
      <ValueField
        value={formattedValue}
        mono={mono}
        unit={formattedValue.isStructured ? undefined : unit}
      />
    </div>
  )
}

interface ComponentsListProps {
  entityId: string
  graphNodes: Map<string, GraphNode>
  graphEdges: Array<GraphEdge>
  generatedComponentRegistry: GeneratedComponentRegistryResource | null
  sourceMode?: DataSourceMode
  onComponentUpdate?: (
    entityId: string,
    typePath: string,
    componentKind: string,
    value: unknown,
  ) => Promise<void> | void
}

function ComponentsList({
  entityId,
  graphNodes,
  graphEdges,
  generatedComponentRegistry,
  sourceMode: _sourceMode,
  onComponentUpdate,
}: ComponentsListProps) {
  const [expandedComponents, setExpandedComponents] = React.useState<
    Set<string>
  >(new Set())

  const components = React.useMemo(() => {
    const componentIds = graphEdges
      .filter(
        (edge) => edge.from === entityId && edge.label === 'HAS_COMPONENT',
      )
      .map((edge) => edge.to)
    return componentIds
      .map((id) => {
        const node = graphNodes.get(id)
        return node ? { id, node } : null
      })
      .filter(
        (entry): entry is { id: string; node: GraphNode } => entry !== null,
      )
  }, [entityId, graphEdges, graphNodes])

  const getStableComponentKey = React.useCallback(
    (id: string, node: GraphNode) => {
      const typePath = node.properties.typePath
      if (typeof typePath === 'string' && typePath.length > 0) {
        return `${entityId}::${typePath}`
      }
      const componentKind = node.properties.component_kind
      if (typeof componentKind === 'string' && componentKind.length > 0) {
        return `${entityId}::${componentKind}`
      }
      return `${entityId}::${node.label}::${id}`
    },
    [entityId],
  )

  const toggleComponent = (componentKey: string) => {
    setExpandedComponents((prev) => {
      const next = new Set(prev)
      if (next.has(componentKey)) {
        next.delete(componentKey)
      } else {
        next.add(componentKey)
      }
      return next
    })
  }

  /** Flatten object values into dotted key paths so e.g. value: { x: 1, y: 2 } → value.x, value.y */
  const flattenProperties = (
    properties: Record<string, unknown>,
    prefix = '',
  ): Array<{ keyPath: string; value: unknown }> => {
    const result: Array<{ keyPath: string; value: unknown }> = []
    for (const [key, value] of Object.entries(properties)) {
      const keyPath = prefix ? `${prefix}.${key}` : key
      const isPlainObject =
        value !== null &&
        typeof value === 'object' &&
        !Array.isArray(value) &&
        Object.getPrototypeOf(value) === Object.prototype
      if (isPlainObject) {
        result.push(
          ...flattenProperties(value as Record<string, unknown>, keyPath),
        )
      } else {
        result.push({ keyPath, value })
      }
    }
    return result
  }

  // Extract the most relevant value to show in the header
  const getPreviewValue = (properties: Record<string, unknown>): string => {
    const entries = Object.entries(properties)
    if (entries.length === 0) return ''

    // Priority order for display values
    const priorityKeys = [
      'value',
      'name',
      'amount',
      'fuel_kg',
      'health',
      'x',
      'pos_x',
      'turn_rate_deg_s',
      'thrust_n',
    ]

    // Check priority keys first
    for (const key of priorityKeys) {
      const entry = entries.find(([k]) => k.toLowerCase().includes(key))
      if (entry) {
        return formatValueCompact(entry[1])
      }
    }

    // If only one property, show it
    if (entries.length === 1) {
      return formatValueCompact(entries[0][1])
    }

    // For objects with x,y show compact position
    if (entries.some(([k]) => k === 'x' || k === 'pos_x')) {
      const x = entries.find(([k]) => k === 'x' || k === 'pos_x')?.[1]
      const y = entries.find(([k]) => k === 'y' || k === 'pos_y')?.[1]
      if (x !== undefined && y !== undefined) {
        return `{x: ${formatNumber(x)}, y: ${formatNumber(y)}}`
      }
    }

    // Multiple properties, show count
    return `{${entries.length} fields}`
  }

  if (components.length === 0) {
    return (
      <div className="text-sm text-muted-foreground text-center py-6">
        <p>No components linked for this entity.</p>
        <p className="mt-1 text-xs">
          Components are read from graph `HAS_COMPONENT` edges.
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-1 flex flex-col">
      {components
        .sort((a, b) => a.node.label.localeCompare(b.node.label))
        .map(({ id, node }) => {
          const componentKey = getStableComponentKey(id, node)
          const isExpanded = expandedComponents.has(componentKey)
          const hasProperties = Object.keys(node.properties).length > 0
          const previewValue = hasProperties
            ? getPreviewValue(node.properties)
            : ''
          const componentEntry = resolveComponentRegistryEntry(
            node,
            generatedComponentRegistry,
          )
          const legacyEditor = getEditorForNode(node)
          const shouldUseLegacyEditor =
            Boolean(legacyEditor) &&
            (!componentEntry ||
              componentEntry.editor_schema.fields.length === 0)
          const hasStructuredEditor =
            Boolean(legacyEditor) || Boolean(componentEntry)
          const legacyTypePath =
            (typeof node.properties.typePath === 'string'
              ? node.properties.typePath
              : getEditableComponentTypeKey(node)) ?? null
          const legacyComponentKind =
            (typeof node.properties.component_kind === 'string'
              ? node.properties.component_kind
              : componentEntry?.component_kind) ?? null

          return (
            <HUDFrame key={componentKey}>
              <Button
                onClick={() => {
                  if (hasProperties) {
                    toggleComponent(componentKey)
                  }
                }}
                type="button"
                variant="ghost"
                className="grid-sidebar-nav__item h-auto w-full justify-start rounded-none px-3 py-2 text-left"
              >
                <span className="flex-1 text-sm font-medium min-w-0 flex items-baseline gap-2 w-0">
                  <span className="flex-none">{node.label}</span>
                  {previewValue && (
                    <span className="text-muted-foreground font-normal font-mono text-xs truncate">
                      {previewValue}
                    </span>
                  )}
                </span>
                {hasProperties && (
                  <ChevronRight
                    className={cn(
                      'h-4 w-4 text-muted-foreground transition-transform flex-none',
                      isExpanded && 'rotate-90',
                    )}
                  />
                )}
              </Button>

              {isExpanded && hasProperties && (
                <div className="px-3 py-2 bg-secondary/20 border-t border-border space-y-3 flex flex-col pt-4">
                  {!hasStructuredEditor ? (
                    <div className="space-y-1 flex flex-row">
                      <div className="space-y-1 flex flex-col grow">
                        {flattenProperties(node.properties).map(
                          ({ keyPath, value }) => (
                            <div
                              key={keyPath}
                              className="flex items-start gap-2 py-0.5 min-w-0 w-full"
                            >
                              <span className="text-xs text-muted-foreground truncate shrink-0">
                                {keyPath.split('.').pop() ?? keyPath}
                              </span>
                              <ValueField
                                value={formatDisplayValue(value)}
                                mono
                                className="text-xs text-foreground/90 min-w-0 flex-1 truncate grow w-0"
                              />
                            </div>
                          ),
                        )}
                      </div>
                    </div>
                  ) : null}
                  <div className="space-y-1 grow">
                    {shouldUseLegacyEditor && legacyEditor ? (
                      React.createElement(legacyEditor, {
                        componentNodeId: id,
                        entityId,
                        node,
                        value: getComponentValue(node),
                        onChange: (value: unknown) => {
                          if (!legacyTypePath || !legacyComponentKind) return
                          onComponentUpdate?.(
                            entityId,
                            legacyTypePath,
                            legacyComponentKind,
                            value,
                          )
                        },
                        readOnly: !onComponentUpdate,
                      })
                    ) : (
                      <ComponentEditorRenderer
                        componentNodeId={id}
                        entityId={entityId}
                        node={node}
                        generatedComponentRegistry={generatedComponentRegistry}
                        onUpdate={(typePath, componentKind, value) => {
                          onComponentUpdate?.(
                            entityId,
                            typePath,
                            componentKind,
                            value,
                          )
                        }}
                        readOnly={!onComponentUpdate}
                      />
                    )}
                  </div>
                </div>
              )}
            </HUDFrame>
          )
        })}
    </div>
  )
}

function formatNumber(value: unknown): string {
  if (typeof value === 'number') {
    if (Number.isInteger(value)) return String(value)
    return value.toFixed(2)
  }
  if (typeof value === 'string') {
    const num = Number(value)
    if (!isNaN(num)) {
      if (Number.isInteger(num)) return String(num)
      return num.toFixed(2)
    }
  }
  return String(value)
}

function formatValueCompact(value: unknown): string {
  if (value === null || value === undefined) return 'null'
  if (typeof value === 'number') {
    if (Number.isInteger(value)) return String(value)
    return value.toFixed(2)
  }
  // if (typeof value === 'string') {
  //   if (value.length > 60) return value.substring(0, 27) + '...'
  //   return value
  // }
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'object') {
    const str = JSON.stringify(value)
    // if (str.length > 40) return str.substring(0, 37) + '...'
    return str
  }
  return String(value)
}

type FormattedValue = {
  text: string
  isStructured: boolean
}

function rightIndentStructuredText(text: string): string {
  return text
    .split('\n')
    .map((line) => {
      const leading = line.match(/^\s+/)?.[0] ?? ''
      if (!leading) return line
      return `${line.trimStart()}${leading}`
    })
    .join('\n')
}

function formatDisplayValue(value: unknown): FormattedValue {
  if (value === null || value === undefined) {
    return { text: 'null', isStructured: false }
  }

  if (typeof value === 'object') {
    return {
      text: rightIndentStructuredText(JSON.stringify(value, null, 2)),
      isStructured: true,
    }
  }

  if (typeof value === 'string') {
    const trimmed = value.trim()
    if (
      (trimmed.startsWith('{') && trimmed.endsWith('}')) ||
      (trimmed.startsWith('[') && trimmed.endsWith(']'))
    ) {
      try {
        const parsed = JSON.parse(trimmed) as unknown
        if (parsed && typeof parsed === 'object') {
          return {
            text: rightIndentStructuredText(JSON.stringify(parsed, null, 2)),
            isStructured: true,
          }
        }
      } catch {
        // Treat as plain string when JSON parsing fails.
      }
    }
    return { text: value, isStructured: false }
  }

  return { text: String(value), isStructured: false }
}

function ValueField({
  value,
  mono,
  unit,
  className,
}: {
  value: FormattedValue
  mono?: boolean
  unit?: string
  className?: string
}) {
  if (value.isStructured) {
    return (
      <pre
        className={cn(
          'min-w-0 flex-1 overflow-hidden text-ellipsis line-clamp-3 text-right text-sm text-foreground',
          mono && 'font-mono text-xs',
          className,
        )}
      >
        {value.text}
      </pre>
    )
  }

  return (
    <span
      className={cn(
        'min-w-0 flex-1 truncate text-right text-sm text-foreground block',
        mono && 'font-mono text-xs',
        className,
      )}
    >
      {value.text}
      {unit && <span className="text-muted-foreground ml-1">{unit}</span>}
    </span>
  )
}

interface ChildEntitiesSectionProps {
  entityId: string
  entities: Array<WorldEntity>
  onSelect: (id: string) => void
}

function ChildEntitiesSection({
  entityId,
  entities,
  onSelect,
}: ChildEntitiesSectionProps) {
  const children = React.useMemo(() => {
    return entities.filter((e) => e.parentEntityId === entityId)
  }, [entityId, entities])

  if (children.length === 0) {
    return null
  }

  return (
    <PropertySection title="Children" icon={Users}>
      <div className="space-y-1">
        {children.map((child) => (
          <Button
            key={child.id}
            onClick={() => onSelect(child.id)}
            type="button"
            variant="ghost"
            className="grid-sidebar-nav__item h-auto w-full justify-start gap-2 px-2 py-1.5 text-left text-sm"
          >
            <Box className="h-3.5 w-3.5 text-primary/60 flex-none" />
            <span className="truncate flex-1">{child.name}</span>
            <Badge variant="outline" className="text-xs">
              {child.componentCount}c
            </Badge>
          </Button>
        ))}
      </div>
    </PropertySection>
  )
}

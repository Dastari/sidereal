import * as React from 'react'
import { Box, ChevronRight, Layers, MapPin, Users } from 'lucide-react'
import type {
  ExpandedNode,
  GraphEdge,
  GraphNode,
  WorldEntity,
} from '@/components/grid/types'
import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

interface DetailPanelProps {
  selectedId: string | null
  entities: Array<WorldEntity>
  expandedNodes: Map<string, ExpandedNode>
  graphNodes: Map<string, GraphNode>
  graphEdges: Array<GraphEdge>
  onSelect: (id: string) => void
  onExpand: (id: string) => void
  onCollapse: (id: string) => void
}

export function DetailPanel({
  selectedId,
  entities,
  expandedNodes,
  graphNodes,
  graphEdges,
  onSelect,
  onExpand,
  onCollapse,
}: DetailPanelProps) {
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

  // Find entity in world entities or expanded nodes
  const worldEntity = entities.find((e) => e.id === selectedId)
  const expandedNode = expandedNodes.get(selectedId)
  const graphNode = graphNodes.get(selectedId)

  const name =
    worldEntity?.name || expandedNode?.label || graphNode?.label || selectedId
  const kind =
    worldEntity?.kind || expandedNode?.kind || graphNode?.kind || 'unknown'
  const isExpanded = expandedNodes.has(selectedId)

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex-none p-5 border-b border-border-subtle">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0">
            <h2 className="font-semibold text-foreground truncate">{name}</h2>
            <div className="flex items-center gap-2 mt-1">
              <Badge variant="secondary" className="capitalize text-xs">
                {kind}
              </Badge>
              {worldEntity && (
                <span className="text-xs text-muted-foreground">
                  Shard {worldEntity.shardId}
                </span>
              )}
            </div>
          </div>
        </div>

        {/* Quick actions */}
        <div className="flex gap-2 mt-3">
          <Button
            size="sm"
            variant="default"
            onClick={() => onExpand(selectedId)}
            className="flex-1"
          >
            Expand
            <ChevronRight className="h-4 w-4 ml-1" />
          </Button>
          {isExpanded && (
            <Button
              size="sm"
              variant="outline"
              onClick={() => onCollapse(selectedId)}
              className="flex-1"
            >
              Collapse
            </Button>
          )}
        </div>
      </div>

      {/* Content */}
      <Tabs
        defaultValue="properties"
        className="flex-1 flex flex-col overflow-hidden"
      >
        <TabsList className="flex-none mx-4 mt-3">
          <TabsTrigger value="properties" className="flex-1">
            Properties
          </TabsTrigger>
          <TabsTrigger value="components" className="flex-1">
            Components
          </TabsTrigger>
          <TabsTrigger value="children" className="flex-1">
            Children
          </TabsTrigger>
        </TabsList>

        <TabsContent value="properties" className="flex-1 overflow-hidden m-0">
          <ScrollArea className="h-full">
            <div className="p-5 space-y-4">
              {/* Position info */}
              {worldEntity && (
                <PropertySection title="Position" icon={MapPin}>
                  <PropertyRow
                    label="X"
                    value={worldEntity.x.toFixed(2)}
                    unit="m"
                  />
                  <PropertyRow
                    label="Y"
                    value={worldEntity.y.toFixed(2)}
                    unit="m"
                  />
                  <PropertyRow
                    label="Z"
                    value={worldEntity.z.toFixed(2)}
                    unit="m"
                  />
                </PropertySection>
              )}

              {/* Entity info */}
              <PropertySection title="Entity" icon={Layers}>
                <PropertyRow label="ID" value={selectedId} mono />
                <PropertyRow
                  label="Components"
                  value={String(worldEntity?.componentCount || 0)}
                />
              </PropertySection>

              {/* Child Entities Section */}
              <ChildEntitiesSection
                entityId={selectedId}
                entities={entities}
                onSelect={onSelect}
              />

              {/* Graph properties */}
              {graphNode?.properties &&
                Object.keys(graphNode.properties).length > 0 && (
                  <PropertySection title="Graph Properties" icon={Box}>
                    {Object.entries(graphNode.properties).map(
                      ([key, value]) => (
                        <PropertyRow key={key} label={key} value={value} mono />
                      ),
                    )}
                  </PropertySection>
                )}

              {/* Expanded node properties */}
              {expandedNode?.properties &&
                Object.keys(expandedNode.properties).length > 0 && (
                  <PropertySection title="Node Properties" icon={Box}>
                    {Object.entries(expandedNode.properties).map(
                      ([key, value]) => (
                        <PropertyRow key={key} label={key} value={value} mono />
                      ),
                    )}
                  </PropertySection>
                )}
            </div>
          </ScrollArea>
        </TabsContent>

        <TabsContent value="components" className="flex-1 overflow-hidden m-0">
          <ScrollArea className="h-full">
            <div className="p-5">
              <ComponentsList
                entityId={selectedId}
                graphNodes={graphNodes}
                graphEdges={graphEdges}
              />
            </div>
          </ScrollArea>
        </TabsContent>
        <TabsContent value="children" className="flex-1 overflow-hidden m-0">
          <ScrollArea className="h-full">
            <div className="p-5">
              <ChildEntitiesList
                entityId={selectedId}
                entities={entities}
                graphNodes={graphNodes}
                graphEdges={graphEdges}
                onSelect={onSelect}
              />
            </div>
          </ScrollArea>
        </TabsContent>
      </Tabs>
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
}

function ComponentsList({
  entityId,
  graphNodes,
  graphEdges,
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

  const toggleComponent = (componentId: string) => {
    setExpandedComponents((prev) => {
      const next = new Set(prev)
      if (next.has(componentId)) {
        next.delete(componentId)
      } else {
        next.add(componentId)
      }
      return next
    })
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

    // For objects with x,y,z show compact position
    if (entries.some(([k]) => k === 'x' || k === 'pos_x')) {
      const x = entries.find(([k]) => k === 'x' || k === 'pos_x')?.[1]
      const y = entries.find(([k]) => k === 'y' || k === 'pos_y')?.[1]
      const z = entries.find(([k]) => k === 'z' || k === 'pos_z')?.[1]
      if (x !== undefined && y !== undefined) {
        return `{x: ${formatNumber(x)}, y: ${formatNumber(y)}, z: ${formatNumber(z ?? 0)}}`
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
    <div className="space-y-1">
      {components.map(({ id, node }) => {
        const isExpanded = expandedComponents.has(id)
        const hasProperties = Object.keys(node.properties).length > 0
        const previewValue = hasProperties
          ? getPreviewValue(node.properties)
          : ''

        return (
          <div
            key={id}
            className="border border-border rounded-md overflow-hidden"
          >
            <button
              onClick={() => {
                if (hasProperties) {
                  toggleComponent(id)
                }
              }}
              className="flex items-center gap-2 w-full px-3 py-2 hover:bg-secondary/50 transition-colors text-left"
            >
              <span className="flex-1 text-sm font-medium min-w-0 flex items-baseline gap-2">
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
            </button>

            {isExpanded && hasProperties && (
              <div className="px-3 py-2 bg-secondary/20 border-t border-border space-y-1">
                {Object.entries(node.properties).map(([key, value]) => (
                  <div
                    key={key}
                    className="flex items-start gap-2 py-0.5 min-w-0"
                  >
                    <span className="text-xs text-muted-foreground truncate shrink-0">
                      {key}
                    </span>
                    <ValueField
                      value={formatDisplayValue(value)}
                      mono
                      className="text-xs text-foreground/90"
                    />
                  </div>
                ))}
              </div>
            )}
          </div>
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
  if (typeof value === 'string') {
    if (value.length > 30) return value.substring(0, 27) + '...'
    return value
  }
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'object') {
    const str = JSON.stringify(value)
    if (str.length > 40) return str.substring(0, 37) + '...'
    return str
  }
  return String(value)
}

type FormattedValue = {
  text: string
  isStructured: boolean
}

function formatDisplayValue(value: unknown): FormattedValue {
  if (value === null || value === undefined) {
    return { text: 'null', isStructured: false }
  }

  if (typeof value === 'object') {
    return { text: JSON.stringify(value, null, 2), isStructured: true }
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
          return { text: JSON.stringify(parsed, null, 2), isStructured: true }
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
          'min-w-0 flex-1 overflow-hidden whitespace-pre-wrap break-words text-left text-sm text-foreground',
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
        'min-w-0 flex-1 truncate text-right text-sm text-foreground',
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
          <button
            key={child.id}
            onClick={() => onSelect(child.id)}
            className="flex items-center gap-2 w-full px-2 py-1 rounded hover:bg-secondary/50 text-sm transition-colors text-left"
          >
            <Box className="h-3.5 w-3.5 text-primary/60 flex-none" />
            <span className="truncate flex-1">{child.name}</span>
            <Badge variant="secondary" className="text-xs">
              {child.componentCount}c
            </Badge>
          </button>
        ))}
      </div>
    </PropertySection>
  )
}

interface ChildEntitiesListProps {
  entityId: string
  entities: Array<WorldEntity>
  graphNodes: Map<string, GraphNode>
  graphEdges: Array<GraphEdge>
  onSelect: (id: string) => void
}

function ChildEntitiesList({
  entityId,
  entities,
  graphNodes,
  graphEdges,
  onSelect,
}: ChildEntitiesListProps) {
  const [expandedChildren, setExpandedChildren] = React.useState<Set<string>>(
    new Set(),
  )

  const children = React.useMemo(() => {
    return entities.filter((e) => e.parentEntityId === entityId)
  }, [entityId, entities])

  const getChildComponents = (childId: string) => {
    const componentIds = graphEdges
      .filter((edge) => edge.from === childId && edge.label === 'HAS_COMPONENT')
      .map((edge) => edge.to)
    return componentIds
      .map((id) => {
        const node = graphNodes.get(id)
        return node ? { id, node } : null
      })
      .filter(
        (entry): entry is { id: string; node: GraphNode } => entry !== null,
      )
  }

  const toggleChild = (childId: string) => {
    setExpandedChildren((prev) => {
      const next = new Set(prev)
      if (next.has(childId)) {
        next.delete(childId)
      } else {
        next.add(childId)
      }
      return next
    })
  }

  if (children.length === 0) {
    return (
      <div className="text-sm text-muted-foreground text-center py-6">
        <p>No child entities found.</p>
        <p className="mt-1 text-xs">
          Child entities have this entity as their parent (mounted_on).
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-2">
      {children.map((child) => {
        const isExpanded = expandedChildren.has(child.id)
        const components = getChildComponents(child.id)

        return (
          <div
            key={child.id}
            className="border border-border rounded-md overflow-hidden"
          >
            <button
              onClick={() => {
                onSelect(child.id)
                toggleChild(child.id)
              }}
              className="flex items-center gap-2 w-full px-3 py-2 hover:bg-secondary/50 transition-colors text-left"
            >
              <Box className="h-4 w-4 text-primary flex-none" />
              <div className="flex-1 min-w-0">
                <div className="font-medium text-sm truncate">{child.name}</div>
                <div className="text-xs text-muted-foreground">
                  {child.kind} • {components.length} components
                </div>
              </div>
              <ChevronRight
                className={cn(
                  'h-4 w-4 text-muted-foreground transition-transform',
                  isExpanded && 'rotate-90',
                )}
              />
            </button>

            {isExpanded && components.length > 0 && (
              <div className="border-t border-border bg-secondary/20">
                {components.map(({ id, node }) => (
                  <button
                    key={id}
                    onClick={() => onSelect(id)}
                    className="flex items-center gap-2 w-full px-3 py-2 hover:bg-secondary/50 transition-colors text-left border-b border-border/50 last:border-0"
                  >
                    <Box className="h-3.5 w-3.5 text-warning flex-none ml-4" />
                    <span className="text-xs font-medium truncate flex-1">
                      {node.label}
                    </span>
                    <span className="text-xs text-muted-foreground">
                      {Object.keys(node.properties).length} props
                    </span>
                  </button>
                ))}
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}

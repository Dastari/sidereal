import * as React from 'react'
import {
  Box,
  ChevronDown,
  ChevronRight,
  Circle,
  Globe,
  Hexagon,
  Rocket,
  Sparkles,
  Trash2,
} from 'lucide-react'
import type { WorldEntity } from '@/components/grid/types'
import type { DataSourceMode } from '@/components/sidebar/Toolbar'
import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'

interface EntityTreeProps {
  entities: Array<WorldEntity>
  selectedId: string | null
  onSelect: (id: string) => void
  sourceMode: DataSourceMode
  onDelete: (entityId: string) => Promise<void>
}

const kindIcons: Record<string, React.ComponentType<{ className?: string }>> = {
  ship: Rocket,
  station: Hexagon,
  asteroid: Circle,
  planet: Globe,
  component: Box,
}

function getKindIcon(kind: string) {
  const icon = kindIcons[kind.toLowerCase()]
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- runtime key lookup
  if (icon) return icon
  return Sparkles
}

function EntityTree({
  entities,
  selectedId,
  onSelect,
  sourceMode,
  onDelete,
}: EntityTreeProps) {
  const [openGroups, setOpenGroups] = React.useState<Record<string, boolean>>(
    {},
  )
  const [openNodes, setOpenNodes] = React.useState<Record<string, boolean>>({})

  const { rootsByKind, childrenByParent } = React.useMemo(() => {
    const byId = new Map<string, WorldEntity>()
    for (const entity of entities) {
      byId.set(entity.id, entity)
    }

    const children = new Map<string, Array<WorldEntity>>()
    const roots = new Map<string, Array<WorldEntity>>()

    for (const entity of entities) {
      const parentId = entity.parentEntityId
      if (parentId && byId.has(parentId)) {
        const list = children.get(parentId)
        if (list) {
          list.push(entity)
        } else {
          children.set(parentId, [entity])
        }
      } else {
        const list = roots.get(entity.kind)
        if (list) {
          list.push(entity)
        } else {
          roots.set(entity.kind, [entity])
        }
      }
    }

    for (const list of roots.values()) {
      list.sort((a, b) => a.name.localeCompare(b.name))
    }
    for (const list of children.values()) {
      list.sort((a, b) => a.name.localeCompare(b.name))
    }

    return { rootsByKind: roots, childrenByParent: children }
  }, [entities])

  const sortedGroups = React.useMemo(() => {
    return Array.from(rootsByKind.entries()).sort(([a], [b]) =>
      a.localeCompare(b),
    )
  }, [rootsByKind])

  const isGroupOpen = React.useCallback(
    (kind: string) => openGroups[kind] ?? true,
    [openGroups],
  )

  const toggleGroup = React.useCallback((kind: string) => {
    setOpenGroups((prev) => ({ ...prev, [kind]: !(prev[kind] ?? true) }))
  }, [])

  const isNodeOpen = React.useCallback(
    (entityId: string) => openNodes[entityId] ?? true,
    [openNodes],
  )

  const toggleNode = React.useCallback((entityId: string) => {
    setOpenNodes((prev) => ({ ...prev, [entityId]: !(prev[entityId] ?? true) }))
  }, [])

  return (
    <ScrollArea className="h-full">
      <div className="p-4 space-y-1">
        {sortedGroups.map(([kind, items]) => (
          <EntityGroup
            key={kind}
            kind={kind}
            entities={items}
            selectedId={selectedId}
            onSelect={onSelect}
            sourceMode={sourceMode}
            onDelete={onDelete}
            childrenByParent={childrenByParent}
            isOpen={isGroupOpen(kind)}
            onToggleOpen={() => toggleGroup(kind)}
            isNodeOpen={isNodeOpen}
            onToggleNode={toggleNode}
          />
        ))}
        {entities.length === 0 && (
          <div className="text-sm text-muted-foreground text-center py-8">
            No entities loaded
          </div>
        )}
      </div>
    </ScrollArea>
  )
}

export { EntityTree }
export default EntityTree

interface EntityGroupProps {
  kind: string
  entities: Array<WorldEntity>
  selectedId: string | null
  onSelect: (id: string) => void
  sourceMode: DataSourceMode
  onDelete: (entityId: string) => Promise<void>
  childrenByParent: Map<string, Array<WorldEntity>>
  isOpen: boolean
  onToggleOpen: () => void
  isNodeOpen: (id: string) => boolean
  onToggleNode: (id: string) => void
}

function EntityGroup({
  kind,
  entities,
  selectedId,
  onSelect,
  sourceMode,
  onDelete,
  childrenByParent,
  isOpen,
  onToggleOpen,
  isNodeOpen,
  onToggleNode,
}: EntityGroupProps) {
  const Icon = getKindIcon(kind)

  return (
    <Collapsible open={isOpen} onOpenChange={onToggleOpen}>
      <CollapsibleTrigger className="flex items-center gap-2 w-full px-2 py-1.5 rounded-md hover:bg-secondary/50 text-sm font-medium text-foreground/90 transition-colors">
        {isOpen ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-4 w-4 text-muted-foreground" />
        )}
        <Icon className="h-4 w-4 text-primary" />
        <span className="capitalize">{kind}</span>
        <span className="ml-auto text-xs text-muted-foreground">
          {entities.length}
        </span>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="ml-4 pl-2 border-l border-border-subtle space-y-0.5 mt-1">
          {entities.map((entity) => (
            <EntityTreeNode
              key={entity.id}
              entity={entity}
              depth={0}
              sourceMode={sourceMode}
              onDelete={onDelete}
              childrenByParent={childrenByParent}
              isNodeOpen={isNodeOpen}
              onToggleNode={onToggleNode}
              isSelected={entity.id === selectedId}
              selectedId={selectedId}
              onSelect={onSelect}
            />
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}

interface EntityTreeNodeProps {
  entity: WorldEntity
  depth: number
  sourceMode: DataSourceMode
  onDelete: (entityId: string) => Promise<void>
  childrenByParent: Map<string, Array<WorldEntity>>
  isNodeOpen: (id: string) => boolean
  onToggleNode: (id: string) => void
  isSelected: boolean
  selectedId: string | null
  onSelect: (id: string) => void
}

function EntityTreeNode({
  entity,
  depth,
  sourceMode,
  onDelete,
  childrenByParent,
  isNodeOpen,
  onToggleNode,
  isSelected,
  selectedId,
  onSelect,
}: EntityTreeNodeProps) {
  const [isDeleting, setIsDeleting] = React.useState(false)
  const children = childrenByParent.get(entity.id) ?? []
  const hasChildren = children.length > 0
  const open = hasChildren ? isNodeOpen(entity.id) : false
  const Icon = getKindIcon(entity.kind)

  const handleDeleteClick = async (e: React.MouseEvent) => {
    e.stopPropagation()
    setIsDeleting(true)
    try {
      await onDelete(entity.id)
    } catch (error) {
      console.error('Failed to delete entity:', error)
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div>
      <div className="flex items-center gap-1 group">
        <span style={{ width: `${depth * 12}px` }} />
        {hasChildren ? (
          <button
            onClick={() => onToggleNode(entity.id)}
            className="h-5 w-5 flex items-center justify-center text-muted-foreground hover:text-foreground transition-colors"
          >
            {open ? (
              <ChevronDown className="h-3.5 w-3.5" />
            ) : (
              <ChevronRight className="h-3.5 w-3.5" />
            )}
          </button>
        ) : (
          <span className="h-5 w-5" />
        )}

        <button
          onClick={() => onSelect(entity.id)}
          className={cn(
            'flex items-center gap-2 flex-1 px-2 py-1 rounded-md text-sm transition-colors text-left min-w-0',
            isSelected
              ? 'bg-primary/15 text-primary'
              : 'hover:bg-secondary/50 text-foreground/80',
          )}
          title={entity.id}
        >
          <Icon className="h-3.5 w-3.5 shrink-0 text-primary/80" />
          <span className="truncate flex-1">{entity.name}</span>
          <span className="text-xs text-muted-foreground font-mono shrink-0">
            {entity.componentCount}c
          </span>
        </button>

        {sourceMode !== 'liveClient' && (
          <button
            onClick={handleDeleteClick}
            disabled={isDeleting}
            className={cn(
              'h-7 w-7 flex items-center justify-center rounded-md opacity-0 group-hover:opacity-100 transition-all shrink-0',
              isDeleting
                ? 'text-muted-foreground cursor-not-allowed'
                : 'text-destructive hover:bg-destructive/10 hover:text-destructive',
            )}
            title={`Delete ${entity.name}`}
          >
            <Trash2
              className={cn('h-3.5 w-3.5', isDeleting && 'animate-pulse')}
            />
          </button>
        )}
      </div>

      {hasChildren && open && (
        <div className="mt-0.5 space-y-0.5">
          {children.map((child) => (
            <EntityTreeNode
              key={child.id}
              entity={child}
              depth={depth + 1}
              sourceMode={sourceMode}
              onDelete={onDelete}
              childrenByParent={childrenByParent}
              isNodeOpen={isNodeOpen}
              onToggleNode={onToggleNode}
              isSelected={child.id === selectedId}
              selectedId={selectedId}
              onSelect={onSelect}
            />
          ))}
        </div>
      )}
    </div>
  )
}

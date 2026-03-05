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
  onContextMenuRequest?: (entityId: string, point: { x: number; y: number }) => void
}

const ENTITY_ROOT_GROUP_KEY = '__entity_root__'
const DEFAULT_GROUP_KEY = 'Entity'

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

function normalizeGroupLabel(groupKey: string): string {
  if (groupKey.toLowerCase() === 'module') return 'Modules'
  return groupKey
}

function getLabelGroupKey(entity: WorldEntity): string | null {
  const labels =
    entity.entity_labels ??
    // Backward compatibility while older snapshots still use sidereal_labels.
    (entity as WorldEntity & { sidereal_labels?: Array<string> })
      .sidereal_labels ??
    undefined
  if (!labels || labels.length === 0) return null

  const nonEntity = labels.find((label) => label.toLowerCase() !== 'entity')
  return nonEntity ?? null
}

function EntityTree({
  entities,
  selectedId,
  onSelect,
  sourceMode,
  onDelete,
  onContextMenuRequest,
}: EntityTreeProps) {
  const [openGroups, setOpenGroups] = React.useState<Record<string, boolean>>(
    {},
  )
  const [openNodes, setOpenNodes] = React.useState<Record<string, boolean>>({})

  const { rootsByGroupKey, childrenByParent, useEntityRoot } =
    React.useMemo(() => {
    const byId = new Map<string, WorldEntity>()
    const byGuid = new Map<string, WorldEntity>()
    for (const entity of entities) {
      byId.set(entity.id, entity)
      if (entity.entityGuid) {
        byGuid.set(entity.entityGuid, entity)
      }
    }

    const useBrpNamePrefixGrouping =
      sourceMode === 'liveServer' || sourceMode === 'liveClient'

    function resolveParent(parentId: string): WorldEntity | null {
      const byIdParent = byId.get(parentId)
      if (byIdParent) return byIdParent
      return byGuid.get(parentId) ?? null
    }

    const children = new Map<string, Array<WorldEntity>>()
    const roots = new Map<string, Array<WorldEntity>>()

    const hasLabelGrouping = entities.some(
      (e) =>
        (e.entity_labels && e.entity_labels.length >= 2) ||
        ((e as WorldEntity & { sidereal_labels?: Array<string> })
          .sidereal_labels?.length ?? 0) >= 2,
    )
    const useDatabaseLabelGrouping =
      sourceMode === 'database' && hasLabelGrouping

    function getGroupKey(entity: WorldEntity): string {
      const labelKey = getLabelGroupKey(entity)
      if (labelKey) {
        return labelKey
      }
      if (useBrpNamePrefixGrouping) {
        const colonIndex = entity.name.indexOf(':')
        return colonIndex >= 0
          ? entity.name.slice(0, colonIndex).trim() || DEFAULT_GROUP_KEY
          : entity.kind || DEFAULT_GROUP_KEY
      }
      return entity.kind || DEFAULT_GROUP_KEY
    }

    for (const entity of entities) {
      const parentId = entity.parentEntityId
      const parent = parentId ? resolveParent(parentId) : null
      if (parent) {
        const list = children.get(parent.id)
        if (list) {
          list.push(entity)
        } else {
          children.set(parent.id, [entity])
        }
      } else {
        const groupKey = getGroupKey(entity)
        const list = roots.get(groupKey)
        if (list) {
          list.push(entity)
        } else {
          roots.set(groupKey, [entity])
        }
      }
    }

    for (const list of roots.values()) {
      list.sort((a, b) => a.name.localeCompare(b.name))
    }
    for (const list of children.values()) {
      list.sort((a, b) => a.name.localeCompare(b.name))
    }

    return {
      rootsByGroupKey: roots,
      childrenByParent: children,
      useEntityRoot:
        useDatabaseLabelGrouping ||
        useBrpNamePrefixGrouping ||
        hasLabelGrouping,
    }
  }, [entities, sourceMode])

  const sortedGroups = React.useMemo(() => {
    return Array.from(rootsByGroupKey.entries())
      .filter(([key]) => key !== ENTITY_ROOT_GROUP_KEY)
      .sort(([a], [b]) => a.localeCompare(b))
  }, [rootsByGroupKey])

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

  const groupContent = (
    <>
      {sortedGroups.map(([groupKey, items]) => (
        <EntityGroup
          key={groupKey}
          kind={groupKey}
          entities={items}
          selectedId={selectedId}
          onSelect={onSelect}
          sourceMode={sourceMode}
          onDelete={onDelete}
          onContextMenuRequest={onContextMenuRequest}
          childrenByParent={childrenByParent}
          isOpen={isGroupOpen(groupKey)}
          onToggleOpen={() => toggleGroup(groupKey)}
          isNodeOpen={isNodeOpen}
          onToggleNode={toggleNode}
        />
      ))}
    </>
  )

  return (
    <ScrollArea className="h-full">
      <div className="p-4 space-y-1">
        {useEntityRoot ? (
          <Collapsible
            open={isGroupOpen(ENTITY_ROOT_GROUP_KEY)}
            onOpenChange={() => toggleGroup(ENTITY_ROOT_GROUP_KEY)}
          >
            <CollapsibleTrigger className="flex items-center gap-2 w-full px-2 py-1.5 rounded-md hover:bg-secondary/50 text-sm font-medium text-foreground/90 transition-colors">
              {isGroupOpen(ENTITY_ROOT_GROUP_KEY) ? (
                <ChevronDown className="h-4 w-4 text-muted-foreground" />
              ) : (
                <ChevronRight className="h-4 w-4 text-muted-foreground" />
              )}
              <Sparkles className="h-4 w-4 text-primary" />
              <span>Entity</span>
              <span className="ml-auto text-xs text-muted-foreground">
                {entities.length}
              </span>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <div className="ml-4 pl-2 border-l border-border-subtle space-y-1 mt-1">
                {groupContent}
              </div>
            </CollapsibleContent>
          </Collapsible>
        ) : (
          groupContent
        )}
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
  onContextMenuRequest?: (entityId: string, point: { x: number; y: number }) => void
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
  onContextMenuRequest,
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
        <span className="capitalize">{normalizeGroupLabel(kind)}</span>
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
              onContextMenuRequest={onContextMenuRequest}
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
  onContextMenuRequest?: (entityId: string, point: { x: number; y: number }) => void
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
  onContextMenuRequest,
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

  const handleContextMenu = (e: React.MouseEvent) => {
    if (!onContextMenuRequest) return
    e.preventDefault()
    onContextMenuRequest(entity.id, { x: e.clientX, y: e.clientY })
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
          onContextMenu={handleContextMenu}
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
          <span
            className="max-w-[14ch] truncate text-xs text-muted-foreground font-mono shrink-0"
            title={entity.entityGuid ?? undefined}
          >
            {entity.entityGuid ?? entity.componentCount}
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
              onContextMenuRequest={onContextMenuRequest}
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

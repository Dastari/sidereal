import { createFileRoute } from '@tanstack/react-router'
import { useCallback, useEffect, useMemo, useState } from 'react'
import type {
  ExpandedNode,
  GraphEdge,
  GraphNode,
  WorldEntity,
} from '@/components/grid/types'
import type { DataSourceMode } from '@/components/sidebar/Toolbar'
import { TooltipProvider } from '@/components/ui/tooltip'
import { ThemeProvider } from '@/components/ThemeProvider'
import {
  AppLayout,
  Panel,
  PanelContent,
  PanelHeader,
} from '@/components/layout/AppLayout'
import { GridCanvas } from '@/components/grid/GridCanvas'
import { EntityTree } from '@/components/sidebar/EntityTree'
import { DetailPanel } from '@/components/sidebar/DetailPanel'
import { StatusBar } from '@/components/sidebar/StatusBar'
import { Toolbar } from '@/components/sidebar/Toolbar'
import { Switch } from '@/components/ui/switch'

export const Route = createFileRoute('/')({ component: DashboardPage })

type ApiGraph = {
  graph: string
  nodes: Array<{
    id: string
    label?: string
    kind?: string
    properties?: Record<string, unknown>
  }>
  edges: Array<{
    id: string
    from: string
    to: string
    label?: string
    properties?: Record<string, unknown>
  }>
  error?: string
}

type ApiWorld = {
  graph: string
  entities: Array<WorldEntity>
  error?: string
}

type ApiLiveWorld = {
  source: 'bevy_remote'
  target: 'server' | 'client' | 'hostClient'
  brpUrl: string
  graph: string
  entities: Array<WorldEntity>
  nodes: Array<{
    id: string
    label?: string
    kind?: string
    properties?: Record<string, unknown>
  }>
  edges: Array<{
    id: string
    from: string
    to: string
    label?: string
    properties?: Record<string, unknown>
  }>
  error?: string
}

const CAMERA_HIDE_SUBSTRING = 'bevy_camera::camera::Camera'

/** True if this entity should be hidden from the map (tree still shows it). */
function isCameraEntity(
  entity: WorldEntity,
  graphNodes: Map<string, GraphNode>,
  graphEdges: Array<GraphEdge>,
): boolean {
  if (
    entity.id.includes(CAMERA_HIDE_SUBSTRING) ||
    entity.name.includes(CAMERA_HIDE_SUBSTRING)
  ) {
    return true
  }
  const hasCameraComponent = graphEdges.some(
    (edge) =>
      edge.from === entity.id &&
      edge.label === 'HAS_COMPONENT' &&
      graphNodes.get(edge.to)?.label === 'Camera',
  )
  return hasCameraComponent
}

function DashboardPage() {
  const [sourceMode, setSourceMode] = useState<DataSourceMode>('database')

  // Data state
  const [entities, setEntities] = useState<Array<WorldEntity>>([])
  const [graphNodes, setGraphNodes] = useState<Map<string, GraphNode>>(
    new Map(),
  )
  const [graphEdges, setGraphEdges] = useState<Array<GraphEdge>>([])
  const [expandedNodes, setExpandedNodes] = useState<Map<string, ExpandedNode>>(
    new Map(),
  )

  // UI state
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [isRefreshing, setIsRefreshing] = useState(false)
  const [sidebarWidth, setSidebarWidth] = useState(280)
  const [detailPanelWidth, setDetailPanelWidth] = useState(320)
  const [filterMapInvisible, setFilterMapInvisible] = useState(true)

  // Status
  const [graphStatus, setGraphStatus] = useState({
    connected: false,
    nodeCount: 0,
    edgeCount: 0,
    graphName: '',
  })
  const [worldStatus, setWorldStatus] = useState({
    loaded: false,
    entityCount: 0,
  })

  const filteredEntities = useMemo(
    () =>
      filterMapInvisible
        ? entities.filter((entity) => {
            // Map Visible Only: show only entities that have an EntityGuid component (BRP and database).
            if (!entity.entityGuid) return false
            if (entity.mapVisible === false) return false
            // Keep child entries available in tree/detail, but hide root entities
            // that do not have a real position sample.
            if (!entity.parentEntityId && entity.hasPosition === false) return false
            return true
          })
        : entities,
    [entities, filterMapInvisible],
  )

  // Map-only: exclude camera entities (id/name contains bevy_camera::camera::Camera, or has Camera component). Tree still shows all.
  const { entitiesForMap, cameraEntityIds } = useMemo(() => {
    const cameraIds = new Set<string>()
    const forMap = filteredEntities.filter((entity) => {
      // Never render entities without source position on the map.
      if (entity.hasPosition === false) {
        return false
      }
      const hide = isCameraEntity(entity, graphNodes, graphEdges)
      if (hide) cameraIds.add(entity.id)
      return !hide
    })
    return { entitiesForMap: forMap, cameraEntityIds: cameraIds }
  }, [filteredEntities, graphNodes, graphEdges])

  // Load data
  const loadData = useCallback(async () => {
    setIsRefreshing(true)

    try {
      if (sourceMode === 'database') {
        const [graphRes, worldRes] = await Promise.all([
          fetch('/api/graph').then((r) => r.json() as Promise<ApiGraph>),
          fetch('/api/world').then((r) => r.json() as Promise<ApiWorld>),
        ])

        const graphOk =
          !graphRes.error &&
          Array.isArray(graphRes.nodes) &&
          Array.isArray(graphRes.edges)
        if (graphOk) {
          const nodeMap = new Map<string, GraphNode>()
          for (const n of graphRes.nodes) {
            nodeMap.set(n.id, {
              id: n.id,
              label: n.label ?? n.id,
              kind: n.kind ?? 'node',
              properties: n.properties ?? {},
            })
          }
          setGraphNodes(nodeMap)

          const edges: Array<GraphEdge> = graphRes.edges.map((e) => ({
            id: e.id,
            from: e.from,
            to: e.to,
            label: e.label ?? 'rel',
            properties: e.properties ?? {},
          }))
          setGraphEdges(edges)

          setGraphStatus({
            connected: true,
            nodeCount: graphRes.nodes.length,
            edgeCount: graphRes.edges.length,
            graphName: graphRes.graph || 'sidereal',
          })
        } else {
          setGraphStatus({
            connected: false,
            nodeCount: 0,
            edgeCount: 0,
            graphName: graphRes.error ? `DB error: ${graphRes.error}` : '',
          })
        }

        const worldOk = !worldRes.error && Array.isArray(worldRes.entities)
        if (worldOk) {
          setEntities(worldRes.entities)
          setWorldStatus({
            loaded: true,
            entityCount: worldRes.entities.length,
          })
        } else {
          setEntities([])
          setWorldStatus({
            loaded: false,
            entityCount: 0,
          })
        }
      } else {
        const liveEndpoint =
          sourceMode === 'liveHostClient'
            ? '/api/live-host-client-world'
            : sourceMode === 'liveClient'
              ? '/api/live-client-world'
              : '/api/live-world'
        const liveRes = await fetch(liveEndpoint).then(
          (r) => r.json() as Promise<ApiLiveWorld>,
        )

        const hasData =
          // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- API response validation
          !liveRes.error && liveRes.entities && liveRes.nodes && liveRes.edges
        if (!hasData) {
          const errorMsg = liveRes.error ? String(liveRes.error) : 'Unavailable'
          setEntities([])
          setGraphNodes(new Map())
          setGraphEdges([])
          setGraphStatus({
            connected: false,
            nodeCount: 0,
            edgeCount: 0,
            graphName: `${
              sourceMode === 'liveHostClient'
                ? 'Host Client'
                : sourceMode === 'liveClient'
                  ? 'Client'
                  : 'Server'
            } BRP error: ${errorMsg}`,
          })
          setWorldStatus({
            loaded: false,
            entityCount: 0,
          })
          return
        }

        const nodeMap = new Map<string, GraphNode>()
        for (const n of liveRes.nodes) {
          nodeMap.set(n.id, {
            id: n.id,
            label: n.label ?? n.id,
            kind: n.kind ?? 'node',
            properties: n.properties ?? {},
          })
        }
        setGraphNodes(nodeMap)
        setGraphEdges(
          liveRes.edges.map((edge) => ({
            id: edge.id,
            from: edge.from,
            to: edge.to,
            label: edge.label ?? 'rel',
            properties: edge.properties ?? {},
          })),
        )
        // Enrich parentEntityId from edges when not set on entity (BRP tree parent-child)
        const parentFromEdges = new Map<string, string>()
        for (const edge of liveRes.edges) {
          const label = (edge.label ?? '').toUpperCase()
          if (label === 'HAS_CHILD' || label === 'PARENT') {
            parentFromEdges.set(edge.to, edge.from)
          }
        }
        const entitiesWithParentFromEdges = liveRes.entities.map((e) => ({
          ...e,
          parentEntityId:
            e.parentEntityId ?? parentFromEdges.get(e.id) ?? undefined,
        }))
        setEntities(entitiesWithParentFromEdges)
        setGraphStatus({
          connected: true,
          nodeCount: liveRes.nodes.length,
          edgeCount: liveRes.edges.length,
          graphName: `${
            liveRes.target === 'hostClient'
              ? 'Host Client'
              : liveRes.target === 'client'
                ? 'Client'
                : 'Server'
          } BRP @ ${liveRes.brpUrl}`,
        })
        setWorldStatus({
          loaded: true,
          entityCount: liveRes.entities.length,
        })
      }
    } catch (err) {
      console.error('Failed to load data:', err)
      setGraphStatus({
        connected: false,
        nodeCount: 0,
        edgeCount: 0,
        graphName: '',
      })
      setWorldStatus({
        loaded: false,
        entityCount: 0,
      })
    } finally {
      setIsRefreshing(false)
    }
  }, [sourceMode])

  useEffect(() => {
    setExpandedNodes(new Map())
    setSelectedId(null)
  }, [sourceMode])

  useEffect(() => {
    if (!selectedId) return
    const selectedVisible = filteredEntities.some((e) => e.id === selectedId)
    if (!selectedVisible) {
      setSelectedId(null)
    }
  }, [filteredEntities, selectedId])

  // Initial load and polling
  useEffect(() => {
    void loadData()
    const interval = setInterval(() => {
      void loadData()
    }, 5000)
    return () => clearInterval(interval)
  }, [loadData])

  // Handle node expansion
  const handleExpand = useCallback(
    (id: string) => {
      setExpandedNodes((prev) => {
        const next = new Map(prev)

        // Find the base entity or existing node position
        const baseEntity = filteredEntities.find((e) => e.id === id)
        const existingNode = prev.get(id)
        const centerX = baseEntity?.x || existingNode?.x || 0
        const centerY = baseEntity?.y || existingNode?.y || 0

        // Only explode child entities in the map graph view.
        const childEntities = filteredEntities.filter(
          (entity) => entity.parentEntityId === id,
        )
        const hiddenChildren = childEntities.filter(
          (child) => child.hasPosition !== false && !next.has(child.id),
        )

        // Position hidden neighbors in a circle around the center with animation-friendly layout
        const radius = Math.max(100, 80 + hiddenChildren.length * 8)
        hiddenChildren.forEach((child, index) => {
          const angle =
            (Math.PI * 2 * index) / Math.max(1, hiddenChildren.length)

          next.set(child.id, {
            id: child.id,
            parentId: id,
            x: centerX + Math.cos(angle) * radius,
            y: centerY + Math.sin(angle) * radius,
            label: child.name,
            kind: child.kind,
            isExpanded: false,
            depth: (existingNode?.depth || 0) + 1,
            properties: {
              shardId: child.shardId,
              vx: child.vx,
              vy: child.vy,
              sampledAtMs: child.sampledAtMs,
              componentCount: child.componentCount,
              parentEntityId: child.parentEntityId,
            },
          })
        })

        // Mark the expanded node
        if (next.has(id)) {
          const node = next.get(id)!
          next.set(id, { ...node, isExpanded: true })
        }

        return next
      })
    },
    [filteredEntities],
  )

  // Update a component value via BRP (Server BRP or Client BRP only)
  const handleComponentUpdate = useCallback(
    async (entityId: string, typePath: string, value: unknown) => {
      if (
        sourceMode !== 'liveServer' &&
        sourceMode !== 'liveClient' &&
        sourceMode !== 'liveHostClient'
      )
        return
      const target =
        sourceMode === 'liveHostClient'
          ? 'hostClient'
          : sourceMode === 'liveClient'
            ? 'client'
            : 'server'
      const url = `/api/live-entity/${entityId}?target=${target}`
      try {
        const res = await fetch(url, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ typePath, value }),
        })
        const data = (await res.json()) as { error?: string; ok?: boolean }
        if (!res.ok || data.error) {
          console.error('Component update failed:', data.error ?? res.statusText)
          return
        }
        void loadData()
      } catch (err) {
        console.error('Component update request failed:', err)
      }
    },
    [sourceMode, loadData],
  )

  // Handle node collapse
  const handleCollapse = useCallback((id: string) => {
    setExpandedNodes((prev) => {
      const next = new Map(prev)

      // Find all nodes that are children of this node (recursively)
      const toRemove = new Set<string>()
      const findChildren = (parentId: string) => {
        for (const [nodeId, node] of next) {
          if (node.parentId === parentId) {
            toRemove.add(nodeId)
            findChildren(nodeId)
          }
        }
      }
      findChildren(id)

      // Remove children
      for (const nodeId of toRemove) {
        next.delete(nodeId)
      }

      // Mark node as collapsed
      if (next.has(id)) {
        const node = next.get(id)!
        next.set(id, { ...node, isExpanded: false })
      }

      return next
    })
  }, [])

  // Handle entity selection from tree
  const handleSelectFromTree = useCallback((id: string) => {
    setSelectedId(id)
  }, [])

  // Placeholder zoom controls (would be connected to GridCanvas camera)
  const handleZoomIn = useCallback(() => {
    // TODO: Implement zoom control via ref
  }, [])

  const handleZoomOut = useCallback(() => {
    // TODO: Implement zoom control via ref
  }, [])

  const handleFitAll = useCallback(() => {
    // TODO: Implement fit all via ref
  }, [])

  const handleResetView = useCallback(() => {
    setExpandedNodes(new Map())
    setSelectedId(null)
  }, [])

  const handleCollapseAll = useCallback(() => {
    setExpandedNodes(new Map())
  }, [])

  const handleDeleteEntity = useCallback(
    async (entityId: string) => {
      const endpoint =
        sourceMode === 'database'
          ? `/api/delete-entity/${entityId}`
          : sourceMode === 'liveServer'
            ? `/api/delete-live-entity/${entityId}?target=server`
            : null

      if (!endpoint) {
        throw new Error('Delete is disabled for client / host client BRP mode')
      }

      const response = await fetch(endpoint, { method: 'DELETE' })
      const result = await response.json()

      if (!result.success) {
        throw new Error(result.error || 'Failed to delete entity')
      }

      // Refresh data after deletion
      await loadData()

      // Clear selection if deleted entity was selected
      if (selectedId === entityId) {
        setSelectedId(null)
      }
    },
    [sourceMode, selectedId, loadData],
  )

  return (
    <ThemeProvider defaultTheme="dark">
      <TooltipProvider delayDuration={200}>
        <AppLayout
          header={
            <Toolbar
              onZoomIn={handleZoomIn}
              onZoomOut={handleZoomOut}
              onFitAll={handleFitAll}
              onResetView={handleResetView}
              onCollapseAll={handleCollapseAll}
              sourceMode={sourceMode}
              onSourceModeChange={setSourceMode}
            />
          }
          sidebar={
            <Panel>
              <PanelHeader className="py-2">
                <div className="flex items-center justify-between gap-2">
                  <h1 className="text-sm font-semibold text-foreground">
                    Sidereal Explorer
                  </h1>
                  <label className="inline-flex items-center gap-2 text-xs text-muted-foreground">
                    <span className="whitespace-nowrap">Map Visible Only</span>
                    <Switch
                      checked={filterMapInvisible}
                      onCheckedChange={setFilterMapInvisible}
                      aria-label="Filter entities with mapVisible false"
                    />
                  </label>
                </div>
              </PanelHeader>
              <PanelContent>
                <EntityTree
                  entities={filteredEntities}
                  selectedId={selectedId}
                  onSelect={handleSelectFromTree}
                  sourceMode={sourceMode}
                  onDelete={handleDeleteEntity}
                />
              </PanelContent>
              <StatusBar
                sourceMode={sourceMode}
                graphStatus={graphStatus}
                worldStatus={worldStatus}
                isRefreshing={isRefreshing}
                onRefresh={loadData}
              />
            </Panel>
          }
          sidebarWidth={sidebarWidth}
          onSidebarResize={setSidebarWidth}
          detailPanelWidth={detailPanelWidth}
          onDetailPanelResize={setDetailPanelWidth}
          detailPanel={
            <Panel>
              <DetailPanel
                selectedId={selectedId}
                entities={filteredEntities}
                expandedNodes={expandedNodes}
                graphNodes={graphNodes}
                graphEdges={graphEdges}
                onSelect={setSelectedId}
                onExpand={handleExpand}
                onCollapse={handleCollapse}
                sourceMode={sourceMode}
                onComponentUpdate={handleComponentUpdate}
              />
            </Panel>
          }
        >
          <GridCanvas
            entities={entitiesForMap}
            graphNodes={graphNodes}
            graphEdges={graphEdges}
            selectedId={selectedId}
            onSelect={setSelectedId}
            onExpand={handleExpand}
            expandedNodes={expandedNodes}
            filterMapInvisible={filterMapInvisible}
            sourceMode={sourceMode}
            excludedFromMapIds={cameraEntityIds}
          />
        </AppLayout>
      </TooltipProvider>
    </ThemeProvider>
  )
}

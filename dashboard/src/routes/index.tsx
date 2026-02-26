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
  target: 'server' | 'client'
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
        ? entities.filter((entity) => entity.mapVisible !== false)
        : entities,
    [entities, filterMapInvisible],
  )

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
          sourceMode === 'liveClient' ? '/api/live-client-world' : '/api/live-world'
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
            graphName: `${sourceMode === 'liveClient' ? 'Client' : 'Server'} BRP error: ${errorMsg}`,
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
        setEntities(liveRes.entities)
        setGraphStatus({
          connected: true,
          nodeCount: liveRes.nodes.length,
          edgeCount: liveRes.edges.length,
          graphName: `${liveRes.target === 'client' ? 'Client' : 'Server'} BRP @ ${liveRes.brpUrl}`,
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
          (child) => !next.has(child.id),
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
        throw new Error('Delete is disabled for client BRP mode')
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
              />
            </Panel>
          }
        >
          <GridCanvas
            entities={filteredEntities}
            graphNodes={graphNodes}
            graphEdges={graphEdges}
            selectedId={selectedId}
            onSelect={setSelectedId}
            onExpand={handleExpand}
            expandedNodes={expandedNodes}
            filterMapInvisible={filterMapInvisible}
          />
        </AppLayout>
      </TooltipProvider>
    </ThemeProvider>
  )
}

import { createFileRoute } from '@tanstack/react-router'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { faker } from '@faker-js/faker'
import type {
  ExpandedNode,
  GraphEdge,
  GraphNode,
  PlayerVisibilityOverlay,
  WorldEntity,
} from '@/components/grid/types'
import type { BrpTab, DataSourceMode } from '@/components/sidebar/Toolbar'
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

type ContextMenuState = {
  open: boolean
  x: number
  y: number
  entityId: string | null
}

const DEFAULT_OWNER_TYPE_PATH = 'sidereal_game::components::owner_id::OwnerId'
const SPAWN_TEMPLATES = [{ templateId: 'corvette', label: 'Corvette' }] as const

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

function asFiniteNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string') {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) return parsed
  }
  return null
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function parseSelectedPlayerVisibilityOverlay(
  selectedId: string | null,
  graphNodes: Map<string, GraphNode>,
  graphEdges: Array<GraphEdge>,
): PlayerVisibilityOverlay | null {
  if (!selectedId) return null
  const componentNodeIds = graphEdges
    .filter((edge) => edge.from === selectedId && edge.label === 'HAS_COMPONENT')
    .map((edge) => edge.to)

  let spatialGridValue: Record<string, unknown> | null = null
  let disclosureValue: Record<string, unknown> | null = null

  for (const componentId of componentNodeIds) {
    const node = graphNodes.get(componentId)
    if (!node || !isObjectRecord(node.properties)) continue
    const typePathRaw = node.properties.typePath
    const componentValue = node.properties.value
    if (typeof typePathRaw !== 'string' || !isObjectRecord(componentValue)) continue
    if (typePathRaw.endsWith('::VisibilitySpatialGrid')) {
      spatialGridValue = componentValue
    } else if (typePathRaw.endsWith('::VisibilityDisclosure')) {
      disclosureValue = componentValue
    }
  }

  if (!spatialGridValue) return null

  const cellSizeM =
    asFiniteNumber(spatialGridValue.cell_size_m) ??
    asFiniteNumber(spatialGridValue.cellSizeM) ??
    asFiniteNumber(spatialGridValue.cellSize)
  if (cellSizeM === null || cellSizeM <= 0) return null
  const deliveryRangeM =
    asFiniteNumber(spatialGridValue.delivery_range_m) ??
    asFiniteNumber(spatialGridValue.deliveryRangeM) ??
    asFiniteNumber(spatialGridValue.deliveryRange) ??
    0

  const queriedCellsRaw = Array.isArray(spatialGridValue.queried_cells)
    ? spatialGridValue.queried_cells
    : Array.isArray(spatialGridValue.queriedCells)
      ? spatialGridValue.queriedCells
      : []
  const queriedCells = queriedCellsRaw
    .map((entry) => {
      if (!isObjectRecord(entry)) return null
      const x = asFiniteNumber(entry.x)
      const y = asFiniteNumber(entry.y)
      if (x === null || y === null) return null
      return { x, y }
    })
    .filter((entry): entry is { x: number; y: number } => entry !== null)

  const scannerSourcesRaw = disclosureValue
    ? Array.isArray(disclosureValue.scanner_sources)
      ? disclosureValue.scanner_sources
      : Array.isArray(disclosureValue.scannerSources)
        ? disclosureValue.scannerSources
        : []
    : []
  const scannerSources = scannerSourcesRaw
    .map((entry) => {
      if (!isObjectRecord(entry)) return null
      const x = asFiniteNumber(entry.x)
      const y = asFiniteNumber(entry.y)
      const z = asFiniteNumber(entry.z)
      const rangeM =
        asFiniteNumber(entry.range_m) ??
        asFiniteNumber(entry.rangeM) ??
        asFiniteNumber(entry.range)
      if (x === null || y === null || rangeM === null) return null
      return {
        x,
        y,
        ...(z === null ? {} : { z }),
        range_m: rangeM,
      }
    })
    .filter(
      (
        entry,
      ): entry is { x: number; y: number; z?: number; range_m: number } =>
        entry !== null,
    )

  return {
    cell_size_m: cellSizeM,
    delivery_range_m: Math.max(0, deliveryRangeM),
    queried_cells: queriedCells,
    scanner_sources: scannerSources,
  }
}

function isPlayerEntity(entity: WorldEntity): boolean {
  if (entity.kind.toLowerCase().includes('player')) return true
  return entity.entity_labels?.some((label) => label.toLowerCase() === 'player') ?? false
}

function playerLabel(entity: WorldEntity): string {
  const guid = entity.entityGuid ?? entity.id
  return `${entity.name} (${guid.slice(0, 8)})`
}

function resolveOwnerTypePath(
  entityId: string,
  graphNodes: Map<string, GraphNode>,
  graphEdges: Array<GraphEdge>,
): string {
  const componentNodeIds = graphEdges
    .filter((edge) => edge.from === entityId && edge.label === 'HAS_COMPONENT')
    .map((edge) => edge.to)
  for (const componentNodeId of componentNodeIds) {
    const node = graphNodes.get(componentNodeId)
    const typePath = node?.properties?.typePath
    if (typeof typePath !== 'string') continue
    if (typePath.endsWith('::OwnerId')) return typePath
  }
  return DEFAULT_OWNER_TYPE_PATH
}

function DashboardPage() {
  const [sourceMode, setSourceMode] = useState<DataSourceMode>('database')
  const [brpTabs, setBrpTabs] = useState<Array<BrpTab>>([
    { id: 'server', label: 'Server BRP', port: 15713, kind: 'server' },
    { id: 'client-1', label: 'Client 1 BRP', port: 15714, kind: 'client' },
  ])
  const [activeBrpTabId, setActiveBrpTabId] = useState<string>('server')

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
  const [centerRequest, setCenterRequest] = useState<{
    position: { x: number; y: number } | null
    seq: number
  }>({
    position: null,
    seq: 0,
  })
  const [isRefreshing, setIsRefreshing] = useState(false)
  const [sidebarWidth, setSidebarWidth] = useState(280)
  const [detailPanelWidth, setDetailPanelWidth] = useState(320)
  const [filterMapInvisible, setFilterMapInvisible] = useState(true)
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    open: false,
    x: 0,
    y: 0,
    entityId: null,
  })
  const [contextStatusText, setContextStatusText] = useState<string | null>(null)

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

  const activeBrpTab = useMemo(() => {
    const found = brpTabs.find((tab) => tab.id === activeBrpTabId)
    if (found) return found
    if (brpTabs.length > 0) return brpTabs[0]
    return {
      id: 'server',
      label: 'Server BRP',
      port: 15713,
      kind: 'server' as const,
    }
  }, [brpTabs, activeBrpTabId])
  const isServerBrpMode =
    sourceMode === 'liveServer' && activeBrpTab.kind === 'server'
  const playerEntities = useMemo(
    () =>
      entities
        .filter((entity) => isPlayerEntity(entity) && Boolean(entity.entityGuid))
        .sort((a, b) => playerLabel(a).localeCompare(playerLabel(b))),
    [entities],
  )

  const filteredEntities = useMemo(
    () =>
      filterMapInvisible
        ? entities.filter((entity) => {
          // Sidereal Entities Only: show only entities that have an EntityGuid component (BRP and database).
          return Boolean(entity.entityGuid)
        })
        : entities,
    [entities, filterMapInvisible],
  )

  const centerOnPosition = centerRequest.position

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

  const selectedPlayerVisibilityOverlay = useMemo(
    () => parseSelectedPlayerVisibilityOverlay(selectedId, graphNodes, graphEdges),
    [selectedId, graphNodes, graphEdges],
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
        const query = new URLSearchParams({
          snapshot: '1',
          port: String(activeBrpTab.port),
          target: activeBrpTab.kind,
        })
        const liveRes = await fetch(`/api/brp?${query.toString()}`).then(
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
            graphName: `${activeBrpTab.label} error: ${errorMsg}`,
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
          graphName: `${activeBrpTab.label} @ ${liveRes.brpUrl}`,
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
  }, [sourceMode, activeBrpTab])

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

  useEffect(() => {
    if (!contextMenu.open) return
    const handlePointerDown = () => {
      setContextMenu((prev) => ({ ...prev, open: false }))
    }
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setContextMenu((prev) => ({ ...prev, open: false }))
      }
    }
    window.addEventListener('pointerdown', handlePointerDown)
    window.addEventListener('keydown', handleEscape)
    return () => {
      window.removeEventListener('pointerdown', handlePointerDown)
      window.removeEventListener('keydown', handleEscape)
    }
  }, [contextMenu.open])

  useEffect(() => {
    if (!contextStatusText) return
    const timer = window.setTimeout(() => setContextStatusText(null), 4500)
    return () => window.clearTimeout(timer)
  }, [contextStatusText])

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
              entity_labels: child.entity_labels,
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
        sourceMode !== 'liveClient'
      )
        return
      const numericEntityId = Number(entityId)
      if (!Number.isFinite(numericEntityId)) {
        console.error('Component update failed: entity ID must be numeric for BRP')
        return
      }
      const url = `/api/brp?port=${activeBrpTab.port}&target=${activeBrpTab.kind}`
      try {
        const res = await fetch(url, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            method: 'world.insert_components',
            params: {
              entity: numericEntityId,
              components: { [typePath]: value },
            },
          }),
        })
        const data = (await res.json()) as {
          error?: string
          ok?: boolean
          result?: unknown
        }
        if (!res.ok || data.error) {
          console.error('Component update failed:', data.error ?? res.statusText)
          return
        }
        void loadData()
      } catch (err) {
        console.error('Component update request failed:', err)
      }
    },
    [sourceMode, loadData, activeBrpTab],
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
    const entity = filteredEntities.find((e) => e.id === id)
    if (
      entity &&
      entity.hasPosition !== false &&
      Number.isFinite(entity.x) &&
      Number.isFinite(entity.y)
    ) {
      setCenterRequest((prev) => ({
        position: { x: entity.x, y: entity.y },
        seq: prev.seq + 1,
      }))
    }
  }, [filteredEntities])

  const handleSelectFromGrid = useCallback((id: string | null) => {
    setSelectedId(id)
    if (id) {
      const entity = filteredEntities.find((e) => e.id === id)
      if (
        entity &&
        entity.hasPosition !== false &&
        Number.isFinite(entity.x) &&
        Number.isFinite(entity.y)
      ) {
        setCenterRequest((prev) => ({
          position: { x: entity.x, y: entity.y },
          seq: prev.seq + 1,
        }))
      }
    }
  }, [filteredEntities])

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
          : null

      if (endpoint) {
        const response = await fetch(endpoint, { method: 'DELETE' })
        const result = await response.json()

        if (!result.success) {
          throw new Error(result.error || 'Failed to delete entity')
        }
      } else {
        if (sourceMode !== 'liveServer' || activeBrpTab.kind !== 'server') {
          throw new Error('Delete is disabled for client BRP mode')
        }
        const numericEntityId = Number(entityId)
        if (!Number.isFinite(numericEntityId)) {
          throw new Error('Entity ID must be numeric for BRP delete')
        }
        const response = await fetch(
          `/api/brp?port=${activeBrpTab.port}&target=${activeBrpTab.kind}`,
          {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              method: 'world.despawn_entity',
              params: {
                entity: numericEntityId,
              },
            }),
          },
        )
        const result = (await response.json()) as {
          error?: string
          success?: boolean
        }
        if (!response.ok || result.error) {
          throw new Error(result.error || 'Failed to delete live entity')
        }
      }

      // Refresh data after deletion
      await loadData()

      // Clear selection if deleted entity was selected
      if (selectedId === entityId) {
        setSelectedId(null)
      }
    },
    [sourceMode, selectedId, loadData, activeBrpTab],
  )

  const handleAddClientTab = useCallback(() => {
    setBrpTabs((prev) => {
      const clientCount = prev.filter((tab) => tab.kind === 'client').length
      const maxPort = prev.reduce((max, tab) => Math.max(max, tab.port), 0)
      const nextClientIndex = clientCount + 1
      const newTab: BrpTab = {
        id: `client-${nextClientIndex}`,
        label: `Client ${nextClientIndex} BRP`,
        port: maxPort + 1,
        kind: 'client',
      }
      setActiveBrpTabId(newTab.id)
      setSourceMode('liveClient')
      return [...prev, newTab]
    })
  }, [])

  const handleOpenContextMenu = useCallback(
    (entityId: string | null, point: { x: number; y: number }) => {
      if (!isServerBrpMode) return
      setContextMenu({
        open: true,
        x: point.x,
        y: point.y,
        entityId,
      })
    },
    [isServerBrpMode],
  )

  const handleSpawnTemplate = useCallback(
    async (templateId: string) => {
      const fallbackOwner = playerEntities[0]?.entityGuid
      if (!fallbackOwner) {
        setContextStatusText('Spawn failed: no player entity available to own new entity')
        return
      }
      const generatedName = `${faker.word.adjective()} ${templateId}`.replace(
        /\b\w/g,
        (part) => part.toUpperCase(),
      )
      const response = await fetch('/api/admin/spawn-entity', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          player_entity_id: fallbackOwner,
          bundle_id: templateId,
          overrides: { display_name: generatedName },
        }),
      })
      const payload = (await response.json()) as { error?: string; spawned_entity_id?: string }
      if (!response.ok) {
        throw new Error(payload.error ?? 'spawn request failed')
      }
      setContextStatusText(`Spawned ${templateId}: ${payload.spawned_entity_id ?? 'unknown'}`)
      await loadData()
    },
    [loadData, playerEntities],
  )

  const handleAssignOwner = useCallback(
    async (targetEntityId: string, ownerPlayerEntityId: string) => {
      const numericEntityId = Number(targetEntityId)
      if (!Number.isFinite(numericEntityId)) {
        throw new Error('Assign owner requires server BRP numeric entity ID')
      }
      const ownerTypePath = resolveOwnerTypePath(targetEntityId, graphNodes, graphEdges)
      const response = await fetch(
        `/api/brp?port=${activeBrpTab.port}&target=${activeBrpTab.kind}`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            method: 'world.insert_components',
            params: {
              entity: numericEntityId,
              components: {
                [ownerTypePath]: ownerPlayerEntityId,
              },
            },
          }),
        },
      )
      const payload = (await response.json()) as { error?: string }
      if (!response.ok || payload.error) {
        throw new Error(payload.error ?? 'owner assignment failed')
      }
      setContextStatusText(
        `Assigned owner ${ownerPlayerEntityId.slice(0, 8)} to entity ${targetEntityId}`,
      )
      await loadData()
    },
    [activeBrpTab.kind, activeBrpTab.port, graphEdges, graphNodes, loadData],
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
              brpTabs={brpTabs}
              activeBrpTabId={activeBrpTab.id}
              onActiveBrpTabIdChange={setActiveBrpTabId}
              onAddClientTab={handleAddClientTab}
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
                    <span className="whitespace-nowrap">Entities Only</span>
                    <Switch
                      checked={filterMapInvisible}
                      onCheckedChange={setFilterMapInvisible}
                      aria-label="Filter to entities with EntityGuid component"
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
                  onContextMenuRequest={(entityId, point) =>
                    handleOpenContextMenu(entityId, point)
                  }
                />
              </PanelContent>
              <StatusBar
                sourceMode={sourceMode}
                liveSourceLabel={activeBrpTab.label}
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
                onClose={() => setSelectedId(null)}
              />
            </Panel>
          }
        >
          <GridCanvas
            entities={entitiesForMap}
            graphNodes={graphNodes}
            graphEdges={graphEdges}
            selectedId={selectedId}
            onSelect={handleSelectFromGrid}
            onExpand={handleExpand}
            expandedNodes={expandedNodes}
            filterMapInvisible={filterMapInvisible}
            sourceMode={sourceMode}
            excludedFromMapIds={cameraEntityIds}
            centerOnPosition={centerOnPosition}
            centerOnRequestSeq={centerRequest.seq}
            selectedPlayerVisibilityOverlay={selectedPlayerVisibilityOverlay}
            onContextMenuRequest={handleOpenContextMenu}
          />
          {contextMenu.open && isServerBrpMode && (
            <div
              className="fixed z-[300] min-w-56 rounded-md border border-border bg-card/95 p-1 shadow-lg backdrop-blur"
              style={{ left: contextMenu.x, top: contextMenu.y }}
              onPointerDown={(event) => event.stopPropagation()}
              onContextMenu={(event) => event.preventDefault()}
            >
              <div className="px-2 py-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                Actions
              </div>
              <div className="space-y-1">
                {SPAWN_TEMPLATES.map((template) => (
                  <button
                    key={template.templateId}
                    type="button"
                    className="block w-full rounded px-2 py-1 text-left text-sm hover:bg-secondary/60"
                    onClick={() => {
                      setContextMenu((prev) => ({ ...prev, open: false }))
                      void handleSpawnTemplate(template.templateId).catch((error) => {
                        setContextStatusText(
                          error instanceof Error ? error.message : 'spawn failed',
                        )
                      })
                    }}
                  >
                    Spawn {template.label}
                  </button>
                ))}
                {contextMenu.entityId ? (
                  <>
                    <div className="border-t border-border-subtle my-1" />
                    <div className="relative group/owner">
                      <button
                        type="button"
                        className="block w-full rounded px-2 py-1 text-left text-sm hover:bg-secondary/60"
                      >
                        Assign Owner ▸
                      </button>
                      <div className="absolute left-full top-0 z-[320] hidden min-w-64 rounded-md border border-border bg-card/95 p-1 shadow-lg backdrop-blur group-hover/owner:block">
                        {playerEntities.length === 0 ? (
                          <div className="px-2 py-1 text-xs text-muted-foreground">
                            No players available
                          </div>
                        ) : (
                          playerEntities.map((player) => (
                            <button
                              key={player.id}
                              type="button"
                              className="block w-full rounded px-2 py-1 text-left text-sm hover:bg-secondary/60"
                              onClick={() => {
                                setContextMenu((prev) => ({ ...prev, open: false }))
                                if (!player.entityGuid || !contextMenu.entityId) return
                                void handleAssignOwner(
                                  contextMenu.entityId,
                                  player.entityGuid,
                                ).catch((error) => {
                                  setContextStatusText(
                                    error instanceof Error
                                      ? error.message
                                      : 'owner assignment failed',
                                  )
                                })
                              }}
                            >
                              {playerLabel(player)}
                            </button>
                          ))
                        )}
                      </div>
                    </div>
                    <div className="border-t border-border-subtle my-1" />
                    <button
                      type="button"
                      className="block w-full rounded px-2 py-1 text-left text-sm text-destructive hover:bg-destructive/10"
                      onClick={() => {
                        setContextMenu((prev) => ({ ...prev, open: false }))
                        const targetEntityId = contextMenu.entityId
                        if (!targetEntityId) return
                        void handleDeleteEntity(targetEntityId).catch((error) => {
                          setContextStatusText(
                            error instanceof Error ? error.message : 'delete failed',
                          )
                        })
                      }}
                    >
                      Delete Entity
                    </button>
                  </>
                ) : null}
              </div>
            </div>
          )}
          {contextStatusText ? (
            <div className="absolute left-4 top-4 z-[250] rounded border border-border bg-card/90 px-3 py-1 text-xs text-muted-foreground backdrop-blur">
              {contextStatusText}
            </div>
          ) : null}
        </AppLayout>
      </TooltipProvider>
    </ThemeProvider>
  )
}

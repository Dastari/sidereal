import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { faker } from '@faker-js/faker'
import {
  parseAsBoolean,
  parseAsString,
  useQueryStates,
} from 'nuqs'
import type {
  ApiGraph,
  ApiLiveWorld,
  BrpResourceRecord,
  ContextMenuState,
  ExplorerWorkspaceProps,
} from '@/features/explorer/explorer-utils'
import type {
  ExpandedNode,
  GraphEdge,
  GraphNode,
  PlayerVisibilityOverlay,
  WorldEntity,
} from '@/components/grid/types'
import type { BrpTab, DataSourceMode } from '@/components/sidebar/Toolbar'
import type { EntityTreeUiState } from '@/components/sidebar/EntityTree'
import { useSessionStorageNumber } from '@/hooks/use-session-storage-number'
import {
  AppLayout,
  Panel,
  PanelContent,
  PanelHeader,
} from '@/components/layout/AppLayout'
import { GridCanvas } from '@/components/grid/GridCanvas'
import {
  EntityTree,
  createDefaultEntityTreeUiState,
} from '@/components/sidebar/EntityTree'
import { DetailPanel } from '@/components/sidebar/DetailPanel'
import { StatusBar } from '@/components/sidebar/StatusBar'
import { Toolbar } from '@/components/sidebar/Toolbar'
import { Switch } from '@/components/ui/switch'
import {
  AMMO_COUNT_SUFFIX,
  FUEL_TANK_SUFFIX,
  GENERATED_COMPONENT_REGISTRY_TYPE_PATH,
  HEALTH_POOL_SUFFIX,
  POSITION_SUFFIX,
  RESOURCE_SELECTION_PREFIX,
  buildEntitiesFromGraph,
  collectEntityAndDescendants,
  explorerSourceParser,
  extractEntityRegistryTemplateIds,
  fetchBrpResourceValue,
  fetchBrpResources,
  findComponentBySuffix,
  isCameraEntity,
  isPlayerEntity,
  isShipEntity,
  normalizeAmmoValue,
  normalizeFuelValue,
  normalizeHealthValue,
  normalizeVec2Value,
  parseSelectedPlayerVisibilityOverlay,
  playerLabel,
  resolveOwnerTypePath,
} from '@/features/explorer/explorer-utils'

type CameraSnapshot = {
  x: number
  y: number
  zoom: number
}

type LiveTabWorkspaceSnapshot = {
  entities: Array<WorldEntity>
  brpResources: Array<BrpResourceRecord>
  graphNodes: Map<string, GraphNode>
  graphEdges: Array<GraphEdge>
  expandedNodes: Map<string, ExpandedNode>
  selectedId: string | null
  pendingSelectedEntityGuid: string | null
  cameraState: CameraSnapshot
  graphStatus: {
    connected: boolean
    nodeCount: number
    edgeCount: number
    graphName: string
  }
  worldStatus: {
    loaded: boolean
    entityCount: number
  }
  entityTreeUiState: EntityTreeUiState
}

const DEFAULT_CAMERA_STATE: CameraSnapshot = {
  x: 0,
  y: 0,
  zoom: 0.5,
}

export function ExplorerWorkspace({
  scope,
  selectedEntityGuid = null,
  onSelectedEntityGuidChange,
  toolbarContent,
}: ExplorerWorkspaceProps) {
  const DEFAULT_FILTER_MAP_INVISIBLE = true
  const scopeIsDatabase = scope === 'database'
  const [routeState, setRouteState] = useQueryStates({
    sourceMode: explorerSourceParser.withDefault(
      scopeIsDatabase ? 'database' : 'liveServer',
    ),
    activeBrpTabId: parseAsString.withDefault('server'),
    selectedEntityId: parseAsString,
    selectedResourceTypePath: parseAsString,
    filterMapInvisible: parseAsBoolean.withDefault(DEFAULT_FILTER_MAP_INVISIBLE),
  })
  const sourceMode = scopeIsDatabase
    ? 'database'
    : ((routeState.sourceMode === 'database'
        ? 'liveServer'
        : routeState.sourceMode) as DataSourceMode)
  const [brpTabs, setBrpTabs] = useState<Array<BrpTab>>([
    { id: 'server', label: 'Server BRP', port: 15713, kind: 'server' },
    { id: 'client-1', label: 'Client 1 BRP', port: 15714, kind: 'client' },
  ])
  const activeBrpTabId = routeState.activeBrpTabId

  // Data state
  const [entities, setEntities] = useState<Array<WorldEntity>>([])
  const [brpResources, setBrpResources] = useState<Array<BrpResourceRecord>>([])
  const [graphNodes, setGraphNodes] = useState<Map<string, GraphNode>>(
    new Map(),
  )
  const [graphEdges, setGraphEdges] = useState<Array<GraphEdge>>([])
  const [expandedNodes, setExpandedNodes] = useState<Map<string, ExpandedNode>>(
    new Map(),
  )

  // UI state
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [pendingSelectedEntityGuid, setPendingSelectedEntityGuid] = useState<
    string | null
  >(null)
  const [centerRequest, setCenterRequest] = useState<{
    position: { x: number; y: number } | null
    seq: number
  }>({
    position: null,
    seq: 0,
  })
  const [isRefreshing, setIsRefreshing] = useState(false)
  const [sidebarWidth, setSidebarWidth] = useSessionStorageNumber(
    `dashboard:${scope}:sidebar-width`,
    280,
  )
  const [detailPanelWidth, setDetailPanelWidth] = useSessionStorageNumber(
    `dashboard:${scope}:detail-panel-width`,
    320,
  )
  const [hasHydrated, setHasHydrated] = useState(false)
  const filterMapInvisible = hasHydrated
    ? routeState.filterMapInvisible
    : DEFAULT_FILTER_MAP_INVISIBLE
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    open: false,
    x: 0,
    y: 0,
    entityId: null,
    worldX: null,
    worldY: null,
  })
  const [contextStatusText, setContextStatusText] = useState<string | null>(null)
  const [cameraState, setCameraState] = useState(DEFAULT_CAMERA_STATE)
  const [entityTreeUiState, setEntityTreeUiState] = useState<EntityTreeUiState>(
    () => createDefaultEntityTreeUiState(),
  )
  const effectiveSelectedEntityGuid =
    selectedEntityGuid ?? pendingSelectedEntityGuid
  const entitiesRef = useRef<Array<WorldEntity>>([])
  const effectiveSelectedEntityGuidRef = useRef<string | null>(null)
  const previousSourceModeRef = useRef<DataSourceMode | null>(null)
  const liveTabSnapshotsRef = useRef<Map<string, LiveTabWorkspaceSnapshot>>(
    new Map(),
  )
  const previousLiveTabIdRef = useRef<string | null>(null)
  const isLiveMode = !scopeIsDatabase && sourceMode !== 'database'

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
  const setSourceMode = useCallback(
    (nextMode: DataSourceMode) => {
      if (scopeIsDatabase) {
        return
      }
      void setRouteState({ sourceMode: nextMode })
    },
    [scopeIsDatabase, setRouteState],
  )
  const setActiveBrpTabId = useCallback(
    (tabId: string) => {
      void setRouteState({ activeBrpTabId: tabId })
    },
    [setRouteState],
  )
  const isServerBrpMode =
    sourceMode === 'liveServer' && activeBrpTab.kind === 'server'
  const resourceSelectionId = scopeIsDatabase && routeState.selectedResourceTypePath
    ? `${RESOURCE_SELECTION_PREFIX}${routeState.selectedResourceTypePath}`
    : null
  const selectedEntityId = useMemo(
    () =>
      selectedId && selectedId.startsWith(RESOURCE_SELECTION_PREFIX)
        ? null
        : selectedId,
    [selectedId],
  )
  const playerEntities = useMemo(
    () =>
      entities
        .filter((entity) => isPlayerEntity(entity) && Boolean(entity.entityGuid))
        .sort((a, b) => playerLabel(a).localeCompare(playerLabel(b))),
    [entities],
  )
  const shipEntities = useMemo(
    () =>
      entities
        .filter((entity) => isShipEntity(entity))
        .sort((a, b) => a.name.localeCompare(b.name)),
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

  const entitiesForTree = filteredEntities

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
    () =>
      parseSelectedPlayerVisibilityOverlay(
        selectedEntityId,
        graphNodes,
        graphEdges,
      ),
    [selectedEntityId, graphNodes, graphEdges],
  )
  const spawnTemplates = useMemo(
    () => extractEntityRegistryTemplateIds(brpResources),
    [brpResources],
  )

  useEffect(() => {
    setHasHydrated(true)
  }, [])

  useEffect(() => {
    entitiesRef.current = entities
  }, [entities])

  useEffect(() => {
    effectiveSelectedEntityGuidRef.current = effectiveSelectedEntityGuid
  }, [effectiveSelectedEntityGuid])

  useEffect(() => {
    if (!selectedEntityGuid) {
      return
    }
    setPendingSelectedEntityGuid((prev) =>
      prev === selectedEntityGuid ? prev : selectedEntityGuid,
    )
  }, [selectedEntityGuid])

  useEffect(() => {
    if (!scopeIsDatabase) {
      if (!effectiveSelectedEntityGuid) {
        return
      }
      const selectedEntity =
        entities.find(
          (entity) => entity.entityGuid === effectiveSelectedEntityGuid,
        ) ?? null
      if (selectedEntity) {
        setSelectedId(selectedEntity.id)
      }
      return
    }

    if (resourceSelectionId) {
      setSelectedId(resourceSelectionId)
      return
    }
    if (effectiveSelectedEntityGuid) {
      const selectedEntity =
        entities.find(
          (entity) => entity.entityGuid === effectiveSelectedEntityGuid,
        ) ?? null
      if (selectedEntity) {
        setSelectedId(selectedEntity.id)
        return
      }
      return
    }
    if (routeState.selectedEntityId) {
      setSelectedId(routeState.selectedEntityId)
      return
    }
    setSelectedId(null)
  }, [
    entities,
    effectiveSelectedEntityGuid,
    resourceSelectionId,
    routeState.selectedEntityId,
    scopeIsDatabase,
  ])

  useEffect(() => {
    if (!isLiveMode) {
      previousLiveTabIdRef.current = null
      return
    }

    const activeTabId = activeBrpTab.id
    const previousTabId = previousLiveTabIdRef.current
    if (previousTabId && previousTabId !== activeTabId) {
      // Persist outgoing tab before restoring incoming tab state.
      liveTabSnapshotsRef.current.set(previousTabId, {
        entities,
        brpResources,
        graphNodes: new Map(graphNodes),
        graphEdges: [...graphEdges],
        expandedNodes: new Map(expandedNodes),
        selectedId,
        pendingSelectedEntityGuid,
        cameraState,
        graphStatus,
        worldStatus,
        entityTreeUiState,
      })
    }

    if (previousTabId === activeTabId) {
      return
    }

    const snapshot = liveTabSnapshotsRef.current.get(activeTabId)
    if (snapshot) {
      setEntities(snapshot.entities)
      setBrpResources(snapshot.brpResources)
      setGraphNodes(new Map(snapshot.graphNodes))
      setGraphEdges([...snapshot.graphEdges])
      setExpandedNodes(new Map(snapshot.expandedNodes))
      setSelectedId(snapshot.selectedId)
      setPendingSelectedEntityGuid(snapshot.pendingSelectedEntityGuid)
      setCameraState(snapshot.cameraState)
      setGraphStatus(snapshot.graphStatus)
      setWorldStatus(snapshot.worldStatus)
      setEntityTreeUiState(snapshot.entityTreeUiState)
      onSelectedEntityGuidChange?.(snapshot.pendingSelectedEntityGuid)
      previousLiveTabIdRef.current = activeTabId
      return
    }

    // New live tab starts with an empty local workspace until first poll.
    setEntities([])
    setBrpResources([])
    setGraphNodes(new Map())
    setGraphEdges([])
    setExpandedNodes(new Map())
    setSelectedId(null)
    setPendingSelectedEntityGuid(null)
    setCameraState(DEFAULT_CAMERA_STATE)
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
    setEntityTreeUiState(createDefaultEntityTreeUiState())
    onSelectedEntityGuidChange?.(null)
    previousLiveTabIdRef.current = activeTabId
  }, [
    activeBrpTab.id,
    brpResources,
    cameraState,
    entities,
    entityTreeUiState,
    expandedNodes,
    graphEdges,
    graphNodes,
    graphStatus,
    isLiveMode,
    onSelectedEntityGuidChange,
    pendingSelectedEntityGuid,
    selectedId,
    worldStatus,
  ])

  // Load data
  const loadData = useCallback(async () => {
    setIsRefreshing(true)

    try {
      if (sourceMode === 'database') {
        const graphRes = await fetch('/api/graph').then(
          (r) => r.json() as Promise<ApiGraph>,
        )

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
          setBrpResources([])
          setWorldStatus({
            loaded: false,
            entityCount: 0,
          })
          return
        }

        const entitiesFromGraph = buildEntitiesFromGraph(graphRes)
        const currentEntities = entitiesRef.current
        const currentSelectedEntityGuid = effectiveSelectedEntityGuidRef.current
        const shouldPreserveSelectedEntity =
          Boolean(currentSelectedEntityGuid) &&
          currentEntities.some(
            (entity) => entity.entityGuid === currentSelectedEntityGuid,
          ) &&
          !entitiesFromGraph.some(
            (entity) => entity.entityGuid === currentSelectedEntityGuid,
          )
        const nextEntities = shouldPreserveSelectedEntity
          ? currentEntities
          : entitiesFromGraph
        setEntities(nextEntities)
        const registryResource = await fetchBrpResourceValue(
          activeBrpTab.port,
          'server',
          GENERATED_COMPONENT_REGISTRY_TYPE_PATH,
        )
        setBrpResources(
          registryResource.error
            ? []
            : [
                {
                  typePath: GENERATED_COMPONENT_REGISTRY_TYPE_PATH,
                  value: registryResource.value,
                },
              ],
        )
        setWorldStatus({
          loaded: true,
          entityCount: nextEntities.length,
        })
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
          setBrpResources([])
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
        const listedResources = await fetchBrpResources(
          activeBrpTab.port,
          activeBrpTab.kind,
        )
        const hydratedResources = await Promise.all(
          listedResources.map(async (resource) => {
            if (resource.error) return resource
            if (
              !resource.typePath.includes('EntityRegistryResource') &&
              resource.typePath !== GENERATED_COMPONENT_REGISTRY_TYPE_PATH
            ) {
              return resource
            }
            const loaded = await fetchBrpResourceValue(
              activeBrpTab.port,
              activeBrpTab.kind,
              resource.typePath,
            )
            return {
              ...resource,
              ...loaded,
            }
          }),
        )
        setBrpResources(hydratedResources)
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
      setBrpResources([])
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
    const previousSourceMode = previousSourceModeRef.current
    previousSourceModeRef.current = sourceMode

    if (previousSourceMode === null || previousSourceMode === sourceMode) {
      return
    }

    const crossedDatabaseBoundary =
      (previousSourceMode === 'database') !== (sourceMode === 'database')
    if (!crossedDatabaseBoundary) {
      return
    }

    setExpandedNodes(new Map())
    setSelectedId(null)
    setPendingSelectedEntityGuid(null)
    if (!scopeIsDatabase) {
      onSelectedEntityGuidChange?.(null)
    }
    void setRouteState({
      selectedEntityId: null,
      selectedResourceTypePath: null,
    })
  }, [
    onSelectedEntityGuidChange,
    scopeIsDatabase,
    setRouteState,
    sourceMode,
  ])

  useEffect(() => {
    if (!selectedId) return
    if (selectedId.startsWith(RESOURCE_SELECTION_PREFIX)) {
      return
    }
    if (effectiveSelectedEntityGuid) {
      const selectedEntity =
        entities.find(
          (entity) => entity.entityGuid === effectiveSelectedEntityGuid,
        ) ?? null
      if (!selectedEntity) {
        return
      }
      if (selectedEntity.id !== selectedId) {
        setSelectedId(selectedEntity.id)
        return
      }
    }
    const selectedExists = entities.some((entity) => entity.id === selectedId)
    if (!selectedExists) {
      setSelectedId(null)
      onSelectedEntityGuidChange?.(null)
      if (scopeIsDatabase) {
        void setRouteState({
          selectedEntityId: null,
          selectedResourceTypePath: null,
        })
      }
    }
  }, [
    entities,
    onSelectedEntityGuidChange,
    selectedId,
    scopeIsDatabase,
    setRouteState,
    effectiveSelectedEntityGuid,
  ])

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

  useEffect(() => {
    if (!selectedId || !selectedId.startsWith(RESOURCE_SELECTION_PREFIX)) return
    if (sourceMode !== 'liveServer' && sourceMode !== 'liveClient') return
    const typePath = selectedId.slice(RESOURCE_SELECTION_PREFIX.length)
    const existing = brpResources.find((resource) => resource.typePath === typePath)
    if (!existing || existing.value !== undefined || existing.error) return
    let cancelled = false
    void fetchBrpResourceValue(activeBrpTab.port, activeBrpTab.kind, typePath).then(
      (loaded) => {
        if (cancelled) return
        setBrpResources((prev) =>
          prev.map((resource) =>
            resource.typePath === typePath
              ? {
                  ...resource,
                  ...loaded,
                }
              : resource,
          ),
        )
      },
    )
    return () => {
      cancelled = true
    }
  }, [activeBrpTab.kind, activeBrpTab.port, brpResources, selectedId, sourceMode])

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
        const baseEntity = entitiesForMap.find((e) => e.id === id)
        const existingNode = prev.get(id)
        const centerX = baseEntity?.x || existingNode?.x || 0
        const centerY = baseEntity?.y || existingNode?.y || 0

        // Only explode child entities in the map graph view.
        const childEntities = entitiesForMap.filter(
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
    [entitiesForMap],
  )

  // Update a component value via BRP (Server BRP or Client BRP only)
  const handleComponentUpdate = useCallback(
    async (
      entityId: string,
      typePath: string,
      componentKind: string,
      value: unknown,
    ) => {
      try {
        let res: Response
        if (sourceMode === 'database') {
          res = await fetch('/api/graph', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              entityId,
              typePath,
              componentKind,
              value,
            }),
          })
        } else {
          const numericEntityId = Number(entityId)
          if (!Number.isFinite(numericEntityId)) {
            console.error('Component update failed: entity ID must be numeric for BRP')
            return
          }
          const url = `/api/brp?port=${activeBrpTab.port}&target=${activeBrpTab.kind}`
          res = await fetch(url, {
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
        }
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

  const updateSelection = useCallback(
    (nextId: string | null) => {
      setSelectedId(nextId)

      if (!nextId) {
        setPendingSelectedEntityGuid(null)
        onSelectedEntityGuidChange?.(null)
        if (scopeIsDatabase) {
          void setRouteState({
            selectedEntityId: null,
            selectedResourceTypePath: null,
          })
        }
        return
      }

      if (nextId.startsWith(RESOURCE_SELECTION_PREFIX)) {
        setPendingSelectedEntityGuid(null)
        onSelectedEntityGuidChange?.(null)
        if (scopeIsDatabase) {
          void setRouteState({
            selectedEntityId: null,
            selectedResourceTypePath: nextId.slice(RESOURCE_SELECTION_PREFIX.length),
          })
        }
        return
      }

      const selectedEntity = entities.find((entity) => entity.id === nextId) ?? null
      const entityGuid = selectedEntity?.entityGuid ?? null
      setPendingSelectedEntityGuid(entityGuid)
      onSelectedEntityGuidChange?.(entityGuid)
      if (scopeIsDatabase) {
        void setRouteState({
          selectedEntityId: entityGuid ? null : nextId,
          selectedResourceTypePath: null,
        })
      }
    },
    [entities, onSelectedEntityGuidChange, scopeIsDatabase, setRouteState],
  )

  const handleCameraStateChange = useCallback(
    (camera: { x: number; y: number; zoom: number }) => {
      setCameraState({
        x: Number(camera.x.toFixed(3)),
        y: Number(camera.y.toFixed(3)),
        zoom: Number(camera.zoom.toFixed(4)),
      })
    },
    [],
  )

  // Handle entity selection from tree
  const handleSelectFromTree = useCallback((id: string) => {
    updateSelection(id)
    if (id.startsWith(RESOURCE_SELECTION_PREFIX)) {
      return
    }
    const entity = entitiesForMap.find((e) => e.id === id)
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
  }, [entitiesForMap, updateSelection])

  const handleSelectFromGrid = useCallback((id: string | null) => {
    updateSelection(id)
  }, [updateSelection])

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
        if (sourceMode !== 'liveServer' && sourceMode !== 'liveClient') {
          throw new Error('Delete is disabled for database mode')
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
        updateSelection(null)
      }
    },
    [sourceMode, selectedId, loadData, activeBrpTab, updateSelection],
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
    (
      entityId: string | null,
      point: { x: number; y: number },
      worldPoint?: { x: number; y: number },
    ) => {
      if (!isServerBrpMode) return
      setContextMenu({
        open: true,
        x: point.x,
        y: point.y,
        entityId,
        worldX: worldPoint?.x ?? null,
        worldY: worldPoint?.y ?? null,
      })
    },
    [isServerBrpMode],
  )

  const handleSpawnTemplate = useCallback(
    async (templateId: string) => {
      const actorPlayerEntityId = playerEntities[0]?.entityGuid
      if (!actorPlayerEntityId) {
        setContextStatusText('Spawn failed: no player entity available for admin actor context')
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
          player_entity_id: actorPlayerEntityId,
          bundle_id: templateId,
          overrides: { display_name: generatedName, owner_id: null },
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

  const insertComponents = useCallback(
    async (entityId: string, components: Record<string, unknown>) => {
      const numericEntityId = Number(entityId)
      if (!Number.isFinite(numericEntityId)) {
        throw new Error('Entity ID must be numeric for BRP writes')
      }
      const response = await fetch(
        `/api/brp?port=${activeBrpTab.port}&target=${activeBrpTab.kind}`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            method: 'world.insert_components',
            params: {
              entity: numericEntityId,
              components,
            },
          }),
        },
      )
      const payload = (await response.json()) as { error?: string }
      if (!response.ok || payload.error) {
        throw new Error(payload.error ?? 'component update failed')
      }
    },
    [activeBrpTab.kind, activeBrpTab.port],
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

  const handleMoveShipTo = useCallback(
    async (shipEntityId: string, x: number, y: number) => {
      const positionComponent = findComponentBySuffix(
        shipEntityId,
        graphNodes,
        graphEdges,
        POSITION_SUFFIX,
      )
      if (!positionComponent) {
        throw new Error(`Move failed: no Position component found for ${shipEntityId}`)
      }
      const normalizedValue = normalizeVec2Value(positionComponent.value, x, y)
      await insertComponents(shipEntityId, {
        [positionComponent.typePath]: normalizedValue,
      })
      setContextStatusText(`Moved ${shipEntityId} to (${x.toFixed(2)}, ${y.toFixed(2)})`)
      await loadData()
    },
    [graphEdges, graphNodes, insertComponents, loadData],
  )

  const handleRepairRefuel = useCallback(
    async (shipEntityId: string) => {
      const targetEntityIds = collectEntityAndDescendants(shipEntityId, entities)
      let mutationCount = 0
      for (const targetEntityId of targetEntityIds) {
        const health = findComponentBySuffix(
          targetEntityId,
          graphNodes,
          graphEdges,
          HEALTH_POOL_SUFFIX,
        )
        const fuel = findComponentBySuffix(
          targetEntityId,
          graphNodes,
          graphEdges,
          FUEL_TANK_SUFFIX,
        )
        const ammo = findComponentBySuffix(
          targetEntityId,
          graphNodes,
          graphEdges,
          AMMO_COUNT_SUFFIX,
        )
        const componentUpdates: Record<string, unknown> = {}
        if (health) {
          const patched = normalizeHealthValue(health.value)
          if (patched !== null) {
            componentUpdates[health.typePath] = patched
          }
        }
        if (fuel) {
          const patched = normalizeFuelValue(fuel.value)
          if (patched !== null) {
            componentUpdates[fuel.typePath] = patched
          }
        }
        if (ammo) {
          const patched = normalizeAmmoValue(ammo.value)
          if (patched !== null) {
            componentUpdates[ammo.typePath] = patched
          }
        }
        if (Object.keys(componentUpdates).length === 0) {
          continue
        }
        await insertComponents(targetEntityId, componentUpdates)
        mutationCount += Object.keys(componentUpdates).length
      }
      if (mutationCount === 0) {
        setContextStatusText(`Repair/Refuel: no compatible components found on ${shipEntityId}`)
      } else {
        setContextStatusText(`Repair/Refuel applied to ${shipEntityId} (${mutationCount} updates)`)
      }
      await loadData()
    },
    [entities, graphEdges, graphNodes, insertComponents, loadData],
  )

  return (
        <AppLayout
          header={
            <Toolbar
              sourceMode={sourceMode}
              onSourceModeChange={setSourceMode}
              brpTabs={brpTabs}
              activeBrpTabId={activeBrpTab.id}
              onActiveBrpTabIdChange={setActiveBrpTabId}
              onAddClientTab={handleAddClientTab}
              showDataSourceTabs={!scopeIsDatabase}
              showDatabaseTab={false}
            >
              {toolbarContent}
            </Toolbar>
          }
          sidebar={
            <Panel>
              <PanelHeader className="py-2">
                <div className="flex items-center justify-between gap-2">
                  <h1 className="text-sm font-semibold text-foreground">
                    {scopeIsDatabase ? 'Database Explorer' : 'Game World Explorer'}
                  </h1>
                  <label className="inline-flex items-center gap-2 text-xs text-muted-foreground">
                    <span className="whitespace-nowrap">Entities Only</span>
                    <Switch
                      checked={filterMapInvisible}
                      onCheckedChange={(checked) => {
                        void setRouteState({ filterMapInvisible: checked })
                      }}
                      aria-label="Filter to entities with EntityGuid component"
                    />
                  </label>
                </div>
              </PanelHeader>
              <PanelContent>
                <EntityTree
                  entities={entitiesForTree}
                  resources={brpResources.filter(
                    (resource) => resource.typePath !== '__error__',
                  )}
                  uiState={entityTreeUiState}
                  onUiStateChange={setEntityTreeUiState}
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
          onSidebarResize={(width) => {
            setSidebarWidth(width)
          }}
          detailPanelWidth={detailPanelWidth}
          onDetailPanelResize={(width) => {
            setDetailPanelWidth(width)
          }}
          detailPanel={
            <Panel>
              <DetailPanel
                selectedId={selectedId}
                entities={entities}
                resources={brpResources.filter(
                  (resource) => resource.typePath !== '__error__',
                )}
                expandedNodes={expandedNodes}
                graphNodes={graphNodes}
                graphEdges={graphEdges}
                onSelect={updateSelection}
                onExpand={handleExpand}
                onCollapse={handleCollapse}
                sourceMode={sourceMode}
                onComponentUpdate={handleComponentUpdate}
                onClose={() => updateSelection(null)}
              />
            </Panel>
          }
        >
          <GridCanvas
            entities={entitiesForMap}
            graphNodes={graphNodes}
            graphEdges={graphEdges}
            selectedId={selectedEntityId}
            onSelect={handleSelectFromGrid}
            onExpand={handleExpand}
            expandedNodes={expandedNodes}
            filterMapInvisible={filterMapInvisible}
            sourceMode={sourceMode}
            excludedFromMapIds={cameraEntityIds}
            centerOnPosition={centerOnPosition}
            centerOnRequestSeq={centerRequest.seq}
            selectedPlayerVisibilityOverlay={selectedPlayerVisibilityOverlay}
            cameraState={cameraState}
            onCameraStateChange={handleCameraStateChange}
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
                {contextMenu.entityId ? (
                  <>
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
                    <button
                      type="button"
                      className="block w-full rounded px-2 py-1 text-left text-sm hover:bg-secondary/60"
                      onClick={() => {
                        setContextMenu((prev) => ({ ...prev, open: false }))
                        const targetEntityId = contextMenu.entityId
                        if (!targetEntityId) return
                        void handleRepairRefuel(targetEntityId).catch((error) => {
                          setContextStatusText(
                            error instanceof Error ? error.message : 'repair/refuel failed',
                          )
                        })
                      }}
                    >
                      Repair & Refuel
                    </button>
                    <button
                      type="button"
                      className="block w-full rounded px-2 py-1 text-left text-sm hover:bg-secondary/60"
                      onClick={() => {
                        setContextMenu((prev) => ({ ...prev, open: false }))
                        const targetEntityId = contextMenu.entityId
                        if (!targetEntityId) return
                        void handleMoveShipTo(targetEntityId, 0, 0).catch((error) => {
                          setContextStatusText(
                            error instanceof Error ? error.message : 'move failed',
                          )
                        })
                      }}
                    >
                      Move to 0,0
                    </button>
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
                ) : (
                  <>
                    <div className="relative group/spawn">
                      <button
                        type="button"
                        className="block w-full rounded px-2 py-1 text-left text-sm hover:bg-secondary/60"
                      >
                        Spawn Ship ▸
                      </button>
                      <div className="absolute left-full top-0 z-[320] hidden min-w-64 rounded-md border border-border bg-card/95 p-1 shadow-lg backdrop-blur group-hover/spawn:block">
                        {spawnTemplates.length > 0 ? (
                          spawnTemplates.map((template) => (
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
                              {template.label}
                            </button>
                          ))
                        ) : (
                          <div className="px-2 py-1 text-xs text-muted-foreground">
                            No ship templates from EntityRegistryResource
                          </div>
                        )}
                      </div>
                    </div>
                    <div className="relative group/movehere">
                      <button
                        type="button"
                        className="block w-full rounded px-2 py-1 text-left text-sm hover:bg-secondary/60"
                      >
                        Move Ship Here ▸
                      </button>
                      <div className="absolute left-full top-0 z-[320] hidden min-w-64 rounded-md border border-border bg-card/95 p-1 shadow-lg backdrop-blur group-hover/movehere:block">
                        {contextMenu.worldX === null || contextMenu.worldY === null ? (
                          <div className="px-2 py-1 text-xs text-muted-foreground">
                            No world position at cursor
                          </div>
                        ) : shipEntities.length === 0 ? (
                          <div className="px-2 py-1 text-xs text-muted-foreground">
                            No ships in world
                          </div>
                        ) : (
                          shipEntities.map((ship) => (
                            <button
                              key={ship.id}
                              type="button"
                              className="block w-full rounded px-2 py-1 text-left text-sm hover:bg-secondary/60"
                              onClick={() => {
                                setContextMenu((prev) => ({ ...prev, open: false }))
                                if (contextMenu.worldX === null || contextMenu.worldY === null) {
                                  return
                                }
                                void handleMoveShipTo(
                                  ship.id,
                                  contextMenu.worldX,
                                  contextMenu.worldY,
                                ).catch((error) => {
                                  setContextStatusText(
                                    error instanceof Error ? error.message : 'move failed',
                                  )
                                })
                              }}
                            >
                              {ship.name}
                            </button>
                          ))
                        )}
                      </div>
                    </div>
                  </>
                )}
              </div>
            </div>
          )}
          {contextStatusText ? (
            <div className="absolute left-4 top-4 z-[250] rounded border border-border bg-card/90 px-3 py-1 text-xs text-muted-foreground backdrop-blur">
              {contextStatusText}
            </div>
          ) : null}
        </AppLayout>
  )
}

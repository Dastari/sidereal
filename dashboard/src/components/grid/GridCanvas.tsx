import * as React from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useGridRenderer } from './useGridRenderer'
import type {
  Camera,
  ExpandedNode,
  GraphEdge,
  PlayerVisibilityOverlay,
  WorldEntity,
} from './types'
import type { DataSourceMode } from '@/components/sidebar/Toolbar'
import { GridScanOverlay } from '@/components/thegridcn/grid-scan-overlay'
import { LocationDisplay } from '@/components/thegridcn/location-display'
import { Reticle } from '@/components/thegridcn/reticle'
import { useGridThemeColors } from '@/hooks/use-grid-theme-colors'
import { DEFAULT_GRID_INTENSITY } from '@/lib/grid-theme'
import { useTheme } from '@/hooks/use-theme'
import { cn } from '@/lib/utils'

interface GridCanvasProps {
  entities: Array<WorldEntity>
  graphNodes: Map<
    string,
    { label: string; kind: string; properties: Record<string, unknown> }
  >
  graphEdges: Array<GraphEdge>
  selectedId: string | null
  onSelect: (id: string | null) => void
  onExpand: (id: string) => void
  expandedNodes: Map<string, ExpandedNode>
  filterMapInvisible: boolean
  sourceMode: DataSourceMode
  /** Entity IDs to exclude from the map (e.g. camera entities). Still shown in tree. */
  excludedFromMapIds?: Set<string>
  /** When set, center the grid camera on this world position (e.g. when selecting an entity from the tree). */
  centerOnPosition?: { x: number; y: number } | null
  /** Monotonic token for one-shot center requests; position updates alone should not retrigger centering. */
  centerOnRequestSeq?: number
  /** Visibility overlay data for selected player entity. */
  selectedPlayerVisibilityOverlay?: PlayerVisibilityOverlay | null
  cameraState?: { x: number; y: number; zoom: number } | null
  onCameraStateChange?: (camera: { x: number; y: number; zoom: number }) => void
  onContextMenuRequest?: (
    entityId: string | null,
    point: { x: number; y: number },
    worldPoint: { x: number; y: number },
  ) => void
}

// graphNodes is used indirectly via expandedNodes

function formatHudCoordinate(value: number): string {
  if (!Number.isFinite(value)) {
    return '0.0'
  }
  const absValue = Math.abs(value)
  if (absValue >= 1000) {
    return value.toFixed(0)
  }
  if (absValue >= 100) {
    return value.toFixed(1)
  }
  return value.toFixed(2)
}

function formatSectorCode(x: number, y: number): string {
  const sectorSize = 1000
  const sectorX = Math.floor(x / sectorSize)
  const sectorY = Math.floor(y / sectorSize)
  const eastWest = sectorX >= 0 ? 'E' : 'W'
  const northSouth = sectorY >= 0 ? 'N' : 'S'
  return `${eastWest}${Math.abs(sectorX).toString().padStart(2, '0')}-${northSouth}${Math.abs(sectorY).toString().padStart(2, '0')}`
}

function formatGridCellCode(x: number, y: number): string {
  const cellSize = 250
  const cellX = Math.floor(x / cellSize)
  const cellY = Math.floor(y / cellSize)
  const letterIndex = Math.abs(cellX) % 26
  const column = String.fromCharCode(65 + letterIndex)
  const row = Math.abs(cellY).toString().padStart(2, '0')
  const xPrefix = cellX >= 0 ? '+' : '-'
  const yPrefix = cellY >= 0 ? '+' : '-'
  return `${column}${row} ${xPrefix}${Math.abs(cellX)}:${yPrefix}${Math.abs(cellY)}`
}

function formatNodeDisplayToken(node: ExpandedNode | null): string | null {
  if (!node) {
    return null
  }
  const label = node.label.trim().replace(/\s+/g, ' ').toUpperCase()
  const compactLabel =
    label.length > 22 ? `${label.slice(0, 22).trimEnd()}...` : label
  const kind = node.kind.trim().replace(/\s+/g, ' ').toUpperCase()
  const compactKind =
    kind.length > 14 ? `${kind.slice(0, 14).trimEnd()}...` : kind
  return compactKind ? `${compactKind} ${compactLabel}` : compactLabel
}

export function GridCanvas({
  entities,
  graphNodes: _graphNodes,
  graphEdges,
  selectedId,
  onSelect,
  onExpand,
  expandedNodes,
  filterMapInvisible,
  sourceMode,
  excludedFromMapIds,
  centerOnPosition,
  centerOnRequestSeq,
  selectedPlayerVisibilityOverlay,
  cameraState,
  onCameraStateChange,
  onContextMenuRequest,
}: GridCanvasProps) {
  const MIN_ZOOM = 1e-6
  const MAX_ZOOM = 1e6
  void _graphNodes // Used indirectly via expandedNodes
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const labelsCanvasRef = useRef<HTMLCanvasElement>(null)
  const [hasMounted, setHasMounted] = useState(false)
  const { resolvedTheme, gridTheme, gridIntensity } = useTheme()
  const effectiveGridIntensity = hasMounted
    ? gridIntensity
    : DEFAULT_GRID_INTENSITY
  const themeColors = useGridThemeColors(
    `${resolvedTheme}:${gridTheme}:${gridIntensity}`,
  )
  const scanOverlayEnabled = effectiveGridIntensity !== 'off'
  const scanOverlaySpeed =
    effectiveGridIntensity === 'heavy'
      ? 5.5
      : effectiveGridIntensity === 'light'
        ? 10
        : 7.5
  const scanOverlayGridSize =
    effectiveGridIntensity === 'heavy'
      ? 72
      : effectiveGridIntensity === 'light'
        ? 108
        : 88

  const cameraRef = useRef<Camera>({ x: 0, y: 0, zoom: 0.5 })
  const draggingRef = useRef(false)
  const movedRef = useRef(false)
  const pointerRef = useRef({ x: 0, y: 0 })
  const [hoveredId, setHoveredId] = useState<string | null>(null)
  const [isPanning, setIsPanning] = useState(false)
  const [showNamePlates, setShowNamePlates] = useState(false)
  const [zoomPercent, setZoomPercent] = useState(50)
  const [cameraHudState, setCameraHudState] = useState({
    x: 0,
    y: 0,
    zoom: 0.5,
  })
  const rafRef = useRef<number>(0)
  const renderQueuedRef = useRef(false)
  const requestRenderRef = useRef<() => void>(() => {})
  const queueCameraStateSyncRef = useRef<() => void>(() => {})
  const markerIconsRef = useRef<
    Partial<Record<'ship' | 'planet' | 'star' | 'asteroid', HTMLImageElement>>
  >({})
  const cameraSyncTimerRef = useRef<number | null>(null)
  const resizeDebounceTimerRef = useRef<number | null>(null)
  const lastAppliedCameraKeyRef = useRef<string | null>(null)

  const { init, resize, render } = useGridRenderer(canvasRef, themeColors)
  const renderNodesRef = useRef<Map<string, ExpandedNode>>(new Map())

  useEffect(() => {
    setHasMounted(true)
  }, [])

  const syncCameraHudState = useCallback(() => {
    const camera = cameraRef.current
    setCameraHudState((previous) => {
      if (
        Math.abs(previous.x - camera.x) < 0.01 &&
        Math.abs(previous.y - camera.y) < 0.01 &&
        Math.abs(previous.zoom - camera.zoom) < 0.0001
      ) {
        return previous
      }
      return {
        x: camera.x,
        y: camera.y,
        zoom: camera.zoom,
      }
    })
  }, [])

  const emitCameraState = useCallback(() => {
    if (!onCameraStateChange) return
    const camera = cameraRef.current
    onCameraStateChange({
      x: camera.x,
      y: camera.y,
      zoom: camera.zoom,
    })
  }, [onCameraStateChange])

  const queueCameraStateSync = useCallback(() => {
    if (!onCameraStateChange) return
    if (cameraSyncTimerRef.current !== null) {
      window.clearTimeout(cameraSyncTimerRef.current)
    }
    cameraSyncTimerRef.current = window.setTimeout(() => {
      cameraSyncTimerRef.current = null
      emitCameraState()
    }, 120)
  }, [emitCameraState, onCameraStateChange])

  // Build combined node map for rendering
  // ONLY render root entities (no parentEntityId) on the map at their x/y positions
  // Database explorer: map shows only roots. Live: child entities shown when expanded.
  const allNodes = React.useMemo(() => {
    const result = new Map<string, ExpandedNode>()

    // Add ONLY root world entities (no parentEntityId) at depth 0
    for (const entity of entities) {
      // Sidereal Entities Only: show only entities that have an EntityGuid component (BRP and database).
      if (filterMapInvisible && !entity.entityGuid) {
        continue
      }
      // Do not render entities that do not have source world position.
      if (entity.hasPosition === false) {
        continue
      }
      // Skip child entities - they should not be rendered on the map
      if (entity.parentEntityId) {
        continue
      }

      result.set(entity.id, {
        id: entity.id,
        parentId: null,
        x: entity.x,
        y: entity.y,
        label: entity.name,
        kind: entity.kind,
        isExpanded: expandedNodes.has(entity.id),
        depth: 0,
        properties: {
          shardId: entity.shardId,
          vx: entity.vx,
          vy: entity.vy,
          sampledAtMs: entity.sampledAtMs,
          componentCount: entity.componentCount,
          entity_labels: entity.entity_labels,
        },
      })
    }

    // Add expanded child entity nodes only for live mode; database map shows roots only
    if (sourceMode !== 'database') {
      for (const [id, node] of expandedNodes) {
        if (excludedFromMapIds?.has(id)) continue
        if (!result.has(id)) {
          result.set(id, node)
        }
      }
    }

    return result
  }, [
    entities,
    expandedNodes,
    filterMapInvisible,
    sourceMode,
    excludedFromMapIds,
  ])

  const visibleGraphEdges = React.useMemo(() => {
    if (allNodes.size === 0) {
      return []
    }

    // Root-only map mode has no visible graph edges. Avoid scanning the full BRP graph.
    if (sourceMode === 'database' || expandedNodes.size === 0) {
      return []
    }

    const visibleNodeIds = new Set(allNodes.keys())
    return graphEdges.filter(
      (edge) => visibleNodeIds.has(edge.from) && visibleNodeIds.has(edge.to),
    )
  }, [allNodes, expandedNodes.size, graphEdges, sourceMode])

  // World-to-screen coordinate conversion
  const worldToScreen = useCallback((wx: number, wy: number) => {
    const canvas = canvasRef.current
    if (!canvas) return { x: 0, y: 0 }
    const cam = cameraRef.current
    return {
      x: (wx - cam.x) * cam.zoom + canvas.width * 0.5,
      y: canvas.height * 0.5 - (wy - cam.y) * cam.zoom,
    }
  }, [])

  const screenToWorld = useCallback((clientX: number, clientY: number) => {
    const canvas = canvasRef.current
    if (!canvas) return { x: 0, y: 0 }
    const rect = canvas.getBoundingClientRect()
    const dpr = window.devicePixelRatio || 1
    const sx = (clientX - rect.left) * dpr
    const sy = (clientY - rect.top) * dpr
    const cam = cameraRef.current
    return {
      x: (sx - canvas.width * 0.5) / cam.zoom + cam.x,
      y: (canvas.height * 0.5 - sy) / cam.zoom + cam.y,
    }
  }, [])

  const cullNodesToViewport = useCallback(
    (nodes: Map<string, ExpandedNode>): Map<string, ExpandedNode> => {
      const canvas = canvasRef.current
      if (!canvas || nodes.size === 0) {
        return nodes
      }

      const cam = cameraRef.current
      const overscanPx = 96
      const halfWidthWorld = (canvas.width * 0.5 + overscanPx) / cam.zoom
      const halfHeightWorld = (canvas.height * 0.5 + overscanPx) / cam.zoom
      const minX = cam.x - halfWidthWorld
      const maxX = cam.x + halfWidthWorld
      const minY = cam.y - halfHeightWorld
      const maxY = cam.y + halfHeightWorld

      const visibleNodes = new Map<string, ExpandedNode>()
      for (const [id, node] of nodes) {
        if (node.x < minX || node.x > maxX || node.y < minY || node.y > maxY) {
          continue
        }
        visibleNodes.set(id, node)
      }
      return visibleNodes
    },
    [],
  )

  // Pick node at screen position
  const pickNode = useCallback(
    (clientX: number, clientY: number): string | null => {
      const canvas = canvasRef.current
      if (!canvas) return null

      const rect = canvas.getBoundingClientRect()
      const dpr = window.devicePixelRatio || 1
      const sx = (clientX - rect.left) * dpr
      const sy = (clientY - rect.top) * dpr

      let closest: string | null = null
      let closestDist = 25 * dpr // Hit radius

      for (const [id, node] of renderNodesRef.current) {
        const screenPos = worldToScreen(node.x, node.y)
        const dist = Math.hypot(screenPos.x - sx, screenPos.y - sy)
        if (dist < closestDist) {
          closestDist = dist
          closest = id
        }
      }

      return closest
    },
    [worldToScreen],
  )

  // Draw labels on 2D canvas
  const drawLabels = useCallback(() => {
    const canvas = canvasRef.current
    const labelsCanvas = labelsCanvasRef.current
    if (!canvas || !labelsCanvas) return

    const ctx = labelsCanvas.getContext('2d')
    if (!ctx) return

    const dpr = window.devicePixelRatio || 1
    if (
      labelsCanvas.width !== canvas.width ||
      labelsCanvas.height !== canvas.height
    ) {
      labelsCanvas.width = canvas.width
      labelsCanvas.height = canvas.height
    }

    ctx.clearRect(0, 0, labelsCanvas.width, labelsCanvas.height)
    ctx.font = `${11 * dpr}px Inter, system-ui, sans-serif`
    const [lr, lg, lb] = themeColors.label
    ctx.fillStyle = `rgba(${Math.round(lr * 255)}, ${Math.round(lg * 255)}, ${Math.round(lb * 255)}, 0.9)`
    ctx.textAlign = 'left'
    ctx.textBaseline = 'middle'

    const cam = cameraRef.current
    const resolveIconKey = (
      entityLabels: Array<string> | undefined,
      kind: string,
    ): 'ship' | 'planet' | 'star' | 'asteroid' | null => {
      const labels = (entityLabels ?? []).map((label) =>
        String(label).toLowerCase(),
      )
      if (labels.some((label) => label.includes('star'))) return 'star'
      if (labels.some((label) => label.includes('asteroid'))) return 'asteroid'
      if (
        labels.some(
          (label) =>
            label.includes('planet') || label.includes('celestialbody'),
        )
      ) {
        return 'planet'
      }
      if (
        labels.some(
          (label) => label.includes('ship') || label.includes('player'),
        )
      ) {
        return 'ship'
      }
      const kindNormalized = kind.toLowerCase()
      if (kindNormalized.includes('star')) return 'star'
      if (kindNormalized.includes('asteroid')) return 'asteroid'
      if (kindNormalized.includes('planet')) return 'planet'
      if (kindNormalized.includes('ship') || kindNormalized.includes('player'))
        return 'ship'
      return null
    }
    const resolveIconColor = (entityLabels: Array<string> | undefined) => {
      const labels = (entityLabels ?? []).map((label) =>
        String(label).toLowerCase(),
      )
      // Match tactical map marker roles: self/friendly blue, contact yellow.
      if (labels.includes('player')) return [0.22, 0.62, 1.0] as const
      return [1.0, 0.9, 0.34] as const
    }
    const drawNodeMarker = (id: string, node: ExpandedNode) => {
      const screenPos = worldToScreen(node.x, node.y)
      const entityLabels = node.properties.entity_labels as
        | Array<string>
        | undefined
      const [r, g, b] = themeColors.getEntityColor(node.kind, entityLabels)
      const zoomBoost = Math.max(
        0,
        Math.min(6, Math.log2(Math.max(cam.zoom, 1) + 1) * 1.5),
      )
      const pointSize = Math.max(
        7,
        Math.min(20, (node.depth === 0 ? 20 : 12) * 0.45 + 5 + zoomBoost),
      )
      const radius = pointSize * 0.5
      const iconKey = resolveIconKey(entityLabels, node.kind)
      const iconImage = iconKey ? markerIconsRef.current[iconKey] : undefined
      const [iconR, iconG, iconB] = resolveIconColor(entityLabels)
      const iconSize = Math.max(8, Math.min(96, cam.zoom * 18))
      const markerRadius = iconImage ? iconSize * 0.5 : radius

      if (iconImage) {
        ctx.save()
        ctx.translate(screenPos.x, screenPos.y)
        const iconX = -iconSize * 0.5
        const iconY = -iconSize * 0.5
        ctx.drawImage(iconImage, iconX, iconY, iconSize, iconSize)
        ctx.globalCompositeOperation = 'source-atop'
        ctx.fillStyle = `rgba(${Math.round(iconR * 255)}, ${Math.round(iconG * 255)}, ${Math.round(iconB * 255)}, 0.95)`
        ctx.fillRect(iconX, iconY, iconSize, iconSize)
        ctx.restore()
      } else {
        ctx.beginPath()
        ctx.arc(screenPos.x, screenPos.y, radius, 0, Math.PI * 2)
        ctx.fillStyle = `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, 0.95)`
        ctx.fill()
      }

      if (id === hoveredId) {
        ctx.lineWidth = Math.max(1, 1.5 * dpr)
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.65)'
        ctx.beginPath()
        ctx.arc(
          screenPos.x,
          screenPos.y,
          markerRadius + 1.5 * dpr,
          0,
          Math.PI * 2,
        )
        ctx.stroke()
      }
    }

    for (const [id, node] of renderNodesRef.current) {
      drawNodeMarker(id, node)
    }

    const exploredCellSizeM =
      selectedPlayerVisibilityOverlay?.explored_cell_size_m
    if (
      selectedPlayerVisibilityOverlay &&
      Number.isFinite(exploredCellSizeM) &&
      (exploredCellSizeM ?? 0) > 0
    ) {
      const exploredCellSize = exploredCellSizeM ?? 0
      ctx.save()
      ctx.fillStyle = 'rgba(7, 10, 16, 0.78)'
      ctx.fillRect(0, 0, labelsCanvas.width, labelsCanvas.height)
      ctx.globalCompositeOperation = 'destination-out'
      for (const cell of selectedPlayerVisibilityOverlay.explored_cells) {
        if (!Number.isFinite(cell.x) || !Number.isFinite(cell.y)) continue
        const minX = cell.x * exploredCellSize
        const minY = cell.y * exploredCellSize
        const cornerA = worldToScreen(minX, minY)
        const cornerB = worldToScreen(
          minX + exploredCellSize,
          minY + exploredCellSize,
        )
        const rectX = Math.min(cornerA.x, cornerB.x)
        const rectY = Math.min(cornerA.y, cornerB.y)
        const widthPx = Math.abs(cornerB.x - cornerA.x)
        const heightPx = Math.abs(cornerB.y - cornerA.y)
        if (!Number.isFinite(widthPx) || !Number.isFinite(heightPx)) continue
        ctx.fillRect(rectX, rectY, widthPx, heightPx)
      }
      for (const source of selectedPlayerVisibilityOverlay.visibility_sources) {
        if (
          !Number.isFinite(source.x) ||
          !Number.isFinite(source.y) ||
          !Number.isFinite(source.range_m)
        ) {
          continue
        }
        const center = worldToScreen(source.x, source.y)
        const radiusPx = Math.max(0, source.range_m * cam.zoom)
        if (!Number.isFinite(radiusPx) || radiusPx <= 0) continue
        ctx.beginPath()
        ctx.arc(center.x, center.y, radiusPx, 0, Math.PI * 2)
        ctx.fill()
      }
      ctx.restore()

      const visibleSourceNodes = new Set<string>()
      for (const [id, node] of renderNodesRef.current) {
        for (const source of selectedPlayerVisibilityOverlay.visibility_sources) {
          if (
            !Number.isFinite(source.x) ||
            !Number.isFinite(source.y) ||
            !Number.isFinite(source.range_m)
          ) {
            continue
          }
          const dx = node.x - source.x
          const dy = node.y - source.y
          if (dx * dx + dy * dy <= source.range_m * source.range_m) {
            visibleSourceNodes.add(id)
            break
          }
        }
      }

      for (const id of visibleSourceNodes) {
        const node = renderNodesRef.current.get(id)
        if (!node) continue
        drawNodeMarker(id, node)
      }
    }

    if (
      selectedPlayerVisibilityOverlay &&
      Number.isFinite(selectedPlayerVisibilityOverlay.cell_size_m) &&
      selectedPlayerVisibilityOverlay.cell_size_m > 0
    ) {
      const cellSizeM = selectedPlayerVisibilityOverlay.cell_size_m

      ctx.save()

      // Draw selected player's BRP delivery range as a yellow circle.
      if (
        selectedId &&
        Number.isFinite(selectedPlayerVisibilityOverlay.delivery_range_m) &&
        selectedPlayerVisibilityOverlay.delivery_range_m > 0
      ) {
        const selectedNode = renderNodesRef.current.get(selectedId)
        if (selectedNode) {
          const center = worldToScreen(selectedNode.x, selectedNode.y)
          const deliveryRadiusPx =
            selectedPlayerVisibilityOverlay.delivery_range_m * cam.zoom
          if (Number.isFinite(deliveryRadiusPx) && deliveryRadiusPx > 0) {
            ctx.strokeStyle = 'rgba(255, 230, 90, 0.9)'
            ctx.lineWidth = Math.max(1, 2 * dpr)
            ctx.beginPath()
            ctx.arc(center.x, center.y, deliveryRadiusPx, 0, Math.PI * 2)
            ctx.stroke()
          }
        }
      }

      // Draw candidate grid cells as stroked rectangles.
      ctx.strokeStyle = 'rgba(80, 190, 255, 0.7)'
      ctx.lineWidth = Math.max(1, 1.5 * dpr)
      for (const cell of selectedPlayerVisibilityOverlay.queried_cells) {
        if (!Number.isFinite(cell.x) || !Number.isFinite(cell.y)) continue
        const minX = cell.x * cellSizeM
        const minY = cell.y * cellSizeM
        const cornerA = worldToScreen(minX, minY)
        const cornerB = worldToScreen(minX + cellSizeM, minY + cellSizeM)
        const rectX = Math.min(cornerA.x, cornerB.x)
        const rectY = Math.min(cornerA.y, cornerB.y)
        const widthPx = Math.abs(cornerB.x - cornerA.x)
        const heightPx = Math.abs(cornerB.y - cornerA.y)
        if (!Number.isFinite(widthPx) || !Number.isFinite(heightPx)) continue
        ctx.strokeRect(rectX, rectY, widthPx, heightPx)
      }

      // Draw scanner sources as circles + center point.
      ctx.strokeStyle = 'rgba(255, 166, 77, 0.8)'
      ctx.fillStyle = 'rgba(255, 166, 77, 0.95)'
      for (const source of selectedPlayerVisibilityOverlay.visibility_sources) {
        if (
          !Number.isFinite(source.x) ||
          !Number.isFinite(source.y) ||
          !Number.isFinite(source.range_m)
        ) {
          continue
        }
        const center = worldToScreen(source.x, source.y)
        const radiusPx = Math.max(0, source.range_m * cam.zoom)
        if (!Number.isFinite(radiusPx)) continue
        ctx.beginPath()
        ctx.arc(center.x, center.y, radiusPx, 0, Math.PI * 2)
        ctx.stroke()
        ctx.beginPath()
        ctx.arc(center.x, center.y, Math.max(2, 2.5 * dpr), 0, Math.PI * 2)
        ctx.fill()
      }

      ctx.restore()
    }

    if (!showNamePlates) {
      return
    }

    for (const [id, node] of renderNodesRef.current) {
      const screenPos = worldToScreen(node.x, node.y)

      // Skip if off-screen
      if (
        screenPos.x < -50 ||
        screenPos.x > labelsCanvas.width + 50 ||
        screenPos.y < -20 ||
        screenPos.y > labelsCanvas.height + 20
      ) {
        continue
      }

      // Only show labels when zoomed in enough or for selected/hovered
      if (cam.zoom < 0.3 && id !== selectedId && id !== hoveredId) {
        continue
      }

      const offset = node.depth === 0 ? 14 : 10
      ctx.fillText(
        node.label,
        screenPos.x + offset * dpr,
        screenPos.y - offset * dpr,
      )
    }
  }, [
    worldToScreen,
    selectedId,
    hoveredId,
    themeColors,
    selectedPlayerVisibilityOverlay,
    showNamePlates,
  ])

  // Main render loop
  const drawFrame = useCallback(() => {
    const visibleNodes = cullNodesToViewport(allNodes)
    renderNodesRef.current = visibleNodes
    const renderedNodes = new Map<string, ExpandedNode>()
    render(
      cameraRef.current,
      renderedNodes,
      visibleGraphEdges,
      selectedId,
      hoveredId,
    )
    drawLabels()
  }, [
    render,
    allNodes,
    cullNodesToViewport,
    visibleGraphEdges,
    selectedId,
    hoveredId,
    drawLabels,
  ])

  const requestRender = useCallback(() => {
    if (renderQueuedRef.current) return
    renderQueuedRef.current = true
    rafRef.current = requestAnimationFrame(() => {
      renderQueuedRef.current = false
      drawFrame()
    })
  }, [drawFrame])

  useEffect(() => {
    requestRenderRef.current = requestRender
  }, [requestRender])

  useEffect(() => {
    queueCameraStateSyncRef.current = queueCameraStateSync
  }, [queueCameraStateSync])

  useEffect(() => {
    let cancelled = false
    const markerIcons: Array<{
      key: 'ship' | 'planet' | 'star' | 'asteroid'
      path: string
    }> = [
      { key: 'ship', path: '/icons/ship.svg' },
      { key: 'planet', path: '/icons/planet.svg' },
      { key: 'star', path: '/icons/star.svg' },
      { key: 'asteroid', path: '/icons/asteroid.svg' },
    ]

    for (const icon of markerIcons) {
      const image = new Image()
      image.onload = () => {
        if (cancelled) return
        markerIconsRef.current[icon.key] = image
        requestRenderRef.current()
      }
      image.onerror = () => {
        if (cancelled) return
        markerIconsRef.current[icon.key] = undefined
      }
      image.src = icon.path
    }

    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    if (
      !cameraState ||
      !Number.isFinite(cameraState.x) ||
      !Number.isFinite(cameraState.y) ||
      !Number.isFinite(cameraState.zoom)
    ) {
      return
    }
    const cameraKey = `${cameraState.x}:${cameraState.y}:${cameraState.zoom}`
    if (lastAppliedCameraKeyRef.current === cameraKey) {
      return
    }
    lastAppliedCameraKeyRef.current = cameraKey
    cameraRef.current = {
      x: cameraState.x,
      y: cameraState.y,
      zoom: cameraState.zoom,
    }
    setZoomPercent(Math.round(cameraState.zoom * 100))
    syncCameraHudState()
    requestRenderRef.current()
  }, [cameraState, syncCameraHudState])

  // Center camera only on explicit center requests, not on live position updates.
  useEffect(() => {
    if (
      centerOnPosition &&
      Number.isFinite(centerOnPosition.x) &&
      Number.isFinite(centerOnPosition.y)
    ) {
      cameraRef.current.x = centerOnPosition.x
      cameraRef.current.y = centerOnPosition.y
      syncCameraHudState()
      queueCameraStateSyncRef.current()
      requestRenderRef.current()
    }
  }, [centerOnRequestSeq, centerOnPosition, syncCameraHudState])

  // Initialize renderer and size handling.
  useEffect(() => {
    init()
    resize()

    const queueResizeRender = () => {
      if (resizeDebounceTimerRef.current !== null) {
        window.clearTimeout(resizeDebounceTimerRef.current)
      }
      resizeDebounceTimerRef.current = window.setTimeout(() => {
        resizeDebounceTimerRef.current = null
        resize()
        requestRenderRef.current()
      }, 80)
    }

    const handleResize = () => {
      queueResizeRender()
    }
    window.addEventListener('resize', handleResize)
    requestRenderRef.current()

    return () => {
      window.removeEventListener('resize', handleResize)
      cancelAnimationFrame(rafRef.current)
      renderQueuedRef.current = false
      if (cameraSyncTimerRef.current !== null) {
        window.clearTimeout(cameraSyncTimerRef.current)
      }
      if (resizeDebounceTimerRef.current !== null) {
        window.clearTimeout(resizeDebounceTimerRef.current)
      }
    }
  }, [init, resize])

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const queueResizeRender = () => {
      if (resizeDebounceTimerRef.current !== null) {
        window.clearTimeout(resizeDebounceTimerRef.current)
      }
      resizeDebounceTimerRef.current = window.setTimeout(() => {
        resizeDebounceTimerRef.current = null
        resize()
        requestRenderRef.current()
      }, 80)
    }

    const observer = new ResizeObserver(() => {
      queueResizeRender()
    })

    observer.observe(container)

    return () => {
      observer.disconnect()
      if (resizeDebounceTimerRef.current !== null) {
        window.clearTimeout(resizeDebounceTimerRef.current)
      }
    }
  }, [resize])

  // Render on state/data changes.
  useEffect(() => {
    requestRender()
  }, [
    requestRender,
    allNodes,
    visibleGraphEdges,
    selectedId,
    hoveredId,
    showNamePlates,
    selectedPlayerVisibilityOverlay,
  ])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.repeat) return
      const target = event.target
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      ) {
        return
      }
      if (event.key.toLowerCase() !== 'v') return
      setShowNamePlates((prev) => !prev)
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [])

  const focusNode = React.useMemo(() => {
    if (selectedId) {
      return allNodes.get(selectedId) ?? null
    }
    if (hoveredId) {
      return allNodes.get(hoveredId) ?? null
    }
    return null
  }, [allNodes, hoveredId, selectedId])

  const locationSector = React.useMemo(
    () => `SECTOR ${formatSectorCode(cameraHudState.x, cameraHudState.y)}`,
    [cameraHudState.x, cameraHudState.y],
  )

  const locationGrid = React.useMemo(() => {
    const focusToken = formatNodeDisplayToken(focusNode)
    if (focusToken) {
      return focusToken
    }
    return `CELL ${formatGridCellCode(cameraHudState.x, cameraHudState.y)}`
  }, [cameraHudState.x, cameraHudState.y, focusNode])

  const locationCoordinates = React.useMemo(
    () =>
      `X ${formatHudCoordinate(cameraHudState.x)} Y ${formatHudCoordinate(cameraHudState.y)} Z ${zoomPercent}%`,
    [cameraHudState.x, cameraHudState.y, zoomPercent],
  )

  const locationStatus = React.useMemo(() => {
    if (isPanning) {
      return 'PANNING'
    }
    if (focusNode) {
      return selectedId ? 'LOCKED' : 'TRACKING'
    }
    return sourceMode === 'database'
      ? 'ARCHIVE'
      : sourceMode === 'liveClient'
        ? 'CLIENT FEED'
        : 'SERVER FEED'
  }, [focusNode, isPanning, selectedId, sourceMode])

  const selectedReticlePosition = React.useMemo(() => {
    if (!selectedId) {
      return null
    }
    const selectedNode = allNodes.get(selectedId)
    const container = containerRef.current
    if (!selectedNode || !container) {
      return null
    }
    const dpr =
      typeof window === 'undefined'
        ? 1
        : Math.max(window.devicePixelRatio || 1, 1)
    const cssZoom = cameraHudState.zoom / dpr
    const x =
      (selectedNode.x - cameraHudState.x) * cssZoom +
      container.clientWidth * 0.5
    const y =
      container.clientHeight * 0.5 -
      (selectedNode.y - cameraHudState.y) * cssZoom
    if (
      x < -64 ||
      x > container.clientWidth + 64 ||
      y < -64 ||
      y > container.clientHeight + 64
    ) {
      return null
    }
    return { x, y }
  }, [
    allNodes,
    cameraHudState.x,
    cameraHudState.y,
    cameraHudState.zoom,
    selectedId,
  ])

  // Mouse handlers
  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (e.button !== 0) return
      draggingRef.current = true
      movedRef.current = false
      pointerRef.current = { x: e.clientX, y: e.clientY }
      setIsPanning(true)
      requestRender()
    },
    [requestRender],
  )

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      // Update hover state
      const hit = pickNode(e.clientX, e.clientY)
      setHoveredId(hit)

      if (!draggingRef.current) return

      const dx = e.clientX - pointerRef.current.x
      const dy = e.clientY - pointerRef.current.y

      if (Math.abs(dx) + Math.abs(dy) > 3) {
        movedRef.current = true
      }

      cameraRef.current.x -= dx / cameraRef.current.zoom
      cameraRef.current.y += dy / cameraRef.current.zoom
      pointerRef.current = { x: e.clientX, y: e.clientY }
      syncCameraHudState()
      queueCameraStateSync()
      requestRender()
    },
    [pickNode, queueCameraStateSync, requestRender, syncCameraHudState],
  )

  const handleMouseUp = useCallback(
    (e: React.MouseEvent) => {
      if (e.button !== 0) return
      draggingRef.current = false
      setIsPanning(false)
      emitCameraState()
      requestRender()

      if (!movedRef.current) {
        const hit = pickNode(e.clientX, e.clientY)
        if (hit) {
          if (hit === selectedId) {
            // Double-select triggers expansion
            onExpand(hit)
          } else {
            onSelect(hit)
          }
        }
        // Click on empty space: keep current selection (do not deselect)
      }
    },
    [emitCameraState, pickNode, selectedId, onSelect, onExpand, requestRender],
  )

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      const hit = pickNode(e.clientX, e.clientY)
      if (hit) {
        onExpand(hit)
      }
    },
    [pickNode, onExpand],
  )

  const handleWheel = useCallback(
    (clientX: number, clientY: number, deltaY: number) => {
      const canvas = canvasRef.current
      if (!canvas) return

      const rect = canvas.getBoundingClientRect()
      const dpr = window.devicePixelRatio || 1
      const sx = (clientX - rect.left) * dpr
      const sy = (clientY - rect.top) * dpr

      const oldZoom = cameraRef.current.zoom
      const zoomFactor = Math.exp(-deltaY * 0.001)
      const newZoom = Math.min(
        MAX_ZOOM,
        Math.max(MIN_ZOOM, oldZoom * zoomFactor),
      )

      // Zoom towards cursor position
      const worldX = (sx - canvas.width * 0.5) / oldZoom + cameraRef.current.x
      const worldY = (canvas.height * 0.5 - sy) / oldZoom + cameraRef.current.y

      cameraRef.current.zoom = newZoom
      cameraRef.current.x = worldX - (sx - canvas.width * 0.5) / newZoom
      cameraRef.current.y = worldY - (canvas.height * 0.5 - sy) / newZoom
      setZoomPercent(Math.round(newZoom * 100))
      syncCameraHudState()
      queueCameraStateSync()
      requestRender()
    },
    [queueCameraStateSync, requestRender, syncCameraHudState],
  )

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const handleNativeWheel = (event: WheelEvent) => {
      event.preventDefault()
      handleWheel(event.clientX, event.clientY, event.deltaY)
    }

    canvas.addEventListener('wheel', handleNativeWheel, { passive: false })
    return () => {
      canvas.removeEventListener('wheel', handleNativeWheel)
    }
  }, [handleWheel])

  const handleContextMenu = useCallback(
    (clientX: number, clientY: number) => {
      if (!onContextMenuRequest) return
      const hit = pickNode(clientX, clientY)
      if (hit) {
        onSelect(hit)
      }
      const worldPoint = screenToWorld(clientX, clientY)
      onContextMenuRequest(hit, { x: clientX, y: clientY }, worldPoint)
    },
    [onContextMenuRequest, onSelect, pickNode, screenToWorld],
  )

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const handleNativeContextMenu = (event: MouseEvent) => {
      event.preventDefault()
      handleContextMenu(event.clientX, event.clientY)
    }

    canvas.addEventListener('contextmenu', handleNativeContextMenu)
    return () => {
      canvas.removeEventListener('contextmenu', handleNativeContextMenu)
    }
  }, [handleContextMenu])

  return (
    <div ref={containerRef} className="relative h-full w-full overflow-hidden">
      <canvas
        ref={canvasRef}
        className={cn(
          'absolute inset-0 w-full h-full',
          hoveredId
            ? 'cursor-default'
            : isPanning
              ? 'cursor-grabbing'
              : 'cursor-default',
        )}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={() => {
          draggingRef.current = false
          setIsPanning(false)
          setHoveredId(null)
          emitCameraState()
        }}
        onDoubleClick={handleDoubleClick}
      />
      {scanOverlayEnabled ? (
        <GridScanOverlay
          aria-hidden="true"
          className="z-[1]"
          gridSize={scanOverlayGridSize}
          scanSpeed={scanOverlaySpeed}
        />
      ) : null}
      <canvas
        ref={labelsCanvasRef}
        className={cn(
          'absolute inset-0 z-[2] h-full w-full pointer-events-none',
        )}
      />
      {selectedReticlePosition ? (
        <div
          className="pointer-events-none absolute z-[3]"
          style={{
            left: selectedReticlePosition.x,
            top: selectedReticlePosition.y,
            transform: 'translate(-50%, -50%)',
          }}
        >
          <Reticle size={64} variant="scanning" />
        </div>
      ) : null}
      <div className="grid-canvas-hud-chip absolute bottom-4 left-4 z-[4] px-3 py-2">
        <LocationDisplay
          sector={locationSector}
          grid={locationGrid}
          coordinates={locationCoordinates}
          status={locationStatus}
        />
      </div>
    </div>
  )
}

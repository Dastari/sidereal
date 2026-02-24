import * as React from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useGridRenderer } from './useGridRenderer'
import type { Camera, ExpandedNode, GraphEdge, WorldEntity } from './types'
import { useTheme } from '@/hooks/use-theme'

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
}

// graphNodes is used indirectly via expandedNodes

export function GridCanvas({
  entities,
  graphNodes: _graphNodes,
  graphEdges,
  selectedId,
  onSelect,
  onExpand,
  expandedNodes,
  filterMapInvisible,
}: GridCanvasProps) {
  void _graphNodes // Used indirectly via expandedNodes
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const labelsCanvasRef = useRef<HTMLCanvasElement>(null)
  const { resolvedTheme } = useTheme()
  const isDark = resolvedTheme === 'dark'

  const cameraRef = useRef<Camera>({ x: 0, y: 0, zoom: 0.5 })
  const draggingRef = useRef(false)
  const movedRef = useRef(false)
  const pointerRef = useRef({ x: 0, y: 0 })
  const [hoveredId, setHoveredId] = useState<string | null>(null)
  const [zoomPercent, setZoomPercent] = useState(50)
  const rafRef = useRef<number>(0)

  const { init, resize, render } = useGridRenderer(canvasRef, isDark)

  // Build combined node map for rendering
  // ONLY render root entities (no parentEntityId) on the map at their x/y positions
  // Only child entities are shown when explicitly expanded
  const allNodes = React.useMemo(() => {
    const result = new Map<string, ExpandedNode>()

    // Add ONLY root world entities (no parentEntityId) at depth 0
    for (const entity of entities) {
      // Keep non-renderable entities in sidebar/detail only.
      if (filterMapInvisible && entity.mapVisible === false) {
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
          z: entity.z,
          componentCount: entity.componentCount,
        },
      })
    }

    // Add expanded child entity nodes
    for (const [id, node] of expandedNodes) {
      if (!result.has(id)) {
        result.set(id, node)
      }
    }

    return result
  }, [entities, expandedNodes, filterMapInvisible])

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

      for (const [id, node] of allNodes) {
        const screenPos = worldToScreen(node.x, node.y)
        const dist = Math.hypot(screenPos.x - sx, screenPos.y - sy)
        if (dist < closestDist) {
          closestDist = dist
          closest = id
        }
      }

      return closest
    },
    [allNodes, worldToScreen],
  )

  // Draw labels on 2D canvas
  const drawLabels = useCallback(() => {
    const canvas = canvasRef.current
    const labelsCanvas = labelsCanvasRef.current
    if (!canvas || !labelsCanvas) return

    const ctx = labelsCanvas.getContext('2d')
    if (!ctx) return

    const dpr = window.devicePixelRatio || 1
    labelsCanvas.width = canvas.width
    labelsCanvas.height = canvas.height

    ctx.clearRect(0, 0, labelsCanvas.width, labelsCanvas.height)
    ctx.font = `${11 * dpr}px Inter, system-ui, sans-serif`
    ctx.fillStyle = isDark
      ? 'rgba(220, 230, 250, 0.9)'
      : 'rgba(30, 40, 60, 0.9)'
    ctx.textAlign = 'left'
    ctx.textBaseline = 'middle'

    const cam = cameraRef.current

    for (const [id, node] of allNodes) {
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
  }, [allNodes, worldToScreen, selectedId, hoveredId, isDark])

  // Main render loop
  const frame = useCallback(() => {
    render(cameraRef.current, allNodes, graphEdges, selectedId, hoveredId)
    drawLabels()
    rafRef.current = requestAnimationFrame(frame)
  }, [render, allNodes, graphEdges, selectedId, hoveredId, drawLabels])

  // Initialize and start render loop
  useEffect(() => {
    init()
    resize()

    const handleResize = () => resize()
    window.addEventListener('resize', handleResize)

    rafRef.current = requestAnimationFrame(frame)

    return () => {
      window.removeEventListener('resize', handleResize)
      cancelAnimationFrame(rafRef.current)
    }
  }, [init, resize, frame])

  // Mouse handlers
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    draggingRef.current = true
    movedRef.current = false
    pointerRef.current = { x: e.clientX, y: e.clientY }
  }, [])

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
    },
    [pickNode],
  )

  const handleMouseUp = useCallback(
    (e: React.MouseEvent) => {
      draggingRef.current = false

      if (!movedRef.current) {
        const hit = pickNode(e.clientX, e.clientY)
        if (hit) {
          if (hit === selectedId) {
            // Double-select triggers expansion
            onExpand(hit)
          } else {
            onSelect(hit)
          }
        } else {
          onSelect(null)
        }
      }
    },
    [pickNode, selectedId, onSelect, onExpand],
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

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault()

    const canvas = canvasRef.current
    if (!canvas) return

    const rect = canvas.getBoundingClientRect()
    const dpr = window.devicePixelRatio || 1
    const sx = (e.clientX - rect.left) * dpr
    const sy = (e.clientY - rect.top) * dpr

    const oldZoom = cameraRef.current.zoom
    const zoomFactor = Math.exp(-e.deltaY * 0.001)
    const newZoom = Math.min(10, Math.max(0.05, oldZoom * zoomFactor))

    // Zoom towards cursor position
    const worldX = (sx - canvas.width * 0.5) / oldZoom + cameraRef.current.x
    const worldY = (canvas.height * 0.5 - sy) / oldZoom + cameraRef.current.y

    cameraRef.current.zoom = newZoom
    cameraRef.current.x = worldX - (sx - canvas.width * 0.5) / newZoom
    cameraRef.current.y = worldY - (canvas.height * 0.5 - sy) / newZoom
    setZoomPercent(Math.round(newZoom * 100))
  }, [])

  return (
    <div className="relative w-full h-full overflow-hidden">
      <canvas
        ref={canvasRef}
        className="absolute inset-0 w-full h-full cursor-grab active:cursor-grabbing"
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={() => {
          draggingRef.current = false
          setHoveredId(null)
        }}
        onDoubleClick={handleDoubleClick}
        onWheel={handleWheel}
      />
      <canvas
        ref={labelsCanvasRef}
        className="absolute inset-0 w-full h-full pointer-events-none"
      />
      {/* Zoom indicator */}
      <div className="absolute bottom-4 left-4 px-3 py-1.5 rounded-md bg-card/80 backdrop-blur border border-border text-xs text-muted-foreground">
        Zoom: {zoomPercent}%
      </div>
      {/* Help hint */}
      <div className="absolute bottom-4 right-4 px-3 py-1.5 rounded-md bg-card/60 backdrop-blur border border-border-subtle text-xs text-muted-foreground/70">
        Scroll to zoom • Drag to pan • Double-click to expand
      </div>
    </div>
  )
}

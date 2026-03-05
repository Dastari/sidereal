export type EntityKind =
  | 'ship'
  | 'station'
  | 'asteroid'
  | 'planet'
  | 'component'
  | 'default'

export interface WorldEntity {
  id: string
  name: string
  kind: string
  parentEntityId?: string
  /** Labels from graph (e.g. ["Entity", "Ship"]); second value used for tree grouping. */
  entity_labels?: string[]
  mapVisible?: boolean
  /** True when source data provided an explicit world position. */
  hasPosition?: boolean
  shardId: number
  x: number
  y: number
  vx: number
  vy: number
  sampledAtMs: number
  componentCount: number
  /** When present, from EntityGuid component; shown in tree instead of component count when available. */
  entityGuid?: string
}

export interface GraphNode {
  id: string
  label: string
  kind: string
  properties: Record<string, unknown>
}

export interface GraphEdge {
  id: string
  from: string
  to: string
  label: string
  properties: Record<string, unknown>
}

export interface ExpandedNode {
  id: string
  parentId: string | null
  x: number
  y: number
  label: string
  kind: string
  isExpanded: boolean
  depth: number
  properties: Record<string, unknown>
}

export interface Camera {
  x: number
  y: number
  zoom: number
}

export interface GridState {
  entities: Array<WorldEntity>
  nodes: Map<string, ExpandedNode>
  edges: Array<GraphEdge>
  selectedId: string | null
  hoveredId: string | null
  camera: Camera
}

export interface VisibilityGridCellOverlay {
  x: number
  y: number
}

export interface VisibilityScannerSourceOverlay {
  x: number
  y: number
  z?: number
  range_m: number
}

export interface PlayerVisibilityOverlay {
  cell_size_m: number
  delivery_range_m: number
  queried_cells: Array<VisibilityGridCellOverlay>
  scanner_sources: Array<VisibilityScannerSourceOverlay>
}

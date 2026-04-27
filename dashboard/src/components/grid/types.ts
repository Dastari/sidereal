export type EntityKind =
  | 'ship'
  | 'station'
  | 'asteroid'
  | 'planet'
  | 'component'
  | 'default'

// Dashboard world coordinates intentionally use JavaScript number values.
// JSON numbers preserve the authoritative f64 payload shape from the server
// while the WebGL canvas projects them to f32 only at render time.
export type WorldCoordinate = number

export interface WorldEntity {
  id: string
  name: string
  kind: string
  parentEntityId?: string
  /** Labels from graph (e.g. ["Entity", "Ship"]); second value used for tree grouping. */
  entity_labels?: Array<string>
  mapVisible?: boolean
  /** True when source data provided an explicit world position. */
  hasPosition?: boolean
  shardId: number
  x: WorldCoordinate
  y: WorldCoordinate
  rotationRad?: number
  vx: WorldCoordinate
  vy: WorldCoordinate
  sampledAtMs: number
  componentCount: number
  /** When present, from EntityGuid component; shown in tree instead of component count when available. */
  entityGuid?: string
  /** Player control target from ControlledEntityGuid when present. */
  controlledEntityGuid?: string
  /** Hide this entity's map marker while preserving it for tree/detail state. */
  hideMapIcon?: boolean
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
  x: WorldCoordinate
  y: WorldCoordinate
  rotationRad?: number
  label: string
  kind: string
  isExpanded: boolean
  depth: number
  properties: Record<string, unknown>
}

export interface Camera {
  x: WorldCoordinate
  y: WorldCoordinate
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
  x: WorldCoordinate
  y: WorldCoordinate
}

export interface VisibilityScannerSourceOverlay {
  x: WorldCoordinate
  y: WorldCoordinate
  z?: WorldCoordinate
  range_m: number
}

export interface PlayerVisibilityOverlay {
  cell_size_m: number
  delivery_range_m: number
  queried_cells: Array<VisibilityGridCellOverlay>
  visibility_sources: Array<VisibilityScannerSourceOverlay>
  explored_cell_size_m: number | null
  explored_cells: Array<VisibilityGridCellOverlay>
}

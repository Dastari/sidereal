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
  mapVisible?: boolean
  shardId: number
  x: number
  y: number
  z: number
  componentCount: number
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

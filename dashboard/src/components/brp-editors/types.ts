import type { GraphNode } from '@/components/grid/types'

export interface ComponentEditorProps {
  /** Stable component node id (e.g. entityId::typePath for BRP). */
  componentNodeId: string
  /** Entity id this component belongs to (for update API). */
  entityId: string
  /** Full component node (label, kind, properties). */
  node: GraphNode
  /** Current value - editor-specific shape (e.g. number for Density). */
  value: unknown
  /** Called when user commits a new value. Payload is editor-specific. */
  onChange: (value: unknown) => void
  /** Whether updates are allowed (e.g. only in BRP mode). */
  readOnly?: boolean
}

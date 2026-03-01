import * as React from 'react'
import {
  getComponentTypeKey,
  getComponentValue,
  getEditorForNode,
} from './registry'
import type { GraphNode } from '@/components/grid/types'

export interface ComponentEditorRendererProps {
  componentNodeId: string
  entityId: string
  node: GraphNode
  onUpdate: (typePath: string, value: unknown) => void
  readOnly?: boolean
}

/**
 * Renders the appropriate editable component UI for a BRP/graph component node.
 * Uses code-split lazy loading per editor type.
 */
export function ComponentEditorRenderer({
  componentNodeId,
  entityId,
  node,
  onUpdate,
  readOnly = false,
}: ComponentEditorRendererProps) {
  const Editor = getEditorForNode(node)
  const value = getComponentValue(node)
  const typePath = getComponentTypeKey(node)

  if (Editor === null || typePath === null) {
    return null
  }

  const handleChange = React.useCallback(
    (newValue: unknown) => {
      onUpdate(typePath, newValue)
    },
    [onUpdate, typePath],
  )

  return (
    <React.Suspense fallback={<div className="text-xs text-muted-foreground">Loading editor…</div>}>
      <Editor
        componentNodeId={componentNodeId}
        entityId={entityId}
        node={node}
        value={value}
        onChange={handleChange}
        readOnly={readOnly}
      />
    </React.Suspense>
  )
}

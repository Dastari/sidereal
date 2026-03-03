import * as React from 'react'
import {
  COMPONENT_TYPE_CHARACTER_MOVEMENT_CONTROLLER,
  COMPONENT_TYPE_DENSITY,
  COMPONENT_TYPE_ENGINE,
  COMPONENT_TYPE_SPACE_BACKGROUND_SHADER_SETTINGS,
  COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS,
  COMPONENT_TYPE_THRUSTER_PLUME_SHADER_SETTINGS,
  getComponentTypeKey,
  isEditableComponent,
} from './constants'
import type { ComponentEditorProps } from './types'
import type { GraphNode } from '@/components/grid/types'

const DensityEditor = React.lazy(() =>
  import('./DensityEditor').then((m) => ({ default: m.DensityEditor })),
)
const CharacterMovementControllerEditor = React.lazy(() =>
  import('./CharacterMovementControllerEditor').then((m) => ({
    default: m.CharacterMovementControllerEditor,
  })),
)
const EngineEditor = React.lazy(() =>
  import('./EngineEditor').then((m) => ({
    default: m.EngineEditor,
  })),
)
const StarfieldShaderSettingsEditor = React.lazy(() =>
  import('./StarfieldShaderSettingsEditor').then((m) => ({
    default: m.StarfieldShaderSettingsEditor,
  })),
)
const SpaceBackgroundShaderSettingsEditor = React.lazy(() =>
  import('./SpaceBackgroundShaderSettingsEditor').then((m) => ({
    default: m.SpaceBackgroundShaderSettingsEditor,
  })),
)
const ThrusterPlumeShaderSettingsEditor = React.lazy(() =>
  import('./ThrusterPlumeShaderSettingsEditor').then((m) => ({
    default: m.ThrusterPlumeShaderSettingsEditor,
  })),
)

const EDITOR_MAP: Record<
  string,
  React.LazyExoticComponent<React.ComponentType<ComponentEditorProps>>
> = {
  [COMPONENT_TYPE_DENSITY]: DensityEditor,
  [COMPONENT_TYPE_CHARACTER_MOVEMENT_CONTROLLER]:
    CharacterMovementControllerEditor,
  [COMPONENT_TYPE_ENGINE]: EngineEditor,
  [COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS]: StarfieldShaderSettingsEditor,
  [COMPONENT_TYPE_SPACE_BACKGROUND_SHADER_SETTINGS]:
    SpaceBackgroundShaderSettingsEditor,
  [COMPONENT_TYPE_THRUSTER_PLUME_SHADER_SETTINGS]:
    ThrusterPlumeShaderSettingsEditor,
}

export { isEditableComponent, getComponentTypeKey }

export function getEditableComponentTypeKey(
  node: GraphNode,
): string | null {
  return getComponentTypeKey(node)
}

export function getEditorForNode(
  node: GraphNode,
): React.LazyExoticComponent<React.ComponentType<ComponentEditorProps>> | null {
  const key = getComponentTypeKey(node)
  if (key === null) return null
  return EDITOR_MAP[key] ?? null
}

/**
 * Extracts the current value for a component node for use in editors.
 * BRP: node.properties.value; Graph: node.properties["0"] or first numeric field for tuple structs.
 */
export function getComponentValue(node: GraphNode): unknown {
  const props = node.properties
  if ('value' in props && props.value !== undefined) {
    return props.value
  }
  if ('0' in props && props[0] !== undefined) {
    return props[0]
  }
  const entries = Object.entries(props)
  if (entries.length === 1 && typeof entries[0][1] === 'number') {
    return entries[0][1]
  }
  return props
}

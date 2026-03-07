import * as React from 'react'
import {
  COMPONENT_TYPE_CHARACTER_MOVEMENT_CONTROLLER,
  COMPONENT_TYPE_DENSITY,
  COMPONENT_TYPE_ENGINE,
  COMPONENT_TYPE_HARDPOINT,
  COMPONENT_TYPE_FLIGHT_TUNING,
  COMPONENT_TYPE_ENVIRONMENT_LIGHTING_STATE,
  COMPONENT_TYPE_MAX_VELOCITY_MPS,
  COMPONENT_TYPE_SIZE_M,
  COMPONENT_TYPE_PLANET_BODY_SHADER_SETTINGS,
  COMPONENT_TYPE_SPACE_BACKGROUND_SHADER_SETTINGS,
  COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS,
  COMPONENT_TYPE_TACTICAL_MAP_UI_SETTINGS,
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
const HardpointEditor = React.lazy(() =>
  import('./HardpointEditor').then((m) => ({
    default: m.HardpointEditor,
  })),
)
const FlightTuningEditor = React.lazy(() =>
  import('./FlightTuningEditor').then((m) => ({
    default: m.FlightTuningEditor,
  })),
)
const MaxVelocityMpsEditor = React.lazy(() =>
  import('./MaxVelocityMpsEditor').then((m) => ({
    default: m.MaxVelocityMpsEditor,
  })),
)
const SizeMEditor = React.lazy(() =>
  import('./SizeMEditor').then((m) => ({
    default: m.SizeMEditor,
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
const PlanetBodyShaderSettingsEditor = React.lazy(() =>
  import('./PlanetBodyShaderSettingsEditor').then((m) => ({
    default: m.PlanetBodyShaderSettingsEditor,
  })),
)
const EnvironmentLightingStateEditor = React.lazy(() =>
  import('./EnvironmentLightingStateEditor').then((m) => ({
    default: m.EnvironmentLightingStateEditor,
  })),
)
const ThrusterPlumeShaderSettingsEditor = React.lazy(() =>
  import('./ThrusterPlumeShaderSettingsEditor').then((m) => ({
    default: m.ThrusterPlumeShaderSettingsEditor,
  })),
)
const TacticalMapUiSettingsEditor = React.lazy(() =>
  import('./TacticalMapUiSettingsEditor').then((m) => ({
    default: m.TacticalMapUiSettingsEditor,
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
  [COMPONENT_TYPE_HARDPOINT]: HardpointEditor,
  [COMPONENT_TYPE_FLIGHT_TUNING]: FlightTuningEditor,
  [COMPONENT_TYPE_MAX_VELOCITY_MPS]: MaxVelocityMpsEditor,
  [COMPONENT_TYPE_SIZE_M]: SizeMEditor,
  [COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS]: StarfieldShaderSettingsEditor,
  [COMPONENT_TYPE_SPACE_BACKGROUND_SHADER_SETTINGS]:
    SpaceBackgroundShaderSettingsEditor,
  [COMPONENT_TYPE_PLANET_BODY_SHADER_SETTINGS]:
    PlanetBodyShaderSettingsEditor,
  [COMPONENT_TYPE_ENVIRONMENT_LIGHTING_STATE]:
    EnvironmentLightingStateEditor,
  [COMPONENT_TYPE_THRUSTER_PLUME_SHADER_SETTINGS]:
    ThrusterPlumeShaderSettingsEditor,
  [COMPONENT_TYPE_TACTICAL_MAP_UI_SETTINGS]: TacticalMapUiSettingsEditor,
}

export { isEditableComponent, getComponentTypeKey }

export function getEditableComponentTypeKey(node: GraphNode): string | null {
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

import {
  COMPONENT_TYPE_CHARACTER_MOVEMENT_CONTROLLER,
  COMPONENT_TYPE_DENSITY,
  COMPONENT_TYPE_ENGINE,
  COMPONENT_TYPE_ENVIRONMENT_LIGHTING_STATE,
  COMPONENT_TYPE_FLIGHT_TUNING,
  COMPONENT_TYPE_HARDPOINT,
  COMPONENT_TYPE_MAX_VELOCITY_MPS,
  COMPONENT_TYPE_PLANET_BODY_SHADER_SETTINGS,
  COMPONENT_TYPE_SIZE_M,
  COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS,
  COMPONENT_TYPE_TACTICAL_MAP_UI_SETTINGS,
  COMPONENT_TYPE_THRUSTER_PLUME_SHADER_SETTINGS,
  getComponentTypeKey,
  isEditableComponent,
} from './constants'
import { CharacterMovementControllerEditor } from './CharacterMovementControllerEditor'
import { DensityEditor } from './DensityEditor'
import { EngineEditor } from './EngineEditor'
import { EnvironmentLightingStateEditor } from './EnvironmentLightingStateEditor'
import { FlightTuningEditor } from './FlightTuningEditor'
import { HardpointEditor } from './HardpointEditor'
import { MaxVelocityMpsEditor } from './MaxVelocityMpsEditor'
import { PlanetBodyShaderSettingsEditor } from './PlanetBodyShaderSettingsEditor'
import { SizeMEditor } from './SizeMEditor'
import { StarfieldShaderSettingsEditor } from './StarfieldShaderSettingsEditor'
import { TacticalMapUiSettingsEditor } from './TacticalMapUiSettingsEditor'
import { ThrusterPlumeShaderSettingsEditor } from './ThrusterPlumeShaderSettingsEditor'
import type * as React from 'react'
import type { ComponentEditorProps } from './types'
import type { GraphNode } from '@/components/grid/types'

const EDITOR_MAP: Record<string, React.ComponentType<ComponentEditorProps>> = {
  [COMPONENT_TYPE_DENSITY]: DensityEditor,
  [COMPONENT_TYPE_CHARACTER_MOVEMENT_CONTROLLER]:
    CharacterMovementControllerEditor,
  [COMPONENT_TYPE_ENGINE]: EngineEditor,
  [COMPONENT_TYPE_HARDPOINT]: HardpointEditor,
  [COMPONENT_TYPE_FLIGHT_TUNING]: FlightTuningEditor,
  [COMPONENT_TYPE_MAX_VELOCITY_MPS]: MaxVelocityMpsEditor,
  [COMPONENT_TYPE_SIZE_M]: SizeMEditor,
  [COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS]: StarfieldShaderSettingsEditor,
  [COMPONENT_TYPE_PLANET_BODY_SHADER_SETTINGS]: PlanetBodyShaderSettingsEditor,
  [COMPONENT_TYPE_ENVIRONMENT_LIGHTING_STATE]: EnvironmentLightingStateEditor,
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
): React.ComponentType<ComponentEditorProps> | null {
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
  if ('0' in props && props['0'] !== undefined) {
    return props['0']
  }
  const entries = Object.entries(props)
  if (entries.length === 1 && typeof entries[0][1] === 'number') {
    return entries[0][1]
  }
  return props
}

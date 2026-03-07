/**
 * Canonical type paths for sidereal_game components that have BRP editors.
 * Used to match graph/BRP component nodes to the correct editor.
 */
export const COMPONENT_TYPE_DENSITY =
  'sidereal_game::components::density::Density'
export const COMPONENT_TYPE_CHARACTER_MOVEMENT_CONTROLLER =
  'sidereal_game::components::character_movement_controller::CharacterMovementController'
export const COMPONENT_TYPE_ENGINE = 'sidereal_game::components::engine::Engine'
export const COMPONENT_TYPE_HARDPOINT =
  'sidereal_game::components::hardpoint::Hardpoint'
export const COMPONENT_TYPE_FLIGHT_TUNING =
  'sidereal_game::components::flight_tuning::FlightTuning'
export const COMPONENT_TYPE_MAX_VELOCITY_MPS =
  'sidereal_game::components::max_velocity_mps::MaxVelocityMps'
export const COMPONENT_TYPE_SIZE_M =
  'sidereal_game::components::size_m::SizeM'
export const COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS =
  'sidereal_game::components::starfield_shader_settings::StarfieldShaderSettings'
export const COMPONENT_TYPE_SPACE_BACKGROUND_SHADER_SETTINGS =
  'sidereal_game::components::space_background_shader_settings::SpaceBackgroundShaderSettings'
export const COMPONENT_TYPE_PLANET_BODY_SHADER_SETTINGS =
  'sidereal_game::components::planet_body_shader_settings::PlanetBodyShaderSettings'
export const COMPONENT_TYPE_ENVIRONMENT_LIGHTING_STATE =
  'sidereal_game::components::environment_lighting_state::EnvironmentLightingState'
export const COMPONENT_TYPE_THRUSTER_PLUME_SHADER_SETTINGS =
  'sidereal_game::components::thruster_plume_shader_settings::ThrusterPlumeShaderSettings'
export const COMPONENT_TYPE_TACTICAL_MAP_UI_SETTINGS =
  'sidereal_game::components::tactical_map_ui_settings::TacticalMapUiSettings'

/** Type paths that have an editable UI in the BRP detail panel. */
export const EDITABLE_COMPONENT_TYPE_PATHS: ReadonlySet<string> = new Set([
  COMPONENT_TYPE_DENSITY,
  COMPONENT_TYPE_CHARACTER_MOVEMENT_CONTROLLER,
  COMPONENT_TYPE_ENGINE,
  COMPONENT_TYPE_HARDPOINT,
  COMPONENT_TYPE_FLIGHT_TUNING,
  COMPONENT_TYPE_MAX_VELOCITY_MPS,
  COMPONENT_TYPE_SIZE_M,
  COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS,
  COMPONENT_TYPE_SPACE_BACKGROUND_SHADER_SETTINGS,
  COMPONENT_TYPE_PLANET_BODY_SHADER_SETTINGS,
  COMPONENT_TYPE_ENVIRONMENT_LIGHTING_STATE,
  COMPONENT_TYPE_THRUSTER_PLUME_SHADER_SETTINGS,
  COMPONENT_TYPE_TACTICAL_MAP_UI_SETTINGS,
])

/**
 * Normalized component identifier: either the full type path or the short label (e.g. "Density").
 * BRP nodes use typePath in properties; graph nodes may use label or component_kind.
 */
export function getComponentTypeKey(node: {
  label: string
  kind: string
  properties: Record<string, unknown>
}): string | null {
  const typePath = node.properties.typePath
  if (typeof typePath === 'string' && typePath.length > 0) {
    return typePath
  }
  // Graph / DB: kind can be "Component" or "component", label "Density" (titleFromSnakeCase of component_kind)
  if (node.kind.toLowerCase() === 'component' && node.label === 'Density') {
    return COMPONENT_TYPE_DENSITY
  }
  if (
    node.kind.toLowerCase() === 'component' &&
    node.label === 'CharacterMovementController'
  ) {
    return COMPONENT_TYPE_CHARACTER_MOVEMENT_CONTROLLER
  }
  if (node.kind.toLowerCase() === 'component' && node.label === 'Engine') {
    return COMPONENT_TYPE_ENGINE
  }
  if (node.kind.toLowerCase() === 'component' && node.label === 'Hardpoint') {
    return COMPONENT_TYPE_HARDPOINT
  }
  if (
    node.kind.toLowerCase() === 'component' &&
    node.label === 'FlightTuning'
  ) {
    return COMPONENT_TYPE_FLIGHT_TUNING
  }
  if (
    node.kind.toLowerCase() === 'component' &&
    node.label === 'MaxVelocityMps'
  ) {
    return COMPONENT_TYPE_MAX_VELOCITY_MPS
  }
  if (
    node.kind.toLowerCase() === 'component' &&
    node.label === 'SizeM'
  ) {
    return COMPONENT_TYPE_SIZE_M
  }
  if (
    node.kind.toLowerCase() === 'component' &&
    node.label === 'StarfieldShaderSettings'
  ) {
    return COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS
  }
  if (
    node.kind.toLowerCase() === 'component' &&
    node.label === 'SpaceBackgroundShaderSettings'
  ) {
    return COMPONENT_TYPE_SPACE_BACKGROUND_SHADER_SETTINGS
  }
  if (
    node.kind.toLowerCase() === 'component' &&
    node.label === 'PlanetBodyShaderSettings'
  ) {
    return COMPONENT_TYPE_PLANET_BODY_SHADER_SETTINGS
  }
  if (
    node.kind.toLowerCase() === 'component' &&
    node.label === 'EnvironmentLightingState'
  ) {
    return COMPONENT_TYPE_ENVIRONMENT_LIGHTING_STATE
  }
  if (
    node.kind.toLowerCase() === 'component' &&
    node.label === 'ThrusterPlumeShaderSettings'
  ) {
    return COMPONENT_TYPE_THRUSTER_PLUME_SHADER_SETTINGS
  }
  if (
    node.kind.toLowerCase() === 'component' &&
    node.label === 'TacticalMapUiSettings'
  ) {
    return COMPONENT_TYPE_TACTICAL_MAP_UI_SETTINGS
  }
  return null
}

export function isEditableComponent(node: {
  label: string
  kind: string
  properties: Record<string, unknown>
}): boolean {
  const key = getComponentTypeKey(node)
  return key !== null && EDITABLE_COMPONENT_TYPE_PATHS.has(key)
}

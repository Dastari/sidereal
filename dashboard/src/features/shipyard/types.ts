export type Vec3Tuple = [number, number, number]

export type ShipyardVisualDefinition = {
  visual_asset_id: string
  map_icon_asset_id: string
}

export type ShipyardDimensionsDefinition = {
  length_m: number
  width_m?: number | null
  height_m: number
  collision_mode: string
  collision_from_texture: boolean
}

export type ShipyardRootDefinition = {
  base_mass_kg: number
  total_mass_kg?: number | null
  cargo_mass_kg?: number | null
  module_mass_kg?: number | null
  angular_inertia?: number | null
  max_velocity_mps: number
  health_pool: unknown
  destructible: unknown
  flight_computer: unknown
  flight_tuning: unknown
  visibility_range_buff_m: unknown
  scanner_component?: unknown
  avian_linear_damping?: number | null
  avian_angular_damping?: number | null
}

export type ShipyardHardpointDefinition = {
  hardpoint_id: string
  display_name: string
  slot_kind: string
  offset_m: Vec3Tuple
  local_rotation_rad: number
  mirror_group?: string | null
  compatible_tags: Array<string>
}

export type ShipyardMountedModuleDefinition = {
  hardpoint_id: string
  module_id: string
  display_name?: string | null
  component_overrides: Record<string, unknown>
}

export type ShipyardShipDefinition = {
  ship_id: string
  bundle_id: string
  script_path: string
  display_name: string
  entity_labels: Array<string>
  tags: Array<string>
  visual: ShipyardVisualDefinition
  dimensions: ShipyardDimensionsDefinition
  root: ShipyardRootDefinition
  hardpoints: Array<ShipyardHardpointDefinition>
  mounted_modules: Array<ShipyardMountedModuleDefinition>
}

export type ShipyardShipEntry = {
  shipId: string
  bundleId: string
  scriptPath: string
  displayName: string
  visualAssetId: string
  spawnEnabled: boolean
  tags: Array<string>
  hasDraft: boolean
  definition: ShipyardShipDefinition
}

export type ShipyardModuleComponentDefinition = {
  kind: string
  properties: unknown
}

export type ShipyardModuleDefinition = {
  module_id: string
  script_path: string
  display_name: string
  category: string
  entity_labels: Array<string>
  compatible_slot_kinds: Array<string>
  tags: Array<string>
  components: Array<ShipyardModuleComponentDefinition>
}

export type ShipyardModuleEntry = {
  moduleId: string
  scriptPath: string
  displayName: string
  category: string
  tags: Array<string>
  hasDraft: boolean
  definition: ShipyardModuleDefinition
}

export type ShipyardAssetEntry = {
  assetId: string
  sourcePath: string
  contentType: string
}

export type ShipyardCatalog = {
  ships: Array<ShipyardShipEntry>
  modules: Array<ShipyardModuleEntry>
  imageAssets: Array<ShipyardAssetEntry>
  shipRegistryHasDraft: boolean
  moduleRegistryHasDraft: boolean
}

export type ShipyardShipDraftRequest = {
  definition: ShipyardShipDefinition
  spawnEnabled: boolean
}

export type ShipyardModuleDraftRequest = {
  definition: ShipyardModuleDefinition
}

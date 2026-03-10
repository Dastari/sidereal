export type ComponentEditorValueKind =
  | 'Bool'
  | 'SignedInteger'
  | 'UnsignedInteger'
  | 'Float'
  | 'String'
  | 'Vec2'
  | 'Vec3'
  | 'Vec4'
  | 'ColorRgb'
  | 'ColorRgba'
  | 'Enum'
  | 'Sequence'
  | 'Struct'
  | 'Tuple'
  | 'Unknown'

export type ComponentEditorFieldSchema = {
  field_path: string
  field_name: string
  display_name: string
  value_kind: ComponentEditorValueKind
  min: number | null
  max: number | null
  step: number | null
  unit: string | null
  options: Array<string>
}

export type ComponentEditorSchema = {
  root_value_kind: ComponentEditorValueKind
  fields: Array<ComponentEditorFieldSchema>
}

export type GeneratedComponentRegistryEntry = {
  component_kind: string
  type_path: string
  replication_visibility: Array<string>
  editor_schema: ComponentEditorSchema
}

export type ShaderEditorOption = {
  value: string
  label: string
}

export type ShaderEditorFieldSchema = {
  field_path: string
  display_name: string
  description: string | null
  value_kind: ComponentEditorValueKind
  min: number | null
  max: number | null
  step: number | null
  options: Array<ShaderEditorOption>
  default_value_json: string | null
  group: string | null
}

export type ShaderEditorPreset = {
  preset_id: string
  display_name: string
  description: string | null
  values_json: string
}

export type ShaderEditorRegistryEntry = {
  asset_id: string
  source_path: string
  shader_family: string | null
  dependencies: Array<string>
  bootstrap_required: boolean
  uniform_schema: Array<ShaderEditorFieldSchema>
  presets: Array<ShaderEditorPreset>
}

export type GeneratedComponentRegistryResource = {
  entries: Array<GeneratedComponentRegistryEntry>
  shader_entries: Array<ShaderEditorRegistryEntry>
}

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

export type GeneratedComponentRegistryResource = {
  entries: Array<GeneratedComponentRegistryEntry>
}

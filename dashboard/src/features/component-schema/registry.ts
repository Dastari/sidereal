import type { GraphNode } from '@/components/grid/types'
import type {
  ComponentEditorFieldSchema,
  GeneratedComponentRegistryEntry,
  GeneratedComponentRegistryResource,
  ShaderEditorFieldSchema,
  ShaderEditorRegistryEntry,
} from './types'

export const GENERATED_COMPONENT_REGISTRY_TYPE_PATH =
  'sidereal_game::generated::components::GeneratedComponentRegistry'

const COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS =
  'sidereal_game::components::starfield_shader_settings::StarfieldShaderSettings'
const COMPONENT_TYPE_SPACE_BACKGROUND_SHADER_SETTINGS =
  'sidereal_game::components::space_background_shader_settings::SpaceBackgroundShaderSettings'
const COMPONENT_TYPE_PLANET_BODY_SHADER_SETTINGS =
  'sidereal_game::components::planet_body_shader_settings::PlanetBodyShaderSettings'

const COMPONENT_SHADER_ASSET_IDS: Record<string, Array<string>> = {
  [COMPONENT_TYPE_STARFIELD_SHADER_SETTINGS]: ['starfield_wgsl'],
  [COMPONENT_TYPE_SPACE_BACKGROUND_SHADER_SETTINGS]: [
    'space_background_base_wgsl',
    'space_background_nebula_wgsl',
  ],
  [COMPONENT_TYPE_PLANET_BODY_SHADER_SETTINGS]: [
    'planet_visual_wgsl',
    'star_visual_wgsl',
  ],
}

const GENERATED_COMPONENT_REGISTRY_SUFFIX =
  '::generated::components::GeneratedComponentRegistry'
const AGE_PROPERTY_IDENTIFIER_MAX_CHARS = 63
const COMPONENT_PAYLOAD_METADATA_KEYS = new Set([
  'component_id',
  'component_kind',
  'entity_id',
  'entityId',
  'last_tick',
  'typePath',
])

function asObjectRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null
    ? (value as Record<string, unknown>)
    : null
}

function sanitizeTypePathKey(typePath: string): string {
  return typePath.replaceAll('::', '__')
}

function agePropertyKeyForTypePath(typePath: string): string {
  return sanitizeTypePathKey(typePath).slice(
    0,
    AGE_PROPERTY_IDENTIFIER_MAX_CHARS,
  )
}

function componentPayloadEnvelopeKeys(typePath: string): Array<string> {
  return Array.from(
    new Set([
      typePath,
      sanitizeTypePathKey(typePath),
      agePropertyKeyForTypePath(typePath),
    ]),
  )
}

export function isGeneratedComponentRegistryTypePath(
  typePath: string,
): boolean {
  return typePath.endsWith(GENERATED_COMPONENT_REGISTRY_SUFFIX)
}

export function parseGeneratedComponentRegistryResource(
  value: unknown,
): GeneratedComponentRegistryResource | null {
  const root = asObjectRecord(value)
  if (!root) return null
  const wrapped = asObjectRecord(root.value) ?? root
  const entriesRaw = wrapped.entries
  if (!Array.isArray(entriesRaw)) return null
  const shaderEntriesRaw = Array.isArray(wrapped.shader_entries)
    ? wrapped.shader_entries
    : []

  const entries = entriesRaw.flatMap(
    (entry): Array<GeneratedComponentRegistryEntry> => {
      const record = asObjectRecord(entry)
      const schema = asObjectRecord(record?.editor_schema)
      const fieldsRaw = Array.isArray(schema?.fields) ? schema.fields : []
      const componentKind = record?.component_kind
      const typePath = record?.type_path
      const rootValueKind = schema?.root_value_kind
      if (
        typeof componentKind !== 'string' ||
        typeof typePath !== 'string' ||
        typeof rootValueKind !== 'string'
      ) {
        return []
      }

      return [
        {
          component_kind: componentKind,
          type_path: typePath,
          replication_visibility: Array.isArray(record?.replication_visibility)
            ? record.replication_visibility.filter(
                (scope): scope is string => typeof scope === 'string',
              )
            : [],
          editor_schema: {
            root_value_kind:
              rootValueKind as GeneratedComponentRegistryEntry['editor_schema']['root_value_kind'],
            fields: fieldsRaw.flatMap(
              (field): Array<ComponentEditorFieldSchema> => {
                const fieldRecord = asObjectRecord(field)
                const fieldPath = fieldRecord?.field_path
                const fieldName = fieldRecord?.field_name
                const displayName = fieldRecord?.display_name
                const valueKind = fieldRecord?.value_kind
                if (
                  typeof fieldPath !== 'string' ||
                  typeof fieldName !== 'string' ||
                  typeof displayName !== 'string' ||
                  typeof valueKind !== 'string'
                ) {
                  return []
                }
                return [
                  {
                    field_path: fieldPath,
                    field_name: fieldName,
                    display_name: displayName,
                    value_kind:
                      valueKind as ComponentEditorFieldSchema['value_kind'],
                    min:
                      typeof fieldRecord?.min === 'number'
                        ? fieldRecord.min
                        : null,
                    max:
                      typeof fieldRecord?.max === 'number'
                        ? fieldRecord.max
                        : null,
                    step:
                      typeof fieldRecord?.step === 'number'
                        ? fieldRecord.step
                        : null,
                    unit:
                      typeof fieldRecord?.unit === 'string'
                        ? fieldRecord.unit
                        : null,
                    options: Array.isArray(fieldRecord?.options)
                      ? fieldRecord.options.filter(
                          (option): option is string =>
                            typeof option === 'string',
                        )
                      : [],
                  },
                ]
              },
            ),
          },
        },
      ]
    },
  )

  const shader_entries = shaderEntriesRaw.flatMap(
    (entry): Array<ShaderEditorRegistryEntry> => {
      const record = asObjectRecord(entry)
      const assetId = record?.asset_id
      const sourcePath = record?.source_path
      if (typeof assetId !== 'string' || typeof sourcePath !== 'string') {
        return []
      }

      const uniformSchemaRaw = Array.isArray(record?.uniform_schema)
        ? record.uniform_schema
        : []
      const presetsRaw = Array.isArray(record?.presets) ? record.presets : []

      return [
        {
          asset_id: assetId,
          source_path: sourcePath,
          shader_family:
            typeof record?.shader_family === 'string'
              ? record.shader_family
              : null,
          dependencies: Array.isArray(record?.dependencies)
            ? record.dependencies.filter(
                (dependency): dependency is string =>
                  typeof dependency === 'string',
              )
            : [],
          bootstrap_required: record?.bootstrap_required === true,
          uniform_schema: uniformSchemaRaw.flatMap(
            (field): Array<ShaderEditorFieldSchema> => {
              const fieldRecord = asObjectRecord(field)
              const fieldPath = fieldRecord?.field_path
              const displayName = fieldRecord?.display_name
              const valueKind = fieldRecord?.value_kind
              if (
                typeof fieldPath !== 'string' ||
                typeof displayName !== 'string' ||
                typeof valueKind !== 'string'
              ) {
                return []
              }
              const optionsRaw = Array.isArray(fieldRecord?.options)
                ? fieldRecord.options
                : []
              return [
                {
                  field_path: fieldPath,
                  display_name: displayName,
                  description:
                    typeof fieldRecord?.description === 'string'
                      ? fieldRecord.description
                      : null,
                  value_kind:
                    valueKind as ShaderEditorFieldSchema['value_kind'],
                  min:
                    typeof fieldRecord?.min === 'number'
                      ? fieldRecord.min
                      : null,
                  max:
                    typeof fieldRecord?.max === 'number'
                      ? fieldRecord.max
                      : null,
                  step:
                    typeof fieldRecord?.step === 'number'
                      ? fieldRecord.step
                      : null,
                  options: optionsRaw.flatMap((option) => {
                    const optionRecord = asObjectRecord(option)
                    const optionValue = optionRecord?.value
                    const label = optionRecord?.label
                    if (
                      typeof optionValue !== 'string' ||
                      typeof label !== 'string'
                    ) {
                      return []
                    }
                    return [{ value: optionValue, label }]
                  }),
                  default_value_json:
                    typeof fieldRecord?.default_value_json === 'string'
                      ? fieldRecord.default_value_json
                      : null,
                  group:
                    typeof fieldRecord?.group === 'string'
                      ? fieldRecord.group
                      : null,
                },
              ]
            },
          ),
          presets: presetsRaw.flatMap((preset) => {
            const presetRecord = asObjectRecord(preset)
            const presetId = presetRecord?.preset_id
            const displayName = presetRecord?.display_name
            if (
              typeof presetId !== 'string' ||
              typeof displayName !== 'string'
            ) {
              return []
            }
            return [
              {
                preset_id: presetId,
                display_name: displayName,
                description:
                  typeof presetRecord?.description === 'string'
                    ? presetRecord.description
                    : null,
                values_json:
                  typeof presetRecord?.values_json === 'string'
                    ? presetRecord.values_json
                    : 'null',
              },
            ]
          }),
        },
      ]
    },
  )

  return { entries, shader_entries }
}

export function findGeneratedComponentRegistryResource(
  resources: Array<{ typePath: string; value?: unknown }>,
): GeneratedComponentRegistryResource | null {
  const match = resources.find((resource) =>
    isGeneratedComponentRegistryTypePath(resource.typePath),
  )
  if (!match) return null
  return parseGeneratedComponentRegistryResource(match.value)
}

function normalizeShaderRegistrySourcePath(path: string): string {
  return path.replace(/\\/g, '/').replace(/^data\//, '')
}

export function resolveShaderRegistryEntry(
  registry: GeneratedComponentRegistryResource | null,
  shader: { assetId?: string | null; sourcePath?: string | null } | null,
): ShaderEditorRegistryEntry | null {
  if (!registry || !shader) return null
  if (typeof shader.assetId === 'string' && shader.assetId.length > 0) {
    const direct = registry.shader_entries.find(
      (entry) => entry.asset_id === shader.assetId,
    )
    if (direct) return direct
  }
  if (typeof shader.sourcePath !== 'string' || shader.sourcePath.length === 0) {
    return null
  }
  const normalizedSourcePath = normalizeShaderRegistrySourcePath(
    shader.sourcePath,
  )
  return (
    registry.shader_entries.find(
      (entry) =>
        normalizeShaderRegistrySourcePath(entry.source_path) ===
        normalizedSourcePath,
    ) ?? null
  )
}

export function resolveShaderRegistryEntryForComponent(
  registry: GeneratedComponentRegistryResource | null,
  componentTypePath: string,
): ShaderEditorRegistryEntry | null {
  if (!registry) return null
  const assetIds = COMPONENT_SHADER_ASSET_IDS[componentTypePath] ?? []
  for (const assetId of assetIds) {
    const match = registry.shader_entries.find(
      (entry) => entry.asset_id === assetId,
    )
    if (match) {
      return match
    }
  }
  return null
}

export function resolveComponentRegistryEntry(
  node: GraphNode,
  registry: GeneratedComponentRegistryResource | null,
): GeneratedComponentRegistryEntry | null {
  if (!registry) return null
  const directTypePath = node.properties.typePath
  if (typeof directTypePath === 'string') {
    return (
      registry.entries.find((entry) => entry.type_path === directTypePath) ??
      null
    )
  }
  const directComponentKind = node.properties.component_kind
  if (typeof directComponentKind === 'string') {
    return (
      registry.entries.find(
        (entry) => entry.component_kind === directComponentKind,
      ) ?? null
    )
  }
  return null
}

export function getComponentPayloadFromNode(
  node: GraphNode,
  entry: GeneratedComponentRegistryEntry | null,
): unknown {
  if (entry) {
    const payloadKeys = new Set(componentPayloadEnvelopeKeys(entry.type_path))
    const fieldRoots = new Set(
      entry.editor_schema.fields
        .map((field) => field.field_path.split('.')[0])
        .filter((fieldRoot): fieldRoot is string => Boolean(fieldRoot)),
    )
    const hasRootField = Array.from(fieldRoots).some(
      (fieldRoot) => node.properties[fieldRoot] !== undefined,
    )
    if (hasRootField) {
      const payload: Record<string, unknown> = {}
      for (const fieldRoot of fieldRoots) {
        if (node.properties[fieldRoot] !== undefined) {
          payload[fieldRoot] = node.properties[fieldRoot]
        }
      }
      return payload
    }

    for (const key of payloadKeys) {
      const envelopePayload = node.properties[key]
      if (envelopePayload !== undefined) return envelopePayload
    }

    const metadataOnly = Object.keys(node.properties).every(
      (key) => COMPONENT_PAYLOAD_METADATA_KEYS.has(key) || payloadKeys.has(key),
    )
    if (!metadataOnly && node.properties.value !== undefined) {
      return node.properties.value
    }
  }
  if ('value' in node.properties && node.properties.value !== undefined) {
    return node.properties.value
  }
  if ('0' in node.properties && node.properties['0'] !== undefined) {
    return node.properties['0']
  }
  return node.properties
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    Object.getPrototypeOf(value) === Object.prototype
  )
}

function shouldTreatFieldAsRootValue(
  payload: unknown,
  schemaFieldCount: number,
  fieldPath: string,
): boolean {
  if (schemaFieldCount !== 1) return false
  if (fieldPath.includes('.')) return false
  if (!isPlainObject(payload)) return true
  return !(fieldPath in payload)
}

export function getSchemaFieldValue(
  payload: unknown,
  field: ComponentEditorFieldSchema,
  schemaFieldCount: number,
): unknown {
  if (
    shouldTreatFieldAsRootValue(payload, schemaFieldCount, field.field_path)
  ) {
    return payload
  }

  let current: unknown = payload
  for (const segment of field.field_path.split('.')) {
    const record = asObjectRecord(current)
    if (!record || !(segment in record)) {
      return undefined
    }
    current = record[segment]
  }
  return current
}

export function setSchemaFieldValue(
  payload: unknown,
  field: ComponentEditorFieldSchema,
  nextValue: unknown,
  schemaFieldCount: number,
): unknown {
  if (
    shouldTreatFieldAsRootValue(payload, schemaFieldCount, field.field_path)
  ) {
    return nextValue
  }

  const segments = field.field_path.split('.')
  const root = isPlainObject(payload) ? { ...payload } : {}
  let current: Record<string, unknown> = root
  for (const segment of segments.slice(0, -1)) {
    const nested = current[segment]
    const nextNested = isPlainObject(nested) ? { ...nested } : {}
    current[segment] = nextNested
    current = nextNested
  }
  const lastSegment = segments[segments.length - 1]
  if (!lastSegment) {
    return root
  }
  current[lastSegment] = nextValue
  return root
}

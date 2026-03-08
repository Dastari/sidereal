import type { GraphNode } from '@/components/grid/types'
import type {
  ComponentEditorFieldSchema,
  GeneratedComponentRegistryEntry,
  GeneratedComponentRegistryResource,
} from './types'

const GENERATED_COMPONENT_REGISTRY_SUFFIX =
  '::generated::components::GeneratedComponentRegistry'

function asObjectRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null
    ? (value as Record<string, unknown>)
    : null
}

function sanitizeTypePathKey(typePath: string): string {
  return typePath.replaceAll('::', '__')
}

export function isGeneratedComponentRegistryTypePath(typePath: string): boolean {
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

  const entries = entriesRaw.flatMap((entry): Array<GeneratedComponentRegistryEntry> => {
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
          root_value_kind: rootValueKind as GeneratedComponentRegistryEntry['editor_schema']['root_value_kind'],
          fields: fieldsRaw.flatMap((field): Array<ComponentEditorFieldSchema> => {
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
                  typeof fieldRecord?.min === 'number' ? fieldRecord.min : null,
                max:
                  typeof fieldRecord?.max === 'number' ? fieldRecord.max : null,
                step:
                  typeof fieldRecord?.step === 'number' ? fieldRecord.step : null,
                unit:
                  typeof fieldRecord?.unit === 'string' ? fieldRecord.unit : null,
                options: Array.isArray(fieldRecord?.options)
                  ? fieldRecord.options.filter(
                      (option): option is string => typeof option === 'string',
                    )
                  : [],
              },
            ]
          }),
        },
      },
    ]
  })

  return { entries }
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

export function resolveComponentRegistryEntry(
  node: GraphNode,
  registry: GeneratedComponentRegistryResource | null,
): GeneratedComponentRegistryEntry | null {
  if (!registry) return null
  const directTypePath = node.properties.typePath
  if (typeof directTypePath === 'string') {
    return (
      registry.entries.find((entry) => entry.type_path === directTypePath) ?? null
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
  if ('value' in node.properties && node.properties.value !== undefined) {
    return node.properties.value
  }
  if (entry) {
    const direct = node.properties[entry.type_path]
    if (direct !== undefined) return direct
    const sanitized = node.properties[sanitizeTypePathKey(entry.type_path)]
    if (sanitized !== undefined) return sanitized
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

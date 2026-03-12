import { parseAsStringLiteral } from 'nuqs'
import type { ReactNode } from 'react'
import type {
  ExpandedNode,
  GraphEdge,
  GraphNode,
  PlayerVisibilityOverlay,
  WorldEntity,
} from '@/components/grid/types'
import { buildBrpReadUrl } from '@/lib/brp-read'

type ApiGraph = {
  graph: string
  nodes: Array<{
    id: string
    label?: string
    kind?: string
    properties?: Record<string, unknown>
  }>
  edges: Array<{
    id: string
    from: string
    to: string
    label?: string
    properties?: Record<string, unknown>
  }>
  error?: string
}

type ApiWorld = {
  graph: string
  entities: Array<WorldEntity>
  error?: string
}

type ApiLiveWorld = {
  source: 'bevy_remote'
  target: 'server' | 'client'
  brpUrl: string
  graph: string
  entities: Array<WorldEntity>
  nodes: Array<{
    id: string
    label?: string
    kind?: string
    properties?: Record<string, unknown>
  }>
  edges: Array<{
    id: string
    from: string
    to: string
    label?: string
    properties?: Record<string, unknown>
  }>
  error?: string
}

type BrpResourceRecord = {
  typePath: string
  value?: unknown
  error?: string
}

type ContextMenuState = {
  open: boolean
  x: number
  y: number
  entityId: string | null
  worldX: number | null
  worldY: number | null
}

const DEFAULT_OWNER_TYPE_PATH = 'sidereal_game::components::owner_id::OwnerId'
const HEALTH_POOL_SUFFIX = '::HealthPool'
const FUEL_TANK_SUFFIX = '::FuelTank'
const AMMO_COUNT_SUFFIX = '::AmmoCount'
const POSITION_SUFFIX = '::Position'
const RESOURCE_SELECTION_PREFIX = 'resource:'
const GENERATED_COMPONENT_REGISTRY_TYPE_PATH =
  'sidereal_game::generated::components::GeneratedComponentRegistry'

const CAMERA_HIDE_SUBSTRING = 'bevy_camera::camera::Camera'
const UI_TRANSFORM_TYPE_NAME = 'UiTransform'

const explorerSourceParser = parseAsStringLiteral([
  'database',
  'liveServer',
  'liveClient',
] as const)

type ExplorerScope = 'database' | 'gameWorld'

export interface ExplorerWorkspaceProps {
  scope: ExplorerScope
  selectedEntityGuid?: string | null
  onSelectedEntityGuidChange?: (entityGuid: string | null) => void
  toolbarContent?: ReactNode
}

/** True if this entity should be hidden from the map (tree still shows it). */
function isCameraEntity(
  entity: WorldEntity,
  graphNodes: Map<string, GraphNode>,
  graphEdges: Array<GraphEdge>,
): boolean {
  if (
    entity.id.includes(CAMERA_HIDE_SUBSTRING) ||
    entity.name.includes(CAMERA_HIDE_SUBSTRING)
  ) {
    return true
  }
  const hasCameraComponent = graphEdges.some(
    (edge) =>
      edge.from === entity.id &&
      edge.label === 'HAS_COMPONENT' &&
      graphNodes.get(edge.to)?.label === 'Camera',
  )
  return hasCameraComponent
}

function hasUiTransformComponent(
  entityId: string,
  graphNodes: Map<string, GraphNode>,
  graphEdges: Array<GraphEdge>,
): boolean {
  return graphEdges.some((edge) => {
    if (edge.from !== entityId || edge.label !== 'HAS_COMPONENT') {
      return false
    }
    const componentNode = graphNodes.get(edge.to)
    if (!componentNode) return false
    const typePath = componentNode.properties.typePath
    if (typeof typePath === 'string' && typePath.endsWith(`::${UI_TRANSFORM_TYPE_NAME}`)) {
      return true
    }
    return componentNode.label === UI_TRANSFORM_TYPE_NAME
  })
}

function asFiniteNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string') {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) return parsed
  }
  return null
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function asObjectRecord(value: unknown): Record<string, unknown> | null {
  return isObjectRecord(value) ? value : null
}

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

function looksLikeUuid(value: string): boolean {
  return UUID_REGEX.test(value.trim())
}

function findStringDeep(value: unknown): string | null {
  if (typeof value === 'string') {
    let normalized = value.trim()
    if (!normalized) return null
    if (
      (normalized.startsWith('"') && normalized.endsWith('"')) ||
      (normalized.startsWith("'") && normalized.endsWith("'"))
    ) {
      try {
        const parsed = JSON.parse(normalized)
        if (typeof parsed === 'string') {
          normalized = parsed.trim()
        }
      } catch {
        normalized = normalized.slice(1, -1).trim()
      }
    }
    normalized = normalized.replace(/^"(.*)"$/, '$1').replace(/^'(.*)'$/, '$1')
    return normalized.length > 0 ? normalized : null
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      const found = findStringDeep(item)
      if (found) return found
    }
    return null
  }
  if (typeof value === 'object' && value !== null) {
    const obj = value as Record<string, unknown>
    for (const key of ['value', 'name', 'display_name', 'displayName', '0']) {
      if (!(key in obj)) continue
      const found = findStringDeep(obj[key])
      if (found) return found
    }
    for (const entry of Object.values(obj)) {
      const found = findStringDeep(entry)
      if (found) return found
    }
  }
  return null
}

function extractPositionFromComponentProps(
  rawProps: unknown,
): [number, number] | null {
  if (rawProps === null || rawProps === undefined) return null
  let props: Record<string, unknown>
  if (
    typeof rawProps === 'object' &&
    'properties' in rawProps &&
    typeof (rawProps as Record<string, unknown>).properties === 'object'
  ) {
    props = (rawProps as Record<string, unknown>).properties as Record<
      string,
      unknown
    >
  } else if (typeof rawProps === 'object') {
    props = rawProps as Record<string, unknown>
  } else {
    return null
  }
  const positionM =
    props.position_m ?? props.position ?? props.Position_m ?? props.Position
  if (Array.isArray(positionM) && positionM.length >= 2) {
    const x = Number(positionM[0])
    const y = Number(positionM[1])
    if (Number.isFinite(x) && Number.isFinite(y)) return [x, y]
  }
  for (const candidate of Object.values(props)) {
    if (!Array.isArray(candidate) || candidate.length < 2) continue
    const x = Number(candidate[0])
    const y = Number(candidate[1])
    if (Number.isFinite(x) && Number.isFinite(y)) return [x, y]
  }
  return null
}

function extractAvianPositionFromComponentProps(
  rawProps: unknown,
): [number, number] | null {
  if (!rawProps || typeof rawProps !== 'object') return null
  const props = rawProps as Record<string, unknown>

  const parseArrayLike = (candidate: unknown): unknown => {
    if (typeof candidate !== 'string') return candidate
    const trimmed = candidate.trim()
    if (!trimmed.startsWith('[') && !trimmed.startsWith('{')) return candidate
    try {
      return JSON.parse(trimmed)
    } catch {
      return candidate
    }
  }

  const fromArrayCandidate = (candidate: unknown): [number, number] | null => {
    const parsed = parseArrayLike(candidate)
    if (
      parsed &&
      typeof parsed === 'object' &&
      !Array.isArray(parsed) &&
      'value' in (parsed as Record<string, unknown>)
    ) {
      return fromArrayCandidate((parsed as Record<string, unknown>).value)
    }
    candidate = parsed
    if (!Array.isArray(candidate) || candidate.length < 2) return null
    const x = Number(candidate[0])
    const y = Number(candidate[1])
    return Number.isFinite(x) && Number.isFinite(y) ? [x, y] : null
  }

  const fromValue = fromArrayCandidate(props.value)
  if (fromValue) return fromValue

  const directPosition = props.position ?? props.Position ?? props['0']
  const fromDirect = fromArrayCandidate(directPosition)
  if (fromDirect) return fromDirect
  if (directPosition && typeof directPosition === 'object') {
    const nested = directPosition as Record<string, unknown>
    const fromNestedValue = fromArrayCandidate(nested.value)
    if (fromNestedValue) return fromNestedValue
    const nx = Number(nested.x)
    const ny = Number(nested.y)
    if (Number.isFinite(nx) && Number.isFinite(ny)) return [nx, ny]
  }

  const x = Number(props.x)
  const y = Number(props.y)
  if (Number.isFinite(x) && Number.isFinite(y)) return [x, y]

  return null
}

function extractParentGuidFromComponentProps(rawProps: unknown): string | null {
  const found = findStringDeep(rawProps)
  if (!found) return null
  const normalized = found.trim()
  return normalized.length > 0 ? normalized : null
}

function extractEntityGuidFromComponentProps(rawProps: unknown): string | null {
  const found = findStringDeep(rawProps)
  if (!found) return null
  const normalized = found.trim().toLowerCase()
  if (looksLikeUuid(normalized)) return normalized
  const suffix = normalized.split(':').pop()?.trim() ?? ''
  return looksLikeUuid(suffix) ? suffix : null
}

function buildEntitiesFromGraph(graph: ApiGraph): Array<WorldEntity> {
  const sampledAtMs = Date.now()
  const nodesById = new Map(graph.nodes.map((node) => [node.id, node]))
  const componentsByEntityId = new Map<string, Array<Record<string, unknown>>>()

  for (const edge of graph.edges) {
    if (edge.label !== 'HAS_COMPONENT') continue
    const componentNode = nodesById.get(edge.to)
    if (!componentNode) continue
    const existing = componentsByEntityId.get(edge.from)
    if (existing) {
      existing.push(componentNode.properties)
    } else {
      componentsByEntityId.set(edge.from, [componentNode.properties])
    }
  }

  const entities = graph.nodes.flatMap((node) => {
    if (typeof node.properties.component_kind === 'string') {
      return []
    }

    const componentProps = componentsByEntityId.get(node.id) ?? []
    const displayName = componentProps.reduce<string | null>((found, props) => {
      if (found) return found
      return props.component_kind === 'display_name' ? findStringDeep(props) : null
    }, null)
    const parentGuidFromComponent = componentProps.reduce<string | null>((found, props) => {
      if (found) return found
      return props.component_kind === 'parent_guid'
        ? extractParentGuidFromComponentProps(props)
        : null
    }, null)
    const entityGuidFromComponent = componentProps.reduce<string | null>((found, props) => {
      if (found) return found
      return props.component_kind === 'entity_guid'
        ? extractEntityGuidFromComponentProps(props)
        : null
    }, null)
    const positionProps = componentProps.find(
      (props) =>
        props.component_kind === 'avian_position' ||
        props.component_kind === 'world_position',
    )
    const velocityProps = componentProps.find(
      (props) => props.component_kind === 'avian_linear_velocity',
    )
    const mountedOnProps = componentProps.find(
      (props) => props.component_kind === 'mounted_on',
    )
    const pos = positionProps
      ? positionProps.component_kind === 'avian_position'
        ? extractAvianPositionFromComponentProps(positionProps)
        : extractPositionFromComponentProps(positionProps)
      : null
    const vel = velocityProps
      ? extractPositionFromComponentProps(velocityProps)
      : null
    const rawLabels = node.properties.entity_labels
    const entityLabels = Array.isArray(rawLabels)
      ? rawLabels.map((value) => (typeof value === 'string' ? value : String(value)))
      : undefined
    const parentEntityId =
      parentGuidFromComponent ??
      (typeof node.properties.parent_entity_id === 'string'
        ? node.properties.parent_entity_id
        : mountedOnProps
            ? extractParentGuidFromComponentProps(mountedOnProps)
            : undefined)
    const entityGuid =
      entityGuidFromComponent ??
      (looksLikeUuid(node.id) ? node.id.toLowerCase() : undefined)
    const kind =
      typeof node.properties.entity_type === 'string' &&
      node.properties.entity_type.length > 0
        ? node.properties.entity_type
        : typeof node.kind === 'string' && node.kind !== 'Entity'
          ? node.kind
          : 'entity'

    return [
      {
        id: node.id,
        name: displayName ?? String(node.properties.name ?? node.label ?? node.id),
        kind,
        parentEntityId,
        entity_labels: entityLabels?.length ? entityLabels : undefined,
        hasPosition: pos !== null,
        shardId: asFiniteNumber(node.properties.shard_id) ?? 1,
        x: pos?.[0] ?? 0,
        y: pos?.[1] ?? 0,
        vx: vel?.[0] ?? 0,
        vy: vel?.[1] ?? 0,
        sampledAtMs,
        componentCount: componentProps.length,
        ...(entityGuid ? { entityGuid } : {}),
      } satisfies WorldEntity,
    ]
  })

  entities.sort((left, right) => {
    const kindCompare = left.kind.localeCompare(right.kind)
    if (kindCompare !== 0) return kindCompare
    return left.name.localeCompare(right.name)
  })

  return entities
}

function decodeBase64NoPad(input: string): Uint8Array | null {
  if (typeof input !== 'string' || input.length === 0) return new Uint8Array()
  const normalized = input.replace(/-/g, '+').replace(/_/g, '/')
  const padLen = (4 - (normalized.length % 4)) % 4
  const padded = normalized + '='.repeat(padLen)
  try {
    const decoded = atob(padded)
    const bytes = new Uint8Array(decoded.length)
    for (let index = 0; index < decoded.length; index += 1) {
      bytes[index] = decoded.charCodeAt(index)
    }
    return bytes
  } catch {
    return null
  }
}

function parseChunkEncoding(value: unknown): 'Bitset' | 'SparseDeltaVarint' | null {
  if (typeof value === 'string') {
    if (value === 'Bitset' || value === 'SparseDeltaVarint') return value
    const normalized = value.toLowerCase()
    if (normalized === 'bitset') return 'Bitset'
    if (normalized === 'sparsedeltavarint') return 'SparseDeltaVarint'
  }
  if (isObjectRecord(value)) {
    if ('Bitset' in value) return 'Bitset'
    if ('SparseDeltaVarint' in value) return 'SparseDeltaVarint'
  }
  return null
}

function decodeSparseDeltaVarintIndices(
  bytes: Uint8Array,
  maxCellCount: number,
): Array<number> {
  const indices: Array<number> = []
  let cursor = 0
  let value = 0
  while (cursor < bytes.length) {
    let shift = 0
    let delta = 0
    let hasTerminator = false
    while (cursor < bytes.length) {
      const byte = bytes[cursor]
      cursor += 1
      delta += (byte & 0x7f) * (2 ** shift)
      if ((byte & 0x80) === 0) {
        hasTerminator = true
        break
      }
      shift += 7
      if (shift >= 32) {
        break
      }
    }
    if (!hasTerminator) break
    value += delta
    if (value >= 0 && value < maxCellCount) {
      indices.push(value)
    }
  }
  return indices
}

function decodeExploredCellsFromChunks(
  exploredCellsValue: Record<string, unknown>,
): {
  cellSizeM: number | null
  cells: Array<{ x: number; y: number }>
} {
  const cellSizeM =
    asFiniteNumber(exploredCellsValue.cell_size_m) ??
    asFiniteNumber(exploredCellsValue.cellSizeM) ??
    asFiniteNumber(exploredCellsValue.cellSize) ??
    null
  const chunkSizeRaw =
    asFiniteNumber(exploredCellsValue.chunk_size_cells) ??
    asFiniteNumber(exploredCellsValue.chunkSizeCells) ??
    asFiniteNumber(exploredCellsValue.chunk_size) ??
    asFiniteNumber(exploredCellsValue.chunkSize)
  const chunkSize = Math.max(1, Math.floor(chunkSizeRaw ?? 64))
  const maxCellCount = chunkSize * chunkSize
  const cellsByKey = new Map<string, { x: number; y: number }>()
  const chunks = Array.isArray(exploredCellsValue.chunks)
    ? exploredCellsValue.chunks
    : []

  for (const chunkRaw of chunks) {
    if (!isObjectRecord(chunkRaw)) continue
    const chunkX = asFiniteNumber(chunkRaw.chunk_x) ?? asFiniteNumber(chunkRaw.chunkX)
    const chunkY = asFiniteNumber(chunkRaw.chunk_y) ?? asFiniteNumber(chunkRaw.chunkY)
    const payloadB64 =
      typeof chunkRaw.payload_b64 === 'string'
        ? chunkRaw.payload_b64
        : typeof chunkRaw.payloadB64 === 'string'
          ? chunkRaw.payloadB64
          : null
    const encoding = parseChunkEncoding(chunkRaw.encoding)
    if (
      chunkX === null ||
      chunkY === null ||
      payloadB64 === null ||
      encoding === null
    ) {
      continue
    }
    const decodedBytes = decodeBase64NoPad(payloadB64)
    if (!decodedBytes) continue

    const setIndices: Array<number> = []
    if (encoding === 'Bitset') {
      for (let index = 0; index < maxCellCount; index += 1) {
        const byteIndex = Math.floor(index / 8)
        if (byteIndex >= decodedBytes.length) break
        const bitMask = 1 << (index % 8)
        if ((decodedBytes[byteIndex] & bitMask) !== 0) {
          setIndices.push(index)
        }
      }
    } else {
      setIndices.push(
        ...decodeSparseDeltaVarintIndices(decodedBytes, maxCellCount),
      )
    }

    for (const index of setIndices) {
      const localX = index % chunkSize
      const localY = Math.floor(index / chunkSize)
      const worldX = Math.floor(chunkX) * chunkSize + localX
      const worldY = Math.floor(chunkY) * chunkSize + localY
      const key = `${worldX},${worldY}`
      cellsByKey.set(key, { x: worldX, y: worldY })
    }
  }

  const cells = Array.from(cellsByKey.values()).sort(
    (left, right) => left.x - right.x || left.y - right.y,
  )
  return {
    cellSizeM: cellSizeM !== null && cellSizeM > 0 ? cellSizeM : null,
    cells,
  }
}

function extractResourceTypePaths(value: unknown): Array<string> {
  if (Array.isArray(value)) {
    return value.filter((entry): entry is string => typeof entry === 'string')
  }
  const objectValue = asObjectRecord(value)
  if (!objectValue) return []
  const directList =
    objectValue.resources ??
    objectValue.resource_types ??
    objectValue.type_paths ??
    objectValue.typePaths ??
    objectValue.value
  if (Array.isArray(directList)) {
    return directList.filter((entry): entry is string => typeof entry === 'string')
  }
  const keys = Object.keys(objectValue).filter((key) => key.includes('::'))
  if (keys.length > 0) return keys
  return []
}

async function callBrpJsonRpc<T = unknown>(
  port: number,
  target: 'server' | 'client',
  method: string,
  params?: unknown,
): Promise<{ result?: T; error?: string }> {
  const readUrl = buildBrpReadUrl({ method, params, port, target })
  const response = await fetch(
    readUrl ?? `/api/brp?port=${port}&target=${target}`,
    readUrl
      ? { method: 'GET' }
      : {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ method, params }),
        },
  )
  const payload = (await response.json()) as {
    result?: T
    error?: { message?: string } | string
  }
  if (!response.ok) {
    return {
      error:
        typeof payload.error === 'string'
          ? payload.error
          : payload.error?.message ?? `BRP ${method} failed`,
    }
  }
  if (payload.error) {
    return {
      error:
        typeof payload.error === 'string'
          ? payload.error
          : payload.error.message ?? `BRP ${method} failed`,
    }
  }
  return { result: payload.result }
}

async function fetchBrpResources(
  port: number,
  target: 'server' | 'client',
): Promise<Array<BrpResourceRecord>> {
  const listed = await callBrpJsonRpc<unknown>(port, target, 'world.list_resources')
  if (listed.error) {
    return [{ typePath: '__error__', error: listed.error }]
  }
  const typePaths = extractResourceTypePaths(listed.result).sort((a, b) =>
    a.localeCompare(b),
  )
  return typePaths.map((typePath) => ({ typePath }))
}

async function fetchBrpResourceValue(
  port: number,
  target: 'server' | 'client',
  typePath: string,
): Promise<{ value?: unknown; error?: string }> {
  const preferred = await callBrpJsonRpc<{ value?: unknown }>(
    port,
    target,
    'world.get_resources',
    { resource: typePath },
  )
  if (!preferred.error) {
    if (
      preferred.result &&
      typeof preferred.result === 'object' &&
      'value' in preferred.result
    ) {
      return { value: preferred.result.value }
    }
    return { value: preferred.result }
  }
  const fallback = await callBrpJsonRpc<{ value?: unknown }>(
    port,
    target,
    'world.get_resource',
    { resource: typePath },
  )
  if (fallback.error) {
    return { error: `${preferred.error}; fallback failed: ${fallback.error}` }
  }
  if (
    fallback.result &&
    typeof fallback.result === 'object' &&
    'value' in fallback.result
  ) {
    return { value: fallback.result.value }
  }
  return { value: fallback.result }
}

function extractEntityRegistryTemplateIds(
  resources: Array<BrpResourceRecord>,
): Array<{ templateId: string; label: string }> {
  const registryResource = resources.find((resource) =>
    resource.typePath.includes('EntityRegistryResource'),
  )
  if (!registryResource || registryResource.value === undefined) return []
  const valueObj = asObjectRecord(registryResource.value)
  const nestedValueObj = asObjectRecord(valueObj?.value)
  const maybeEntries = valueObj?.entries ?? nestedValueObj?.entries
  if (!Array.isArray(maybeEntries)) return []
  const out: Array<{ templateId: string; label: string }> = []
  for (const entry of maybeEntries) {
    const entryObj = asObjectRecord(entry)
    if (!entryObj) continue
    const templateIdRaw = entryObj.entity_id ?? entryObj.entityId
    const entityClassRaw = entryObj.entity_class ?? entryObj.entityClass
    if (typeof templateIdRaw !== 'string') continue
    if (typeof entityClassRaw === 'string' && entityClassRaw !== 'ship') continue
    const label = templateIdRaw
      .split('.')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ')
    out.push({ templateId: templateIdRaw, label })
  }
  return out.sort((a, b) => a.templateId.localeCompare(b.templateId))
}

function parseSelectedPlayerVisibilityOverlay(
  selectedId: string | null,
  graphNodes: Map<string, GraphNode>,
  graphEdges: Array<GraphEdge>,
): PlayerVisibilityOverlay | null {
  if (!selectedId) return null
  const componentNodeIds = graphEdges
    .filter((edge) => edge.from === selectedId && edge.label === 'HAS_COMPONENT')
    .map((edge) => edge.to)

  let spatialGridValue: Record<string, unknown> | null = null
  let disclosureValue: Record<string, unknown> | null = null
  let exploredCellsValue: Record<string, unknown> | null = null

  for (const componentId of componentNodeIds) {
    const node = graphNodes.get(componentId)
    if (!node || !isObjectRecord(node.properties)) continue
    const typePathRaw = node.properties.typePath
    const componentValue = node.properties.value
    if (typeof typePathRaw !== 'string' || !isObjectRecord(componentValue)) continue
    if (typePathRaw.endsWith('::VisibilitySpatialGrid')) {
      spatialGridValue = componentValue
    } else if (typePathRaw.endsWith('::VisibilityDisclosure')) {
      disclosureValue = componentValue
    } else if (typePathRaw.endsWith('::PlayerExploredCells')) {
      exploredCellsValue = componentValue
    }
  }

  if (!spatialGridValue && !exploredCellsValue) return null

  const parseCellList = (cellsRaw: unknown): Array<{ x: number; y: number }> =>
    (Array.isArray(cellsRaw) ? cellsRaw : [])
    .map((entry) => {
      if (!isObjectRecord(entry)) return null
      const x = asFiniteNumber(entry.x)
      const y = asFiniteNumber(entry.y)
      if (x === null || y === null) return null
      return { x, y }
    })
    .filter((entry): entry is { x: number; y: number } => entry !== null)

  const cellSizeM = spatialGridValue
    ? asFiniteNumber(spatialGridValue.cell_size_m) ??
      asFiniteNumber(spatialGridValue.cellSizeM) ??
      asFiniteNumber(spatialGridValue.cellSize)
    : null
  const deliveryRangeM = spatialGridValue
    ? asFiniteNumber(spatialGridValue.delivery_range_m) ??
      asFiniteNumber(spatialGridValue.deliveryRangeM) ??
      asFiniteNumber(spatialGridValue.deliveryRange) ??
      0
    : 0

  const queriedCells = spatialGridValue
    ? parseCellList(
        Array.isArray(spatialGridValue.queried_cells)
          ? spatialGridValue.queried_cells
          : spatialGridValue.queriedCells,
      )
    : []

  const decodedExplored = exploredCellsValue
    ? decodeExploredCellsFromChunks(exploredCellsValue)
    : null
  const exploredCellSizeM = decodedExplored?.cellSizeM ?? null
  const exploredCells = exploredCellsValue
    ? decodedExplored && decodedExplored.cells.length > 0
      ? decodedExplored.cells
      : parseCellList(
          Array.isArray(exploredCellsValue.cells)
            ? exploredCellsValue.cells
            : exploredCellsValue.explored_cells,
        )
    : []

  const visibilitySourcesRaw = disclosureValue
    ? Array.isArray(disclosureValue.visibility_sources)
      ? disclosureValue.visibility_sources
      : Array.isArray(disclosureValue.visibilitySources)
        ? disclosureValue.visibilitySources
        : Array.isArray(disclosureValue.scanner_sources)
          ? disclosureValue.scanner_sources
          : Array.isArray(disclosureValue.scannerSources)
            ? disclosureValue.scannerSources
            : []
    : []
  const visibilitySources = visibilitySourcesRaw
    .map((entry) => {
      if (!isObjectRecord(entry)) return null
      const x = asFiniteNumber(entry.x)
      const y = asFiniteNumber(entry.y)
      const z = asFiniteNumber(entry.z)
      const rangeM =
        asFiniteNumber(entry.range_m) ??
        asFiniteNumber(entry.rangeM) ??
        asFiniteNumber(entry.range)
      if (x === null || y === null || rangeM === null) return null
      return {
        x,
        y,
        ...(z === null ? {} : { z }),
        range_m: rangeM,
      }
    })
    .filter(
      (
        entry,
      ): entry is { x: number; y: number; z?: number; range_m: number } =>
        entry !== null,
    )

  return {
    cell_size_m: cellSizeM !== null && cellSizeM > 0 ? cellSizeM : 0,
    delivery_range_m: Math.max(0, deliveryRangeM),
    queried_cells: queriedCells,
    visibility_sources: visibilitySources,
    explored_cell_size_m:
      exploredCellSizeM !== null && exploredCellSizeM > 0
        ? exploredCellSizeM
        : null,
    explored_cells: exploredCells,
  }
}

function isPlayerEntity(entity: WorldEntity): boolean {
  if (entity.kind.toLowerCase().includes('player')) return true
  return entity.entity_labels?.some((label) => label.toLowerCase() === 'player') ?? false
}

function playerLabel(entity: WorldEntity): string {
  const guid = entity.entityGuid ?? entity.id
  return `${entity.name} (${guid.slice(0, 8)})`
}

function resolveOwnerTypePath(
  entityId: string,
  graphNodes: Map<string, GraphNode>,
  graphEdges: Array<GraphEdge>,
): string {
  const componentNodeIds = graphEdges
    .filter((edge) => edge.from === entityId && edge.label === 'HAS_COMPONENT')
    .map((edge) => edge.to)
  for (const componentNodeId of componentNodeIds) {
    const node = graphNodes.get(componentNodeId)
    const typePath = node?.properties.typePath
    if (typeof typePath !== 'string') continue
    if (typePath.endsWith('::OwnerId')) return typePath
  }
  return DEFAULT_OWNER_TYPE_PATH
}

type EntityComponentDescriptor = {
  typePath: string
  value: unknown
}

function getEntityComponentDescriptors(
  entityId: string,
  graphNodes: Map<string, GraphNode>,
  graphEdges: Array<GraphEdge>,
): Array<EntityComponentDescriptor> {
  const componentNodeIds = graphEdges
    .filter((edge) => edge.from === entityId && edge.label === 'HAS_COMPONENT')
    .map((edge) => edge.to)
  const descriptors: Array<EntityComponentDescriptor> = []
  for (const nodeId of componentNodeIds) {
    const node = graphNodes.get(nodeId)
    if (!node) continue
    const typePath = node.properties.typePath
    if (typeof typePath !== 'string') continue
    descriptors.push({ typePath, value: node.properties.value })
  }
  return descriptors
}

function findComponentBySuffix(
  entityId: string,
  graphNodes: Map<string, GraphNode>,
  graphEdges: Array<GraphEdge>,
  suffix: string,
): EntityComponentDescriptor | null {
  return (
    getEntityComponentDescriptors(entityId, graphNodes, graphEdges).find((component) =>
      component.typePath.endsWith(suffix),
    ) ?? null
  )
}

function normalizeVec2Value(existingValue: unknown, x: number, y: number): unknown {
  if (Array.isArray(existingValue)) {
    return [x, y]
  }
  if (isObjectRecord(existingValue)) {
    if (Array.isArray(existingValue.value)) {
      return { ...existingValue, value: [x, y] }
    }
    if (isObjectRecord(existingValue.value)) {
      return {
        ...existingValue,
        value: {
          ...existingValue.value,
          x,
          y,
        },
      }
    }
    if (isObjectRecord(existingValue.position)) {
      return {
        ...existingValue,
        position: {
          ...existingValue.position,
          x,
          y,
        },
      }
    }
    return {
      ...existingValue,
      x,
      y,
    }
  }
  return { x, y }
}

function normalizeHealthValue(existingValue: unknown): unknown | null {
  if (!isObjectRecord(existingValue)) return null
  const current = asFiniteNumber(existingValue.current)
  const maximum = asFiniteNumber(existingValue.maximum)
  if (current === null || maximum === null) return null
  return {
    ...existingValue,
    current: maximum,
  }
}

function normalizeFuelValue(existingValue: unknown): unknown | null {
  if (!isObjectRecord(existingValue)) return null
  const currentFuel = asFiniteNumber(existingValue.fuel_kg)
  if (currentFuel === null) return null
  const explicitMaxFuel = asFiniteNumber(existingValue.maximum_kg)
  const targetFuel = explicitMaxFuel ?? Math.max(currentFuel, 1000)
  return {
    ...existingValue,
    fuel_kg: targetFuel,
  }
}

function normalizeAmmoValue(existingValue: unknown): unknown | null {
  if (!isObjectRecord(existingValue)) return null
  const capacity = asFiniteNumber(existingValue.capacity)
  if (capacity === null) return null
  return {
    ...existingValue,
    current: capacity,
  }
}

function collectEntityAndDescendants(
  rootEntityId: string,
  entities: Array<WorldEntity>,
): Array<string> {
  const childrenByParent = new Map<string, Array<string>>()
  for (const entity of entities) {
    if (!entity.parentEntityId) continue
    const list = childrenByParent.get(entity.parentEntityId)
    if (list) {
      list.push(entity.id)
    } else {
      childrenByParent.set(entity.parentEntityId, [entity.id])
    }
  }
  const out: Array<string> = []
  const queue: Array<string> = [rootEntityId]
  const seen = new Set<string>()
  while (queue.length > 0) {
    const next = queue.shift()
    if (!next || seen.has(next)) continue
    seen.add(next)
    out.push(next)
    for (const childId of childrenByParent.get(next) ?? []) {
      queue.push(childId)
    }
  }
  return out
}

function isShipEntity(entity: WorldEntity): boolean {
  if (entity.kind.toLowerCase() === 'ship') return true
  return entity.entity_labels?.some((label) => label.toLowerCase() === 'ship') ?? false
}

export {
  AMMO_COUNT_SUFFIX,
  CAMERA_HIDE_SUBSTRING,
  DEFAULT_OWNER_TYPE_PATH,
  FUEL_TANK_SUFFIX,
  GENERATED_COMPONENT_REGISTRY_TYPE_PATH,
  HEALTH_POOL_SUFFIX,
  POSITION_SUFFIX,
  RESOURCE_SELECTION_PREFIX,
  UI_TRANSFORM_TYPE_NAME,
  asFiniteNumber,
  asObjectRecord,
  buildEntitiesFromGraph,
  collectEntityAndDescendants,
  decodeBase64NoPad,
  decodeExploredCellsFromChunks,
  decodeSparseDeltaVarintIndices,
  explorerSourceParser,
  extractEntityRegistryTemplateIds,
  fetchBrpResourceValue,
  fetchBrpResources,
  findComponentBySuffix,
  getEntityComponentDescriptors,
  hasUiTransformComponent,
  isCameraEntity,
  isObjectRecord,
  isPlayerEntity,
  isShipEntity,
  normalizeAmmoValue,
  normalizeFuelValue,
  normalizeHealthValue,
  normalizeVec2Value,
  parseChunkEncoding,
  parseSelectedPlayerVisibilityOverlay,
  playerLabel,
  resolveOwnerTypePath,
}

export type {
  ApiGraph,
  ApiLiveWorld,
  ApiWorld,
  BrpResourceRecord,
  ContextMenuState,
  EntityComponentDescriptor,
  ExplorerScope,
}

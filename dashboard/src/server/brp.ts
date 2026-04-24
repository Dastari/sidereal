import { execSync } from 'node:child_process'

type JsonRpcRequest = {
  jsonrpc?: string
  id?: unknown
  method: string
  params?: unknown
}

type JsonRpcError = {
  code: number
  message: string
  data?: unknown
}

type JsonRpcResponse = {
  jsonrpc?: string
  id?: unknown
  result?: unknown
  error?: JsonRpcError
}

type BrpCallResult = {
  response: JsonRpcResponse
  resolvedUrl: string
}

type BrpQueryRow = {
  entity: number | string
  components?: Record<string, unknown>
  has?: Record<string, unknown>
}

// BRP/world-coordinate payloads are f64 on the Rust side and JSON numbers at
// the dashboard boundary. Keep them as TypeScript number; never coerce through
// integer or f32-specific formatting while parsing.
export type WorldCoordinate = number

export type BrpTarget = 'server' | 'client' | 'hostClient'
export type BrpCallOptions = {
  target?: BrpTarget
  host?: string
  port?: number
}

export type LiveWorldEntity = {
  id: string
  name: string
  kind: string
  parentEntityId?: string
  entity_labels?: Array<string>
  mapVisible?: boolean
  hasPosition?: boolean
  shardId: number
  x: WorldCoordinate
  y: WorldCoordinate
  rotationRad?: number
  vx: WorldCoordinate
  vy: WorldCoordinate
  sampledAtMs: number
  componentCount: number
  entityGuid?: string
}

export type LiveGraphNode = {
  id: string
  label: string
  kind: string
  properties: Record<string, unknown>
}

export type LiveGraphEdge = {
  id: string
  from: string
  to: string
  label: string
  properties: Record<string, unknown>
}

export type LiveWorldSnapshot = {
  source: 'bevy_remote'
  target: BrpTarget
  brpUrl: string
  graph: string
  entities: Array<LiveWorldEntity>
  nodes: Array<LiveGraphNode>
  edges: Array<LiveGraphEdge>
}

function normalizeUrl(url: string): string {
  return url.endsWith('/') ? url : `${url}/`
}

/** Resolves default gateway via ip route (Linux). Cached; fallback 127.0.0.1. */
let cachedGateway: string | null = null
function getDefaultGateway(): string {
  if (cachedGateway !== null) return cachedGateway
  try {
    const out = execSync("ip route | awk '/default/ {print $3}'", {
      encoding: 'utf8',
      timeout: 2000,
    })
    const ip = out.trim()
    if (ip && /^[\d.]+$/.test(ip)) {
      cachedGateway = ip
      return cachedGateway
    }
  } catch {
    // Non-Linux or no default route
  }
  cachedGateway = '127.0.0.1'
  return cachedGateway
}

function normalizePort(port: number | undefined): number | null {
  if (port === undefined) return null
  if (!Number.isInteger(port) || port < 1 || port > 65535) return null
  return port
}

function normalizeHost(host: string | undefined): string | null {
  const trimmed = host?.trim()
  if (!trimmed) return null
  if (!/^[A-Za-z0-9.-]+$/.test(trimmed)) return null
  return trimmed
}

function defaultPortForTarget(target: BrpTarget): number {
  if (target === 'client') return 15714
  if (target === 'hostClient') return 15715
  return 15713
}

function getTargetBrpDefaults(target: BrpTarget, port?: number): Array<string> {
  const resolvedPort = normalizePort(port) ?? defaultPortForTarget(target)
  if (target === 'client') {
    return [
      `http://127.0.0.1:${resolvedPort}/`,
      `http://host.docker.internal:${resolvedPort}/`,
    ]
  }
  if (target === 'hostClient') {
    const host = getDefaultGateway()
    return [`http://${host}:${resolvedPort}/`]
  }
  return [
    `http://127.0.0.1:${resolvedPort}/`,
    `http://host.docker.internal:${resolvedPort}/`,
    `http://sidereal-replication:${resolvedPort}/`,
    `http://replication:${resolvedPort}/`,
  ]
}

function getTargetBrpEnvVars(target: BrpTarget): Array<string> {
  if (target === 'client') {
    return ['CLIENT_BRP_URL', 'SIDEREAL_CLIENT_BRP_URL', 'BRP_CLIENT_URL']
  }
  if (target === 'hostClient') {
    return [
      'HOST_CLIENT_BRP_URL',
      'SIDEREAL_HOST_CLIENT_BRP_URL',
      'CLIENT_BRP_URL',
      'SIDEREAL_CLIENT_BRP_URL',
      'BRP_CLIENT_URL',
    ]
  }
  return ['REPLICATION_BRP_URL', 'SIDEREAL_SERVER_BRP_URL', 'BRP_SERVER_URL']
}

function getBrpUrlFromEnv(target: BrpTarget): string | null {
  for (const envName of getTargetBrpEnvVars(target)) {
    const value = process.env[envName]?.trim()
    if (value) {
      return normalizeUrl(value)
    }
  }
  return null
}

function getLegacyBrpUrlFromEnv(): string | null {
  const raw = process.env.BRP_URL?.trim()
  if (!raw) return null
  return normalizeUrl(raw)
}

export function getBrpUrl(options: BrpCallOptions | BrpTarget = {}): string {
  const resolvedOptions: BrpCallOptions =
    typeof options === 'string' ? { target: options } : options
  const target = resolvedOptions.target ?? 'server'
  const resolvedPort = normalizePort(resolvedOptions.port)
  const resolvedHost = normalizeHost(resolvedOptions.host)
  if (resolvedHost && resolvedPort) {
    return `http://${resolvedHost}:${resolvedPort}/`
  }
  return (
    getBrpUrlFromEnv(target) ??
    getLegacyBrpUrlFromEnv() ??
    getTargetBrpDefaults(target, resolvedPort ?? undefined)[0]
  )
}

function getBrpAuthToken(target: BrpTarget): string | undefined {
  const envNames =
    target === 'client' || target === 'hostClient'
      ? [
          'SIDEREAL_CLIENT_BRP_AUTH_TOKEN',
          'CLIENT_BRP_AUTH_TOKEN',
          'SIDEREAL_BRP_AUTH_TOKEN',
          'BRP_AUTH_TOKEN',
        ]
      : [
          'SIDEREAL_REPLICATION_BRP_AUTH_TOKEN',
          'REPLICATION_BRP_AUTH_TOKEN',
          'SIDEREAL_SERVER_BRP_AUTH_TOKEN',
          'SERVER_BRP_AUTH_TOKEN',
          'SIDEREAL_BRP_AUTH_TOKEN',
          'BRP_AUTH_TOKEN',
        ]
  for (const envName of envNames) {
    const value = process.env[envName]?.trim()
    if (value) return value
  }
  return undefined
}

function getBrpHeaders(target: BrpTarget): Record<string, string> {
  const token = getBrpAuthToken(target)
  if (!token) return { 'content-type': 'application/json' }
  return {
    'content-type': 'application/json',
    authorization: `Bearer ${token}`,
  }
}

function getBrpCandidates(target: BrpTarget, port?: number): Array<string> {
  const resolvedPort = normalizePort(port) ?? undefined
  const preferred = getBrpUrl({ target, port: resolvedPort })
  const candidates = [preferred, ...getTargetBrpDefaults(target, resolvedPort)]
  if (target === 'server' && resolvedPort === undefined) {
    candidates.push('http://sidereal-shard:15712/', 'http://shard:15712/')
  }
  return Array.from(new Set(candidates.map(normalizeUrl)))
}

function getBrpCandidatesForOptions(options: BrpCallOptions): Array<string> {
  const target = options.target ?? 'server'
  const resolvedPort = normalizePort(options.port) ?? undefined
  const resolvedHost = normalizeHost(options.host)
  if (resolvedHost && resolvedPort) {
    return [`http://${resolvedHost}:${resolvedPort}/`]
  }
  return getBrpCandidates(target, resolvedPort)
}

function getBrpGraphName(target: BrpTarget): string {
  return target === 'client' || target === 'hostClient'
    ? 'bevy_remote_live_client_world'
    : 'bevy_remote_live_server_world'
}

function getBrpSourceLabel(target: BrpTarget): string {
  if (target === 'hostClient') return 'hostClient'
  return target === 'client' ? 'client' : 'server'
}

function makeId(): string {
  return `${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`
}

export async function callBrp(
  request: JsonRpcRequest,
  options: BrpCallOptions | BrpTarget = {},
): Promise<JsonRpcResponse> {
  const { response } = await callBrpWithMeta(request, options)
  return response
}

export async function callBrpWithMeta(
  request: JsonRpcRequest,
  options: BrpCallOptions | BrpTarget = {},
): Promise<BrpCallResult> {
  const resolvedOptions: BrpCallOptions =
    typeof options === 'string' ? { target: options } : options
  const target = resolvedOptions.target ?? 'server'
  const payload = JSON.stringify({
    jsonrpc: '2.0',
    id: request.id ?? makeId(),
    ...request,
  })
  const errors: Array<string> = []

  for (const url of getBrpCandidatesForOptions(resolvedOptions)) {
    try {
      const response = await fetch(url, {
        method: 'POST',
        headers: getBrpHeaders(target),
        body: payload,
      })

      const text = await response.text()
      if (!response.ok) {
        errors.push(
          `${url} -> HTTP ${response.status}: ${text || response.statusText}`,
        )
        continue
      }

      let parsed: JsonRpcResponse
      try {
        parsed = JSON.parse(text) as JsonRpcResponse
      } catch {
        errors.push(`${url} -> invalid JSON`)
        continue
      }
      return {
        response: parsed,
        resolvedUrl: url,
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      errors.push(`${url} -> ${message}`)
    }
  }

  throw new Error(
    `Unable to reach ${getBrpSourceLabel(target)} bevy_remote. Attempts: ${errors.join(' | ')}`,
  )
}

function shortTypeName(typePath: string): string {
  const last = typePath.split('::').pop()
  return last && last.length > 0 ? last : typePath
}

function looksLikeUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
    value,
  )
}

function findStringDeep(value: unknown): string | null {
  if (typeof value === 'string' && value.trim().length > 0) {
    return value.trim()
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      const found = findStringDeep(item)
      if (found) return found
    }
    return null
  }
  if (value && typeof value === 'object') {
    const obj = value as Record<string, unknown>
    for (const key of ['name', 'entity_id', 'id', 'guid', '0']) {
      if (!(key in obj)) continue
      const found = findStringDeep(obj[key])
      if (found) return found
    }
    for (const item of Object.values(obj)) {
      const found = findStringDeep(item)
      if (found) return found
    }
  }
  return null
}

function getNameFromComponents(
  components: Record<string, unknown>,
): string | null {
  // 1) Sidereal DisplayName component (canonical live-world label)
  for (const [key, value] of Object.entries(components)) {
    if (!key.endsWith('::DisplayName')) continue
    const found = findStringDeep(value)
    if (found) return found
  }

  // 2) Bevy Name component
  for (const [key, value] of Object.entries(components)) {
    if (!key.endsWith('::Name')) continue
    const found = findStringDeep(value)
    if (found) return found
  }

  // 3) Sidereal IDs / labels (EntityGuid, entity_id, etc.)
  for (const [key, value] of Object.entries(components)) {
    if (
      !(
        key.endsWith('::EntityGuid') ||
        key.endsWith('::EntityId') ||
        key.endsWith('::Label') ||
        /entity[_:]?id/i.test(key) ||
        /guid/i.test(key)
      )
    ) {
      continue
    }
    const found = findStringDeep(value)
    if (found) return found
  }

  // 4) Any nested field that looks like a proper UUID.
  for (const value of Object.values(components)) {
    const found = findStringDeep(value)
    if (found && looksLikeUuid(found)) return found
  }

  return null
}

function getKindFromComponents(components: Record<string, unknown>): string {
  const componentNames = Object.keys(components).map(shortTypeName)
  if (componentNames.some((name) => /ship/i.test(name))) return 'ship'
  if (componentNames.some((name) => /asteroid/i.test(name))) return 'asteroid'
  if (componentNames.some((name) => /planet/i.test(name))) return 'planet'
  if (componentNames.some((name) => /station/i.test(name))) return 'station'
  return 'entity'
}

function parseEntityRef(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (typeof value === 'number' && Number.isFinite(value)) return String(value)
  if (typeof value === 'string' && value.length > 0) return value
  if (Array.isArray(value)) {
    for (const entry of value) {
      const parsed = parseEntityRef(entry)
      if (parsed) return parsed
    }
    return null
  }
  if (typeof value === 'object') {
    const obj = value as Record<string, unknown>
    const directKeys = ['parent', 'entity', 'id', '0']
    for (const key of directKeys) {
      if (key in obj) {
        const parsed = parseEntityRef(obj[key])
        if (parsed) return parsed
      }
    }
    for (const entry of Object.values(obj)) {
      const parsed = parseEntityRef(entry)
      if (parsed) return parsed
    }
  }
  return null
}

function getParentEntityIdFromComponents(
  components: Record<string, unknown>,
): string | null {
  // 0) ParentGuid component is the canonical hierarchy link.
  for (const [componentPath, value] of Object.entries(components)) {
    if (
      !(
        componentPath.endsWith('::ParentGuid') ||
        componentPath.includes('::parent_guid::')
      )
    ) {
      continue
    }
    const parsed = parseEntityRef(value)
    if (parsed) return parsed
  }

  // 1) Canonical parent link from explicit parentEntityId/parent_entity_id fields.
  for (const value of Object.values(components)) {
    if (!value || typeof value !== 'object') continue
    const obj = value as Record<string, unknown>
    const parentCandidate = obj.parentEntityId ?? obj.parent_entity_id
    const parsed = parseEntityRef(parentCandidate)
    if (parsed) {
      return parsed
    }
  }

  // 2) Check for Bevy hierarchy components (fallback).
  let hasHierarchyParent = false
  for (const [componentPath, value] of Object.entries(components)) {
    if (
      componentPath.endsWith('::Parent') ||
      componentPath.endsWith('::ChildOf') ||
      /hierarchy::(Parent|ChildOf)$/.test(componentPath)
    ) {
      hasHierarchyParent = true
      const parsed = parseEntityRef(value)
      if (parsed) return parsed
    }
  }
  if (hasHierarchyParent) {
    // Preserve "this is a child" semantics even when BRP payload shape is unknown.
    return '__hierarchy_parent__'
  }
  return null
}

function findStringArrayDeep(value: unknown): Array<string> | null {
  if (Array.isArray(value)) {
    const strings = value
      .filter((entry): entry is string => typeof entry === 'string')
      .map((entry) => entry.trim())
      .filter((entry) => entry.length > 0)
    if (strings.length > 0) return strings

    for (const entry of value) {
      const nested = findStringArrayDeep(entry)
      if (nested && nested.length > 0) return nested
    }
    return null
  }
  if (!value || typeof value !== 'object') return null

  const obj = value as Record<string, unknown>
  for (const key of ['value', 'labels', 'entity_labels', '0']) {
    if (!(key in obj)) continue
    const nested = findStringArrayDeep(obj[key])
    if (nested && nested.length > 0) return nested
  }
  for (const entry of Object.values(obj)) {
    const nested = findStringArrayDeep(entry)
    if (nested && nested.length > 0) return nested
  }
  return null
}

function getEntityLabelsFromComponents(
  components: Record<string, unknown>,
): Array<string> | null {
  for (const [componentPath, value] of Object.entries(components)) {
    if (
      !(
        componentPath.endsWith('::EntityLabels') ||
        componentPath.includes('components::entity_labels::EntityLabels')
      )
    ) {
      continue
    }
    const labels = findStringArrayDeep(value)
    if (labels && labels.length > 0) return labels
  }
  return null
}

function normalizeGuidLike(value: string): string | null {
  const trimmed = value.trim()
  if (!trimmed) return null
  if (looksLikeUuid(trimmed)) return trimmed.toLowerCase()
  const suffix = trimmed.split(':').pop()?.trim() ?? ''
  if (looksLikeUuid(suffix)) return suffix.toLowerCase()
  return null
}

function getEntityGuidFromComponents(
  components: Record<string, unknown>,
): string | null {
  for (const [componentPath, value] of Object.entries(components)) {
    if (!componentPath.endsWith('::EntityGuid')) {
      continue
    }
    const found = findStringDeep(value)
    if (!found) continue
    const normalized = normalizeGuidLike(found)
    if (normalized) return normalized
  }
  return null
}

function asNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string') {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) return parsed
  }
  return null
}

function asBoolean(value: unknown): boolean | null {
  if (typeof value === 'boolean') return value
  if (typeof value === 'number') {
    if (value === 1) return true
    if (value === 0) return false
    return null
  }
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase()
    if (normalized === 'true' || normalized === '1') return true
    if (normalized === 'false' || normalized === '0') return false
  }
  return null
}

function findMapVisibleBooleanDeep(value: unknown): boolean | null {
  const direct = asBoolean(value)
  if (direct !== null) return direct
  if (Array.isArray(value)) {
    for (const entry of value) {
      const resolved = findMapVisibleBooleanDeep(entry)
      if (resolved !== null) return resolved
    }
    return null
  }
  if (!value || typeof value !== 'object') return null

  const obj = value as Record<string, unknown>
  for (const key of [
    'mapVisible',
    'map_visible',
    'visible',
    'isVisible',
    'enabled',
    'value',
    '0',
  ]) {
    if (!(key in obj)) continue
    const resolved = findMapVisibleBooleanDeep(obj[key])
    if (resolved !== null) return resolved
  }
  return null
}

function getMapVisibleFromComponents(
  components: Record<string, unknown>,
): boolean | null {
  let foundTrue = false

  for (const [componentPath, value] of Object.entries(components)) {
    const componentPathLower = componentPath.toLowerCase()
    const looksLikeVisibilityComponent =
      componentPathLower.endsWith('::mapvisible') ||
      componentPathLower.endsWith('::map_visibility') ||
      componentPathLower.includes('map_visible')

    const candidate = findMapVisibleBooleanDeep(value)
    if (candidate === null) continue

    const containsMapVisibleField =
      value &&
      typeof value === 'object' &&
      (Object.prototype.hasOwnProperty.call(value, 'mapVisible') ||
        Object.prototype.hasOwnProperty.call(value, 'map_visible'))

    if (!looksLikeVisibilityComponent && !containsMapVisibleField) {
      continue
    }

    if (!candidate) return false
    foundTrue = true
  }

  return foundTrue ? true : null
}

function parseXYFromObject(
  value: Record<string, unknown>,
): [number, number] | null {
  const keys = [
    ['x', 'y'],
    ['pos_x', 'pos_y'],
    ['x_m', 'y_m'],
    ['position_x', 'position_y'],
  ] as const

  for (const [xk, yk] of keys) {
    const x = asNumber(value[xk])
    const y = asNumber(value[yk])
    if (x !== null && y !== null) return [x, y]
  }

  return null
}

function getPositionFromComponents(
  components: Record<string, unknown>,
): [number, number] | null {
  const readVec2 = (candidate: unknown): [number, number] | null => {
    if (Array.isArray(candidate) && candidate.length >= 2) {
      const x = asNumber(candidate[0])
      const y = asNumber(candidate[1])
      if (x !== null && y !== null) return [x, y]
    }
    if (candidate && typeof candidate === 'object') {
      const obj = candidate as Record<string, unknown>
      const nestedValue = readVec2(obj.value)
      if (nestedValue) return nestedValue
      const nestedPos = readVec2(obj.position ?? obj.Position ?? obj['0'])
      if (nestedPos) return nestedPos
      const xy = parseXYFromObject(obj)
      if (xy) return xy
    }
    return null
  }

  // Static non-physics world entities use WorldPosition; simulated bodies use Avian Position.
  for (const [componentPath, value] of Object.entries(components)) {
    const isAvianPosition =
      componentPath.endsWith('::Position') &&
      componentPath.includes('physics_transform::transform::Position')
    const isWorldPosition =
      componentPath.endsWith('::WorldPosition') ||
      componentPath.includes('::world_position::')
    if (!isAvianPosition && !isWorldPosition) continue
    const xy = readVec2(value)
    if (xy) return xy
  }

  return null
}

function getVelocityFromComponents(
  components: Record<string, unknown>,
): [number, number] | null {
  for (const [componentPath, value] of Object.entries(components)) {
    if (!componentPath.endsWith('::LinearVelocity')) continue
    if (!componentPath.includes('dynamics::rigid_body::')) continue
    if (Array.isArray(value) && value.length >= 2) {
      const x = asNumber(value[0])
      const y = asNumber(value[1])
      if (x !== null && y !== null) return [x, y]
    }
    if (value && typeof value === 'object') {
      const xy = parseXYFromObject(value as Record<string, unknown>)
      if (xy) return xy
    }
  }

  return null
}

function getRotationFromComponents(
  components: Record<string, unknown>,
): number | null {
  const readRotation = (candidate: unknown): number | null => {
    const direct = asNumber(candidate)
    if (direct !== null) return direct

    if (Array.isArray(candidate)) {
      if (candidate.length >= 4) {
        const z = asNumber(candidate[2])
        const w = asNumber(candidate[3])
        if (z !== null && w !== null) {
          return 2 * Math.atan2(z, w)
        }
      }
      if (candidate.length >= 2) {
        const sin = asNumber(candidate[0])
        const cos = asNumber(candidate[1])
        if (sin !== null && cos !== null) {
          return Math.atan2(sin, cos)
        }
      }
    }

    if (!candidate || typeof candidate !== 'object') {
      return null
    }

    const record = candidate as Record<string, unknown>
    const nested = readRotation(
      record.value ?? record.rotation ?? record.Rotation ?? record['0'],
    )
    if (nested !== null) return nested

    const sin = asNumber(record.sin)
    const cos = asNumber(record.cos)
    if (sin !== null && cos !== null) {
      return Math.atan2(sin, cos)
    }

    const z = asNumber(record.z)
    const w = asNumber(record.w)
    if (z !== null && w !== null) {
      return 2 * Math.atan2(z, w)
    }

    return null
  }

  for (const [componentPath, value] of Object.entries(components)) {
    const isAvianRotation =
      componentPath.endsWith('::Rotation') &&
      componentPath.includes('physics_transform::transform::Rotation')
    const isWorldRotation =
      componentPath.endsWith('::WorldRotation') ||
      componentPath.includes('::world_rotation::')
    const isTransform =
      componentPath.endsWith('::Transform') ||
      componentPath.includes('transform::Transform')

    if (!isAvianRotation && !isWorldRotation && !isTransform) {
      continue
    }

    const rotation =
      isTransform && value && typeof value === 'object'
        ? readRotation((value as Record<string, unknown>).rotation)
        : readRotation(value)
    if (rotation !== null && Number.isFinite(rotation)) {
      return rotation
    }
  }

  return null
}

export async function getLiveWorldSnapshot(
  options: BrpCallOptions | BrpTarget = {},
): Promise<LiveWorldSnapshot> {
  const resolvedOptions: BrpCallOptions =
    typeof options === 'string' ? { target: options } : options
  const target = resolvedOptions.target ?? 'server'
  const { response: queryRes, resolvedUrl } = await callBrpWithMeta(
    {
      method: 'world.query',
      params: {
        data: {
          components: [],
          option: 'all',
          has: [],
        },
        filter: {
          with: [],
          without: [],
        },
        strict: false,
      },
    },
    resolvedOptions,
  )

  if (queryRes.error) {
    throw new Error(
      `BRP world.query failed (${queryRes.error.code}): ${queryRes.error.message}`,
    )
  }

  const rows = (
    Array.isArray(queryRes.result) ? queryRes.result : []
  ) as Array<BrpQueryRow>
  const entities: Array<LiveWorldEntity> = []
  const nodes: Array<LiveGraphNode> = []
  const edges: Array<LiveGraphEdge> = []

  const sampledAtMs = Date.now()
  rows.forEach((row) => {
    const entityId = String(row.entity)
    const components = row.components ?? {}
    const extractedName = getNameFromComponents(components)
    const name =
      extractedName && extractedName !== entityId
        ? extractedName
        : `Entity ${entityId}`
    const kind = getKindFromComponents(components)
    const xy = getPositionFromComponents(components)
    const velocity = getVelocityFromComponents(components)
    const rotationRad = getRotationFromComponents(components)
    const hasPosition = xy !== null
    const x = xy ? xy[0] : 0
    const y = xy ? xy[1] : 0
    const vx = velocity ? velocity[0] : 0
    const vy = velocity ? velocity[1] : 0
    const componentEntries = Object.entries(components)
    const mapVisibleFromComponents = getMapVisibleFromComponents(components)
    const mapVisible =
      mapVisibleFromComponents === null
        ? componentEntries.length > 0
        : mapVisibleFromComponents
    const parentEntityId =
      getParentEntityIdFromComponents(components) ?? undefined
    const entityLabels = getEntityLabelsFromComponents(components) ?? undefined
    const entityGuid = getEntityGuidFromComponents(components)

    entities.push({
      id: entityId,
      name,
      kind,
      parentEntityId,
      entity_labels: entityLabels,
      hasPosition,
      shardId: 1,
      x,
      y,
      ...(rotationRad !== null ? { rotationRad } : {}),
      vx,
      vy,
      sampledAtMs,
      mapVisible,
      componentCount: componentEntries.length,
      ...(entityGuid ? { entityGuid } : {}),
    })

    nodes.push({
      id: entityId,
      label: name,
      kind,
      properties: {
        source: 'bevy_remote',
        entity: row.entity,
        ...(rotationRad !== null ? { rotationRad } : {}),
        mapVisible,
        componentCount: componentEntries.length,
      },
    })

    for (const [componentPath, componentValue] of componentEntries) {
      const componentId = `${entityId}::${componentPath}`
      nodes.push({
        id: componentId,
        label: shortTypeName(componentPath),
        kind: 'component',
        properties: { typePath: componentPath, value: componentValue },
      })
      edges.push({
        id: `has_component:${entityId}:${componentPath}`,
        from: entityId,
        to: componentId,
        label: 'HAS_COMPONENT',
        properties: {},
      })
    }
  })

  return {
    source: 'bevy_remote',
    target,
    brpUrl: resolvedUrl,
    graph: getBrpGraphName(target),
    entities,
    nodes,
    edges,
  }
}

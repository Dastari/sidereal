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

type BrpQueryRow = {
  entity: number | string
  components?: Record<string, unknown>
  has?: Record<string, unknown>
}

export type BrpTarget = 'server' | 'client'

export type LiveWorldEntity = {
  id: string
  name: string
  kind: string
  parentEntityId?: string
  mapVisible?: boolean
  shardId: number
  x: number
  y: number
  z: number
  componentCount: number
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

function getTargetBrpDefaults(target: BrpTarget): Array<string> {
  if (target === 'client') {
    return ['http://127.0.0.1:15714/', 'http://host.docker.internal:15714/']
  }
  return [
    'http://127.0.0.1:15713/',
    'http://host.docker.internal:15713/',
    'http://sidereal-replication:15713/',
    'http://replication:15713/',
  ]
}

function getTargetBrpEnvVars(target: BrpTarget): Array<string> {
  if (target === 'client') {
    return ['CLIENT_BRP_URL', 'SIDEREAL_CLIENT_BRP_URL', 'BRP_CLIENT_URL']
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

export function getBrpUrl(target: BrpTarget = 'server'): string {
  return (
    getBrpUrlFromEnv(target) ??
    getLegacyBrpUrlFromEnv() ??
    getTargetBrpDefaults(target)[0]
  )
}

function getBrpAuthToken(target: BrpTarget): string | undefined {
  const envNames =
    target === 'client'
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

function getBrpCandidates(target: BrpTarget): Array<string> {
  const preferred = getBrpUrl(target)
  const candidates = [preferred, ...getTargetBrpDefaults(target)]
  if (target === 'server') {
    candidates.push('http://sidereal-shard:15712/', 'http://shard:15712/')
  }
  return Array.from(new Set(candidates.map(normalizeUrl)))
}

function getBrpGraphName(target: BrpTarget): string {
  return target === 'client'
    ? 'bevy_remote_live_client_world'
    : 'bevy_remote_live_server_world'
}

function getBrpSourceLabel(target: BrpTarget): string {
  return target === 'client' ? 'client' : 'server'
}

function makeId(): string {
  return `${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`
}

export async function callBrp(
  request: JsonRpcRequest,
  target: BrpTarget = 'server',
): Promise<JsonRpcResponse> {
  const payload = JSON.stringify({
    jsonrpc: '2.0',
    id: request.id ?? makeId(),
    ...request,
  })
  const errors: Array<string> = []

  for (const url of getBrpCandidates(target)) {
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
      return parsed
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
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
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
  // 1) Check for sidereal_game::MountedOn component
  for (const [componentPath, value] of Object.entries(components)) {
    if (
      componentPath.endsWith('::MountedOn') ||
      componentPath.includes('MountedOn')
    ) {
      if (value && typeof value === 'object') {
        const obj = value as Record<string, unknown>
        const parentId =
          obj.parent_entity_id ?? obj.parentEntityId ?? obj['parent_entity_id']
        if (typeof parentId === 'string' && parentId.length > 0) {
          return parentId
        }
      }
    }
  }

  // 2) Check for Bevy hierarchy components (fallback)
  for (const [componentPath, value] of Object.entries(components)) {
    if (
      componentPath.endsWith('::Parent') ||
      componentPath.endsWith('::ChildOf') ||
      /hierarchy::(Parent|ChildOf)$/.test(componentPath)
    ) {
      const parsed = parseEntityRef(value)
      if (parsed) return parsed
    }
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

function parseXYZFromObject(
  value: Record<string, unknown>,
): [number, number, number] | null {
  const keys = [
    ['x', 'y', 'z'],
    ['pos_x', 'pos_y', 'pos_z'],
    ['x_m', 'y_m', 'z_m'],
    ['position_x', 'position_y', 'position_z'],
  ] as const

  for (const [xk, yk, zk] of keys) {
    const x = asNumber(value[xk])
    const y = asNumber(value[yk])
    const z = asNumber(value[zk] ?? 0)
    if (x !== null && y !== null) return [x, y, z ?? 0]
  }

  return null
}

function getPositionFromComponents(
  components: Record<string, unknown>,
): [number, number, number] | null {
  // Prefer authoritative Avian position if present.
  for (const [componentPath, value] of Object.entries(components)) {
    if (!componentPath.endsWith('::Position')) continue
    if (!componentPath.includes('physics_transform::transform::Position'))
      continue
    if (Array.isArray(value) && value.length >= 3) {
      const x = asNumber(value[0])
      const y = asNumber(value[1])
      const z = asNumber(value[2])
      if (x !== null && y !== null) return [x, y, z ?? 0]
    }
    if (value && typeof value === 'object') {
      const xyz = parseXYZFromObject(value as Record<string, unknown>)
      if (xyz) return xyz
    }
  }

  for (const value of Object.values(components)) {
    if (!value || typeof value !== 'object') continue

    if (Array.isArray(value)) {
      if (value.length >= 12) {
        const x = asNumber(value[9])
        const y = asNumber(value[10])
        const z = asNumber(value[11])
        if (x !== null && y !== null) return [x, y, z ?? 0]
      }
      continue
    }

    const obj = value as Record<string, unknown>
    if (obj.translation && typeof obj.translation === 'object') {
      const translation = obj.translation as Record<string, unknown>
      if (Array.isArray(obj.translation) && obj.translation.length >= 3) {
        const x = asNumber(obj.translation[0])
        const y = asNumber(obj.translation[1])
        const z = asNumber(obj.translation[2])
        if (x !== null && y !== null) return [x, y, z ?? 0]
      }
      const xyz = parseXYZFromObject(translation)
      if (xyz) return xyz
    }

    const xyz = parseXYZFromObject(obj)
    if (xyz) return xyz
  }

  return null
}

export async function getLiveWorldSnapshot(
  target: BrpTarget = 'server',
): Promise<LiveWorldSnapshot> {
  const queryRes = await callBrp(
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
    target,
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

  rows.forEach((row, index) => {
    const entityId = String(row.entity)
    const components = row.components ?? {}
    const extractedName = getNameFromComponents(components)
    const name =
      extractedName && extractedName !== entityId
        ? extractedName
        : `Entity ${entityId}`
    const kind = getKindFromComponents(components)
    const xyz = getPositionFromComponents(components)
    const fallbackX =
      Math.cos((index / Math.max(1, rows.length)) * Math.PI * 2) * 200
    const fallbackY =
      Math.sin((index / Math.max(1, rows.length)) * Math.PI * 2) * 200
    const x = xyz ? xyz[0] : fallbackX
    const y = xyz ? xyz[1] : fallbackY
    const z = xyz ? xyz[2] : 0
    const componentEntries = Object.entries(components)
    const mapVisible = componentEntries.length > 0
    const parentEntityId =
      getParentEntityIdFromComponents(components) ?? undefined

    entities.push({
      id: entityId,
      name,
      kind,
      parentEntityId,
      shardId: 1,
      x,
      y,
      z,
      mapVisible,
      componentCount: componentEntries.length,
    })

    nodes.push({
      id: entityId,
      label: name,
      kind,
      properties: {
        source: 'bevy_remote',
        entity: row.entity,
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
    brpUrl: getBrpUrl(target),
    graph: getBrpGraphName(target),
    entities,
    nodes,
    edges,
  }
}

import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'

type WorldEntity = {
  id: string
  name: string
  kind: string
  parentEntityId?: string
  entity_labels?: string[]
  hasPosition?: boolean
  shardId: number
  x: number
  y: number
  vx: number
  vy: number
  sampledAtMs: number
  componentCount: number
  entityGuid?: string
}

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

function looksLikeUuid(s: string): boolean {
  return UUID_REGEX.test(s.trim())
}

function findStringDeep(value: unknown): string | null {
  if (typeof value === 'string') {
    let normalized = value.trim()
    if (!normalized) return null

    // Some AGE/JSON envelopes surface tuple-string values like "\"Corvette\"".
    // Decode once and strip any remaining single wrapping quotes.
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
  if (value && typeof value === 'object') {
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

type PgClient = {
  query: (sql: string, params?: Array<unknown>) => Promise<{ rows: Array<any> }>
  release: () => void
}

type PgPool = {
  connect: () => Promise<PgClient>
  on: (event: 'error', listener: (error: Error) => void) => void
}

type GlobalWithPool = typeof globalThis & {
  __siderealPgPool?: PgPool
  __siderealPgPoolErrorHandlerInstalled?: boolean
}
const globalPoolRef = globalThis as GlobalWithPool

async function getPool(): Promise<PgPool> {
  if (globalPoolRef.__siderealPgPool) {
    if (!globalPoolRef.__siderealPgPoolErrorHandlerInstalled) {
      globalPoolRef.__siderealPgPool.on('error', (error) => {
        console.error('[dashboard] postgres pool idle client error:', error)
      })
      globalPoolRef.__siderealPgPoolErrorHandlerInstalled = true
    }
    return globalPoolRef.__siderealPgPool
  }
  const { Pool } = await import('pg')
  const connectionString =
    process.env.REPLICATION_DATABASE_URL?.trim() ||
    process.env.DATABASE_URL?.trim() ||
    undefined
  const pool = new Pool({
    connectionString,
    host: process.env.PGHOST || '127.0.0.1',
    port: Number(process.env.PGPORT || 5432),
    database: process.env.PGDATABASE || 'sidereal',
    user: process.env.PGUSER || 'sidereal',
    password: process.env.PGPASSWORD || 'sidereal',
    max: 8,
  })
  pool.on('error', (error) => {
    console.error('[dashboard] postgres pool idle client error:', error)
  })
  globalPoolRef.__siderealPgPool = pool
  globalPoolRef.__siderealPgPoolErrorHandlerInstalled = true
  return pool
}

function parseAgtype(raw: unknown): any {
  if (raw === null || raw === undefined) return null
  const text = String(raw).trim()
  // Strip trailing ::agtype, ::vertex etc. so JSON.parse works
  const stripped = text.replace(/\s*::(agtype|vertex|edge|path)\s*$/i, '').trim()
  const firstPart = stripped.split('::')[0]?.trim() ?? stripped
  if (!firstPart || firstPart === 'null') return null
  try {
    return JSON.parse(firstPart)
  } catch {
    return firstPart.replace(/^"(.*)"$/, '$1')
  }
}

function extractPositionFromComponentProps(
  rawProps: unknown,
): [number, number] | null {
  if (rawProps === null || rawProps === undefined) return null
  // Unwrap if nested under "properties" (e.g. vertex-style agtype)
  let props: Record<string, unknown>
  if (
    typeof rawProps === 'object' &&
    rawProps !== null &&
    'properties' in rawProps &&
    typeof (rawProps as Record<string, unknown>).properties === 'object'
  ) {
    props = (rawProps as Record<string, unknown>).properties as Record<
      string,
      unknown
    >
  } else if (typeof rawProps === 'object' && rawProps !== null) {
    props = rawProps as Record<string, unknown>
  } else {
    return null
  }
  // Prefer explicit position_m array [x, y] or [x, y, z] (any casing)
  const positionM =
    props.position_m ?? props.position ?? props.Position_m ?? props.Position
  if (Array.isArray(positionM) && positionM.length >= 2) {
    const x = Number(positionM[0])
    const y = Number(positionM[1])
    if (Number.isFinite(x) && Number.isFinite(y)) return [x, y]
  }
  const candidates = Object.values(props)
  for (const candidate of candidates) {
    if (!Array.isArray(candidate) || candidate.length < 2) continue
    const x = Number(candidate[0])
    const y = Number(candidate[1])
    if (Number.isFinite(x) && Number.isFinite(y)) {
      return [x, y]
    }
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

  // Preferred shape for non-object payloads persisted via scalar wrapper.
  const fromValue = fromArrayCandidate(props.value)
  if (fromValue) return fromValue

  // Backward/alternate encoded envelopes may nest vectors.
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

  // Legacy flattened object form.
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
  return looksLikeUuid(normalized) ? normalized : null
}

function safeGraphName(input: string): string {
  const cleaned = input.trim()
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(cleaned)) {
    return 'sidereal'
  }
  return cleaned
}

export const Route = createFileRoute('/api/world')({
  server: {
    handlers: {
      GET: async () => {
        const graphName = safeGraphName(process.env.GRAPH_NAME || 'sidereal')
        const pool = await getPool()
        const client = await pool.connect()
        try {
          await client.query("LOAD 'age'")
          await client.query('SET search_path = ag_catalog, public')

          const rows = await client.query(
            `SELECT id::text AS id, kind::text AS kind, parent_id::text AS parent_id, parent_guid_props::text AS parent_guid_props, entity_guid_props::text AS entity_guid_props, entity_props::text AS entity_props, shard_id::text AS shard_id, pos_props::text AS pos_props, vel_props::text AS vel_props, display_props::text AS display_props, c::text AS component_count
             FROM ag_catalog.cypher('${graphName}', $$
               MATCH (e:Entity)
               OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(parent_guid:Component {component_kind:'parent_guid'})
               OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(entity_guid:Component {component_kind:'entity_guid'})
               OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(position:Component {component_kind:'avian_position'})
               OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(velocity:Component {component_kind:'avian_linear_velocity'})
               OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(display_name:Component {component_kind:'display_name'})
               OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(mounted_on:Component {component_kind:'mounted_on'})
               WITH e, position, velocity, display_name, mounted_on, parent_guid, entity_guid
               OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(component:Component)
               WITH e, position, velocity, display_name, mounted_on, parent_guid, entity_guid, count(component) AS component_count
               RETURN e.entity_id,
                      CASE
                        WHEN e.entity_type IS NOT NULL AND e.entity_type <> '' THEN e.entity_type
                        WHEN e.length_m IS NOT NULL AND e.width_m IS NOT NULL AND e.height_m IS NOT NULL THEN 'ship'
                        ELSE 'entity'
                      END,
                      coalesce(e.parent_entity_id, mounted_on.parent_entity_id),
                      properties(parent_guid), properties(entity_guid), properties(e), coalesce(e.shard_id, 1), properties(position), properties(velocity), properties(display_name), component_count
               ORDER BY coalesce(e.entity_type, 'entity'), coalesce(e.name, e.entity_id)
             $$) AS (id agtype, kind agtype, parent_id agtype, parent_guid_props agtype, entity_guid_props agtype, entity_props agtype, shard_id agtype, pos_props agtype, vel_props agtype, display_props agtype, c agtype);`,
          )

          const sampledAtMs = Date.now()
          const entities: Array<WorldEntity> = rows.rows.map((row) => {
            const entityProps = parseAgtype(row.entity_props) as Record<
              string,
              unknown
            > | null
            const pos = extractAvianPositionFromComponentProps(
              parseAgtype(row.pos_props),
            )
            const vel = extractPositionFromComponentProps(
              parseAgtype(row.vel_props),
            )
            const displayName = findStringDeep(parseAgtype(row.display_props))
            const entityId = String(parseAgtype(row.id) ?? '')
            const parentGuidFromComponent = extractParentGuidFromComponentProps(
              parseAgtype(row.parent_guid_props),
            )
            const entityGuidFromComponent = extractEntityGuidFromComponentProps(
              parseAgtype(row.entity_guid_props),
            )
            const entityGuid =
              entityGuidFromComponent ??
              (looksLikeUuid(entityId) ? entityId.toLowerCase() : undefined)
            const rawLabels = entityProps?.entity_labels
            const entity_labels = Array.isArray(rawLabels)
              ? rawLabels.map((v) => (typeof v === 'string' ? v : String(v)))
              : undefined
            return {
              id: entityId,
              name: displayName ?? entityId,
              kind: String(parseAgtype(row.kind) ?? 'entity'),
              parentEntityId:
                parentGuidFromComponent ??
                (parseAgtype(row.parent_id) === null
                  ? undefined
                  : String(parseAgtype(row.parent_id))),
              entity_labels: entity_labels?.length ? entity_labels : undefined,
              hasPosition: pos !== null,
              shardId: Number(parseAgtype(row.shard_id) ?? 1),
              x: pos?.[0] ?? 0,
              y: pos?.[1] ?? 0,
              vx: vel?.[0] ?? 0,
              vy: vel?.[1] ?? 0,
              sampledAtMs,
              componentCount: Number(parseAgtype(row.component_count) ?? 0),
              ...(entityGuid ? { entityGuid } : {}),
            }
          })
          entities.sort((a, b) => {
            const kindCmp = a.kind.localeCompare(b.kind)
            if (kindCmp !== 0) return kindCmp
            return a.name.localeCompare(b.name)
          })

          return json({ graph: graphName, entities })
        } catch (error) {
          if (error instanceof Error) {
            const pg = error as Error & { code?: string; detail?: string }
            const message = [error.message, pg.code, pg.detail]
              .filter(Boolean)
              .join(' | ')
            return json({ error: message }, { status: 500 })
          }
          return json({ error: 'Unknown error' }, { status: 500 })
        } finally {
          client.release()
        }
      },
    },
  },
  component: () => null,
})

import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'

type WorldEntity = {
  id: string
  name: string
  kind: string
  parentEntityId?: string
  shardId: number
  x: number
  y: number
  z: number
  componentCount: number
}

type PgClient = {
  query: (sql: string, params?: Array<unknown>) => Promise<{ rows: Array<any> }>
  release: () => void
}

type PgPool = {
  connect: () => Promise<PgClient>
}

type GlobalWithPool = typeof globalThis & { __siderealPgPool?: PgPool }
const globalPoolRef = globalThis as GlobalWithPool

async function getPool(): Promise<PgPool> {
  if (globalPoolRef.__siderealPgPool) return globalPoolRef.__siderealPgPool
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
  globalPoolRef.__siderealPgPool = pool
  return pool
}

function parseAgtype(raw: unknown): any {
  if (raw === null || raw === undefined) return null
  const text = String(raw).trim()
  const stripped = text.split('::')[0]?.trim() ?? text
  if (!stripped || stripped === 'null') return null
  try {
    return JSON.parse(stripped)
  } catch {
    return stripped.replace(/^"(.*)"$/, '$1')
  }
}

function extractPositionFromComponentProps(
  rawProps: unknown,
): [number, number, number] | null {
  if (!rawProps || typeof rawProps !== 'object') return null
  const props = rawProps as Record<string, unknown>
  const candidates = Object.values(props)
  for (const candidate of candidates) {
    if (!Array.isArray(candidate) || candidate.length < 2) continue
    const x = Number(candidate[0])
    const y = Number(candidate[1])
    const z = Number(candidate[2] ?? 0)
    if (Number.isFinite(x) && Number.isFinite(y) && Number.isFinite(z)) {
      return [x, y, z]
    }
  }
  return null
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
            `SELECT id::text AS id, name::text AS name, kind::text AS kind, parent_id::text AS parent_id, shard_id::text AS shard_id, pos_props::text AS pos_props, c::text AS component_count
             FROM ag_catalog.cypher('${graphName}', $$
               MATCH (e:Entity)
               OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(component:Component)
               OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(position:Component {component_kind:'position_m'})
               OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(mounted_on:Component {component_kind:'mounted_on'})
               WITH e, mounted_on, position, count(component) AS component_count
               RETURN e.entity_id,
                      coalesce(e.name, e.entity_id),
                      CASE
                        WHEN e.entity_type IS NOT NULL AND e.entity_type <> '' THEN e.entity_type
                        WHEN e.length_m IS NOT NULL AND e.width_m IS NOT NULL AND e.height_m IS NOT NULL THEN 'ship'
                        ELSE 'entity'
                      END,
                      mounted_on.parent_entity_id,
                      coalesce(e.shard_id, 1), properties(position), component_count
               ORDER BY coalesce(e.entity_type, 'entity'), coalesce(e.name, e.entity_id)
             $$) AS (id agtype, name agtype, kind agtype, parent_id agtype, shard_id agtype, pos_props agtype, c agtype);`,
          )

          const entities: Array<WorldEntity> = rows.rows.map((row) => {
            const pos = extractPositionFromComponentProps(
              parseAgtype(row.pos_props),
            )
            return {
              id: String(parseAgtype(row.id) ?? ''),
              name: String(parseAgtype(row.name) ?? 'unnamed'),
              kind: String(parseAgtype(row.kind) ?? 'entity'),
              parentEntityId:
                parseAgtype(row.parent_id) === null
                  ? undefined
                  : String(parseAgtype(row.parent_id)),
              shardId: Number(parseAgtype(row.shard_id) ?? 1),
              x: pos?.[0] ?? 0,
              y: pos?.[1] ?? 0,
              z: pos?.[2] ?? 0,
              componentCount: Number(parseAgtype(row.component_count) ?? 0),
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

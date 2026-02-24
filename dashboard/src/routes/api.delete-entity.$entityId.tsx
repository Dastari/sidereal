import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'

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

function safeGraphName(input: string): string {
  const cleaned = input.trim()
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(cleaned)) {
    return 'sidereal'
  }
  return cleaned
}

export const Route = createFileRoute('/api/delete-entity/$entityId')({
  server: {
    handlers: {
      DELETE: async ({ params }) => {
        const { entityId } = params
        const graphName = safeGraphName(process.env.GRAPH_NAME || 'sidereal')
        const pool = await getPool()
        const client = await pool.connect()

        try {
          await client.query("LOAD 'age'")
          await client.query('SET search_path = ag_catalog, public')

          // Delete entity and all its components from the graph
          await client.query(
            `SELECT * FROM ag_catalog.cypher('${graphName}', $$
              MATCH (e:Entity {entity_id: $1})
              OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(c:Component)
              DETACH DELETE e, c
            $$) AS (result agtype)`,
            [entityId],
          )

          return json({ success: true, entityId })
        } catch (error) {
          if (error instanceof Error) {
            const pg = error as Error & { code?: string; detail?: string }
            const message = [error.message, pg.code, pg.detail]
              .filter(Boolean)
              .join(' | ')
            return json({ error: message, success: false }, { status: 500 })
          }
          return json(
            { error: 'Unknown error', success: false },
            { status: 500 },
          )
        } finally {
          client.release()
        }
      },
    },
  },
  component: () => null,
})

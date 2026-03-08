export type PgClient = {
  query: (sql: string, params?: Array<unknown>) => Promise<{ rows: Array<any> }>
  release: () => void
}

export type PgPool = {
  connect: () => Promise<PgClient>
  on: (event: 'error', listener: (error: Error) => void) => void
}

type GlobalWithPool = typeof globalThis & {
  __siderealPgPool?: PgPool
  __siderealPgPoolErrorHandlerInstalled?: boolean
}

const globalPoolRef = globalThis as GlobalWithPool

export async function getPostgresPool(): Promise<PgPool> {
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

export function safeGraphName(input: string): string {
  const cleaned = input.trim()
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(cleaned)) {
    return 'sidereal'
  }
  return cleaned
}

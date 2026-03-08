import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { getPostgresPool, safeGraphName } from '@/server/postgres'

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

function parseGatewayUrl(): string {
  const raw = process.env.GATEWAY_API_URL?.trim() || 'http://127.0.0.1:8080'
  return raw.endsWith('/') ? raw.slice(0, -1) : raw
}

function looksLikeUuid(value: string): boolean {
  return UUID_REGEX.test(value.trim())
}

function isSafeIdentifier(value: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(value)
}

async function resolveAccountsQualifiedName(
  client: { query: (sql: string, params?: Array<unknown>) => Promise<{ rows: Array<any> }> },
): Promise<string | null> {
  const graphName = safeGraphName(process.env.GRAPH_NAME || 'sidereal')
  for (const schemaName of [graphName, 'public']) {
    if (!isSafeIdentifier(schemaName)) continue
    const qualified = `${schemaName}.auth_accounts`
    const result = await client.query(
      'SELECT to_regclass($1) IS NOT NULL AS present',
      [qualified],
    )
    if (result.rows[0]?.present === true) {
      return `"${schemaName}"."auth_accounts"`
    }
  }
  return null
}

export const Route = createFileRoute(
  '/api/database/accounts/$accountId/password-reset',
)({
  server: {
    handlers: {
      POST: async ({ params }) => {
        const accountId = params.accountId?.trim()
        if (!accountId || !looksLikeUuid(accountId)) {
          return json({ error: 'accountId must be a UUID' }, { status: 400 })
        }

        const pool = await getPostgresPool()
        const client = await pool.connect()
        try {
          const accountsQualifiedName = await resolveAccountsQualifiedName(client)
          if (!accountsQualifiedName) {
            return json({ error: 'auth_accounts table not found' }, { status: 404 })
          }
          const accountRow = await client.query(
            `
              SELECT email
              FROM ${accountsQualifiedName}
              WHERE account_id::text = $1
              LIMIT 1
            `,
            [accountId],
          )
          const email = accountRow.rows[0]?.email
          if (typeof email !== 'string' || email.length === 0) {
            return json({ error: 'account not found' }, { status: 404 })
          }

          const response = await fetch(
            `${parseGatewayUrl()}/auth/password-reset/request`,
            {
              method: 'POST',
              headers: { 'content-type': 'application/json' },
              body: JSON.stringify({ email }),
            },
          )
          const payload = (await response.json().catch(() => ({}))) as Record<
            string,
            unknown
          >
          if (!response.ok) {
            const error =
              typeof payload.error === 'string'
                ? payload.error
                : `gateway request failed with status ${response.status}`
            return json({ error }, { status: response.status })
          }
          return json({
            accepted: payload.accepted === true,
            resetToken:
              typeof payload.reset_token === 'string'
                ? payload.reset_token
                : null,
          })
        } catch (error) {
          return json(
            {
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to request password reset',
            },
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

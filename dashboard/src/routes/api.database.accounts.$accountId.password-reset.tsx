import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { passwordResetParamsSchema } from '@/lib/schemas/dashboard'
import { requireDashboardAdmin } from '@/server/dashboard-auth'
import { getPostgresPool, safeGraphName } from '@/server/postgres'

function parseGatewayUrl(): string {
  const raw = process.env.GATEWAY_API_URL?.trim() || 'http://127.0.0.1:8080'
  return raw.endsWith('/') ? raw.slice(0, -1) : raw
}

function isSafeIdentifier(value: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(value)
}

async function resolveAccountsQualifiedName(client: {
  query: (sql: string, params?: Array<unknown>) => Promise<{ rows: Array<any> }>
}): Promise<string | null> {
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
      POST: async ({ request, params }) => {
        const authFailure = requireDashboardAdmin(
          request,
          'dashboard:database:write',
        )
        if (authFailure) {
          return authFailure
        }

        const parsedParams = passwordResetParamsSchema.safeParse(params)
        if (!parsedParams.success) {
          return json(
            {
              error:
                parsedParams.error.issues[0]?.message ??
                'accountId must be a UUID',
            },
            { status: 400 },
          )
        }
        const accountId = parsedParams.data.accountId

        const pool = await getPostgresPool()
        const client = await pool.connect()
        try {
          const accountsQualifiedName =
            await resolveAccountsQualifiedName(client)
          if (!accountsQualifiedName) {
            return json(
              { error: 'auth_accounts table not found' },
              { status: 404 },
            )
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
            `${parseGatewayUrl()}/auth/v1/password-reset/request`,
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

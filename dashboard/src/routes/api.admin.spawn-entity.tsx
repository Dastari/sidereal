import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { spawnEntityBodySchema } from '@/lib/schemas/dashboard'
import {
  getDashboardSession,
  requireDashboardAdmin,
} from '@/server/dashboard-auth'

function parseGatewayUrl(): string {
  const raw = process.env.GATEWAY_API_URL?.trim() || 'http://127.0.0.1:8080'
  return raw.endsWith('/') ? raw.slice(0, -1) : raw
}

type SpawnEntityBody = {
  player_entity_id?: unknown
  bundle_id?: unknown
  overrides?: unknown
}

export const Route = createFileRoute('/api/admin/spawn-entity')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const authFailure = requireDashboardAdmin(request, 'admin:spawn')
        if (authFailure) {
          return authFailure
        }
        const session = getDashboardSession(request)
        if (!session) {
          return json(
            { error: 'Dashboard account session required' },
            { status: 403 },
          )
        }

        let body: SpawnEntityBody
        try {
          body = (await request.json()) as SpawnEntityBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        const parsedBody = spawnEntityBodySchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            {
              error:
                parsedBody.error.issues[0]?.message ?? 'Invalid request body',
            },
            { status: 400 },
          )
        }

        const gatewayBaseUrl = parseGatewayUrl()

        const response = await fetch(`${gatewayBaseUrl}/admin/spawn-entity`, {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            authorization: `Bearer ${session.accessToken}`,
          },
          body: JSON.stringify({
            player_entity_id: parsedBody.data.player_entity_id,
            bundle_id: parsedBody.data.bundle_id,
            overrides: parsedBody.data.overrides,
          }),
        })
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
        return json(payload)
      },
    },
  },
  component: () => null,
})

import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'

type SpawnEntityBody = {
  player_entity_id?: unknown
  bundle_id?: unknown
  overrides?: unknown
}

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

function parseGatewayUrl(): string {
  const raw = process.env.GATEWAY_API_URL?.trim() || 'http://127.0.0.1:8080'
  return raw.endsWith('/') ? raw.slice(0, -1) : raw
}

function parseBearerToken(): string | null {
  const token = process.env.SIDEREAL_DASHBOARD_ADMIN_BEARER_TOKEN?.trim()
  return token && token.length > 0 ? token : null
}

function looksLikeUuid(value: unknown): value is string {
  return typeof value === 'string' && UUID_REGEX.test(value.trim())
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

export const Route = createFileRoute('/api/admin/spawn-entity')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        let body: SpawnEntityBody
        try {
          body = (await request.json()) as SpawnEntityBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        if (!looksLikeUuid(body.player_entity_id)) {
          return json({ error: 'player_entity_id must be a UUID' }, { status: 400 })
        }
        if (typeof body.bundle_id !== 'string' || body.bundle_id.trim().length === 0) {
          return json({ error: 'bundle_id is required' }, { status: 400 })
        }
        if (body.overrides !== undefined && !isJsonObject(body.overrides)) {
          return json({ error: 'overrides must be an object when provided' }, { status: 400 })
        }

        const gatewayBaseUrl = parseGatewayUrl()
        const bearer = parseBearerToken()
        if (!bearer) {
          return json(
            { error: 'SIDEREAL_DASHBOARD_ADMIN_BEARER_TOKEN is not configured' },
            { status: 500 },
          )
        }

        const response = await fetch(`${gatewayBaseUrl}/admin/spawn-entity`, {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            authorization: `Bearer ${bearer}`,
          },
          body: JSON.stringify({
            player_entity_id: body.player_entity_id.trim(),
            bundle_id: body.bundle_id.trim(),
            overrides: body.overrides ?? {},
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

import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import {
  discardShipyardShipDraft,
  saveShipyardShipDraft,
} from '@/lib/shipyard.server'
import {
  shipyardShipDraftBodySchema,
  shipyardShipParamsSchema,
} from '@/lib/schemas/dashboard'
import {
  getDashboardSession,
  requireDashboardAdmin,
} from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/shipyard/ships/$shipId/draft')({
  server: {
    handlers: {
      POST: async ({ params, request }) => {
        const authFailure = requireDashboardAdmin(request, 'scripts:write')
        if (authFailure) return authFailure
        const session = getDashboardSession(request)
        if (!session) {
          return json(
            { error: 'Dashboard account session required' },
            { status: 403 },
          )
        }
        const parsedParams = shipyardShipParamsSchema.safeParse(params)
        if (!parsedParams.success) {
          return json({ error: 'Invalid Shipyard ship id' }, { status: 400 })
        }
        const body = await request.json().catch(() => null)
        const parsedBody = shipyardShipDraftBodySchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            { error: 'Invalid Shipyard ship draft payload' },
            { status: 400 },
          )
        }
        if (parsedBody.data.definition.ship_id !== parsedParams.data.shipId) {
          return json({ error: 'Ship id mismatch' }, { status: 400 })
        }

        try {
          return json(
            await saveShipyardShipDraft(parsedBody.data, session.accessToken),
          )
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : 'Failed to save Shipyard ship draft'
          return json({ error: message }, { status: 500 })
        }
      },
      DELETE: async ({ params, request }) => {
        const authFailure = requireDashboardAdmin(request, 'scripts:write')
        if (authFailure) return authFailure
        const session = getDashboardSession(request)
        if (!session) {
          return json(
            { error: 'Dashboard account session required' },
            { status: 403 },
          )
        }
        const parsedParams = shipyardShipParamsSchema.safeParse(params)
        if (!parsedParams.success) {
          return json({ error: 'Invalid Shipyard ship id' }, { status: 400 })
        }

        try {
          return json(
            await discardShipyardShipDraft(
              parsedParams.data.shipId,
              session.accessToken,
            ),
          )
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : 'Failed to discard Shipyard ship draft'
          return json({ error: message }, { status: 500 })
        }
      },
    },
  },
  component: () => null,
})

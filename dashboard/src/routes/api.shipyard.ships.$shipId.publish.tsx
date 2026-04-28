import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { publishShipyardShipDraft } from '@/lib/shipyard.server'
import { shipyardShipParamsSchema } from '@/lib/schemas/dashboard'
import {
  getDashboardSession,
  requireDashboardAdmin,
} from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/shipyard/ships/$shipId/publish')({
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

        try {
          return json(
            await publishShipyardShipDraft(
              parsedParams.data.shipId,
              session.accessToken,
            ),
          )
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : 'Failed to publish Shipyard ship draft'
          return json({ error: message }, { status: 500 })
        }
      },
    },
  },
  component: () => null,
})

import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { publishShipyardModuleDraft } from '@/lib/shipyard.server'
import { shipyardModuleParamsSchema } from '@/lib/schemas/dashboard'
import {
  getDashboardSession,
  requireDashboardAdmin,
} from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/shipyard/modules/$moduleId/publish')(
  {
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
          const parsedParams = shipyardModuleParamsSchema.safeParse(params)
          if (!parsedParams.success) {
            return json(
              { error: 'Invalid Shipyard module id' },
              { status: 400 },
            )
          }

          try {
            return json(
              await publishShipyardModuleDraft(
                parsedParams.data.moduleId,
                session.accessToken,
              ),
            )
          } catch (error) {
            const message =
              error instanceof Error
                ? error.message
                : 'Failed to publish Shipyard module draft'
            return json({ error: message }, { status: 500 })
          }
        },
      },
    },
    component: () => null,
  },
)

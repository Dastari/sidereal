import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import {
  discardShipyardModuleDraft,
  saveShipyardModuleDraft,
} from '@/lib/shipyard.server'
import {
  shipyardModuleDraftBodySchema,
  shipyardModuleParamsSchema,
} from '@/lib/schemas/dashboard'
import {
  getDashboardSession,
  requireDashboardAdmin,
} from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/shipyard/modules/$moduleId/draft')({
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
          return json({ error: 'Invalid Shipyard module id' }, { status: 400 })
        }
        const body = await request.json().catch(() => null)
        const parsedBody = shipyardModuleDraftBodySchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            { error: 'Invalid Shipyard module draft payload' },
            { status: 400 },
          )
        }
        if (
          parsedBody.data.definition.module_id !== parsedParams.data.moduleId
        ) {
          return json({ error: 'Module id mismatch' }, { status: 400 })
        }

        try {
          return json(
            await saveShipyardModuleDraft(parsedBody.data, session.accessToken),
          )
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : 'Failed to save Shipyard module draft'
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
        const parsedParams = shipyardModuleParamsSchema.safeParse(params)
        if (!parsedParams.success) {
          return json({ error: 'Invalid Shipyard module id' }, { status: 400 })
        }

        try {
          return json(
            await discardShipyardModuleDraft(
              parsedParams.data.moduleId,
              session.accessToken,
            ),
          )
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : 'Failed to discard Shipyard module draft'
          return json({ error: message }, { status: 500 })
        }
      },
    },
  },
  component: () => null,
})

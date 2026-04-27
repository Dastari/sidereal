import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { publishGenesisPlanetDraft } from '@/lib/genesis.server'
import { genesisPlanetParamsSchema } from '@/lib/schemas/dashboard'
import {
  getDashboardSession,
  requireDashboardAdmin,
} from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/genesis/planets/$planetId/publish')({
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

        const parsedParams = genesisPlanetParamsSchema.safeParse(params)
        if (!parsedParams.success) {
          return json(
            {
              error:
                parsedParams.error.issues[0]?.message ?? 'Invalid planet id',
            },
            { status: 400 },
          )
        }

        try {
          return json(
            await publishGenesisPlanetDraft(
              parsedParams.data.planetId,
              session.accessToken,
            ),
          )
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : 'Failed to publish Genesis draft'
          return json({ error: message }, { status: 500 })
        }
      },
    },
  },
  component: () => null,
})

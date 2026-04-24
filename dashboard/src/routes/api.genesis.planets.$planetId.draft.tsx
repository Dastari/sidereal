import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import {
  genesisPlanetDraftBodySchema,
  genesisPlanetParamsSchema,
} from '@/lib/schemas/dashboard'
import {
  discardGenesisPlanetDraft,
  saveGenesisPlanetDraft,
} from '@/lib/genesis.server'
import { requireDashboardAdmin } from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/genesis/planets/$planetId/draft')({
  server: {
    handlers: {
      POST: async ({ params, request }) => {
        const authFailure = requireDashboardAdmin(request)
        if (authFailure) return authFailure

        const parsedParams = genesisPlanetParamsSchema.safeParse(params)
        if (!parsedParams.success) {
          return json(
            { error: parsedParams.error.issues[0]?.message ?? 'Invalid planet id' },
            { status: 400 },
          )
        }

        const body = (await request.json().catch(() => null)) as unknown
        const parsedBody = genesisPlanetDraftBodySchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            {
              error:
                parsedBody.error.issues[0]?.message ?? 'Invalid Genesis draft payload',
            },
            { status: 400 },
          )
        }
        if (parsedBody.data.definition.planet_id !== parsedParams.data.planetId) {
          return json({ error: 'planet id mismatch' }, { status: 400 })
        }

        try {
          return json(await saveGenesisPlanetDraft(parsedBody.data))
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Failed to save Genesis draft'
          return json({ error: message }, { status: 500 })
        }
      },
      DELETE: async ({ params, request }) => {
        const authFailure = requireDashboardAdmin(request)
        if (authFailure) return authFailure

        const parsedParams = genesisPlanetParamsSchema.safeParse(params)
        if (!parsedParams.success) {
          return json(
            { error: parsedParams.error.issues[0]?.message ?? 'Invalid planet id' },
            { status: 400 },
          )
        }

        try {
          return json(await discardGenesisPlanetDraft(parsedParams.data.planetId))
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Failed to discard Genesis draft'
          return json({ error: message }, { status: 500 })
        }
      },
    },
  },
  component: () => null,
})

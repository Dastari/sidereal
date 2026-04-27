import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { renameCharacterParamsSchema } from '@/lib/schemas/dashboard'
import {
  createDashboardSessionCookie,
  dashboardSessionStatus,
  getDashboardSession,
  isDashboardAdminConfigured,
  refreshDashboardSession,
  rejectCrossOriginMutation,
  resetDashboardCharacter,
} from '@/server/dashboard-auth'

export const Route = createFileRoute(
  '/api/account/characters/$playerEntityId/reset',
)({
  server: {
    handlers: {
      POST: async ({ request, params }) => {
        const crossOriginFailure = rejectCrossOriginMutation(request)
        if (crossOriginFailure) return crossOriginFailure

        const parsedParams = renameCharacterParamsSchema.safeParse({
          playerEntityId: params.playerEntityId,
        })
        if (!parsedParams.success) {
          return json({ error: 'Invalid player entity id' }, { status: 400 })
        }

        const prepared = await prepareAccountSession(request)
        if (prepared instanceof Response) return prepared

        try {
          return json(
            await resetDashboardCharacter(
              prepared.session,
              params.playerEntityId,
            ),
            { headers: prepared.headers },
          )
        } catch (error) {
          return json(
            {
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to reset character',
            },
            { status: 403, headers: prepared.headers },
          )
        }
      },
    },
  },
  component: () => null,
})

async function prepareAccountSession(request: Request) {
  if (!isDashboardAdminConfigured()) {
    return json(
      {
        error:
          'Dashboard auth is not configured. Set SIDEREAL_DASHBOARD_SESSION_SECRET.',
      },
      { status: 503 },
    )
  }

  const session = getDashboardSession(request)
  if (!session) {
    return json(dashboardSessionStatus(null), { status: 403 })
  }

  try {
    const refreshed = await refreshDashboardSession(session)
    return {
      session: refreshed,
      headers:
        refreshed.accessToken === session.accessToken
          ? undefined
          : {
              'set-cookie': createDashboardSessionCookie(request, refreshed),
            },
    }
  } catch {
    return json(dashboardSessionStatus(null), { status: 403 })
  }
}

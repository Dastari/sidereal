import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import {
  createDashboardSessionCookie,
  dashboardSessionStatus,
  enrollDashboardTotp,
  getDashboardSession,
  isDashboardAdminConfigured,
  refreshDashboardSession,
  rejectCrossOriginMutation,
} from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/account/mfa/totp/enroll')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const crossOriginFailure = rejectCrossOriginMutation(request)
        if (crossOriginFailure) return crossOriginFailure

        const prepared = await prepareAccountSession(request)
        if (prepared instanceof Response) return prepared

        try {
          return json(await enrollDashboardTotp(prepared.session), {
            headers: prepared.headers,
          })
        } catch (error) {
          return json(
            {
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to start authenticator enrollment',
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

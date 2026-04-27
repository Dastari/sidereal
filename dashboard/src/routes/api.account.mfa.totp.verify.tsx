import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { dashboardTotpEnrollmentVerifySchema } from '@/lib/schemas/dashboard'
import {
  createDashboardSessionCookie,
  dashboardSessionStatus,
  getDashboardSession,
  isDashboardAdminConfigured,
  refreshDashboardSession,
  rejectCrossOriginMutation,
  verifyDashboardTotpEnrollment,
} from '@/server/dashboard-auth'

type VerifyBody = {
  enrollmentId?: unknown
  code?: unknown
}

export const Route = createFileRoute('/api/account/mfa/totp/verify')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const crossOriginFailure = rejectCrossOriginMutation(request)
        if (crossOriginFailure) return crossOriginFailure

        const prepared = await prepareAccountSession(request)
        if (prepared instanceof Response) return prepared

        let body: VerifyBody
        try {
          body = (await request.json()) as VerifyBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        const parsedBody = dashboardTotpEnrollmentVerifySchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            {
              error:
                parsedBody.error.issues[0]?.message ?? 'Invalid request body',
            },
            { status: 400 },
          )
        }

        try {
          const session = await verifyDashboardTotpEnrollment(
            prepared.session,
            parsedBody.data.enrollmentId,
            parsedBody.data.code,
          )
          return json(dashboardSessionStatus(session), {
            headers: {
              ...prepared.headers,
              'set-cookie': createDashboardSessionCookie(request, session),
            },
          })
        } catch (error) {
          return json(
            {
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to verify authenticator code',
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

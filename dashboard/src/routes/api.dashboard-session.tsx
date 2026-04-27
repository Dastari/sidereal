import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import {
  dashboardMfaLoginSchema,
  dashboardPasswordLoginSchema,
  dashboardRegisterSchema,
} from '@/lib/schemas/dashboard'
import {
  clearDashboardSessionCookie,
  createDashboardSessionCookie,
  createDashboardSessionFromPassword,
  createDashboardSessionFromRegistration,
  createDashboardSessionFromTotpChallenge,
  dashboardSessionStatus,
  getDashboardSession,
  isDashboardAdminConfigured,
  refreshDashboardSession,
  rejectCrossOriginMutation,
} from '@/server/dashboard-auth'

type SessionLoginBody = {
  email?: unknown
  password?: unknown
  challenge_id?: unknown
  code?: unknown
  mode?: unknown
}

export const Route = createFileRoute('/api/dashboard-session')({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const session = getDashboardSession(request)
        if (!session) {
          return json(dashboardSessionStatus(null))
        }

        try {
          const refreshed = await refreshDashboardSession(session)
          const headers =
            refreshed.accessToken === session.accessToken
              ? undefined
              : {
                  'set-cookie': createDashboardSessionCookie(
                    request,
                    refreshed,
                  ),
                }
          return json(dashboardSessionStatus(refreshed), { headers })
        } catch {
          return json(dashboardSessionStatus(null), {
            headers: {
              'set-cookie': clearDashboardSessionCookie(),
            },
          })
        }
      },
      POST: async ({ request }) => {
        const crossOriginFailure = rejectCrossOriginMutation(request)
        if (crossOriginFailure) {
          return crossOriginFailure
        }

        if (!isDashboardAdminConfigured()) {
          return json(
            {
              error:
                'Dashboard auth is not configured. Set SIDEREAL_DASHBOARD_SESSION_SECRET.',
            },
            { status: 503 },
          )
        }

        let body: SessionLoginBody
        try {
          body = (await request.json()) as SessionLoginBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        const mfaBody = dashboardMfaLoginSchema.safeParse(body)
        if (mfaBody.success) {
          try {
            const session = await createDashboardSessionFromTotpChallenge(
              mfaBody.data.challenge_id,
              mfaBody.data.code,
            )
            return json(dashboardSessionStatus(session), {
              headers: {
                'set-cookie': createDashboardSessionCookie(request, session),
              },
            })
          } catch (error) {
            return json(
              {
                error:
                  error instanceof Error
                    ? error.message
                    : 'MFA verification failed',
              },
              { status: 403 },
            )
          }
        }

        const registerBody = dashboardRegisterSchema.safeParse(body)
        if (registerBody.success) {
          try {
            const session = await createDashboardSessionFromRegistration(
              registerBody.data.email,
              registerBody.data.password,
            )
            return json(dashboardSessionStatus(session), {
              headers: {
                'set-cookie': createDashboardSessionCookie(request, session),
              },
            })
          } catch (error) {
            return json(
              {
                error:
                  error instanceof Error
                    ? error.message
                    : 'Registration failed',
              },
              { status: 403 },
            )
          }
        }

        const parsedBody = dashboardPasswordLoginSchema.safeParse(body)
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
          const result = await createDashboardSessionFromPassword(
            parsedBody.data.email,
            parsedBody.data.password,
          )
          if (result.status === 'mfa_required') {
            return json({
              authenticated: false,
              configured: true,
              mfaRequired: true,
              challengeId: result.challengeId,
              challengeType: result.challengeType,
              expiresInS: result.expiresInS,
            })
          }
          return json(dashboardSessionStatus(result.session), {
            headers: {
              'set-cookie': createDashboardSessionCookie(
                request,
                result.session,
              ),
            },
          })
        } catch (error) {
          return json(
            {
              error: error instanceof Error ? error.message : 'Login failed',
            },
            { status: 403 },
          )
        }
      },
      DELETE: ({ request }) => {
        const crossOriginFailure = rejectCrossOriginMutation(request)
        if (crossOriginFailure) {
          return crossOriginFailure
        }

        return json(dashboardSessionStatus(null), {
          headers: {
            'set-cookie': clearDashboardSessionCookie(),
          },
        })
      },
    },
  },
  component: () => null,
})

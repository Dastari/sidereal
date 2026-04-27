import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { dashboardSetupAdminSchema } from '@/lib/schemas/dashboard'
import {
  createDashboardSessionCookie,
  createDashboardSessionFromBootstrapAdmin,
  dashboardBootstrapStatus,
  dashboardSessionStatus,
  isDashboardAdminConfigured,
  rejectCrossOriginMutation,
} from '@/server/dashboard-auth'

type SetupBody = {
  email?: unknown
  password?: unknown
  setupToken?: unknown
}

export const Route = createFileRoute('/api/bootstrap')({
  server: {
    handlers: {
      GET: async () => {
        try {
          return json(await dashboardBootstrapStatus())
        } catch (error) {
          return json(
            {
              required: false,
              configured: false,
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to load bootstrap status',
            },
            { status: 502 },
          )
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

        let body: SetupBody
        try {
          body = (await request.json()) as SetupBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        const parsedBody = dashboardSetupAdminSchema.safeParse(body)
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
          const session = await createDashboardSessionFromBootstrapAdmin(
            parsedBody.data.email,
            parsedBody.data.password,
            parsedBody.data.setupToken,
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
                  : 'Failed to create first administrator',
            },
            { status: 403 },
          )
        }
      },
    },
  },
  component: () => null,
})

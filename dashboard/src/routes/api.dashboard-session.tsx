import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { dashboardSessionLoginSchema } from '@/lib/schemas/dashboard'
import {
  clearDashboardAdminSessionCookie,
  createDashboardAdminSessionCookie,
  getDashboardSession,
  isDashboardAdminConfigured,
  rejectCrossOriginMutation,
  verifyDashboardAdminPassword,
} from '@/server/dashboard-auth'

type LoginBody = {
  password?: unknown
}

export const Route = createFileRoute('/api/dashboard-session')({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const session = getDashboardSession(request)
        return json({
          authenticated: session?.role === 'admin',
          configured: isDashboardAdminConfigured(),
        })
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
                'Dashboard admin auth is not configured. Set SIDEREAL_DASHBOARD_ADMIN_PASSWORD.',
            },
            { status: 503 },
          )
        }

        let body: LoginBody
        try {
          body = (await request.json()) as LoginBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }
        const parsedBody = dashboardSessionLoginSchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            {
              error:
                parsedBody.error.issues[0]?.message ?? 'Invalid request body',
            },
            { status: 400 },
          )
        }

        if (!verifyDashboardAdminPassword(parsedBody.data.password)) {
          return json({ error: 'Invalid admin password' }, { status: 403 })
        }

        return json(
          { authenticated: true },
          {
            headers: {
              'set-cookie': createDashboardAdminSessionCookie(request),
            },
          },
        )
      },
      DELETE: async ({ request }) => {
        const crossOriginFailure = rejectCrossOriginMutation(request)
        if (crossOriginFailure) {
          return crossOriginFailure
        }

        return json(
          { authenticated: false },
          {
            headers: {
              'set-cookie': clearDashboardAdminSessionCookie(),
            },
          },
        )
      },
    },
  },
  component: () => null,
})

import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { loadShipyardCatalog } from '@/lib/shipyard.server'
import {
  getDashboardSession,
  requireDashboardAdmin,
} from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/shipyard/catalog')({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const authFailure = requireDashboardAdmin(request, 'scripts:read')
        if (authFailure) return authFailure
        const session = getDashboardSession(request)
        if (!session) {
          return json(
            { error: 'Dashboard account session required' },
            { status: 403 },
          )
        }

        try {
          return json(await loadShipyardCatalog(session.accessToken))
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : 'Failed to load Shipyard catalog'
          return json({ error: message }, { status: 500 })
        }
      },
    },
  },
  component: () => null,
})

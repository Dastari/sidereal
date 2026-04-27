import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import {
  createDatabaseAdminErrorPayload,
  loadDatabaseAdminPayload,
} from '@/server/database-admin'
import { requireDashboardAdmin } from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/database')({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const authFailure = requireDashboardAdmin(
          request,
          'dashboard:database:read',
        )
        if (authFailure) return authFailure

        try {
          return json(await loadDatabaseAdminPayload())
        } catch (error) {
          return json(createDatabaseAdminErrorPayload(error), { status: 500 })
        }
      },
    },
  },
  component: () => null,
})

import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { listShaderCatalog } from '@/lib/shader-workbench.server'
import { requireDashboardAdmin } from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/shaders')({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const authFailure = requireDashboardAdmin(request, 'scripts:read')
        if (authFailure) return authFailure

        try {
          const payload = await listShaderCatalog()
          return json(payload)
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown error'
          return json({ error: message }, { status: 500 })
        }
      },
    },
  },
  component: () => null,
})

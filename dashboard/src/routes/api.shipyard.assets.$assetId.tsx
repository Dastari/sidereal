import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { loadShipyardAssetBytes } from '@/lib/shipyard.server'
import {
  getDashboardSession,
  requireDashboardAdmin,
} from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/shipyard/assets/$assetId')({
  server: {
    handlers: {
      GET: async ({ params, request }) => {
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
          const asset = await loadShipyardAssetBytes(
            params.assetId,
            session.accessToken,
          )
          return new Response(asset.bytes, {
            headers: {
              'content-type': asset.contentType,
              'cache-control': 'no-store',
            },
          })
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : 'Failed to load Shipyard image asset'
          return json({ error: message }, { status: 404 })
        }
      },
    },
  },
  component: () => null,
})

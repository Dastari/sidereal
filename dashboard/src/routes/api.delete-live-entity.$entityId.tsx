import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { callBrp, type BrpTarget } from '@/server/brp'

function parseTarget(value: unknown): BrpTarget {
  return value === 'client' ? 'client' : 'server'
}

export const Route = createFileRoute('/api/delete-live-entity/$entityId')({
  server: {
    handlers: {
      DELETE: async ({ params, request }) => {
        const { entityId } = params
        const url = new URL(request.url)
        const target = parseTarget(url.searchParams.get('target'))
        if (target === 'client') {
          return json(
            {
              error: 'Deleting entities from client BRP is disabled',
              success: false,
            },
            { status: 400 },
          )
        }

        try {
          // Parse entity ID - BRP expects numeric entity index
          const entityIndex = parseInt(entityId, 10)
          if (isNaN(entityIndex)) {
            return json(
              {
                error: `Invalid entity ID: ${entityId} (must be numeric for BRP)`,
                success: false,
              },
              { status: 400 },
            )
          }

          // Call BRP to despawn the entity using world.despawn_entity
          const response = await callBrp({
            method: 'world.despawn_entity',
            params: {
              entity: entityIndex,
            },
          }, target)

          if (response.error) {
            return json(
              {
                error: `BRP error (${response.error.code}): ${response.error.message}`,
                success: false,
              },
              { status: 500 },
            )
          }

          return json({ success: true, entityId })
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown error'
          return json({ error: message, success: false }, { status: 502 })
        }
      },
    },
  },
  component: () => null,
})

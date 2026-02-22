import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { getLiveWorldSnapshot } from '@/server/brp'

export const Route = createFileRoute('/api/live-client-world')({
  server: {
    handlers: {
      GET: async () => {
        try {
          const snapshot = await getLiveWorldSnapshot('client')
          return json(snapshot)
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown error'
          return json({ error: message }, { status: 502 })
        }
      },
    },
  },
  component: () => null,
})

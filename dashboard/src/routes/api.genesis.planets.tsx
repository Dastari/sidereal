import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { loadGenesisPlanetCatalog } from '@/lib/genesis.server'

export const Route = createFileRoute('/api/genesis/planets')({
  server: {
    handlers: {
      GET: async () => {
        try {
          return json(await loadGenesisPlanetCatalog())
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : 'Failed to load Genesis planet catalog'
          return json({ error: message }, { status: 500 })
        }
      },
    },
  },
  component: () => null,
})

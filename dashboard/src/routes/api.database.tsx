import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import {
  createDatabaseAdminErrorPayload,
  loadDatabaseAdminPayload,
} from '@/server/database-admin'

export const Route = createFileRoute('/api/database')({
  server: {
    handlers: {
      GET: async () => {
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

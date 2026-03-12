import { createServerFn } from '@tanstack/react-start'
import { loadDatabaseAdminPayload } from '@/server/database-admin'

export const loadDatabaseAdminData = createServerFn({
  method: 'GET',
}).handler(async () => {
  return loadDatabaseAdminPayload()
})

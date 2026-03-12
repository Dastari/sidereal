import { createFileRoute } from '@tanstack/react-router'
import { loadDatabaseAdminData } from '@/lib/server-fns/database-admin'
import { DatabaseEntitiesPage } from '@/routes-lazy/database-pages'

export const Route = createFileRoute('/_dashboard/database/')({
  loader: () => loadDatabaseAdminData(),
  component: DatabaseEntitiesIndexPage,
})

function DatabaseEntitiesIndexPage() {
  const initialData = Route.useLoaderData()

  return (
    <DatabaseEntitiesPage selectedEntityGuid={null} initialData={initialData} />
  )
}

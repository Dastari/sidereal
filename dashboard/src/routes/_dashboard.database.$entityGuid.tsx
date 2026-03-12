import { createFileRoute } from '@tanstack/react-router'
import { loadDatabaseAdminData } from '@/lib/server-fns/database-admin'
import { DatabaseEntitiesPage } from '@/routes-lazy/database-pages'

export const Route = createFileRoute('/_dashboard/database/$entityGuid')({
  loader: () => loadDatabaseAdminData(),
  component: DatabaseEntityRoutePage,
})

function DatabaseEntityRoutePage() {
  const { entityGuid } = Route.useParams()
  const initialData = Route.useLoaderData()

  return (
    <DatabaseEntitiesPage
      selectedEntityGuid={entityGuid}
      initialData={initialData}
    />
  )
}

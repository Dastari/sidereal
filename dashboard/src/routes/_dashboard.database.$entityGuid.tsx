import { createFileRoute } from '@tanstack/react-router'
import { DatabaseEntitiesPage } from './_dashboard.database'

export const Route = createFileRoute('/_dashboard/database/$entityGuid')({
  component: DatabaseEntityRoutePage,
})

function DatabaseEntityRoutePage() {
  const { entityGuid } = Route.useParams()
  return <DatabaseEntitiesPage selectedEntityGuid={entityGuid} />
}

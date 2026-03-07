import { createFileRoute } from '@tanstack/react-router'
import { DatabaseToolPage } from './_dashboard.database'

export const Route = createFileRoute('/_dashboard/database/$entityGuid')({
  component: DatabaseEntityRoutePage,
})

function DatabaseEntityRoutePage() {
  const { entityGuid } = Route.useParams()
  return <DatabaseToolPage selectedEntityGuid={entityGuid} />
}

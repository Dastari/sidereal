import { createFileRoute } from '@tanstack/react-router'
import { DatabaseEntitiesPage } from './_dashboard.database'

export const Route = createFileRoute('/_dashboard/database/')({
  component: DatabaseEntitiesIndexPage,
})

function DatabaseEntitiesIndexPage() {
  return <DatabaseEntitiesPage selectedEntityGuid={null} />
}

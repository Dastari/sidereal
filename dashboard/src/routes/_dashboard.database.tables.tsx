import { createFileRoute } from '@tanstack/react-router'
import { DatabaseTablesPage } from './_dashboard.database'

export const Route = createFileRoute('/_dashboard/database/tables')({
  component: DatabaseTablesPage,
})

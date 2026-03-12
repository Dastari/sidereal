import { createFileRoute } from '@tanstack/react-router'
import { databaseTablesSearchSchema } from '@/lib/schemas/dashboard'
import { loadDatabaseAdminData } from '@/lib/server-fns/database-admin'
import { DatabaseTablesPage } from '@/routes-lazy/database-pages'

export const Route = createFileRoute('/_dashboard/database/tables')({
  validateSearch: (search: Record<string, unknown>) =>
    databaseTablesSearchSchema.parse(search),
  loaderDeps: ({ search }) => databaseTablesSearchSchema.parse(search),
  loader: () => loadDatabaseAdminData(),
  component: DatabaseTablesRoutePage,
})

function DatabaseTablesRoutePage() {
  const initialData = Route.useLoaderData()

  return <DatabaseTablesPage initialData={initialData} />
}

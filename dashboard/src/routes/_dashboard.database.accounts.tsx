import { createFileRoute } from '@tanstack/react-router'
import { databaseAccountsSearchSchema } from '@/lib/schemas/dashboard'
import { loadDatabaseAdminData } from '@/lib/server-fns/database-admin'
import { DatabaseAccountsPage } from '@/routes-lazy/database-pages'

export const Route = createFileRoute('/_dashboard/database/accounts')({
  validateSearch: (search: Record<string, unknown>) =>
    databaseAccountsSearchSchema.parse(search),
  loaderDeps: ({ search }) => databaseAccountsSearchSchema.parse(search),
  loader: () => loadDatabaseAdminData(),
  component: DatabaseAccountsRoutePage,
})

function DatabaseAccountsRoutePage() {
  const initialData = Route.useLoaderData()

  return <DatabaseAccountsPage initialData={initialData} />
}

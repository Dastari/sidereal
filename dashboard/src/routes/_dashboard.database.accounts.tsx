import { createFileRoute } from '@tanstack/react-router'
import { DatabaseAccountsPage } from './_dashboard.database'

export const Route = createFileRoute('/_dashboard/database/accounts')({
  component: DatabaseAccountsPage,
})

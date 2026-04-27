import { createFileRoute } from '@tanstack/react-router'
import { DashboardShell } from '@/components/layout/DashboardShell'
import { requireDashboardRoute } from '@/lib/dashboard-auth'

export const Route = createFileRoute('/_dashboard')({
  beforeLoad: async ({ location }) => {
    await requireDashboardRoute(location.href, location.pathname)
  },
  component: DashboardShell,
})

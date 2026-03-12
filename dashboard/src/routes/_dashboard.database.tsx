import { Outlet, createFileRoute } from '@tanstack/react-router'
import { RouteErrorState } from '@/components/feedback/route-feedback'

export const Route = createFileRoute('/_dashboard/database')({
  errorComponent: ({ error }) => (
    <RouteErrorState title="Database route failed" error={error} />
  ),
  component: DatabaseRouteLayout,
})

function DatabaseRouteLayout() {
  return <Outlet />
}

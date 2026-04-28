import { createFileRoute } from '@tanstack/react-router'
import { RouteErrorState } from '@/components/feedback/route-feedback'
import { ShipyardToolPage } from '@/routes-lazy/shipyard-route'

export const Route = createFileRoute('/_dashboard/shipyard')({
  errorComponent: ({ error }) => (
    <RouteErrorState title="Shipyard route failed" error={error} />
  ),
  component: ShipyardToolPage,
})

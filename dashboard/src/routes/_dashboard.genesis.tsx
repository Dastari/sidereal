import { createFileRoute } from '@tanstack/react-router'
import { RouteErrorState } from '@/components/feedback/route-feedback'
import { GenesisToolPage } from '@/routes-lazy/genesis-route'

export const Route = createFileRoute('/_dashboard/genesis')({
  errorComponent: ({ error }) => (
    <RouteErrorState title="Genesis route failed" error={error} />
  ),
  component: GenesisToolPage,
})

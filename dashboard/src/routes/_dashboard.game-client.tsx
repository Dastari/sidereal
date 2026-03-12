import { createFileRoute } from '@tanstack/react-router'
import { RouteErrorState } from '@/components/feedback/route-feedback'
import { GameClientPage } from '@/routes-lazy/game-client-route'

export const Route = createFileRoute('/_dashboard/game-client')({
  errorComponent: ({ error }) => (
    <RouteErrorState title="Game client route failed" error={error} />
  ),
  component: GameClientRoutePage,
})

function GameClientRoutePage() {
  return <GameClientPage />
}

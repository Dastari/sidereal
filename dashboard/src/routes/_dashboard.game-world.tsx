import { createFileRoute } from '@tanstack/react-router'
import { RouteErrorState } from '@/components/feedback/route-feedback'
import { GameWorldToolPage } from '@/routes-lazy/game-world-route'

export const Route = createFileRoute('/_dashboard/game-world')({
  errorComponent: ({ error }) => (
    <RouteErrorState title="Game world route failed" error={error} />
  ),
  component: GameWorldRoutePage,
})

function GameWorldRoutePage() {
  return <GameWorldToolPage selectedEntityGuid={null} />
}

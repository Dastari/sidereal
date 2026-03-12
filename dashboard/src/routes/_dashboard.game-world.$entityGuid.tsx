import { createFileRoute } from '@tanstack/react-router'
import { GameWorldToolPage } from '@/routes-lazy/game-world-route'

export const Route = createFileRoute('/_dashboard/game-world/$entityGuid')({
  component: GameWorldEntityRoutePage,
})

function GameWorldEntityRoutePage() {
  const { entityGuid } = Route.useParams()
  return <GameWorldToolPage selectedEntityGuid={entityGuid} />
}

import { createFileRoute } from '@tanstack/react-router'
import { GameWorldToolPage } from './_dashboard.game-world'

export const Route = createFileRoute('/_dashboard/game-world/$entityGuid')({
  component: GameWorldEntityRoutePage,
})

function GameWorldEntityRoutePage() {
  const { entityGuid } = Route.useParams()
  return <GameWorldToolPage selectedEntityGuid={entityGuid} />
}

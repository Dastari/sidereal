import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ExplorerWorkspace } from '@/features/explorer/ExplorerWorkspace'

export const Route = createFileRoute('/_dashboard/game-world')({
  component: GameWorldRoutePage,
})

export function GameWorldRoutePage() {
  return <GameWorldToolPage selectedEntityGuid={null} />
}

export function GameWorldToolPage({
  selectedEntityGuid,
}: {
  selectedEntityGuid: string | null
}) {
  const navigate = useNavigate()

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1">
        <ExplorerWorkspace
          scope="gameWorld"
          selectedEntityGuid={selectedEntityGuid}
          onSelectedEntityGuidChange={(entityGuid) => {
            void navigate({
              to: entityGuid ? '/game-world/$entityGuid' : '/game-world',
              params: entityGuid ? { entityGuid } : {},
              search: (prev) => prev,
              replace: true,
            })
          }}
        />
      </div>
    </div>
  )
}

import { createFileRoute } from '@tanstack/react-router'
import { loadAudioStudioData } from '@/lib/server-fns/audio-studio'
import { SoundStudioToolPage } from '@/routes-lazy/sound-studio-route'

export const Route = createFileRoute('/_dashboard/sound-studio/$soundId')({
  loader: () => loadAudioStudioData(),
  component: SoundStudioEntityRoutePage,
})

function SoundStudioEntityRoutePage() {
  const initialData = Route.useLoaderData()
  const { soundId } = Route.useParams()
  return (
    <SoundStudioToolPage
      initialData={initialData}
      selectedSoundId={soundId}
    />
  )
}

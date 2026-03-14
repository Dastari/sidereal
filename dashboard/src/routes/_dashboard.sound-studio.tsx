import { createFileRoute } from '@tanstack/react-router'
import { RouteErrorState } from '@/components/feedback/route-feedback'
import { loadAudioStudioData } from '@/lib/server-fns/audio-studio'
import { SoundStudioToolPage } from '@/routes-lazy/sound-studio-route'

export const Route = createFileRoute('/_dashboard/sound-studio')({
  loader: () => loadAudioStudioData(),
  errorComponent: ({ error }) => (
    <RouteErrorState title="Sound studio route failed" error={error} />
  ),
  component: SoundStudioRoutePage,
})

function SoundStudioRoutePage() {
  const initialData = Route.useLoaderData()
  return <SoundStudioToolPage initialData={initialData} selectedSoundId={null} />
}

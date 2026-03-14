import { useNavigate } from '@tanstack/react-router'
import type { AudioStudioCatalog } from '@/features/audio-studio/types'
import { SoundStudioPage } from '@/features/audio-studio/SoundStudioPage'

export function SoundStudioToolPage({
  initialData,
  selectedSoundId,
}: {
  initialData: AudioStudioCatalog
  selectedSoundId: string | null
}) {
  const navigate = useNavigate()

  return (
    <SoundStudioPage
      initialData={initialData}
      selectedSoundId={selectedSoundId}
      onSelectedSoundIdChange={(soundId) => {
        void navigate({
          to: soundId ? '/sound-studio/$soundId' : '/sound-studio',
          params: soundId ? { soundId } : {},
          search: (prev) => prev,
          replace: true,
        })
      }}
    />
  )
}

import { createServerFn } from '@tanstack/react-start'
import { loadAudioStudioCatalog } from '@/lib/audio-studio.server'

export const loadAudioStudioData = createServerFn({
  method: 'GET',
}).handler(async () => {
  return loadAudioStudioCatalog()
})

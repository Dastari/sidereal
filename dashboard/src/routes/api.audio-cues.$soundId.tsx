import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import {
  loadAudioCueAssetBytes,
  saveAudioCueMarkers,
} from '@/lib/audio-studio.server'
import {
  audioStudioMarkerBodySchema,
  audioStudioParamsSchema,
} from '@/lib/schemas/dashboard'
import { requireDashboardAdmin } from '@/server/dashboard-auth'

export const Route = createFileRoute('/api/audio-cues/$soundId')({
  server: {
    handlers: {
      GET: async ({ request, params }) => {
        const authFailure = requireDashboardAdmin(request, 'scripts:read')
        if (authFailure) {
          return authFailure
        }

        const parsedParams = audioStudioParamsSchema.safeParse(params)
        if (!parsedParams.success) {
          return json(
            {
              error:
                parsedParams.error.issues[0]?.message ?? 'Invalid sound id',
            },
            { status: 400 },
          )
        }

        try {
          const payload = await loadAudioCueAssetBytes(
            parsedParams.data.soundId,
          )
          return new Response(new Uint8Array(payload.bytes), {
            status: 200,
            headers: {
              'content-type': payload.contentType,
              'cache-control': 'no-store',
              'content-length': payload.bytes.byteLength.toString(),
            },
          })
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown error'
          const status = message.includes('not found') ? 404 : 400
          return json({ error: message }, { status })
        }
      },
      POST: async ({ request, params }) => {
        const authFailure = requireDashboardAdmin(request, 'scripts:write')
        if (authFailure) {
          return authFailure
        }

        const parsedParams = audioStudioParamsSchema.safeParse(params)
        if (!parsedParams.success) {
          return json(
            {
              error:
                parsedParams.error.issues[0]?.message ?? 'Invalid sound id',
            },
            { status: 400 },
          )
        }

        let body: unknown
        try {
          body = await request.json()
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        const parsedBody = audioStudioMarkerBodySchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            {
              error:
                parsedBody.error.issues[0]?.message ?? 'Invalid marker payload',
            },
            { status: 400 },
          )
        }

        try {
          const entry = await saveAudioCueMarkers(
            parsedParams.data.soundId,
            parsedBody.data,
          )
          return json({ entry })
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Failed to save markers'
          const status = message.includes('not found') ? 404 : 400
          return json({ error: message }, { status })
        }
      },
    },
  },
  component: () => null,
})

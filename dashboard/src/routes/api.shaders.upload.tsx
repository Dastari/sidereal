import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { uploadShaderFile } from '@/lib/shader-workbench.server'
import { requireDashboardAdmin } from '@/server/dashboard-auth'

type UploadShaderBody = {
  filename?: unknown
  source?: unknown
}

export const Route = createFileRoute('/api/shaders/upload')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const authFailure = requireDashboardAdmin(request, 'scripts:write')
        if (authFailure) return authFailure

        let body: UploadShaderBody
        try {
          body = (await request.json()) as UploadShaderBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        if (
          typeof body.filename !== 'string' ||
          body.filename.trim().length === 0
        ) {
          return json({ error: 'filename is required' }, { status: 400 })
        }
        if (typeof body.source !== 'string') {
          return json({ error: 'source must be a string' }, { status: 400 })
        }

        try {
          const payload = await uploadShaderFile(body.filename, body.source)
          return json(payload)
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown error'
          return json({ error: message }, { status: 400 })
        }
      },
    },
  },
  component: () => null,
})

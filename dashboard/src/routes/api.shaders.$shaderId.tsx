import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { loadShaderFile } from '@/lib/shader-workbench.server'

export const Route = createFileRoute('/api/shaders/$shaderId')({
  server: {
    handlers: {
      GET: async ({ params }) => {
        try {
          const payload = await loadShaderFile(params.shaderId)
          return json(payload)
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown error'
          const status = message.includes('Invalid shader id') ? 400 : 404
          return json({ error: message }, { status })
        }
      },
    },
  },
  component: () => null,
})

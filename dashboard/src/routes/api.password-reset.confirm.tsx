import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { publicPasswordResetConfirmSchema } from '@/lib/schemas/dashboard'
import { rejectCrossOriginMutation } from '@/server/dashboard-auth'

type PasswordResetConfirmBody = {
  resetToken?: unknown
  newPassword?: unknown
}

function parseGatewayUrl(): string {
  const raw = process.env.GATEWAY_API_URL?.trim() || 'http://127.0.0.1:8080'
  return raw.endsWith('/') ? raw.slice(0, -1) : raw
}

export const Route = createFileRoute('/api/password-reset/confirm')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const crossOriginFailure = rejectCrossOriginMutation(request)
        if (crossOriginFailure) return crossOriginFailure

        let body: PasswordResetConfirmBody
        try {
          body = (await request.json()) as PasswordResetConfirmBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        const parsedBody = publicPasswordResetConfirmSchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            {
              error:
                parsedBody.error.issues[0]?.message ?? 'Invalid request body',
            },
            { status: 400 },
          )
        }

        const response = await fetch(
          `${parseGatewayUrl()}/auth/v1/password-reset/confirm`,
          {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({
              reset_token: parsedBody.data.resetToken,
              new_password: parsedBody.data.newPassword,
            }),
          },
        )
        const payload = (await response.json().catch(() => ({}))) as Record<
          string,
          unknown
        >
        if (!response.ok) {
          return json(
            {
              error:
                typeof payload.error === 'string'
                  ? payload.error
                  : `gateway request failed with status ${response.status}`,
            },
            { status: response.status },
          )
        }

        return json({ accepted: payload.accepted === true })
      },
    },
  },
  component: () => null,
})

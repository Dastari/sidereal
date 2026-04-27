import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { accountCharacterCreateSchema } from '@/lib/schemas/dashboard'
import {
  createDashboardCharacter,
  createDashboardSessionCookie,
  dashboardSessionStatus,
  getDashboardSession,
  isDashboardAdminConfigured,
  loadDashboardCharacters,
  refreshDashboardSession,
  rejectCrossOriginMutation,
} from '@/server/dashboard-auth'

type CharacterBody = {
  displayName?: unknown
}

export const Route = createFileRoute('/api/account/characters')({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const prepared = await prepareAccountSession(request)
        if (prepared instanceof Response) return prepared

        try {
          return json(await loadDashboardCharacters(prepared.session), {
            headers: prepared.headers,
          })
        } catch (error) {
          return json(
            {
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to load characters',
            },
            { status: 502, headers: prepared.headers },
          )
        }
      },
      POST: async ({ request }) => {
        const crossOriginFailure = rejectCrossOriginMutation(request)
        if (crossOriginFailure) return crossOriginFailure

        const prepared = await prepareAccountSession(request)
        if (prepared instanceof Response) return prepared

        let body: CharacterBody
        try {
          body = (await request.json()) as CharacterBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        const parsedBody = accountCharacterCreateSchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            {
              error:
                parsedBody.error.issues[0]?.message ?? 'Invalid request body',
            },
            { status: 400 },
          )
        }

        try {
          return json(
            await createDashboardCharacter(
              prepared.session,
              parsedBody.data.displayName,
            ),
            { headers: prepared.headers },
          )
        } catch (error) {
          return json(
            {
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to create character',
            },
            { status: 403, headers: prepared.headers },
          )
        }
      },
    },
  },
  component: () => null,
})

async function prepareAccountSession(request: Request): Promise<
  | Response
  | {
      session: NonNullable<ReturnType<typeof getDashboardSession>>
      headers?: HeadersInit
    }
> {
  if (!isDashboardAdminConfigured()) {
    return json(
      {
        error:
          'Dashboard auth is not configured. Set SIDEREAL_DASHBOARD_SESSION_SECRET.',
      },
      { status: 503 },
    )
  }

  const session = getDashboardSession(request)
  if (!session) {
    return json(dashboardSessionStatus(null), { status: 403 })
  }

  try {
    const refreshed = await refreshDashboardSession(session)
    return {
      session: refreshed,
      headers:
        refreshed.accessToken === session.accessToken
          ? undefined
          : {
              'set-cookie': createDashboardSessionCookie(request, refreshed),
            },
    }
  } catch {
    return json(dashboardSessionStatus(null), { status: 403 })
  }
}

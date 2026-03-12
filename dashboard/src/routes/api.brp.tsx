import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import {
  brpPortSchema,
  brpRequestSchema,
  brpTargetSchema,
} from '@/lib/schemas/dashboard'
import { requireDashboardAdmin } from '@/server/dashboard-auth'
import type { BrpTarget } from '@/server/brp'
import { callBrp, getBrpUrl, getLiveWorldSnapshot } from '@/server/brp'

type BrpRequestBody = {
  id?: unknown
  method?: unknown
  params?: unknown
  target?: unknown
  port?: unknown
}

function parseTarget(value: unknown): BrpTarget {
  const parsed = brpTargetSchema.safeParse(value)
  return parsed.success ? parsed.data : 'server'
}

function parsePort(value: unknown): number | null {
  const parsed = brpPortSchema.safeParse(value)
  return parsed.success ? parsed.data : null
}

export const Route = createFileRoute('/api/brp')({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const url = new URL(request.url)
        const snapshot = url.searchParams.get('snapshot')
        const target = parseTarget(url.searchParams.get('target'))
        const port = parsePort(url.searchParams.get('port'))
        const options = { target, ...(port ? { port } : {}) }

        if (snapshot === '1' || snapshot === 'true') {
          try {
            const live = await getLiveWorldSnapshot(options)
            return json(live)
          } catch (error) {
            const message =
              error instanceof Error ? error.message : 'Unknown error'
            return json({ error: message }, { status: 502 })
          }
        }
        try {
          const discover = await callBrp(
            {
              id: `${target}-discover`,
              method: 'rpc.discover',
            },
            options,
          )

          return json({
            ok: !discover.error,
            target,
            brpUrl: getBrpUrl(options),
            discover: discover.result ?? null,
            error: discover.error ?? null,
          })
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown error'
          return json(
            {
              ok: false,
              target,
              brpUrl: getBrpUrl(options),
              error: message,
            },
            { status: 502 },
          )
        }
      },
      POST: async ({ request }) => {
        const authFailure = requireDashboardAdmin(request)
        if (authFailure) {
          return authFailure
        }

        let body: BrpRequestBody
        try {
          body = (await request.json()) as BrpRequestBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        const parsedBody = brpRequestSchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            {
              error:
                parsedBody.error.issues[0]?.message ?? 'Invalid request body',
            },
            { status: 400 },
          )
        }
        const url = new URL(request.url)
        const target = parseTarget(
          parsedBody.data.target ?? url.searchParams.get('target'),
        )
        const port = parsePort(
          parsedBody.data.port ?? url.searchParams.get('port'),
        )
        const options = { target, ...(port ? { port } : {}) }

        try {
          const response = await callBrp(
            {
              id: parsedBody.data.id,
              method: parsedBody.data.method,
              params: parsedBody.data.params,
            },
            options,
          )
          return json({
            target,
            brpUrl: getBrpUrl(options),
            ...response,
          })
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown error'
          return json(
            {
              error: message,
              target,
              brpUrl: getBrpUrl(options),
            },
            { status: 502 },
          )
        }
      },
    },
  },
  component: () => null,
})

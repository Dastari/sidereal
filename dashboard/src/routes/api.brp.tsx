import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
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
  if (value === 'hostClient') return 'hostClient'
  return value === 'client' ? 'client' : 'server'
}

function parsePort(value: unknown): number | null {
  if (typeof value === 'number' && Number.isInteger(value)) {
    return value >= 1 && value <= 65535 ? value : null
  }
  if (typeof value === 'string' && value.trim().length > 0) {
    const parsed = Number.parseInt(value, 10)
    if (Number.isInteger(parsed) && parsed >= 1 && parsed <= 65535) {
      return parsed
    }
  }
  return null
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
          const discover = await callBrp({
            id: `${target}-discover`,
            method: 'rpc.discover',
          }, options)

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
        let body: BrpRequestBody
        try {
          body = (await request.json()) as BrpRequestBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }

        if (typeof body.method !== 'string' || body.method.length === 0) {
          return json(
            { error: 'Body must include a JSON-RPC method string' },
            { status: 400 },
          )
        }
        const url = new URL(request.url)
        const target = parseTarget(body.target ?? url.searchParams.get('target'))
        const port = parsePort(body.port ?? url.searchParams.get('port'))
        const options = { target, ...(port ? { port } : {}) }

        try {
          const response = await callBrp({
            id: body.id,
            method: body.method,
            params: body.params,
          }, options)
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

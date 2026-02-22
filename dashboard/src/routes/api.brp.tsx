import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { callBrp, getBrpUrl, type BrpTarget } from '@/server/brp'

type BrpRequestBody = {
  id?: unknown
  method?: unknown
  params?: unknown
  target?: unknown
}

function parseTarget(value: unknown): BrpTarget {
  return value === 'client' ? 'client' : 'server'
}

export const Route = createFileRoute('/api/brp')({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const url = new URL(request.url)
        const target = parseTarget(url.searchParams.get('target'))
        try {
          const discover = await callBrp({
            id: `${target}-discover`,
            method: 'rpc.discover',
          }, target)

          return json({
            ok: !discover.error,
            target,
            brpUrl: getBrpUrl(target),
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
              brpUrl: getBrpUrl(target),
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
        const target = parseTarget(body.target)

        try {
          const response = await callBrp({
            id: body.id,
            method: body.method,
            params: body.params,
          }, target)
          return json({
            target,
            brpUrl: getBrpUrl(target),
            ...response,
          })
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown error'
          return json(
            {
              error: message,
              target,
              brpUrl: getBrpUrl(target),
            },
            { status: 502 },
          )
        }
      },
    },
  },
  component: () => null,
})

import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { callBrp, getBrpUrl, type BrpTarget } from '@/server/brp'

type ComponentMap = Record<string, unknown>

function parseTarget(value: unknown): BrpTarget {
  return value === 'client' ? 'client' : 'server'
}

export const Route = createFileRoute('/api/live-entity/$entityId')({
  server: {
    handlers: {
      GET: async ({ params, request }) => {
        const entityIdRaw = params.entityId
        const entityId = Number(entityIdRaw)
        const url = new URL(request.url)
        const target = parseTarget(url.searchParams.get('target'))
        if (!Number.isFinite(entityId)) {
          return json({ error: 'Entity ID must be numeric' }, { status: 400 })
        }

        try {
          const listRes = await callBrp({
            method: 'world.list_components',
            params: { entity: entityId },
          }, target)
          if (listRes.error) {
            return json(
              {
                error: `world.list_components failed (${listRes.error.code}): ${listRes.error.message}`,
                target,
                brpUrl: getBrpUrl(target),
              },
              { status: 502 },
            )
          }

          const componentNames = Array.isArray(listRes.result)
            ? listRes.result.filter((value) => typeof value === 'string')
            : []

          const getRes = await callBrp({
            method: 'world.get_components',
            params: {
              entity: entityId,
              components: componentNames,
              strict: false,
            },
          }, target)
          if (getRes.error) {
            return json(
              {
                error: `world.get_components failed (${getRes.error.code}): ${getRes.error.message}`,
                target,
                brpUrl: getBrpUrl(target),
              },
              { status: 502 },
            )
          }

          const resultObj =
            getRes.result && typeof getRes.result === 'object'
              ? (getRes.result as Record<string, unknown>)
              : {}
          const components =
            resultObj.components && typeof resultObj.components === 'object'
              ? (resultObj.components as ComponentMap)
              : {}
          const errors =
            resultObj.errors && typeof resultObj.errors === 'object'
              ? (resultObj.errors as ComponentMap)
              : {}

          return json({
            source: 'bevy_remote',
            target,
            brpUrl: getBrpUrl(target),
            entity: entityId,
            componentCount: componentNames.length,
            components,
            errors,
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

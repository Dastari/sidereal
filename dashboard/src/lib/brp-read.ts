export type BrpReadTarget = 'server' | 'client' | 'hostClient'

const READ_ONLY_BRP_METHODS = new Set([
  'world.list_resources',
  'world.get_resource',
  'world.get_resources',
] as const)

type ReadOnlyBrpMethod =
  | 'world.list_resources'
  | 'world.get_resource'
  | 'world.get_resources'

function asRecord(value: unknown): Record<string, unknown> | null {
  if (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    Object.getPrototypeOf(value) === Object.prototype
  ) {
    return value as Record<string, unknown>
  }
  return null
}

export function isReadOnlyBrpMethod(
  method: string,
): method is ReadOnlyBrpMethod {
  return READ_ONLY_BRP_METHODS.has(method as ReadOnlyBrpMethod)
}

export function getBrpReadResourceParam(params: unknown): string | null {
  const record = asRecord(params)
  const resource = record?.resource
  return typeof resource === 'string' && resource.length > 0 ? resource : null
}

export function buildBrpReadUrl(options: {
  method: string
  params?: unknown
  port: number
  target: BrpReadTarget
}): string | null {
  if (!isReadOnlyBrpMethod(options.method)) {
    return null
  }

  const query = new URLSearchParams({
    port: String(options.port),
    target: options.target,
    method: options.method,
  })

  if (options.method !== 'world.list_resources') {
    const resource = getBrpReadResourceParam(options.params)
    if (!resource) {
      return null
    }
    query.set('resource', resource)
  }

  return `/api/brp?${query.toString()}`
}

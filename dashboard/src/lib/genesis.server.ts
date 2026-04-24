import type {
  GenesisPlanetCatalog,
  GenesisPlanetEntry,
} from '@/features/genesis/types'

type ScriptDetailResponse = {
  script_path?: string
  active_source?: string
  draft_source?: string | null
}

function parseGatewayUrl(): string {
  const raw = process.env.GATEWAY_API_URL?.trim() || 'http://127.0.0.1:8080'
  return raw.endsWith('/') ? raw.slice(0, -1) : raw
}

function parseBearerToken(): string {
  const token = process.env.SIDEREAL_DASHBOARD_ADMIN_BEARER_TOKEN?.trim()
  if (!token) {
    throw new Error('SIDEREAL_DASHBOARD_ADMIN_BEARER_TOKEN is not configured')
  }
  return token
}

async function fetchScriptDetail(scriptPath: string): Promise<ScriptDetailResponse> {
  const response = await fetch(
    `${parseGatewayUrl()}/admin/scripts/detail/${scriptPath}`,
    {
      headers: {
        authorization: `Bearer ${parseBearerToken()}`,
      },
    },
  )
  const payload = (await response.json().catch(() => ({}))) as Record<
    string,
    unknown
  >
  if (!response.ok) {
    throw new Error(
      typeof payload.error === 'string'
        ? payload.error
        : `gateway request failed with status ${response.status}`,
    )
  }
  return payload as ScriptDetailResponse
}

function parseLuaStringList(raw: string): Array<string> {
  return (raw.match(/"([^"]+)"/g) ?? []).map((value) => value.slice(1, -1))
}

function parseRegistryEntries(source: string): Array<{
  planetId: string
  scriptPath: string
  spawnEnabled: boolean
  tags: Array<string>
}> {
  const entries: Array<{
    planetId: string
    scriptPath: string
    spawnEnabled: boolean
    tags: Array<string>
  }> = []
  const entryPattern = /\{\s*planet_id\s*=\s*"([^"]+)"[\s\S]*?script\s*=\s*"([^"]+)"[\s\S]*?\}/g
  for (const match of source.matchAll(entryPattern)) {
    const block = match[0]
    const tagsBlock = block.match(/tags\s*=\s*\{([\s\S]*?)\}/)?.[1] ?? ''
    entries.push({
      planetId: match[1],
      scriptPath: match[2],
      spawnEnabled: block.match(/spawn_enabled\s*=\s*true/) !== null,
      tags: parseLuaStringList(tagsBlock),
    })
  }
  return entries.filter((entry) => entry.planetId && entry.scriptPath)
}

function parseNumberField(source: string, field: string): number | null {
  const match = source.match(new RegExp(`${field}\\s*=\\s*(-?\\d+(?:\\.\\d+)?)`))
  return match ? Number(match[1]) : null
}

function parseStringField(source: string, field: string): string | null {
  return source.match(new RegExp(`${field}\\s*=\\s*"([^"]+)"`))?.[1] ?? null
}

export async function loadGenesisPlanetCatalog(): Promise<GenesisPlanetCatalog> {
  const registry = await fetchScriptDetail('planets/registry.lua')
  const registrySource = registry.draft_source ?? registry.active_source ?? ''
  const registryEntries = parseRegistryEntries(registrySource)
  const entries = await Promise.all(
    registryEntries.map(async (entry): Promise<GenesisPlanetEntry> => {
      const detail = await fetchScriptDetail(entry.scriptPath)
      const source = detail.draft_source ?? detail.active_source ?? ''
      return {
        planetId: entry.planetId,
        scriptPath: entry.scriptPath,
        displayName: parseStringField(source, 'display_name') ?? entry.planetId,
        bodyKind: parseNumberField(source, 'body_kind'),
        planetType: parseNumberField(source, 'planet_type'),
        seed: parseNumberField(source, 'seed'),
        spawnEnabled: entry.spawnEnabled,
        tags: entry.tags,
        hasDraft: typeof detail.draft_source === 'string',
      }
    }),
  )
  return {
    entries,
    registryHasDraft: typeof registry.draft_source === 'string',
  }
}

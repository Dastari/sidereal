import path from 'node:path'
import { promises as fs } from 'node:fs'
import type {
  ShipyardAssetEntry,
  ShipyardCatalog,
  ShipyardModuleDefinition,
  ShipyardModuleDraftRequest,
  ShipyardModuleEntry,
  ShipyardShipDefinition,
  ShipyardShipDraftRequest,
  ShipyardShipEntry,
} from '@/features/shipyard/types'

type ScriptDetailResponse = {
  script_path?: string
  active_source?: string
  draft_source?: string | null
}

type ShipRegistryEntry = {
  shipId: string
  bundleId: string
  scriptPath: string
  spawnEnabled: boolean
  tags: Array<string>
}

type ModuleRegistryEntry = {
  moduleId: string
  scriptPath: string
  tags: Array<string>
}

const SHIP_REGISTRY_SCRIPT_PATH = 'ships/registry.lua'
const MODULE_REGISTRY_SCRIPT_PATH = 'ship_modules/registry.lua'
const ASSET_REGISTRY_SCRIPT_PATH = 'assets/registry.lua'

function parseGatewayUrl(): string {
  const raw = process.env.GATEWAY_API_URL?.trim() || 'http://127.0.0.1:8080'
  return raw.endsWith('/') ? raw.slice(0, -1) : raw
}

async function pathExists(targetPath: string): Promise<boolean> {
  try {
    await fs.access(targetPath)
    return true
  } catch {
    return false
  }
}

async function resolveRepoRoot(): Promise<string> {
  const candidates = [
    process.env.SIDEREAL_REPO_ROOT?.trim(),
    process.cwd(),
    path.resolve(process.cwd(), '..'),
  ].filter((value): value is string => Boolean(value && value.length > 0))

  for (const candidate of candidates) {
    const repoRoot = path.resolve(candidate)
    if (await pathExists(path.join(repoRoot, 'data', 'scripts'))) {
      return repoRoot
    }
  }

  throw new Error('Unable to locate repository root for Shipyard disk fallback')
}

async function resolveScriptsRoot(): Promise<string> {
  return path.join(await resolveRepoRoot(), 'data', 'scripts')
}

async function resolveAssetRoot(): Promise<string> {
  const configured = process.env.ASSET_ROOT?.trim()
  if (configured) return path.resolve(configured)
  return path.join(await resolveRepoRoot(), 'data')
}

function validateScriptCatalogPath(scriptPath: string): string {
  const normalized = scriptPath.replace(/\\/g, '/')
  if (
    normalized.startsWith('/') ||
    normalized.includes('../') ||
    normalized.includes('..\\') ||
    normalized.includes('\0') ||
    !normalized.endsWith('.lua')
  ) {
    throw new Error(`script path is not allowed: ${scriptPath}`)
  }
  return normalized
}

function parseBearerToken(bearerToken?: string): string {
  const token =
    bearerToken?.trim() ??
    process.env.SIDEREAL_DASHBOARD_ADMIN_BEARER_TOKEN?.trim()
  if (!token) {
    throw new Error('gateway bearer token is not available')
  }
  return token
}

async function gatewayJson<T>(
  scriptPath: string,
  init?: RequestInit,
  bearerToken?: string,
): Promise<T> {
  const response = await fetch(
    `${parseGatewayUrl()}/admin/scripts/${scriptPath}`,
    {
      ...init,
      headers: {
        'content-type': 'application/json',
        authorization: `Bearer ${parseBearerToken(bearerToken)}`,
        ...(init?.headers ?? {}),
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
  return payload as T
}

async function fetchScriptDetail(
  scriptPath: string,
  bearerToken?: string,
): Promise<ScriptDetailResponse> {
  return gatewayJson<ScriptDetailResponse>(
    `detail/${scriptPath}`,
    undefined,
    bearerToken,
  )
}

async function fetchScriptDetailForCatalogRead(
  scriptPath: string,
  bearerToken?: string,
): Promise<ScriptDetailResponse> {
  try {
    return await fetchScriptDetail(scriptPath, bearerToken)
  } catch {
    const scriptsRoot = await resolveScriptsRoot()
    const normalized = validateScriptCatalogPath(scriptPath)
    const sourcePath = path.resolve(scriptsRoot, normalized)
    const safePrefix = `${scriptsRoot}${path.sep}`
    if (!sourcePath.startsWith(safePrefix)) {
      throw new Error(`script path escapes data/scripts: ${scriptPath}`)
    }
    const diskSource = await fs.readFile(sourcePath, 'utf8')
    return {
      script_path: normalized,
      active_source: diskSource,
      draft_source: null,
    }
  }
}

async function saveScriptDraft(
  scriptPath: string,
  source: string,
  family: string,
  bearerToken?: string,
): Promise<void> {
  await gatewayJson(
    `draft/${scriptPath}`,
    {
      method: 'POST',
      body: JSON.stringify({
        source,
        origin: 'shipyard_dashboard',
        family,
      }),
    },
    bearerToken,
  )
}

async function publishScriptDraftIfPresent(
  scriptPath: string,
  bearerToken?: string,
): Promise<void> {
  const detail = await fetchScriptDetail(scriptPath, bearerToken)
  if (typeof detail.draft_source !== 'string') return
  await gatewayJson(`publish/${scriptPath}`, { method: 'POST' }, bearerToken)
}

async function discardScriptDraftIfPresent(
  scriptPath: string,
  bearerToken?: string,
): Promise<void> {
  const detail = await fetchScriptDetail(scriptPath, bearerToken)
  if (typeof detail.draft_source !== 'string') return
  await gatewayJson(`draft/${scriptPath}`, { method: 'DELETE' }, bearerToken)
}

function activeSource(detail: ScriptDetailResponse): string {
  return detail.draft_source ?? detail.active_source ?? ''
}

class LuaSubsetParser {
  private index = 0

  constructor(private readonly source: string) {}

  parse(): unknown {
    this.skipWhitespace()
    const value = this.parseValue()
    this.skipWhitespace()
    return value
  }

  private parseValue(): unknown {
    this.skipWhitespace()
    const char = this.source[this.index]
    if (char === '{') return this.parseTable()
    if (char === '"') return this.parseString()
    if (char === '-' || /\d/.test(char)) return this.parseNumber()
    if (this.consumeKeyword('true')) return true
    if (this.consumeKeyword('false')) return false
    if (this.consumeKeyword('nil')) return null
    throw new Error(`unsupported Lua value near offset ${this.index}`)
  }

  private parseTable(): unknown {
    this.expect('{')
    const arrayValues: Array<unknown> = []
    const objectValues: Record<string, unknown> = {}
    let hasObjectFields = false
    let hasArrayFields = false

    while (this.index < this.source.length) {
      this.skipWhitespace()
      if (this.peek() === '}') {
        this.index += 1
        break
      }

      const key = this.tryParseKey()
      if (key) {
        hasObjectFields = true
        this.skipWhitespace()
        this.expect('=')
        objectValues[key] = this.parseValue()
      } else {
        hasArrayFields = true
        arrayValues.push(this.parseValue())
      }

      this.skipWhitespace()
      if (this.peek() === ',') {
        this.index += 1
      }
    }
    if (this.source[this.index - 1] !== '}') {
      throw new Error('unterminated Lua table')
    }

    if (hasObjectFields && !hasArrayFields) return objectValues
    if (!hasObjectFields) return arrayValues

    arrayValues.forEach((value, idx) => {
      objectValues[String(idx + 1)] = value
    })
    return objectValues
  }

  private tryParseKey(): string | null {
    this.skipWhitespace()
    const start = this.index
    const ident = this.parseIdentifier()
    if (!ident) {
      this.index = start
      return null
    }
    this.skipWhitespace()
    if (this.peek() !== '=') {
      this.index = start
      return null
    }
    return ident
  }

  private parseIdentifier(): string | null {
    const match = /^[A-Za-z_][A-Za-z0-9_]*/.exec(this.source.slice(this.index))
    if (!match) return null
    this.index += match[0].length
    return match[0]
  }

  private parseString(): string {
    this.expect('"')
    let out = ''
    while (this.index < this.source.length) {
      const char = this.source[this.index]
      this.index += 1
      if (char === '"') return out
      if (char === '\\') {
        const escaped = this.source[this.index]
        this.index += 1
        out += escaped === 'n' ? '\n' : escaped
      } else {
        out += char
      }
    }
    throw new Error('unterminated Lua string')
  }

  private parseNumber(): number {
    const match = /^-?\d+(?:\.\d+)?/.exec(this.source.slice(this.index))
    if (!match) throw new Error(`expected number near offset ${this.index}`)
    this.index += match[0].length
    return Number(match[0])
  }

  private consumeKeyword(keyword: string): boolean {
    if (!this.source.startsWith(keyword, this.index)) return false
    const next = this.source[this.index + keyword.length]
    if (next && /[A-Za-z0-9_]/.test(next)) return false
    this.index += keyword.length
    return true
  }

  private skipWhitespace(): void {
    while (this.index < this.source.length) {
      const char = this.source[this.index]
      if (/\s/.test(char)) {
        this.index += 1
        continue
      }
      if (this.source.startsWith('--', this.index)) {
        const nextLine = this.source.indexOf('\n', this.index + 2)
        this.index = nextLine < 0 ? this.source.length : nextLine + 1
        continue
      }
      break
    }
  }

  private peek(): string {
    return this.source[this.index] ?? ''
  }

  private expect(expected: string): void {
    if (this.source[this.index] !== expected) {
      throw new Error(`expected ${expected} near offset ${this.index}`)
    }
    this.index += 1
  }
}

function parseReturnedTable(source: string): unknown {
  const returnIndex = source.lastIndexOf('return')
  if (returnIndex < 0) throw new Error('Lua source does not return a table')
  const tableStart = source.indexOf('{', returnIndex)
  if (tableStart < 0) throw new Error('Lua return value is not a table')
  return new LuaSubsetParser(source.slice(tableStart)).parse()
}

function objectValue(value: unknown, context: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${context} must be a Lua object table`)
  }
  return value as Record<string, unknown>
}

function recordValue(value: unknown, context: string): Record<string, unknown> {
  if (Array.isArray(value) && value.length === 0) return {}
  return objectValue(value, context)
}

function arrayValue(value: unknown): Array<unknown> {
  return Array.isArray(value) ? value : []
}

function stringValue(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback
}

function numberValue(value: unknown, fallback = 0): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback
}

function booleanValue(value: unknown, fallback = false): boolean {
  return typeof value === 'boolean' ? value : fallback
}

function stringArray(value: unknown): Array<string> {
  return arrayValue(value).filter(
    (entry): entry is string => typeof entry === 'string',
  )
}

function vec3Value(value: unknown): [number, number, number] {
  const values = arrayValue(value)
  return [
    numberValue(values[0], 0),
    numberValue(values[1], 0),
    numberValue(values[2], 0),
  ]
}

export function parseShipRegistrySource(
  source: string,
): Array<ShipRegistryEntry> {
  const root = objectValue(
    parseReturnedTable(source),
    SHIP_REGISTRY_SCRIPT_PATH,
  )
  return arrayValue(root.ships).map((raw) => {
    const entry = objectValue(raw, 'ships[]')
    return {
      shipId: stringValue(entry.ship_id),
      bundleId: stringValue(entry.bundle_id),
      scriptPath: stringValue(entry.script),
      spawnEnabled: booleanValue(entry.spawn_enabled),
      tags: stringArray(entry.tags),
    }
  })
}

export function parseModuleRegistrySource(
  source: string,
): Array<ModuleRegistryEntry> {
  const root = objectValue(
    parseReturnedTable(source),
    MODULE_REGISTRY_SCRIPT_PATH,
  )
  return arrayValue(root.modules).map((raw) => {
    const entry = objectValue(raw, 'modules[]')
    return {
      moduleId: stringValue(entry.module_id),
      scriptPath: stringValue(entry.script),
      tags: stringArray(entry.tags),
    }
  })
}

export function parseShipDefinitionSource(
  source: string,
  entry: ShipRegistryEntry,
): ShipyardShipDefinition {
  const root = objectValue(parseReturnedTable(source), entry.scriptPath)
  const visual = objectValue(root.visual, `${entry.scriptPath}.visual`)
  const dimensions = objectValue(
    root.dimensions,
    `${entry.scriptPath}.dimensions`,
  )
  const rootComponents = objectValue(root.root, `${entry.scriptPath}.root`)
  return {
    ship_id: stringValue(root.ship_id, entry.shipId),
    bundle_id: stringValue(root.bundle_id, entry.bundleId),
    script_path: entry.scriptPath,
    display_name: stringValue(root.display_name, entry.shipId),
    entity_labels: stringArray(root.entity_labels),
    tags: stringArray(root.tags),
    visual: {
      visual_asset_id: stringValue(visual.visual_asset_id),
      map_icon_asset_id: stringValue(
        visual.map_icon_asset_id,
        'map_icon_ship_svg',
      ),
    },
    dimensions: {
      length_m: numberValue(dimensions.length_m, 1),
      width_m:
        typeof dimensions.width_m === 'number' ? dimensions.width_m : null,
      height_m: numberValue(dimensions.height_m, 8),
      collision_mode: stringValue(dimensions.collision_mode, 'Aabb'),
      collision_from_texture: booleanValue(
        dimensions.collision_from_texture,
        true,
      ),
    },
    root: {
      base_mass_kg: numberValue(rootComponents.base_mass_kg, 0),
      total_mass_kg:
        typeof rootComponents.total_mass_kg === 'number'
          ? rootComponents.total_mass_kg
          : null,
      cargo_mass_kg:
        typeof rootComponents.cargo_mass_kg === 'number'
          ? rootComponents.cargo_mass_kg
          : null,
      module_mass_kg:
        typeof rootComponents.module_mass_kg === 'number'
          ? rootComponents.module_mass_kg
          : null,
      angular_inertia:
        typeof rootComponents.angular_inertia === 'number'
          ? rootComponents.angular_inertia
          : null,
      max_velocity_mps: numberValue(rootComponents.max_velocity_mps, 0),
      health_pool: rootComponents.health_pool ?? {},
      destructible: rootComponents.destructible ?? {},
      flight_computer: rootComponents.flight_computer ?? {},
      flight_tuning: rootComponents.flight_tuning ?? {},
      visibility_range_buff_m: rootComponents.visibility_range_buff_m ?? {},
      scanner_component: rootComponents.scanner_component,
      avian_linear_damping:
        typeof rootComponents.avian_linear_damping === 'number'
          ? rootComponents.avian_linear_damping
          : null,
      avian_angular_damping:
        typeof rootComponents.avian_angular_damping === 'number'
          ? rootComponents.avian_angular_damping
          : null,
    },
    hardpoints: arrayValue(root.hardpoints).map((raw) => {
      const hardpoint = objectValue(raw, 'hardpoints[]')
      return {
        hardpoint_id: stringValue(hardpoint.hardpoint_id),
        display_name: stringValue(hardpoint.display_name),
        slot_kind: stringValue(hardpoint.slot_kind),
        offset_m: vec3Value(hardpoint.offset_m),
        local_rotation_rad: numberValue(hardpoint.local_rotation_rad, 0),
        mirror_group:
          typeof hardpoint.mirror_group === 'string'
            ? hardpoint.mirror_group
            : null,
        compatible_tags: stringArray(hardpoint.compatible_tags),
      }
    }),
    mounted_modules: arrayValue(root.mounted_modules).map((raw) => {
      const mount = objectValue(raw, 'mounted_modules[]')
      return {
        hardpoint_id: stringValue(mount.hardpoint_id),
        module_id: stringValue(mount.module_id),
        display_name:
          typeof mount.display_name === 'string' ? mount.display_name : null,
        component_overrides: recordValue(
          mount.component_overrides ?? {},
          'component_overrides',
        ),
      }
    }),
  }
}

export function parseModuleDefinitionSource(
  source: string,
  entry: ModuleRegistryEntry,
): ShipyardModuleDefinition {
  const root = objectValue(parseReturnedTable(source), entry.scriptPath)
  return {
    module_id: stringValue(root.module_id, entry.moduleId),
    script_path: entry.scriptPath,
    display_name: stringValue(root.display_name, entry.moduleId),
    category: stringValue(root.category, 'module'),
    entity_labels: stringArray(root.entity_labels),
    compatible_slot_kinds: stringArray(root.compatible_slot_kinds),
    tags: stringArray(root.tags),
    components: arrayValue(root.components).map((raw) => {
      const component = objectValue(raw, 'components[]')
      return {
        kind: stringValue(component.kind),
        properties: component.properties ?? {},
      }
    }),
  }
}

function parseAssetRegistryEntries(source: string): Array<ShipyardAssetEntry> {
  const entries: Array<ShipyardAssetEntry> = []
  const entryPattern =
    /\{\s*asset_id\s*=\s*"([^"]+)"[\s\S]*?source_path\s*=\s*"([^"]+)"[\s\S]*?content_type\s*=\s*"([^"]+)"[\s\S]*?\}/g
  for (const match of source.matchAll(entryPattern)) {
    if (!match[3].startsWith('image/')) continue
    entries.push({
      assetId: match[1],
      sourcePath: match[2],
      contentType: match[3],
    })
  }
  return entries.sort((a, b) => a.assetId.localeCompare(b.assetId))
}

function escapeLuaString(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')
}

function luaString(value: string): string {
  return `"${escapeLuaString(value)}"`
}

function formatNumber(value: number): string {
  if (!Number.isFinite(value)) return '0'
  if (Number.isInteger(value)) return String(value)
  return Number(value.toFixed(6)).toString()
}

function formatLuaValue(value: unknown, indent = 0): string {
  if (value === null || value === undefined) return 'nil'
  if (typeof value === 'string') return luaString(value)
  if (typeof value === 'number') return formatNumber(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (Array.isArray(value)) {
    if (value.length === 0) return '{}'
    if (value.every((item) => typeof item !== 'object' || item === null)) {
      return `{ ${value.map((item) => formatLuaValue(item, indent)).join(', ')} }`
    }
    const nextIndent = ' '.repeat(indent + 2)
    const closingIndent = ' '.repeat(indent)
    return `{\n${value
      .map((item) => `${nextIndent}${formatLuaValue(item, indent + 2)},`)
      .join('\n')}\n${closingIndent}}`
  }
  if (typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>).filter(
      ([, entryValue]) => entryValue !== undefined,
    )
    if (entries.length === 0) return '{}'
    const nextIndent = ' '.repeat(indent + 2)
    const closingIndent = ' '.repeat(indent)
    return `{\n${entries
      .map(
        ([key, entryValue]) =>
          `${nextIndent}${key} = ${formatLuaValue(entryValue, indent + 2)},`,
      )
      .join('\n')}\n${closingIndent}}`
  }
  return 'nil'
}

export function serializeShipRegistry(
  entries: Array<ShipRegistryEntry>,
): string {
  return `return ${formatLuaValue(
    {
      schema_version: 1,
      ships: entries.map((entry) => ({
        ship_id: entry.shipId,
        bundle_id: entry.bundleId,
        script: entry.scriptPath,
        spawn_enabled: entry.spawnEnabled,
        tags: entry.tags,
      })),
    },
    0,
  )}\n`
}

export function serializeModuleRegistry(
  entries: Array<ModuleRegistryEntry>,
): string {
  return `return ${formatLuaValue(
    {
      schema_version: 1,
      modules: entries.map((entry) => ({
        module_id: entry.moduleId,
        script: entry.scriptPath,
        tags: entry.tags,
      })),
    },
    0,
  )}\n`
}

export function serializeShipDefinition(
  definition: ShipyardShipDefinition,
): string {
  const { script_path: _scriptPath, ...payload } = definition
  return `return ${formatLuaValue(payload, 0)}\n`
}

export function serializeModuleDefinition(
  definition: ShipyardModuleDefinition,
): string {
  const { script_path: _scriptPath, ...payload } = definition
  return `return ${formatLuaValue(payload, 0)}\n`
}

export async function loadShipyardCatalog(
  bearerToken?: string,
): Promise<ShipyardCatalog> {
  const [shipRegistry, moduleRegistry, assetRegistry] = await Promise.all([
    fetchScriptDetailForCatalogRead(SHIP_REGISTRY_SCRIPT_PATH, bearerToken),
    fetchScriptDetailForCatalogRead(MODULE_REGISTRY_SCRIPT_PATH, bearerToken),
    fetchScriptDetailForCatalogRead(ASSET_REGISTRY_SCRIPT_PATH, bearerToken),
  ])
  const shipEntries = parseShipRegistrySource(activeSource(shipRegistry))
  const moduleEntries = parseModuleRegistrySource(activeSource(moduleRegistry))
  const imageAssets = parseAssetRegistryEntries(activeSource(assetRegistry))

  const modules = await Promise.all(
    moduleEntries.map(async (entry): Promise<ShipyardModuleEntry> => {
      const detail = await fetchScriptDetailForCatalogRead(
        entry.scriptPath,
        bearerToken,
      )
      const definition = parseModuleDefinitionSource(
        activeSource(detail),
        entry,
      )
      return {
        moduleId: entry.moduleId,
        scriptPath: entry.scriptPath,
        displayName: definition.display_name,
        category: definition.category,
        tags: entry.tags.length > 0 ? entry.tags : definition.tags,
        hasDraft: typeof detail.draft_source === 'string',
        definition,
      }
    }),
  )

  const ships = await Promise.all(
    shipEntries.map(async (entry): Promise<ShipyardShipEntry> => {
      const detail = await fetchScriptDetailForCatalogRead(
        entry.scriptPath,
        bearerToken,
      )
      const definition = parseShipDefinitionSource(activeSource(detail), entry)
      return {
        shipId: entry.shipId,
        bundleId: entry.bundleId,
        scriptPath: entry.scriptPath,
        displayName: definition.display_name,
        visualAssetId: definition.visual.visual_asset_id,
        spawnEnabled: entry.spawnEnabled,
        tags: entry.tags.length > 0 ? entry.tags : definition.tags,
        hasDraft: typeof detail.draft_source === 'string',
        definition,
      }
    }),
  )

  return {
    ships,
    modules,
    imageAssets,
    shipRegistryHasDraft: typeof shipRegistry.draft_source === 'string',
    moduleRegistryHasDraft: typeof moduleRegistry.draft_source === 'string',
  }
}

export async function saveShipyardShipDraft(
  request: ShipyardShipDraftRequest,
  bearerToken?: string,
): Promise<ShipyardCatalog> {
  const registry = await fetchScriptDetail(
    SHIP_REGISTRY_SCRIPT_PATH,
    bearerToken,
  )
  const entries = parseShipRegistrySource(activeSource(registry))
  const entryIndex = entries.findIndex(
    (entry) => entry.shipId === request.definition.ship_id,
  )
  if (entryIndex < 0) {
    throw new Error(`unknown Shipyard ship ${request.definition.ship_id}`)
  }
  const entry = entries[entryIndex]
  if (entry.scriptPath !== request.definition.script_path) {
    throw new Error('ship script_path cannot be changed in this editor slice')
  }
  entries[entryIndex] = {
    ...entry,
    bundleId: request.definition.bundle_id,
    spawnEnabled: request.spawnEnabled,
    tags: request.definition.tags,
  }
  await saveScriptDraft(
    request.definition.script_path,
    serializeShipDefinition(request.definition),
    'ship_definition',
    bearerToken,
  )
  await saveScriptDraft(
    SHIP_REGISTRY_SCRIPT_PATH,
    serializeShipRegistry(entries),
    'ship_registry',
    bearerToken,
  )
  return loadShipyardCatalog(bearerToken)
}

export async function publishShipyardShipDraft(
  shipId: string,
  bearerToken?: string,
): Promise<ShipyardCatalog> {
  const catalog = await loadShipyardCatalog(bearerToken)
  const entry = catalog.ships.find((candidate) => candidate.shipId === shipId)
  if (!entry) throw new Error(`unknown Shipyard ship ${shipId}`)
  await publishScriptDraftIfPresent(entry.scriptPath, bearerToken)
  await publishScriptDraftIfPresent(SHIP_REGISTRY_SCRIPT_PATH, bearerToken)
  return loadShipyardCatalog(bearerToken)
}

export async function discardShipyardShipDraft(
  shipId: string,
  bearerToken?: string,
): Promise<ShipyardCatalog> {
  const catalog = await loadShipyardCatalog(bearerToken)
  const entry = catalog.ships.find((candidate) => candidate.shipId === shipId)
  if (!entry) throw new Error(`unknown Shipyard ship ${shipId}`)
  await discardScriptDraftIfPresent(entry.scriptPath, bearerToken)
  await discardScriptDraftIfPresent(SHIP_REGISTRY_SCRIPT_PATH, bearerToken)
  return loadShipyardCatalog(bearerToken)
}

export async function saveShipyardModuleDraft(
  request: ShipyardModuleDraftRequest,
  bearerToken?: string,
): Promise<ShipyardCatalog> {
  const registry = await fetchScriptDetail(
    MODULE_REGISTRY_SCRIPT_PATH,
    bearerToken,
  )
  const entries = parseModuleRegistrySource(activeSource(registry))
  const entryIndex = entries.findIndex(
    (entry) => entry.moduleId === request.definition.module_id,
  )
  if (entryIndex < 0) {
    throw new Error(`unknown Shipyard module ${request.definition.module_id}`)
  }
  const entry = entries[entryIndex]
  if (entry.scriptPath !== request.definition.script_path) {
    throw new Error('module script_path cannot be changed in this editor slice')
  }
  entries[entryIndex] = {
    ...entry,
    tags: request.definition.tags,
  }
  await saveScriptDraft(
    request.definition.script_path,
    serializeModuleDefinition(request.definition),
    'ship_module_definition',
    bearerToken,
  )
  await saveScriptDraft(
    MODULE_REGISTRY_SCRIPT_PATH,
    serializeModuleRegistry(entries),
    'ship_module_registry',
    bearerToken,
  )
  return loadShipyardCatalog(bearerToken)
}

export async function publishShipyardModuleDraft(
  moduleId: string,
  bearerToken?: string,
): Promise<ShipyardCatalog> {
  const catalog = await loadShipyardCatalog(bearerToken)
  const entry = catalog.modules.find(
    (candidate) => candidate.moduleId === moduleId,
  )
  if (!entry) throw new Error(`unknown Shipyard module ${moduleId}`)
  await publishScriptDraftIfPresent(entry.scriptPath, bearerToken)
  await publishScriptDraftIfPresent(MODULE_REGISTRY_SCRIPT_PATH, bearerToken)
  return loadShipyardCatalog(bearerToken)
}

export async function discardShipyardModuleDraft(
  moduleId: string,
  bearerToken?: string,
): Promise<ShipyardCatalog> {
  const catalog = await loadShipyardCatalog(bearerToken)
  const entry = catalog.modules.find(
    (candidate) => candidate.moduleId === moduleId,
  )
  if (!entry) throw new Error(`unknown Shipyard module ${moduleId}`)
  await discardScriptDraftIfPresent(entry.scriptPath, bearerToken)
  await discardScriptDraftIfPresent(MODULE_REGISTRY_SCRIPT_PATH, bearerToken)
  return loadShipyardCatalog(bearerToken)
}

export async function loadShipyardAssetBytes(
  assetId: string,
  bearerToken?: string,
): Promise<{ bytes: Uint8Array; contentType: string }> {
  const detail = await fetchScriptDetailForCatalogRead(
    ASSET_REGISTRY_SCRIPT_PATH,
    bearerToken,
  )
  const asset = parseAssetRegistryEntries(activeSource(detail)).find(
    (entry) => entry.assetId === assetId,
  )
  if (!asset) throw new Error(`unknown Shipyard image asset ${assetId}`)
  const assetRoot = await resolveAssetRoot()
  const assetPath = path.resolve(assetRoot, asset.sourcePath)
  const safePrefix = `${assetRoot}${path.sep}`
  if (!assetPath.startsWith(safePrefix)) {
    throw new Error(`asset path escapes data root: ${asset.sourcePath}`)
  }
  const bytes = await fs.readFile(assetPath)
  return { bytes, contentType: asset.contentType }
}

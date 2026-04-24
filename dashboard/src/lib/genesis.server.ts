import type {
  GenesisPlanetCatalog,
  GenesisPlanetDefinition,
  GenesisPlanetDraftRequest,
  GenesisPlanetEntry,
  GenesisPlanetShaderSettings,
  GenesisPlanetSpawn,
  Vec2Tuple,
  Vec3Tuple,
} from '@/features/genesis/types'

type ScriptDetailResponse = {
  script_path?: string
  active_source?: string
  draft_source?: string | null
}

type RegistryEntry = {
  planetId: string
  scriptPath: string
  spawnEnabled: boolean
  tags: Array<string>
}

const REGISTRY_SCRIPT_PATH = 'planets/registry.lua'

const DEFAULT_SHADER_SETTINGS: GenesisPlanetShaderSettings = {
  enabled: true,
  enable_surface_detail: true,
  enable_craters: true,
  enable_clouds: true,
  enable_atmosphere: true,
  enable_specular: true,
  enable_night_lights: true,
  enable_emissive: true,
  enable_ocean_specular: true,
  body_kind: 0,
  planet_type: 0,
  seed: 1,
  base_radius_scale: 0.5,
  normal_strength: 0.55,
  detail_level: 0.3,
  rotation_speed: 0.004,
  light_wrap: 0.2,
  ambient_strength: 0.16,
  specular_strength: 0.12,
  specular_power: 18.0,
  rim_strength: 0.28,
  rim_power: 3.6,
  fresnel_strength: 0.4,
  cloud_shadow_strength: 0.18,
  night_glow_strength: 0.05,
  continent_size: 0.58,
  ocean_level: 0.46,
  mountain_height: 0.34,
  roughness: 0.44,
  terrain_octaves: 5,
  terrain_lacunarity: 2.1,
  terrain_gain: 0.5,
  crater_density: 0.18,
  crater_size: 0.33,
  volcano_density: 0.04,
  ice_cap_size: 0.18,
  storm_intensity: 0.1,
  bands_count: 6.0,
  spot_density: 0.08,
  surface_activity: 0.12,
  corona_intensity: 0.0,
  cloud_coverage: 0.34,
  cloud_scale: 1.3,
  cloud_speed: 0.18,
  cloud_alpha: 0.42,
  atmosphere_thickness: 0.12,
  atmosphere_falloff: 2.8,
  atmosphere_alpha: 0.48,
  city_lights: 0.04,
  emissive_strength: 0.0,
  sun_intensity: 1.0,
  surface_saturation: 1.12,
  surface_contrast: 1.08,
  light_color_mix: 0.14,
  sun_direction_xy: [0.74, 0.52],
  color_primary_rgb: [0.24, 0.48, 0.22],
  color_secondary_rgb: [0.52, 0.42, 0.28],
  color_tertiary_rgb: [0.08, 0.2, 0.48],
  color_atmosphere_rgb: [0.36, 0.62, 1.0],
  color_clouds_rgb: [0.95, 0.97, 1.0],
  color_night_lights_rgb: [1.0, 0.76, 0.4],
  color_emissive_rgb: [1.0, 0.42, 0.18],
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

async function gatewayJson<T>(
  scriptPath: string,
  init?: RequestInit,
): Promise<T> {
  const response = await fetch(`${parseGatewayUrl()}/admin/scripts/${scriptPath}`, {
    ...init,
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${parseBearerToken()}`,
      ...(init?.headers ?? {}),
    },
  })
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
): Promise<ScriptDetailResponse> {
  return gatewayJson<ScriptDetailResponse>(`detail/${scriptPath}`)
}

async function saveScriptDraft(
  scriptPath: string,
  source: string,
  family: string,
): Promise<void> {
  await gatewayJson(`draft/${scriptPath}`, {
    method: 'POST',
    body: JSON.stringify({
      source,
      origin: 'genesis_dashboard',
      family,
    }),
  })
}

async function publishScriptDraftIfPresent(scriptPath: string): Promise<void> {
  const detail = await fetchScriptDetail(scriptPath)
  if (typeof detail.draft_source !== 'string') return
  await gatewayJson(`publish/${scriptPath}`, { method: 'POST' })
}

async function discardScriptDraftIfPresent(scriptPath: string): Promise<void> {
  const detail = await fetchScriptDetail(scriptPath)
  if (typeof detail.draft_source !== 'string') return
  await gatewayJson(`draft/${scriptPath}`, { method: 'DELETE' })
}

function activeSource(detail: ScriptDetailResponse): string {
  return detail.draft_source ?? detail.active_source ?? ''
}

function parseLuaStringList(raw: string): Array<string> {
  return (raw.match(/"([^"]+)"/g) ?? []).map((value) => value.slice(1, -1))
}

function parseRegistryEntries(source: string): Array<RegistryEntry> {
  const entries: Array<RegistryEntry> = []
  const entryPattern =
    /\{\s*planet_id\s*=\s*"([^"]+)"[\s\S]*?script\s*=\s*"([^"]+)"[\s\S]*?\}/g
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

function parseNumberField(
  source: string,
  field: string,
  fallback: number,
): number {
  const match = source.match(new RegExp(`${field}\\s*=\\s*(-?\\d+(?:\\.\\d+)?)`))
  return match ? Number(match[1]) : fallback
}

function parseIntegerField(
  source: string,
  field: string,
  fallback: number,
): number {
  return Math.trunc(parseNumberField(source, field, fallback))
}

function parseBooleanField(
  source: string,
  field: string,
  fallback: boolean,
): boolean {
  const match = source.match(new RegExp(`${field}\\s*=\\s*(true|false)`))
  return match ? match[1] === 'true' : fallback
}

function parseStringField(
  source: string,
  field: string,
  fallback: string,
): string {
  return source.match(new RegExp(`${field}\\s*=\\s*"([^"]+)"`))?.[1] ?? fallback
}

function parseStringListField(
  source: string,
  field: string,
  fallback: Array<string>,
): Array<string> {
  const match = source.match(new RegExp(`${field}\\s*=\\s*\\{([\\s\\S]*?)\\}`))
  return match ? parseLuaStringList(match[1]) : fallback
}

function parseVec2Field(
  source: string,
  field: string,
  fallback: Vec2Tuple,
): Vec2Tuple {
  const match = source.match(
    new RegExp(`${field}\\s*=\\s*\\{\\s*(-?\\d+(?:\\.\\d+)?)\\s*,\\s*(-?\\d+(?:\\.\\d+)?)\\s*\\}`),
  )
  return match ? [Number(match[1]), Number(match[2])] : fallback
}

function parseVec3Field(
  source: string,
  field: string,
  fallback: Vec3Tuple,
): Vec3Tuple {
  const match = source.match(
    new RegExp(
      `${field}\\s*=\\s*\\{\\s*(-?\\d+(?:\\.\\d+)?)\\s*,\\s*(-?\\d+(?:\\.\\d+)?)\\s*,\\s*(-?\\d+(?:\\.\\d+)?)\\s*\\}`,
    ),
  )
  return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : fallback
}

function parseShaderSettings(source: string): GenesisPlanetShaderSettings {
  const defaults = DEFAULT_SHADER_SETTINGS
  return {
    enabled: parseBooleanField(source, 'enabled', defaults.enabled),
    enable_surface_detail: parseBooleanField(
      source,
      'enable_surface_detail',
      defaults.enable_surface_detail,
    ),
    enable_craters: parseBooleanField(source, 'enable_craters', defaults.enable_craters),
    enable_clouds: parseBooleanField(source, 'enable_clouds', defaults.enable_clouds),
    enable_atmosphere: parseBooleanField(
      source,
      'enable_atmosphere',
      defaults.enable_atmosphere,
    ),
    enable_specular: parseBooleanField(
      source,
      'enable_specular',
      defaults.enable_specular,
    ),
    enable_night_lights: parseBooleanField(
      source,
      'enable_night_lights',
      defaults.enable_night_lights,
    ),
    enable_emissive: parseBooleanField(
      source,
      'enable_emissive',
      defaults.enable_emissive,
    ),
    enable_ocean_specular: parseBooleanField(
      source,
      'enable_ocean_specular',
      defaults.enable_ocean_specular,
    ),
    body_kind: parseIntegerField(source, 'body_kind', defaults.body_kind),
    planet_type: parseIntegerField(source, 'planet_type', defaults.planet_type),
    seed: parseIntegerField(source, 'seed', defaults.seed),
    base_radius_scale: parseNumberField(
      source,
      'base_radius_scale',
      defaults.base_radius_scale,
    ),
    normal_strength: parseNumberField(source, 'normal_strength', defaults.normal_strength),
    detail_level: parseNumberField(source, 'detail_level', defaults.detail_level),
    rotation_speed: parseNumberField(source, 'rotation_speed', defaults.rotation_speed),
    light_wrap: parseNumberField(source, 'light_wrap', defaults.light_wrap),
    ambient_strength: parseNumberField(
      source,
      'ambient_strength',
      defaults.ambient_strength,
    ),
    specular_strength: parseNumberField(
      source,
      'specular_strength',
      defaults.specular_strength,
    ),
    specular_power: parseNumberField(source, 'specular_power', defaults.specular_power),
    rim_strength: parseNumberField(source, 'rim_strength', defaults.rim_strength),
    rim_power: parseNumberField(source, 'rim_power', defaults.rim_power),
    fresnel_strength: parseNumberField(
      source,
      'fresnel_strength',
      defaults.fresnel_strength,
    ),
    cloud_shadow_strength: parseNumberField(
      source,
      'cloud_shadow_strength',
      defaults.cloud_shadow_strength,
    ),
    night_glow_strength: parseNumberField(
      source,
      'night_glow_strength',
      defaults.night_glow_strength,
    ),
    continent_size: parseNumberField(source, 'continent_size', defaults.continent_size),
    ocean_level: parseNumberField(source, 'ocean_level', defaults.ocean_level),
    mountain_height: parseNumberField(source, 'mountain_height', defaults.mountain_height),
    roughness: parseNumberField(source, 'roughness', defaults.roughness),
    terrain_octaves: parseIntegerField(
      source,
      'terrain_octaves',
      defaults.terrain_octaves,
    ),
    terrain_lacunarity: parseNumberField(
      source,
      'terrain_lacunarity',
      defaults.terrain_lacunarity,
    ),
    terrain_gain: parseNumberField(source, 'terrain_gain', defaults.terrain_gain),
    crater_density: parseNumberField(source, 'crater_density', defaults.crater_density),
    crater_size: parseNumberField(source, 'crater_size', defaults.crater_size),
    volcano_density: parseNumberField(
      source,
      'volcano_density',
      defaults.volcano_density,
    ),
    ice_cap_size: parseNumberField(source, 'ice_cap_size', defaults.ice_cap_size),
    storm_intensity: parseNumberField(
      source,
      'storm_intensity',
      defaults.storm_intensity,
    ),
    bands_count: parseNumberField(source, 'bands_count', defaults.bands_count),
    spot_density: parseNumberField(source, 'spot_density', defaults.spot_density),
    surface_activity: parseNumberField(
      source,
      'surface_activity',
      defaults.surface_activity,
    ),
    corona_intensity: parseNumberField(
      source,
      'corona_intensity',
      defaults.corona_intensity,
    ),
    cloud_coverage: parseNumberField(source, 'cloud_coverage', defaults.cloud_coverage),
    cloud_scale: parseNumberField(source, 'cloud_scale', defaults.cloud_scale),
    cloud_speed: parseNumberField(source, 'cloud_speed', defaults.cloud_speed),
    cloud_alpha: parseNumberField(source, 'cloud_alpha', defaults.cloud_alpha),
    atmosphere_thickness: parseNumberField(
      source,
      'atmosphere_thickness',
      defaults.atmosphere_thickness,
    ),
    atmosphere_falloff: parseNumberField(
      source,
      'atmosphere_falloff',
      defaults.atmosphere_falloff,
    ),
    atmosphere_alpha: parseNumberField(
      source,
      'atmosphere_alpha',
      defaults.atmosphere_alpha,
    ),
    city_lights: parseNumberField(source, 'city_lights', defaults.city_lights),
    emissive_strength: parseNumberField(
      source,
      'emissive_strength',
      defaults.emissive_strength,
    ),
    sun_intensity: parseNumberField(source, 'sun_intensity', defaults.sun_intensity),
    surface_saturation: parseNumberField(
      source,
      'surface_saturation',
      defaults.surface_saturation,
    ),
    surface_contrast: parseNumberField(
      source,
      'surface_contrast',
      defaults.surface_contrast,
    ),
    light_color_mix: parseNumberField(
      source,
      'light_color_mix',
      defaults.light_color_mix,
    ),
    sun_direction_xy: parseVec2Field(
      source,
      'sun_direction_xy',
      defaults.sun_direction_xy,
    ),
    color_primary_rgb: parseVec3Field(
      source,
      'color_primary_rgb',
      defaults.color_primary_rgb,
    ),
    color_secondary_rgb: parseVec3Field(
      source,
      'color_secondary_rgb',
      defaults.color_secondary_rgb,
    ),
    color_tertiary_rgb: parseVec3Field(
      source,
      'color_tertiary_rgb',
      defaults.color_tertiary_rgb,
    ),
    color_atmosphere_rgb: parseVec3Field(
      source,
      'color_atmosphere_rgb',
      defaults.color_atmosphere_rgb,
    ),
    color_clouds_rgb: parseVec3Field(source, 'color_clouds_rgb', defaults.color_clouds_rgb),
    color_night_lights_rgb: parseVec3Field(
      source,
      'color_night_lights_rgb',
      defaults.color_night_lights_rgb,
    ),
    color_emissive_rgb: parseVec3Field(
      source,
      'color_emissive_rgb',
      defaults.color_emissive_rgb,
    ),
  }
}

function parseSpawnDefinition(source: string): GenesisPlanetSpawn {
  return {
    entity_id: parseStringField(source, 'entity_id', ''),
    owner_id: parseStringField(source, 'owner_id', 'world:system'),
    size_m: parseNumberField(source, 'size_m', 640),
    spawn_position: parseVec2Field(source, 'spawn_position', [0, 0]),
    spawn_rotation_rad: parseNumberField(source, 'spawn_rotation_rad', 0),
    map_icon_asset_id: parseStringField(
      source,
      'map_icon_asset_id',
      'map_icon_planet_svg',
    ),
    planet_visual_shader_asset_id: parseStringField(
      source,
      'planet_visual_shader_asset_id',
      'planet_visual_wgsl',
    ),
  }
}

function parsePlanetDefinition(
  source: string,
  entry: RegistryEntry,
): GenesisPlanetDefinition {
  return {
    planet_id: parseStringField(source, 'planet_id', entry.planetId),
    script_path: entry.scriptPath,
    display_name: parseStringField(source, 'display_name', entry.planetId),
    entity_labels: parseStringListField(source, 'entity_labels', ['Planet', 'CelestialBody']),
    tags: parseStringListField(source, 'tags', entry.tags),
    spawn: parseSpawnDefinition(source),
    shader_settings: parseShaderSettings(source),
  }
}

function escapeLuaString(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')
}

function luaString(value: string): string {
  return `"${escapeLuaString(value)}"`
}

function formatNumber(value: number): string {
  if (Number.isInteger(value)) return String(value)
  return Number(value.toFixed(6)).toString()
}

function formatLuaStringList(values: Array<string>): string {
  return `{ ${values.map(luaString).join(', ')} }`
}

function formatVec2(value: Vec2Tuple): string {
  return `{ ${formatNumber(value[0])}, ${formatNumber(value[1])} }`
}

function formatVec3(value: Vec3Tuple): string {
  return `{ ${formatNumber(value[0])}, ${formatNumber(value[1])}, ${formatNumber(value[2])} }`
}

function formatBoolean(value: boolean): string {
  return value ? 'true' : 'false'
}

function serializePlanetDefinition(definition: GenesisPlanetDefinition): string {
  const settings = definition.shader_settings
  return `return {
  planet_id = ${luaString(definition.planet_id)},
  display_name = ${luaString(definition.display_name)},
  entity_labels = ${formatLuaStringList(definition.entity_labels)},
  tags = ${formatLuaStringList(definition.tags)},
  spawn = {
    entity_id = ${luaString(definition.spawn.entity_id)},
    owner_id = ${luaString(definition.spawn.owner_id)},
    size_m = ${formatNumber(definition.spawn.size_m)},
    spawn_position = ${formatVec2(definition.spawn.spawn_position)},
    spawn_rotation_rad = ${formatNumber(definition.spawn.spawn_rotation_rad)},
    map_icon_asset_id = ${luaString(definition.spawn.map_icon_asset_id)},
    planet_visual_shader_asset_id = ${luaString(definition.spawn.planet_visual_shader_asset_id)},
  },
  shader_settings = {
    enabled = ${formatBoolean(settings.enabled)},
    enable_surface_detail = ${formatBoolean(settings.enable_surface_detail)},
    enable_craters = ${formatBoolean(settings.enable_craters)},
    enable_clouds = ${formatBoolean(settings.enable_clouds)},
    enable_atmosphere = ${formatBoolean(settings.enable_atmosphere)},
    enable_specular = ${formatBoolean(settings.enable_specular)},
    enable_night_lights = ${formatBoolean(settings.enable_night_lights)},
    enable_emissive = ${formatBoolean(settings.enable_emissive)},
    enable_ocean_specular = ${formatBoolean(settings.enable_ocean_specular)},
    body_kind = ${formatNumber(settings.body_kind)},
    planet_type = ${formatNumber(settings.planet_type)},
    seed = ${formatNumber(settings.seed)},
    base_radius_scale = ${formatNumber(settings.base_radius_scale)},
    normal_strength = ${formatNumber(settings.normal_strength)},
    detail_level = ${formatNumber(settings.detail_level)},
    rotation_speed = ${formatNumber(settings.rotation_speed)},
    light_wrap = ${formatNumber(settings.light_wrap)},
    ambient_strength = ${formatNumber(settings.ambient_strength)},
    specular_strength = ${formatNumber(settings.specular_strength)},
    specular_power = ${formatNumber(settings.specular_power)},
    rim_strength = ${formatNumber(settings.rim_strength)},
    rim_power = ${formatNumber(settings.rim_power)},
    fresnel_strength = ${formatNumber(settings.fresnel_strength)},
    cloud_shadow_strength = ${formatNumber(settings.cloud_shadow_strength)},
    night_glow_strength = ${formatNumber(settings.night_glow_strength)},
    continent_size = ${formatNumber(settings.continent_size)},
    ocean_level = ${formatNumber(settings.ocean_level)},
    mountain_height = ${formatNumber(settings.mountain_height)},
    roughness = ${formatNumber(settings.roughness)},
    terrain_octaves = ${formatNumber(settings.terrain_octaves)},
    terrain_lacunarity = ${formatNumber(settings.terrain_lacunarity)},
    terrain_gain = ${formatNumber(settings.terrain_gain)},
    crater_density = ${formatNumber(settings.crater_density)},
    crater_size = ${formatNumber(settings.crater_size)},
    volcano_density = ${formatNumber(settings.volcano_density)},
    ice_cap_size = ${formatNumber(settings.ice_cap_size)},
    storm_intensity = ${formatNumber(settings.storm_intensity)},
    bands_count = ${formatNumber(settings.bands_count)},
    spot_density = ${formatNumber(settings.spot_density)},
    surface_activity = ${formatNumber(settings.surface_activity)},
    corona_intensity = ${formatNumber(settings.corona_intensity)},
    cloud_coverage = ${formatNumber(settings.cloud_coverage)},
    cloud_scale = ${formatNumber(settings.cloud_scale)},
    cloud_speed = ${formatNumber(settings.cloud_speed)},
    cloud_alpha = ${formatNumber(settings.cloud_alpha)},
    atmosphere_thickness = ${formatNumber(settings.atmosphere_thickness)},
    atmosphere_falloff = ${formatNumber(settings.atmosphere_falloff)},
    atmosphere_alpha = ${formatNumber(settings.atmosphere_alpha)},
    city_lights = ${formatNumber(settings.city_lights)},
    emissive_strength = ${formatNumber(settings.emissive_strength)},
    sun_intensity = ${formatNumber(settings.sun_intensity)},
    surface_saturation = ${formatNumber(settings.surface_saturation)},
    surface_contrast = ${formatNumber(settings.surface_contrast)},
    light_color_mix = ${formatNumber(settings.light_color_mix)},
    sun_direction_xy = ${formatVec2(settings.sun_direction_xy)},
    color_primary_rgb = ${formatVec3(settings.color_primary_rgb)},
    color_secondary_rgb = ${formatVec3(settings.color_secondary_rgb)},
    color_tertiary_rgb = ${formatVec3(settings.color_tertiary_rgb)},
    color_atmosphere_rgb = ${formatVec3(settings.color_atmosphere_rgb)},
    color_clouds_rgb = ${formatVec3(settings.color_clouds_rgb)},
    color_night_lights_rgb = ${formatVec3(settings.color_night_lights_rgb)},
    color_emissive_rgb = ${formatVec3(settings.color_emissive_rgb)},
  },
}
`
}

function serializeRegistry(entries: Array<RegistryEntry>): string {
  const rows = entries
    .map(
      (entry) => `  {
    planet_id = ${luaString(entry.planetId)},
    script = ${luaString(entry.scriptPath)},
    spawn_enabled = ${formatBoolean(entry.spawnEnabled)},
    tags = ${formatLuaStringList(entry.tags)},
  },`,
    )
    .join('\n')
  return `local PlanetRegistry = {}

PlanetRegistry.schema_version = 1

PlanetRegistry.planets = {
${rows}
}

return PlanetRegistry
`
}

export async function loadGenesisPlanetCatalog(): Promise<GenesisPlanetCatalog> {
  const registry = await fetchScriptDetail(REGISTRY_SCRIPT_PATH)
  const registryEntries = parseRegistryEntries(activeSource(registry))
  const entries = await Promise.all(
    registryEntries.map(async (entry): Promise<GenesisPlanetEntry> => {
      const detail = await fetchScriptDetail(entry.scriptPath)
      const source = activeSource(detail)
      const definition = parsePlanetDefinition(source, entry)
      return {
        planetId: entry.planetId,
        scriptPath: entry.scriptPath,
        displayName: definition.display_name,
        bodyKind: definition.shader_settings.body_kind,
        planetType: definition.shader_settings.planet_type,
        seed: definition.shader_settings.seed,
        spawnEnabled: entry.spawnEnabled,
        tags: entry.tags,
        hasDraft: typeof detail.draft_source === 'string',
        definition,
      }
    }),
  )
  return {
    entries,
    registryHasDraft: typeof registry.draft_source === 'string',
  }
}

export async function saveGenesisPlanetDraft(
  request: GenesisPlanetDraftRequest,
): Promise<GenesisPlanetCatalog> {
  const registry = await fetchScriptDetail(REGISTRY_SCRIPT_PATH)
  const entries = parseRegistryEntries(activeSource(registry))
  const entryIndex = entries.findIndex(
    (entry) => entry.planetId === request.definition.planet_id,
  )
  if (entryIndex < 0) {
    throw new Error(`unknown Genesis planet ${request.definition.planet_id}`)
  }
  const entry = entries[entryIndex]
  if (entry.scriptPath !== request.definition.script_path) {
    throw new Error('planet script_path cannot be changed in this editor slice')
  }

  entries[entryIndex] = {
    ...entry,
    spawnEnabled: request.spawnEnabled,
    tags: request.definition.tags,
  }

  await saveScriptDraft(
    request.definition.script_path,
    serializePlanetDefinition(request.definition),
    'planet',
  )
  await saveScriptDraft(REGISTRY_SCRIPT_PATH, serializeRegistry(entries), 'planet_registry')
  return loadGenesisPlanetCatalog()
}

export async function publishGenesisPlanetDraft(
  planetId: string,
): Promise<GenesisPlanetCatalog> {
  const catalog = await loadGenesisPlanetCatalog()
  const entry = catalog.entries.find((candidate) => candidate.planetId === planetId)
  if (!entry) throw new Error(`unknown Genesis planet ${planetId}`)
  await publishScriptDraftIfPresent(entry.scriptPath)
  await publishScriptDraftIfPresent(REGISTRY_SCRIPT_PATH)
  return loadGenesisPlanetCatalog()
}

export async function discardGenesisPlanetDraft(
  planetId: string,
): Promise<GenesisPlanetCatalog> {
  const catalog = await loadGenesisPlanetCatalog()
  const entry = catalog.entries.find((candidate) => candidate.planetId === planetId)
  if (!entry) throw new Error(`unknown Genesis planet ${planetId}`)
  await discardScriptDraftIfPresent(entry.scriptPath)
  await discardScriptDraftIfPresent(REGISTRY_SCRIPT_PATH)
  return loadGenesisPlanetCatalog()
}

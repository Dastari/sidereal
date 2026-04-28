import { promises as fs } from 'node:fs'
import path from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  loadShipyardCatalog,
  parseModuleRegistrySource,
  parseShipRegistrySource,
  saveShipyardShipDraft,
  serializeModuleRegistry,
  serializeShipRegistry,
} from './shipyard.server'
import { shipyardShipDraftBodySchema } from '@/lib/schemas/dashboard'

const shipRegistrySource = `return {
  schema_version = 1,
  ships = {
    {
      ship_id = "ship.test",
      bundle_id = "ship.test",
      script = "ships/test.lua",
      spawn_enabled = true,
      tags = { "test" },
    },
  },
}
`

const shipSource = `return {
  ship_id = "ship.test",
  bundle_id = "ship.test",
  display_name = "Test Ship",
  entity_labels = { "Ship" },
  tags = { "test" },
  visual = { visual_asset_id = "test_ship", map_icon_asset_id = "map_icon_ship_svg" },
  dimensions = { length_m = 12.0, width_m = 6.0, height_m = 3.0, collision_mode = "Aabb", collision_from_texture = false },
  root = {
    base_mass_kg = 1000.0,
    max_velocity_mps = 80.0,
    health_pool = { current = 100.0, maximum = 100.0 },
    destructible = { destruction_profile_id = "test", destroy_delay_s = 0.1 },
    flight_computer = { profile = "test", throttle = 0.0, yaw_input = 0.0, brake_active = false, turn_rate_deg_s = 90.0 },
    flight_tuning = { max_linear_accel_mps2 = 10.0, passive_brake_accel_mps2 = 1.0, active_brake_accel_mps2 = 2.0, drag_per_s = 0.1 },
    visibility_range_buff_m = { additive_m = 10.0, multiplier = 1.0 },
  },
  hardpoints = {
    { hardpoint_id = "engine_aft", display_name = "Engine Aft", slot_kind = "engine", offset_m = { 0.0, -5.0, 0.0 }, local_rotation_rad = 0.0, compatible_tags = { "engine" } },
  },
  mounted_modules = {
    { hardpoint_id = "engine_aft", module_id = "module.engine.test", component_overrides = {} },
  },
}
`

const moduleRegistrySource = `return {
  schema_version = 1,
  modules = {
    { module_id = "module.engine.test", script = "ship_modules/engine_test.lua", tags = { "engine" } },
  },
}
`

const moduleSource = `return {
  module_id = "module.engine.test",
  display_name = "Test Engine",
  category = "engine",
  entity_labels = { "Module", "Engine" },
  compatible_slot_kinds = { "engine" },
  tags = { "engine" },
  components = {
    { kind = "mass_kg", properties = 10.0 },
  },
}
`

const assetRegistrySource = `local AssetRegistry = {}
AssetRegistry.assets = {
  { asset_id = "test_ship", source_path = "sprites/test_ship.png", content_type = "image/png", dependencies = {}, bootstrap_required = false },
  { asset_id = "map_icon_ship_svg", source_path = "icons/ship.svg", content_type = "image/svg+xml", dependencies = {}, bootstrap_required = false },
}
return AssetRegistry
`

const sources: Record<string, string> = {
  'ships/registry.lua': shipRegistrySource,
  'ships/test.lua': shipSource,
  'ship_modules/registry.lua': moduleRegistrySource,
  'ship_modules/engine_test.lua': moduleSource,
  'assets/registry.lua': assetRegistrySource,
}

function mockScriptFetch(
  draftSources: Record<string, string | null> = {},
  writes: Array<{ scriptPath: string; source: string }> = [],
) {
  process.env.SIDEREAL_DASHBOARD_ADMIN_BEARER_TOKEN = 'test-token'
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input)
      const detailMarker = '/admin/scripts/detail/'
      const draftMarker = '/admin/scripts/draft/'
      if (url.includes(detailMarker)) {
        const scriptPath = decodeURIComponent(url.split(detailMarker)[1])
        return Promise.resolve(
          Response.json({
            script_path: scriptPath,
            active_source: sources[scriptPath] ?? '',
            draft_source: draftSources[scriptPath] ?? null,
          }),
        )
      }
      if (url.includes(draftMarker) && init?.method === 'POST') {
        const scriptPath = decodeURIComponent(url.split(draftMarker)[1])
        const body = JSON.parse(String(init.body)) as { source: string }
        writes.push({ scriptPath, source: body.source })
        sources[scriptPath] = body.source
        return Promise.resolve(Response.json({ ok: true }))
      }
      return Promise.resolve(
        Response.json({ error: 'not found' }, { status: 404 }),
      )
    }),
  )
}

afterEach(() => {
  vi.unstubAllGlobals()
  delete process.env.SIDEREAL_DASHBOARD_ADMIN_BEARER_TOKEN
})

describe('Shipyard server helpers', () => {
  it('parses and serializes workspace ship registry files', async () => {
    const registryPath = path.resolve(
      process.cwd(),
      '..',
      'data/scripts/ships/registry.lua',
    )
    const source = await fs.readFile(registryPath, 'utf8')
    const entries = parseShipRegistrySource(source)
    expect(entries.some((entry) => entry.shipId === 'ship.corvette')).toBe(true)
    expect(parseShipRegistrySource(serializeShipRegistry(entries))).toEqual(
      entries,
    )
  })

  it('parses and serializes workspace module registry files', async () => {
    const registryPath = path.resolve(
      process.cwd(),
      '..',
      'data/scripts/ship_modules/registry.lua',
    )
    const source = await fs.readFile(registryPath, 'utf8')
    const entries = parseModuleRegistrySource(source)
    expect(
      entries.some((entry) => entry.moduleId === 'module.engine.main_mk1'),
    ).toBe(true)
    expect(parseModuleRegistrySource(serializeModuleRegistry(entries))).toEqual(
      entries,
    )
  })

  it('preserves draft status while loading catalog entries', async () => {
    mockScriptFetch({
      'ships/test.lua': shipSource.replace('Test Ship', 'Draft Ship'),
    })
    const catalog = await loadShipyardCatalog()
    const ship = catalog.ships[0]
    expect(ship.displayName).toBe('Draft Ship')
    expect(ship.hasDraft).toBe(true)
  })

  it('writes ship and registry drafts together', async () => {
    const writes: Array<{ scriptPath: string; source: string }> = []
    mockScriptFetch({}, writes)
    const catalog = await loadShipyardCatalog()
    const definition = {
      ...catalog.ships[0].definition,
      display_name: 'Saved Ship',
    }

    await saveShipyardShipDraft(
      { definition, spawnEnabled: false },
      'test-token',
    )

    expect(writes.map((write) => write.scriptPath)).toEqual([
      'ships/test.lua',
      'ships/registry.lua',
    ])
    expect(writes[0].source).toContain('display_name = "Saved Ship"')
    expect(writes[1].source).toContain('spawn_enabled = false')
  })

  it('validates bad ship payloads', () => {
    const parsed = shipyardShipDraftBodySchema.safeParse({
      definition: {
        ship_id: 'ship.test',
        bundle_id: 'ship.test',
        script_path: 'ships/test.lua',
        display_name: 'Test',
        entity_labels: ['Ship'],
        tags: [],
        visual: {
          visual_asset_id: 'test_ship',
          map_icon_asset_id: 'map_icon_ship_svg',
        },
        dimensions: {
          length_m: 10,
          width_m: 5,
          height_m: 2,
          collision_mode: 'Aabb',
          collision_from_texture: false,
        },
        root: {
          base_mass_kg: 1,
          max_velocity_mps: 1,
          health_pool: {},
          destructible: {},
          flight_computer: {},
          flight_tuning: {},
          visibility_range_buff_m: {},
        },
        hardpoints: [
          {
            hardpoint_id: 'bad_z',
            display_name: 'Bad Z',
            slot_kind: 'engine',
            offset_m: [0, 0, 1],
            local_rotation_rad: 0,
            compatible_tags: [],
          },
        ],
        mounted_modules: [],
      },
      spawnEnabled: true,
    })
    expect(parsed.success).toBe(false)
  })
})

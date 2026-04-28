import { describe, expect, it } from 'vitest'
import {
  getComponentPayloadFromNode,
  getSchemaFieldValue,
  parseGeneratedComponentRegistryResource,
  resolveComponentRegistryEntry,
  resolveShaderRegistryEntry,
  resolveShaderRegistryEntryForComponent,
  setSchemaFieldValue,
} from './registry'

describe('component schema registry helpers', () => {
  const registry = parseGeneratedComponentRegistryResource({
    entries: [
      {
        component_kind: 'ammo_count',
        type_path: 'sidereal_game::components::ammo_count::AmmoCount',
        replication_visibility: ['OwnerOnly'],
        editor_schema: {
          root_value_kind: 'Struct',
          fields: [
            {
              field_path: 'current',
              field_name: 'current',
              display_name: 'Current',
              value_kind: 'UnsignedInteger',
              min: 0,
              max: null,
              step: 1,
              unit: null,
              options: [],
            },
          ],
        },
      },
      {
        component_kind: 'stellar_light_source',
        type_path:
          'sidereal_game::components::stellar_light_source::StellarLightSource',
        replication_visibility: ['Public'],
        editor_schema: {
          root_value_kind: 'Struct',
          fields: [
            {
              field_path: 'enabled',
              field_name: 'enabled',
              display_name: 'Enabled',
              value_kind: 'Bool',
              min: null,
              max: null,
              step: null,
              unit: null,
              options: [],
            },
            {
              field_path: 'color_rgb',
              field_name: 'color_rgb',
              display_name: 'Color RGB',
              value_kind: 'Vec3',
              min: 0,
              max: 1,
              step: 0.01,
              unit: null,
              options: [],
            },
            {
              field_path: 'intensity',
              field_name: 'intensity',
              display_name: 'Intensity',
              value_kind: 'Float',
              min: 0,
              max: null,
              step: 0.01,
              unit: null,
              options: [],
            },
          ],
        },
      },
      {
        component_kind: 'avian_rigid_body',
        type_path: 'avian2d::dynamics::rigid_body::RigidBody',
        replication_visibility: ['Public'],
        editor_schema: {
          root_value_kind: 'Struct',
          fields: [
            {
              field_path: 'RigidBody',
              field_name: 'RigidBody',
              display_name: 'RigidBody',
              value_kind: 'Enum',
              min: null,
              max: null,
              step: null,
              unit: null,
              options: ['Dynamic', 'Static', 'Kinematic'],
            },
          ],
        },
      },
    ],
    shader_entries: [
      {
        asset_id: 'planet_visual_wgsl',
        source_path: 'shaders/planet_visual.wgsl',
        shader_family: 'world_polygon_planet',
        dependencies: ['noise_lut_png'],
        bootstrap_required: true,
        uniform_schema: [
          {
            field_path: 'atmosphere_alpha',
            display_name: 'Atmosphere Alpha',
            description: 'Alpha for atmosphere rim.',
            value_kind: 'Float',
            min: 0,
            max: 1,
            step: 0.01,
            options: [],
            default_value_json: '0.48',
            group: 'Atmosphere',
          },
          {
            field_path: 'blend_mode',
            display_name: 'Blend Mode',
            description: null,
            value_kind: 'Enum',
            min: null,
            max: null,
            step: null,
            options: [
              { value: 'screen', label: 'Screen' },
              { value: 'add', label: 'Add' },
            ],
            default_value_json: '"screen"',
            group: null,
          },
        ],
        presets: [
          {
            preset_id: 'earth_like',
            display_name: 'Earth-like',
            description: null,
            values_json: '{"atmosphere_alpha":0.48}',
          },
        ],
      },
    ],
  })
  if (!registry) {
    throw new Error('expected generated registry fixture to parse')
  }

  it('parses generated component registry payloads', () => {
    expect(registry.entries).toHaveLength(3)
    expect(registry.shader_entries).toHaveLength(1)
    expect(registry.entries[0]?.component_kind).toBe('ammo_count')
    expect(registry.shader_entries[0]?.asset_id).toBe('planet_visual_wgsl')
    expect(
      registry.shader_entries[0]?.uniform_schema[1]?.options[0]?.value,
    ).toBe('screen')
  })

  it('resolves shader registry entries by asset id and source path', () => {
    expect(
      resolveShaderRegistryEntry(registry, {
        assetId: 'planet_visual_wgsl',
        sourcePath: null,
      }),
    ).toMatchObject({ asset_id: 'planet_visual_wgsl' })

    expect(
      resolveShaderRegistryEntry(registry, {
        assetId: null,
        sourcePath: 'data/shaders/planet_visual.wgsl',
      }),
    ).toMatchObject({ source_path: 'shaders/planet_visual.wgsl' })

    expect(
      resolveShaderRegistryEntryForComponent(
        registry,
        'sidereal_game::components::planet_body_shader_settings::PlanetBodyShaderSettings',
      ),
    ).toMatchObject({ asset_id: 'planet_visual_wgsl' })
  })

  it('resolves live and database component nodes against registry metadata', () => {
    expect(
      resolveComponentRegistryEntry(
        {
          id: 'live',
          label: 'AmmoCount',
          kind: 'Component',
          properties: {
            typePath: 'sidereal_game::components::ammo_count::AmmoCount',
            value: { current: 5 },
          },
        },
        registry,
      )?.component_kind,
    ).toBe('ammo_count')

    expect(
      resolveComponentRegistryEntry(
        {
          id: 'db',
          label: 'Ammo Count',
          kind: 'Component',
          properties: {
            component_kind: 'ammo_count',
            sidereal_game__components__ammo_count__AmmoCount: { current: 8 },
          },
        },
        registry,
      )?.type_path,
    ).toBe('sidereal_game::components::ammo_count::AmmoCount')
  })

  it('extracts persisted component envelopes and updates both nested and root fields', () => {
    const ammoEntry = registry.entries[0]
    const rigidBodyEntry = registry.entries[1]
    expect(ammoEntry).toBeTruthy()
    expect(rigidBodyEntry).toBeTruthy()

    const ammoPayload = getComponentPayloadFromNode(
      {
        id: 'db',
        label: 'Ammo Count',
        kind: 'Component',
        properties: {
          component_kind: 'ammo_count',
          sidereal_game__components__ammo_count__AmmoCount: { current: 8 },
        },
      },
      ammoEntry,
    )
    expect(ammoPayload).toEqual({ current: 8 })

    const ammoField = ammoEntry.editor_schema.fields[0]
    expect(ammoField).toBeTruthy()
    expect(getSchemaFieldValue(ammoPayload, ammoField, 1)).toBe(8)
    expect(setSchemaFieldValue(ammoPayload, ammoField, 12, 1)).toEqual({
      current: 12,
    })

    const rigidBodyField = rigidBodyEntry.editor_schema.fields[0]
    expect(rigidBodyField).toBeTruthy()
    expect(getSchemaFieldValue('Dynamic', rigidBodyField, 1)).toBe('Dynamic')
    expect(setSchemaFieldValue('Dynamic', rigidBodyField, 'Static', 1)).toBe(
      'Static',
    )
  })

  it('recovers component payloads from AGE-truncated type path envelope keys', () => {
    const stellarEntry = registry.entries.find(
      (entry) => entry.component_kind === 'stellar_light_source',
    )
    expect(stellarEntry).toBeTruthy()

    const payload = getComponentPayloadFromNode(
      {
        id: 'stellar',
        label: 'Stellar Light Source',
        kind: 'Component',
        properties: {
          component_id: 'star:stellar_light_source',
          component_kind: 'stellar_light_source',
          sidereal_game__components__stellar_light_source__StellarLightSo: {
            enabled: true,
            color_rgb: [0.12, 0.38, 1],
            intensity: 1.25,
          },
        },
      },
      stellarEntry ?? null,
    )

    expect(payload).toEqual({
      enabled: true,
      color_rgb: [0.12, 0.38, 1],
      intensity: 1.25,
    })
  })

  it('prefers top-level component fields over stale nested envelopes', () => {
    const stellarEntry = registry.entries.find(
      (entry) => entry.component_kind === 'stellar_light_source',
    )
    expect(stellarEntry).toBeTruthy()

    const payload = getComponentPayloadFromNode(
      {
        id: 'stellar',
        label: 'Stellar Light Source',
        kind: 'Component',
        properties: {
          component_id: 'star:stellar_light_source',
          component_kind: 'stellar_light_source',
          last_tick: 1777340154057270500,
          enabled: true,
          color_rgb: [0.12, 0.38, 1],
          intensity: 1.25,
          sidereal_game__components__stellar_light_source__StellarLightSo: {
            enabled: true,
            color_rgb: [1, 0.58, 0.12],
            intensity: 0,
          },
        },
      },
      stellarEntry ?? null,
    )

    expect(payload).toEqual({
      enabled: true,
      color_rgb: [0.12, 0.38, 1],
      intensity: 1.25,
    })
  })
})

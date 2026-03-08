import { describe, expect, it } from 'vitest'
import {
  getComponentPayloadFromNode,
  getSchemaFieldValue,
  parseGeneratedComponentRegistryResource,
  resolveComponentRegistryEntry,
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
  })

  it('parses generated component registry payloads', () => {
    expect(registry?.entries).toHaveLength(2)
    expect(registry?.entries[0]?.component_kind).toBe('ammo_count')
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
    const ammoEntry = registry?.entries[0]
    const rigidBodyEntry = registry?.entries[1]
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
      ammoEntry ?? null,
    )
    expect(ammoPayload).toEqual({ current: 8 })

    const ammoField = ammoEntry?.editor_schema.fields[0]
    expect(ammoField).toBeTruthy()
    if (!ammoField) {
      throw new Error('ammoField missing')
    }
    expect(getSchemaFieldValue(ammoPayload, ammoField, 1)).toBe(8)
    expect(setSchemaFieldValue(ammoPayload, ammoField, 12, 1)).toEqual({
      current: 12,
    })

    const rigidBodyField = rigidBodyEntry?.editor_schema.fields[0]
    expect(rigidBodyField).toBeTruthy()
    if (!rigidBodyField) {
      throw new Error('rigidBodyField missing')
    }
    expect(getSchemaFieldValue('Dynamic', rigidBodyField, 1)).toBe('Dynamic')
    expect(setSchemaFieldValue('Dynamic', rigidBodyField, 'Static', 1)).toBe(
      'Static',
    )
  })
})

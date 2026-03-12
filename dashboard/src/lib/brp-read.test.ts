import { describe, expect, it } from 'vitest'
import {
  buildBrpReadUrl,
  getBrpReadResourceParam,
  isReadOnlyBrpMethod,
} from '@/lib/brp-read'

describe('brp read helpers', () => {
  it('recognizes allowed read-only BRP methods', () => {
    expect(isReadOnlyBrpMethod('world.list_resources')).toBe(true)
    expect(isReadOnlyBrpMethod('world.insert_components')).toBe(false)
  })

  it('extracts the resource parameter from read-only payloads', () => {
    expect(
      getBrpReadResourceParam({
        resource: 'sidereal_game::components::registry::GeneratedComponentRegistry',
      }),
    ).toBe('sidereal_game::components::registry::GeneratedComponentRegistry')
    expect(getBrpReadResourceParam({ resource: 17 })).toBeNull()
  })

  it('builds GET URLs only for supported read-only requests', () => {
    expect(
      buildBrpReadUrl({
        method: 'world.list_resources',
        port: 15713,
        target: 'server',
      }),
    ).toBe('/api/brp?port=15713&target=server&method=world.list_resources')

    expect(
      buildBrpReadUrl({
        method: 'world.get_resources',
        params: { resource: 'foo::Bar' },
        port: 15714,
        target: 'client',
      }),
    ).toBe(
      '/api/brp?port=15714&target=client&method=world.get_resources&resource=foo%3A%3ABar',
    )

    expect(
      buildBrpReadUrl({
        method: 'world.insert_components',
        params: { resource: 'foo::Bar' },
        port: 15713,
        target: 'server',
      }),
    ).toBeNull()
  })
})

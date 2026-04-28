import { describe, expect, it } from 'vitest'
import { buildEntityClipboardExport } from './DetailPanel'
import type {
  ExpandedNode,
  GraphEdge,
  GraphNode,
  WorldEntity,
} from '@/components/grid/types'

type EntityOverrides = Partial<WorldEntity> & Pick<WorldEntity, 'id' | 'name'>

function makeEntity({ id, name, ...overrides }: EntityOverrides): WorldEntity {
  return {
    id,
    name,
    kind: overrides.kind ?? 'entity',
    shardId: overrides.shardId ?? 1,
    x: overrides.x ?? 0,
    y: overrides.y ?? 0,
    vx: overrides.vx ?? 0,
    vy: overrides.vy ?? 0,
    sampledAtMs: overrides.sampledAtMs ?? 0,
    componentCount: overrides.componentCount ?? 0,
    ...overrides,
  }
}

describe('buildEntityClipboardExport', () => {
  it('walks nested hardpoint descendants whose parent links reference entity GUIDs', () => {
    const entities: Array<WorldEntity> = [
      makeEntity({
        id: 'bevy-ship',
        name: 'Ship',
        kind: 'ship',
        entityGuid: 'ship-guid',
      }),
      makeEntity({
        id: 'bevy-hardpoint',
        name: 'Fore Hardpoint',
        kind: 'hardpoint',
        entityGuid: 'hardpoint-guid',
        parentEntityId: 'ship-guid',
      }),
      makeEntity({
        id: 'bevy-module',
        name: 'Mounted Engine',
        kind: 'module',
        entityGuid: 'module-guid',
        parentEntityId: 'hardpoint-guid',
        componentCount: 1,
      }),
      makeEntity({
        id: 'bevy-nested-child',
        name: 'Nested Child',
        kind: 'module',
        entityGuid: 'nested-guid',
        parentEntityId: 'module-guid',
      }),
    ]
    const graphNodes = new Map<string, GraphNode>([
      [
        'component-engine',
        {
          id: 'component-engine',
          label: 'Engine',
          kind: 'Component',
          properties: {
            typePath: 'sidereal_game::components::engine::Engine',
            value: { thrust_n: 1200 },
          },
        },
      ],
    ])
    const graphEdges: Array<GraphEdge> = [
      {
        id: 'edge-engine',
        from: 'bevy-module',
        to: 'component-engine',
        label: 'HAS_COMPONENT',
        properties: {},
      },
    ]

    const result = buildEntityClipboardExport(
      'bevy-ship',
      entities,
      new Map<string, ExpandedNode>(),
      graphNodes,
      graphEdges,
    )

    const hardpoint = result?.entityTree.children[0]
    const module = hardpoint?.children[0]
    const nested = module?.children[0]

    expect(hardpoint?.entity?.id).toBe('bevy-hardpoint')
    expect(module?.entity?.id).toBe('bevy-module')
    expect(module?.components).toHaveLength(1)
    expect(module?.components[0]?.typePath).toBe(
      'sidereal_game::components::engine::Engine',
    )
    expect(nested?.entity?.id).toBe('bevy-nested-child')
  })

  it('uses hardpoint and mount relationship edges as fallback hierarchy links', () => {
    const entities: Array<WorldEntity> = [
      makeEntity({ id: 'ship', name: 'Ship', kind: 'ship' }),
      makeEntity({
        id: 'hardpoint',
        name: 'Utility Hardpoint',
        kind: 'hardpoint',
      }),
      makeEntity({ id: 'sensor', name: 'Sensor Module', kind: 'module' }),
    ]
    const graphEdges: Array<GraphEdge> = [
      {
        id: 'edge-hardpoint',
        from: 'ship',
        to: 'hardpoint',
        label: 'HAS_HARDPOINT',
        properties: {},
      },
      {
        id: 'edge-mounted',
        from: 'sensor',
        to: 'hardpoint',
        label: 'MOUNTED_ON',
        properties: {},
      },
    ]

    const result = buildEntityClipboardExport(
      'ship',
      entities,
      new Map<string, ExpandedNode>(),
      new Map<string, GraphNode>(),
      graphEdges,
    )

    const hardpoint = result?.entityTree.children[0]
    const mountedChild = hardpoint?.children[0]

    expect(hardpoint?.entity?.id).toBe('hardpoint')
    expect(mountedChild?.entity?.id).toBe('sensor')
  })
})

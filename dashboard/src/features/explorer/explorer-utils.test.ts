import { describe, expect, it } from 'vitest'
import {
  buildEntitiesFromGraph,
  parseSelectedPlayerVisibilityOverlay,
  shouldHideControlledPlayerMapIcon,
} from './explorer-utils'

describe('parseSelectedPlayerVisibilityOverlay', () => {
  it('reads current visibility_sources payloads for player visibility debug', () => {
    const selectedId = 'player-1'
    const graphNodes = new Map([
      [
        'grid-component',
        {
          id: 'grid-component',
          label: 'Visibility Spatial Grid',
          kind: 'Component',
          properties: {
            typePath:
              'sidereal_game::components::visibility_spatial_grid::VisibilitySpatialGrid',
            value: {
              cell_size_m: 200,
              delivery_range_m: 1200,
              queried_cells: [
                { x: 3, y: 4 },
                { x: 4, y: 4 },
              ],
            },
          },
        },
      ],
      [
        'disclosure-component',
        {
          id: 'disclosure-component',
          label: 'Visibility Disclosure',
          kind: 'Component',
          properties: {
            typePath:
              'sidereal_game::components::visibility_disclosure::VisibilityDisclosure',
            value: {
              visibility_sources: [{ x: 10, y: 20, z: 5, range_m: 900 }],
            },
          },
        },
      ],
    ])
    const graphEdges = [
      {
        id: 'edge-grid',
        from: selectedId,
        to: 'grid-component',
        label: 'HAS_COMPONENT',
        properties: {},
      },
      {
        id: 'edge-disclosure',
        from: selectedId,
        to: 'disclosure-component',
        label: 'HAS_COMPONENT',
        properties: {},
      },
    ]

    expect(
      parseSelectedPlayerVisibilityOverlay(selectedId, graphNodes, graphEdges),
    ).toEqual({
      cell_size_m: 200,
      delivery_range_m: 1200,
      queried_cells: [
        { x: 3, y: 4 },
        { x: 4, y: 4 },
      ],
      visibility_sources: [{ x: 10, y: 20, z: 5, range_m: 900 }],
      explored_cell_size_m: null,
      explored_cells: [],
    })
  })

  it('keeps legacy scanner_sources compatibility for older snapshots', () => {
    const selectedId = 'player-1'
    const graphNodes = new Map([
      [
        'grid-component',
        {
          id: 'grid-component',
          label: 'Visibility Spatial Grid',
          kind: 'Component',
          properties: {
            typePath:
              'sidereal_game::components::visibility_spatial_grid::VisibilitySpatialGrid',
            value: {
              cell_size_m: 100,
              queried_cells: [{ x: 0, y: 0 }],
            },
          },
        },
      ],
      [
        'disclosure-component',
        {
          id: 'disclosure-component',
          label: 'Visibility Disclosure',
          kind: 'Component',
          properties: {
            typePath:
              'sidereal_game::components::visibility_disclosure::VisibilityDisclosure',
            value: {
              scanner_sources: [{ x: 1, y: 2, range_m: 300 }],
            },
          },
        },
      ],
    ])
    const graphEdges = [
      {
        id: 'edge-grid',
        from: selectedId,
        to: 'grid-component',
        label: 'HAS_COMPONENT',
        properties: {},
      },
      {
        id: 'edge-disclosure',
        from: selectedId,
        to: 'disclosure-component',
        label: 'HAS_COMPONENT',
        properties: {},
      },
    ]

    expect(
      parseSelectedPlayerVisibilityOverlay(selectedId, graphNodes, graphEdges),
    ).toMatchObject({
      queried_cells: [{ x: 0, y: 0 }],
      visibility_sources: [{ x: 1, y: 2, range_m: 300 }],
    })
  })
})

describe('buildEntitiesFromGraph', () => {
  it('accepts project EntityGuid values that are not RFC version-constrained UUIDs', () => {
    const guid = '0012ebad-0000-0000-0000-000000000012'
    const graph = {
      graph: 'sidereal',
      nodes: [
        {
          id: '25769803075',
          label: 'Helion',
          kind: 'Entity',
          properties: {
            name: 'Helion',
            shard_id: 1,
          },
        },
        {
          id: 'component-entity-guid',
          label: 'EntityGuid',
          kind: 'Component',
          properties: {
            component_kind: 'entity_guid',
            typePath: 'sidereal_game::components::entity_guid::EntityGuid',
            value: guid,
          },
        },
      ],
      edges: [
        {
          id: 'edge-entity-guid',
          from: '25769803075',
          to: 'component-entity-guid',
          label: 'HAS_COMPONENT',
          properties: {},
        },
      ],
    }

    const entities = buildEntitiesFromGraph(graph)
    expect(entities).toHaveLength(1)
    expect(entities[0]?.entityGuid).toBe(guid)
  })

  it('reads controlled entity guid values from player graph components', () => {
    const playerGuid = '0012ebad-0000-0000-0000-000000000012'
    const shipGuid = '0012ebad-0000-0000-0000-000000000099'
    const graph = {
      graph: 'sidereal',
      nodes: [
        {
          id: 'player-entity',
          label: 'Pilot',
          kind: 'Entity',
          properties: {
            entity_labels: ['Entity', 'player'],
          },
        },
        {
          id: 'component-entity-guid',
          label: 'EntityGuid',
          kind: 'Component',
          properties: {
            component_kind: 'entity_guid',
            typePath: 'sidereal_game::components::entity_guid::EntityGuid',
            value: playerGuid,
          },
        },
        {
          id: 'component-controlled-entity-guid',
          label: 'ControlledEntityGuid',
          kind: 'Component',
          properties: {
            component_kind: 'controlled_entity_guid',
            typePath:
              'sidereal_game::components::controlled_entity_guid::ControlledEntityGuid',
            value: shipGuid,
          },
        },
      ],
      edges: [
        {
          id: 'edge-entity-guid',
          from: 'player-entity',
          to: 'component-entity-guid',
          label: 'HAS_COMPONENT',
          properties: {},
        },
        {
          id: 'edge-controlled-entity-guid',
          from: 'player-entity',
          to: 'component-controlled-entity-guid',
          label: 'HAS_COMPONENT',
          properties: {},
        },
      ],
    }

    const entities = buildEntitiesFromGraph(graph)
    expect(entities[0]?.controlledEntityGuid).toBe(shipGuid)
    expect(shouldHideControlledPlayerMapIcon(entities[0])).toBe(true)
  })

  it('keeps self-controlled player anchors visible on the map', () => {
    expect(
      shouldHideControlledPlayerMapIcon({
        id: 'player-entity',
        name: 'Pilot',
        kind: 'player',
        entityGuid: '0012ebad-0000-0000-0000-000000000012',
        controlledEntityGuid: '0012ebad-0000-0000-0000-000000000012',
        shardId: 1,
        x: 0,
        y: 0,
        vx: 0,
        vy: 0,
        sampledAtMs: 0,
        componentCount: 0,
      }),
    ).toBe(false)
  })
})

import { afterEach, describe, expect, it, vi } from 'vitest'

import { getLiveWorldSnapshot } from './brp'

describe('getLiveWorldSnapshot', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('uses EntityGuid rather than ControlledEntityGuid for live entity identity', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      statusText: 'OK',
      text: () =>
        JSON.stringify({
          jsonrpc: '2.0',
          id: 'test',
          result: [
            {
              entity: 123,
              components: {
                'sidereal_game::components::controlled_entity_guid::ControlledEntityGuid':
                  '17673582-a203-4609-9f96-5a35633d89a5',
                'sidereal_game::components::entity_guid::EntityGuid':
                  '2782e15e-52d9-47fc-995a-1f4e612c7cfe',
                'sidereal_game::components::display_name::DisplayName':
                  'pilot@example.com',
              },
            },
          ],
        }),
    })
    vi.stubGlobal('fetch', fetchMock)

    const snapshot = await getLiveWorldSnapshot({ target: 'server', port: 15713 })

    expect(snapshot.entities).toHaveLength(1)
    expect(snapshot.entities[0]?.entityGuid).toBe(
      '2782e15e-52d9-47fc-995a-1f4e612c7cfe',
    )
  })
})

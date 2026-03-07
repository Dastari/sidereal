import { describe, expect, it } from 'vitest'
import { parseAssetRegistryEntries } from './shader-workbench.server'

describe('parseAssetRegistryEntries', () => {
  it('extracts shader dependencies from the Lua asset registry', () => {
    const entries = parseAssetRegistryEntries(`
      local AssetRegistry = {}
      AssetRegistry.assets = {
        {
          asset_id = "space_background_wgsl",
          source_path = "shaders/space_background.wgsl",
          content_type = "text/plain; charset=utf-8",
          shader_role = "space_background",
          dependencies = {
            "space_bg_flare_white_png",
            "space_bg_flare_blue_png",
          },
          bootstrap_required = true,
        },
        {
          asset_id = "asteroid_wgsl",
          source_path = "shaders/asteroid.wgsl",
          content_type = "text/plain; charset=utf-8",
          shader_role = "asteroid_sprite",
          dependencies = {},
          bootstrap_required = false,
        },
      }
      return AssetRegistry
    `)

    expect(entries).toEqual([
      {
        assetId: 'space_background_wgsl',
        sourcePath: 'shaders/space_background.wgsl',
        shaderRole: 'space_background',
        bootstrapRequired: true,
        dependencies: ['space_bg_flare_white_png', 'space_bg_flare_blue_png'],
      },
      {
        assetId: 'asteroid_wgsl',
        sourcePath: 'shaders/asteroid.wgsl',
        shaderRole: 'asteroid_sprite',
        bootstrapRequired: false,
        dependencies: [],
      },
    ])
  })
})

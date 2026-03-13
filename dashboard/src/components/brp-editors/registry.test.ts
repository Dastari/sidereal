import { describe, expect, it } from 'vitest'
import { getEditorForNode } from './registry'

describe('BRP editor registry', () => {
  it('falls back to generated schema for space background shader settings', () => {
    expect(
      getEditorForNode({
        id: 'space-bg',
        label: 'SpaceBackgroundShaderSettings',
        kind: 'Component',
        properties: {},
      }),
    ).toBeNull()
  })

  it('keeps bespoke editors for components that still require them', () => {
    expect(
      getEditorForNode({
        id: 'planet',
        label: 'PlanetBodyShaderSettings',
        kind: 'Component',
        properties: {},
      }),
    ).not.toBeNull()
  })
})

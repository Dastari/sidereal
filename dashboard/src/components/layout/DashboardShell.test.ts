import { describe, expect, it } from 'vitest'
import { getActiveTool, toolNavItems } from './DashboardShell'

describe('DashboardShell navigation', () => {
  it('includes the game client tool entry', () => {
    expect(toolNavItems.some((item) => item.to === '/game-client')).toBe(true)
  })

  it('includes the shipyard tool entry', () => {
    expect(toolNavItems.some((item) => item.to === '/shipyard')).toBe(true)
  })

  it('resolves the game client route as active', () => {
    expect(getActiveTool('/game-client').label).toBe('Game Client')
    expect(getActiveTool('/game-client/session').label).toBe('Game Client')
  })

  it('resolves the shipyard route as active', () => {
    expect(getActiveTool('/shipyard').label).toBe('Shipyard')
    expect(getActiveTool('/shipyard/corvette').label).toBe('Shipyard')
  })
})

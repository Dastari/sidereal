import { describe, expect, it } from 'vitest'
import { getActiveTool, toolNavItems } from './DashboardShell'

describe('DashboardShell navigation', () => {
  it('includes the game client tool entry', () => {
    expect(toolNavItems.some((item) => item.to === '/game-client')).toBe(true)
  })

  it('resolves the game client route as active', () => {
    expect(getActiveTool('/game-client').label).toBe('Game Client')
    expect(getActiveTool('/game-client/session').label).toBe('Game Client')
  })
})

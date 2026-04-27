import { describe, expect, it } from 'vitest'
import { isDashboardAdminRoute } from '@/lib/dashboard-auth'

describe('dashboard route auth policy', () => {
  it('keeps My Account as the non-admin authenticated route', () => {
    expect(isDashboardAdminRoute('/')).toBe(false)
    expect(isDashboardAdminRoute('/?tab=characters')).toBe(false)
  })

  it('treats dashboard tools as admin-only routes', () => {
    expect(isDashboardAdminRoute('/database')).toBe(true)
    expect(isDashboardAdminRoute('/game-world/abc')).toBe(true)
  })
})

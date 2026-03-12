import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  createDashboardAdminSessionCookie,
  getDashboardSession,
  requireDashboardAdmin,
  verifyDashboardAdminPassword,
} from '@/server/dashboard-auth'

const ORIGINAL_PASSWORD = process.env.SIDEREAL_DASHBOARD_ADMIN_PASSWORD
const ORIGINAL_SECRET = process.env.SIDEREAL_DASHBOARD_SESSION_SECRET

function restoreEnv() {
  if (ORIGINAL_PASSWORD === undefined) {
    delete process.env.SIDEREAL_DASHBOARD_ADMIN_PASSWORD
  } else {
    process.env.SIDEREAL_DASHBOARD_ADMIN_PASSWORD = ORIGINAL_PASSWORD
  }

  if (ORIGINAL_SECRET === undefined) {
    delete process.env.SIDEREAL_DASHBOARD_SESSION_SECRET
  } else {
    process.env.SIDEREAL_DASHBOARD_SESSION_SECRET = ORIGINAL_SECRET
  }
}

afterEach(() => {
  restoreEnv()
  vi.useRealTimers()
})

describe('dashboard auth', () => {
  it('verifies the configured admin password', () => {
    process.env.SIDEREAL_DASHBOARD_ADMIN_PASSWORD = 'sidereal-test-password'

    expect(verifyDashboardAdminPassword('sidereal-test-password')).toBe(true)
    expect(verifyDashboardAdminPassword('wrong-password')).toBe(false)
  })

  it('accepts same-origin requests with a valid admin session cookie', () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-03-12T00:00:00Z'))
    process.env.SIDEREAL_DASHBOARD_ADMIN_PASSWORD = 'sidereal-test-password'
    process.env.SIDEREAL_DASHBOARD_SESSION_SECRET = 'sidereal-test-secret'

    const seedRequest = new Request(
      'http://127.0.0.1:3000/api/dashboard-session',
    )
    const cookie = createDashboardAdminSessionCookie(seedRequest)
      .split(';')[0]
      .trim()

    const request = new Request('http://127.0.0.1:3000/api/graph', {
      headers: {
        cookie,
        origin: 'http://127.0.0.1:3000',
      },
    })

    expect(getDashboardSession(request)).not.toBeNull()
    expect(requireDashboardAdmin(request)).toBeNull()
  })

  it('rejects mutation requests without an admin session', async () => {
    process.env.SIDEREAL_DASHBOARD_ADMIN_PASSWORD = 'sidereal-test-password'

    const request = new Request('http://127.0.0.1:3000/api/graph', {
      headers: {
        origin: 'http://127.0.0.1:3000',
      },
    })

    const response = requireDashboardAdmin(request)

    expect(response?.status).toBe(403)
    await expect(response?.json()).resolves.toMatchObject({
      error: 'Dashboard admin session required',
    })
  })
})

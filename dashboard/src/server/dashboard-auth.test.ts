import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  createDashboardSessionCookie,
  getDashboardSession,
  requireDashboardAdmin,
} from '@/server/dashboard-auth'

const ORIGINAL_SECRET = process.env.SIDEREAL_DASHBOARD_SESSION_SECRET

function restoreEnv() {
  if (ORIGINAL_SECRET === undefined) {
    delete process.env.SIDEREAL_DASHBOARD_SESSION_SECRET
  } else {
    process.env.SIDEREAL_DASHBOARD_SESSION_SECRET = ORIGINAL_SECRET
  }
}

function testSession(
  overrides: Partial<Parameters<typeof createDashboardSessionCookie>[1]> = {},
) {
  return {
    accountId: '11111111-1111-1111-1111-111111111111',
    email: 'admin@example.com',
    accessToken: 'header.payload.signature',
    refreshToken: 'refresh-token',
    accessTokenExpiresAtEpochS: Math.floor(Date.now() / 1000) + 300,
    roles: ['admin'],
    scopes: ['admin:spawn', 'dashboard:access'],
    mfaVerified: true,
    ...overrides,
  }
}

afterEach(() => {
  restoreEnv()
  vi.useRealTimers()
})

describe('dashboard auth', () => {
  it('accepts same-origin requests with a valid gateway-backed admin session cookie', () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-03-12T00:00:00Z'))
    process.env.SIDEREAL_DASHBOARD_SESSION_SECRET = 'sidereal-test-secret'

    const seedRequest = new Request(
      'http://127.0.0.1:3000/api/dashboard-session',
    )
    const cookie = createDashboardSessionCookie(seedRequest, testSession())
      .split(';')[0]
      .trim()

    const request = new Request('http://127.0.0.1:3000/api/graph', {
      method: 'POST',
      headers: {
        cookie,
        origin: 'http://127.0.0.1:3000',
      },
    })

    expect(getDashboardSession(request)).not.toBeNull()
    expect(requireDashboardAdmin(request, 'admin:spawn')).toBeNull()
  })

  it('rejects mutation requests without an account session', async () => {
    process.env.SIDEREAL_DASHBOARD_SESSION_SECRET = 'sidereal-test-secret'

    const request = new Request('http://127.0.0.1:3000/api/graph', {
      method: 'POST',
      headers: {
        origin: 'http://127.0.0.1:3000',
      },
    })

    const response = requireDashboardAdmin(request)

    expect(response?.status).toBe(403)
    await expect(response?.json()).resolves.toMatchObject({
      error: 'Dashboard account session required',
    })
  })

  it('rejects admin sessions without the required route scope', async () => {
    process.env.SIDEREAL_DASHBOARD_SESSION_SECRET = 'sidereal-test-secret'
    const seedRequest = new Request(
      'http://127.0.0.1:3000/api/dashboard-session',
    )
    const cookie = createDashboardSessionCookie(
      seedRequest,
      testSession({ scopes: ['dashboard:access', 'scripts:read'] }),
    )
      .split(';')[0]
      .trim()
    const request = new Request('http://127.0.0.1:3000/api/graph', {
      method: 'POST',
      headers: {
        cookie,
        origin: 'http://127.0.0.1:3000',
      },
    })

    const response = requireDashboardAdmin(request, 'admin:spawn')

    expect(response?.status).toBe(403)
    await expect(response?.json()).resolves.toMatchObject({
      error: 'Dashboard scope required: admin:spawn',
    })
  })

  it('rejects regular account sessions for admin routes', async () => {
    process.env.SIDEREAL_DASHBOARD_SESSION_SECRET = 'sidereal-test-secret'
    const seedRequest = new Request(
      'http://127.0.0.1:3000/api/dashboard-session',
    )
    const cookie = createDashboardSessionCookie(
      seedRequest,
      testSession({
        email: 'pilot@example.com',
        roles: [],
        scopes: [],
        mfaVerified: false,
      }),
    )
      .split(';')[0]
      .trim()
    const request = new Request('http://127.0.0.1:3000/api/graph', {
      method: 'POST',
      headers: {
        cookie,
        origin: 'http://127.0.0.1:3000',
      },
    })

    const response = requireDashboardAdmin(request, 'admin:spawn')

    expect(response?.status).toBe(403)
    await expect(response?.json()).resolves.toMatchObject({
      error: 'Dashboard admin role and verified MFA required',
    })
  })
})

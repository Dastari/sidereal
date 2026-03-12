import { createHmac, timingSafeEqual } from 'node:crypto'

const COOKIE_NAME = 'sidereal_dashboard_admin'
const SESSION_TTL_SECONDS = 60 * 60 * 8
const SESSION_VERSION = 'v1'

type DashboardSession = {
  role: 'admin'
  expiresAtEpochS: number
}

function getAdminPassword(): string | null {
  const value = process.env.SIDEREAL_DASHBOARD_ADMIN_PASSWORD?.trim()
  return value && value.length > 0 ? value : null
}

function getSessionSecret(): string | null {
  const explicitSecret = process.env.SIDEREAL_DASHBOARD_SESSION_SECRET?.trim()
  if (explicitSecret && explicitSecret.length > 0) {
    return explicitSecret
  }
  return getAdminPassword()
}

function parseCookies(header: string | null): Map<string, string> {
  const cookies = new Map<string, string>()
  if (!header) {
    return cookies
  }
  for (const entry of header.split(';')) {
    const [rawName, ...rawValueParts] = entry.trim().split('=')
    if (!rawName) continue
    cookies.set(rawName, rawValueParts.join('='))
  }
  return cookies
}

function signPayload(payload: string, secret: string): string {
  return createHmac('sha256', secret).update(payload).digest('base64url')
}

function createSessionValue(expiresAtEpochS: number, secret: string): string {
  const payload = `${SESSION_VERSION}.${expiresAtEpochS}`
  const signature = signPayload(payload, secret)
  return `${payload}.${signature}`
}

function parseSessionValue(
  value: string,
  nowEpochS: number,
  secret: string,
): DashboardSession | null {
  const parts = value.split('.')
  if (parts.length !== 3) {
    return null
  }
  const [version, expiresAtRaw, signature] = parts
  if (version !== SESSION_VERSION) {
    return null
  }
  const expiresAtEpochS = Number.parseInt(expiresAtRaw, 10)
  if (!Number.isInteger(expiresAtEpochS) || expiresAtEpochS <= nowEpochS) {
    return null
  }

  const payload = `${version}.${expiresAtRaw}`
  const expectedSignature = signPayload(payload, secret)
  const providedBytes = Buffer.from(signature)
  const expectedBytes = Buffer.from(expectedSignature)
  if (
    providedBytes.length !== expectedBytes.length ||
    !timingSafeEqual(providedBytes, expectedBytes)
  ) {
    return null
  }

  return {
    role: 'admin',
    expiresAtEpochS,
  }
}

function isSameOriginRequest(request: Request): boolean {
  const origin = request.headers.get('origin')
  const referer = request.headers.get('referer')
  const requestOrigin = new URL(request.url).origin

  if (origin) {
    return origin === requestOrigin
  }
  if (referer) {
    try {
      return new URL(referer).origin === requestOrigin
    } catch {
      return false
    }
  }
  return false
}

export function rejectCrossOriginMutation(request: Request): Response | null {
  if (!isSameOriginRequest(request)) {
    return new Response(
      JSON.stringify({ error: 'Cross-origin mutation request rejected' }),
      {
        status: 403,
        headers: { 'content-type': 'application/json' },
      },
    )
  }
  return null
}

export function isDashboardAdminConfigured(): boolean {
  return getAdminPassword() !== null && getSessionSecret() !== null
}

export function getDashboardSession(request: Request): DashboardSession | null {
  const secret = getSessionSecret()
  if (!secret) {
    return null
  }
  const cookies = parseCookies(request.headers.get('cookie'))
  const sessionValue = cookies.get(COOKIE_NAME)
  if (!sessionValue) {
    return null
  }
  return parseSessionValue(
    decodeURIComponent(sessionValue),
    Math.floor(Date.now() / 1000),
    secret,
  )
}

export function createDashboardAdminSessionCookie(request: Request): string {
  const secret = getSessionSecret()
  if (!secret) {
    throw new Error(
      'SIDEREAL_DASHBOARD_ADMIN_PASSWORD or SIDEREAL_DASHBOARD_SESSION_SECRET must be configured',
    )
  }
  const expiresAtEpochS = Math.floor(Date.now() / 1000) + SESSION_TTL_SECONDS
  const sessionValue = encodeURIComponent(
    createSessionValue(expiresAtEpochS, secret),
  )
  return [
    `${COOKIE_NAME}=${sessionValue}`,
    'Path=/',
    'HttpOnly',
    'SameSite=Strict',
    ...(new URL(request.url).protocol === 'https:' ? ['Secure'] : []),
    `Max-Age=${SESSION_TTL_SECONDS}`,
  ].join('; ')
}

export function clearDashboardAdminSessionCookie(): string {
  return [
    `${COOKIE_NAME}=`,
    'Path=/',
    'HttpOnly',
    'SameSite=Strict',
    'Max-Age=0',
  ].join('; ')
}

export function verifyDashboardAdminPassword(password: string): boolean {
  const configuredPassword = getAdminPassword()
  if (!configuredPassword) {
    return false
  }
  const providedBytes = Buffer.from(password)
  const expectedBytes = Buffer.from(configuredPassword)
  if (providedBytes.length !== expectedBytes.length) {
    return false
  }
  return timingSafeEqual(providedBytes, expectedBytes)
}

export function requireDashboardAdmin(request: Request): Response | null {
  if (!isDashboardAdminConfigured()) {
    return new Response(
      JSON.stringify({
        error:
          'Dashboard admin auth is not configured. Set SIDEREAL_DASHBOARD_ADMIN_PASSWORD.',
      }),
      {
        status: 503,
        headers: { 'content-type': 'application/json' },
      },
    )
  }

  const crossOriginFailure = rejectCrossOriginMutation(request)
  if (crossOriginFailure) {
    return crossOriginFailure
  }

  const session = getDashboardSession(request)
  if (!session) {
    return new Response(
      JSON.stringify({ error: 'Dashboard admin session required' }),
      {
        status: 403,
        headers: { 'content-type': 'application/json' },
      },
    )
  }

  return null
}

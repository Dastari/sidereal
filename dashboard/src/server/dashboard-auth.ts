import {
  createCipheriv,
  createDecipheriv,
  createHash,
  randomBytes,
} from 'node:crypto'

const COOKIE_NAME = 'sidereal_dashboard_auth'
const SESSION_VERSION = 'v2'
const SESSION_TTL_SECONDS = 60 * 60 * 8
const REFRESH_SKEW_SECONDS = 60

type GatewayAuthTokens = {
  access_token: string
  refresh_token: string
  token_type: string
  expires_in_s: number
}

type GatewayPasswordLoginResponse = {
  status: string
  tokens?: GatewayAuthTokens | null
  challenge_id?: string | null
  challenge_type?: string | null
  expires_in_s: number
}

type GatewayMeResponse = {
  account_id: string
  email: string
  player_entity_id?: string
}

type GatewayBootstrapStatusResponse = {
  required: boolean
  configured: boolean
}

type GatewayTotpEnrollResponse = {
  enrollment_id: string
  issuer: string
  account_label: string
  provisioning_uri: string
  qr_svg: string
  manual_secret: string
  expires_in_s: number
}

type GatewayTotpVerifyResponse = {
  accepted: boolean
  tokens?: GatewayAuthTokens | null
}

type GatewayCharacterSummary = {
  player_entity_id: string
  display_name: string
  created_at_epoch_s: number
  status: string
}

type GatewayCharactersResponse = {
  characters: Array<GatewayCharacterSummary>
}

export type DashboardSession = {
  accountId: string
  email: string
  accessToken: string
  refreshToken: string
  accessTokenExpiresAtEpochS: number
  roles: Array<string>
  scopes: Array<string>
  mfaVerified: boolean
}

export type DashboardSessionStatus = {
  authenticated: boolean
  configured: boolean
  accountId: string | null
  email: string | null
  roles: Array<string>
  scopes: Array<string>
  mfaVerified: boolean
}

export type DashboardBootstrapStatus = {
  required: boolean
  configured: boolean
}

export type DashboardCharacterSummary = {
  playerEntityId: string
  displayName: string
  createdAtEpochS: number
  status: string
}

export type DashboardCharactersPayload = {
  characters: Array<DashboardCharacterSummary>
}

export type DashboardTotpEnrollment = {
  enrollmentId: string
  issuer: string
  accountLabel: string
  provisioningUri: string
  qrSvg: string
  manualSecret: string
  expiresInS: number
}

type AccessTokenClaims = {
  sub?: string
  exp?: number
  roles?: Array<string>
  scope?: string
  session_context?: {
    mfa_verified?: boolean
    active_scope?: Array<string>
  }
}

function parseGatewayUrl(): string {
  const raw = process.env.GATEWAY_API_URL?.trim() || 'http://127.0.0.1:8080'
  return raw.endsWith('/') ? raw.slice(0, -1) : raw
}

function getSessionSecret(): string | null {
  const explicitSecret = process.env.SIDEREAL_DASHBOARD_SESSION_SECRET?.trim()
  if (explicitSecret) {
    return explicitSecret
  }
  return null
}

function encryptionKey(secret: string): Buffer {
  return createHash('sha256').update(secret).digest()
}

function encodeBase64Url(value: Buffer): string {
  return value.toString('base64url')
}

function decodeBase64Url(value: string): Buffer {
  return Buffer.from(value, 'base64url')
}

function encryptSession(session: DashboardSession, secret: string): string {
  const iv = randomBytes(12)
  const cipher = createCipheriv('aes-256-gcm', encryptionKey(secret), iv)
  const plaintext = Buffer.from(JSON.stringify(session), 'utf8')
  const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()])
  const tag = cipher.getAuthTag()
  return [
    SESSION_VERSION,
    encodeBase64Url(iv),
    encodeBase64Url(tag),
    encodeBase64Url(ciphertext),
  ].join('.')
}

function decryptSession(
  value: string,
  secret: string,
): DashboardSession | null {
  const [version, ivRaw, tagRaw, ciphertextRaw] = value.split('.')
  if (version !== SESSION_VERSION || !ivRaw || !tagRaw || !ciphertextRaw) {
    return null
  }
  try {
    const decipher = createDecipheriv(
      'aes-256-gcm',
      encryptionKey(secret),
      decodeBase64Url(ivRaw),
    )
    decipher.setAuthTag(decodeBase64Url(tagRaw))
    const plaintext = Buffer.concat([
      decipher.update(decodeBase64Url(ciphertextRaw)),
      decipher.final(),
    ])
    return normalizeSession(JSON.parse(plaintext.toString('utf8')))
  } catch {
    return null
  }
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

function decodeAccessToken(accessToken: string): AccessTokenClaims | null {
  const [, payload] = accessToken.split('.')
  if (!payload) {
    return null
  }
  try {
    return JSON.parse(
      decodeBase64Url(payload).toString('utf8'),
    ) as AccessTokenClaims
  } catch {
    return null
  }
}

function unique(values: Array<string>): Array<string> {
  return Array.from(
    new Set(values.filter((value) => value.trim().length > 0)),
  ).sort()
}

function sessionFromTokens(
  tokens: GatewayAuthTokens,
  me: GatewayMeResponse,
): DashboardSession {
  const claims = decodeAccessToken(tokens.access_token)
  const tokenScopes = claims?.scope?.split(/\s+/) ?? []
  const activeScopes = claims?.session_context?.active_scope ?? []
  return {
    accountId: me.account_id,
    email: me.email,
    accessToken: tokens.access_token,
    refreshToken: tokens.refresh_token,
    accessTokenExpiresAtEpochS:
      typeof claims?.exp === 'number'
        ? claims.exp
        : Math.floor(Date.now() / 1000) + tokens.expires_in_s,
    roles: unique(claims?.roles ?? []),
    scopes: unique([...tokenScopes, ...activeScopes]),
    mfaVerified: claims?.session_context?.mfa_verified === true,
  }
}

function dashboardCharacterFromGateway(
  character: GatewayCharacterSummary,
): DashboardCharacterSummary {
  return {
    playerEntityId: character.player_entity_id,
    displayName: character.display_name,
    createdAtEpochS: character.created_at_epoch_s,
    status: character.status,
  }
}

function normalizeSession(value: unknown): DashboardSession | null {
  if (!value || typeof value !== 'object') {
    return null
  }
  const session = value as Partial<DashboardSession>
  if (
    typeof session.accountId !== 'string' ||
    typeof session.email !== 'string' ||
    typeof session.accessToken !== 'string' ||
    typeof session.refreshToken !== 'string' ||
    typeof session.accessTokenExpiresAtEpochS !== 'number' ||
    !Array.isArray(session.roles) ||
    !Array.isArray(session.scopes)
  ) {
    return null
  }
  return {
    accountId: session.accountId,
    email: session.email,
    accessToken: session.accessToken,
    refreshToken: session.refreshToken,
    accessTokenExpiresAtEpochS: session.accessTokenExpiresAtEpochS,
    roles: session.roles.filter(
      (role): role is string => typeof role === 'string',
    ),
    scopes: session.scopes.filter(
      (scope): scope is string => typeof scope === 'string',
    ),
    mfaVerified: session.mfaVerified === true,
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

async function gatewayJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${parseGatewayUrl()}${path}`, {
    ...init,
    headers: {
      'content-type': 'application/json',
      ...(init?.headers ?? {}),
    },
  })
  const payload = (await response.json().catch(() => ({}))) as Record<
    string,
    unknown
  >
  if (!response.ok) {
    throw new Error(
      typeof payload.error === 'string'
        ? payload.error
        : `gateway request failed with status ${response.status}`,
    )
  }
  return payload as T
}

async function loadGatewayMe(accessToken: string): Promise<GatewayMeResponse> {
  return gatewayJson<GatewayMeResponse>('/auth/v1/me', {
    method: 'GET',
    headers: {
      authorization: `Bearer ${accessToken}`,
    },
  })
}

export function rejectCrossOriginMutation(request: Request): Response | null {
  if (
    request.method === 'GET' ||
    request.method === 'HEAD' ||
    request.method === 'OPTIONS'
  ) {
    return null
  }
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
  return getSessionSecret() !== null
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
  const session = decryptSession(decodeURIComponent(sessionValue), secret)
  if (!session) {
    return null
  }
  return session
}

export function createDashboardSessionCookie(
  request: Request,
  session: DashboardSession,
): string {
  const secret = getSessionSecret()
  if (!secret) {
    throw new Error('SIDEREAL_DASHBOARD_SESSION_SECRET must be configured')
  }
  const sessionValue = encodeURIComponent(encryptSession(session, secret))
  return [
    `${COOKIE_NAME}=${sessionValue}`,
    'Path=/',
    'HttpOnly',
    'SameSite=Strict',
    ...(new URL(request.url).protocol === 'https:' ? ['Secure'] : []),
    `Max-Age=${SESSION_TTL_SECONDS}`,
  ].join('; ')
}

export function clearDashboardSessionCookie(): string {
  return [
    `${COOKIE_NAME}=`,
    'Path=/',
    'HttpOnly',
    'SameSite=Strict',
    'Max-Age=0',
  ].join('; ')
}

export function dashboardSessionStatus(
  session: DashboardSession | null,
): DashboardSessionStatus {
  return {
    authenticated: session !== null,
    configured: isDashboardAdminConfigured(),
    accountId: session?.accountId ?? null,
    email: session?.email ?? null,
    roles: session?.roles ?? [],
    scopes: session?.scopes ?? [],
    mfaVerified: session?.mfaVerified ?? false,
  }
}

export async function createDashboardSessionFromPassword(
  email: string,
  password: string,
): Promise<
  | { status: 'authenticated'; session: DashboardSession }
  | {
      status: 'mfa_required'
      challengeId: string
      challengeType: string
      expiresInS: number
    }
> {
  const result = await gatewayJson<GatewayPasswordLoginResponse>(
    '/auth/v1/login/password',
    {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    },
  )
  if (result.status === 'mfa_required' && result.challenge_id) {
    return {
      status: 'mfa_required',
      challengeId: result.challenge_id,
      challengeType: result.challenge_type ?? 'totp',
      expiresInS: result.expires_in_s,
    }
  }
  if (result.status !== 'authenticated' || !result.tokens) {
    throw new Error('gateway returned an invalid login response')
  }
  const me = await loadGatewayMe(result.tokens.access_token)
  return {
    status: 'authenticated',
    session: sessionFromTokens(result.tokens, me),
  }
}

export async function createDashboardSessionFromRegistration(
  email: string,
  password: string,
): Promise<DashboardSession> {
  const tokens = await gatewayJson<GatewayAuthTokens>('/auth/v1/register', {
    method: 'POST',
    body: JSON.stringify({ email, password }),
  })
  const me = await loadGatewayMe(tokens.access_token)
  return sessionFromTokens(tokens, me)
}

export async function dashboardBootstrapStatus(): Promise<DashboardBootstrapStatus> {
  const gatewayStatus = await gatewayJson<GatewayBootstrapStatusResponse>(
    '/auth/v1/bootstrap/status',
    {
      method: 'GET',
    },
  )
  return {
    required: gatewayStatus.required,
    configured: gatewayStatus.configured && isDashboardAdminConfigured(),
  }
}

export async function createDashboardSessionFromBootstrapAdmin(
  email: string,
  password: string,
  setupToken: string,
): Promise<DashboardSession> {
  const tokens = await gatewayJson<GatewayAuthTokens>(
    '/auth/v1/bootstrap/admin',
    {
      method: 'POST',
      body: JSON.stringify({
        email,
        password,
        setup_token: setupToken,
      }),
    },
  )
  const me = await loadGatewayMe(tokens.access_token)
  return sessionFromTokens(tokens, me)
}

export async function enrollDashboardTotp(
  session: DashboardSession,
): Promise<DashboardTotpEnrollment> {
  const enrollment = await gatewayJson<GatewayTotpEnrollResponse>(
    '/auth/v1/mfa/totp/enroll',
    {
      method: 'POST',
      headers: {
        authorization: `Bearer ${session.accessToken}`,
      },
    },
  )
  return {
    enrollmentId: enrollment.enrollment_id,
    issuer: enrollment.issuer,
    accountLabel: enrollment.account_label,
    provisioningUri: enrollment.provisioning_uri,
    qrSvg: enrollment.qr_svg,
    manualSecret: enrollment.manual_secret,
    expiresInS: enrollment.expires_in_s,
  }
}

export async function verifyDashboardTotpEnrollment(
  session: DashboardSession,
  enrollmentId: string,
  code: string,
): Promise<DashboardSession> {
  const result = await gatewayJson<GatewayTotpVerifyResponse>(
    '/auth/v1/mfa/totp/verify',
    {
      method: 'POST',
      headers: {
        authorization: `Bearer ${session.accessToken}`,
      },
      body: JSON.stringify({ enrollment_id: enrollmentId, code }),
    },
  )
  if (!result.accepted || !result.tokens) {
    throw new Error('gateway returned an invalid TOTP verification response')
  }
  const me = await loadGatewayMe(result.tokens.access_token)
  return sessionFromTokens(result.tokens, me)
}

export async function loadDashboardCharacters(
  session: DashboardSession,
): Promise<DashboardCharactersPayload> {
  const payload = await gatewayJson<GatewayCharactersResponse>(
    '/auth/v1/characters',
    {
      method: 'GET',
      headers: {
        authorization: `Bearer ${session.accessToken}`,
      },
    },
  )
  return {
    characters: payload.characters.map(dashboardCharacterFromGateway),
  }
}

export async function createDashboardCharacter(
  session: DashboardSession,
  displayName: string,
): Promise<DashboardCharacterSummary> {
  const character = await gatewayJson<GatewayCharacterSummary>(
    '/auth/v1/characters',
    {
      method: 'POST',
      headers: {
        authorization: `Bearer ${session.accessToken}`,
      },
      body: JSON.stringify({ display_name: displayName }),
    },
  )
  return dashboardCharacterFromGateway(character)
}

export async function deleteDashboardCharacter(
  session: DashboardSession,
  playerEntityId: string,
): Promise<void> {
  await gatewayJson<{ accepted: boolean }>(
    `/auth/v1/characters/${encodeURIComponent(playerEntityId)}`,
    {
      method: 'DELETE',
      headers: {
        authorization: `Bearer ${session.accessToken}`,
      },
    },
  )
}

export async function resetDashboardCharacter(
  session: DashboardSession,
  playerEntityId: string,
): Promise<DashboardCharacterSummary> {
  const character = await gatewayJson<GatewayCharacterSummary>(
    `/auth/v1/characters/${encodeURIComponent(playerEntityId)}/reset`,
    {
      method: 'POST',
      headers: {
        authorization: `Bearer ${session.accessToken}`,
      },
    },
  )
  return dashboardCharacterFromGateway(character)
}

export async function createDashboardSessionFromTotpChallenge(
  challengeId: string,
  code: string,
): Promise<DashboardSession> {
  const tokens = await gatewayJson<GatewayAuthTokens>(
    '/auth/v1/login/challenge/totp',
    {
      method: 'POST',
      body: JSON.stringify({ challenge_id: challengeId, code }),
    },
  )
  const me = await loadGatewayMe(tokens.access_token)
  return sessionFromTokens(tokens, me)
}

export async function refreshDashboardSession(
  session: DashboardSession,
): Promise<DashboardSession> {
  if (
    session.accessTokenExpiresAtEpochS >
    Math.floor(Date.now() / 1000) + REFRESH_SKEW_SECONDS
  ) {
    return session
  }
  const tokens = await gatewayJson<GatewayAuthTokens>('/auth/v1/refresh', {
    method: 'POST',
    body: JSON.stringify({ refresh_token: session.refreshToken }),
  })
  const me = await loadGatewayMe(tokens.access_token)
  return sessionFromTokens(tokens, me)
}

export function requireDashboardAdmin(
  request: Request,
  requiredScope?: string,
): Response | null {
  if (!isDashboardAdminConfigured()) {
    return new Response(
      JSON.stringify({
        error:
          'Dashboard auth is not configured. Set SIDEREAL_DASHBOARD_SESSION_SECRET.',
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
      JSON.stringify({ error: 'Dashboard account session required' }),
      {
        status: 403,
        headers: { 'content-type': 'application/json' },
      },
    )
  }

  if (session.accessTokenExpiresAtEpochS <= Math.floor(Date.now() / 1000)) {
    return new Response(
      JSON.stringify({ error: 'Dashboard account session expired' }),
      {
        status: 403,
        headers: { 'content-type': 'application/json' },
      },
    )
  }

  const hasAdminRole = session.roles.some((role) =>
    ['admin', 'dev_tool', 'developer'].includes(role.toLowerCase()),
  )
  if (!hasAdminRole || !session.mfaVerified) {
    return new Response(
      JSON.stringify({
        error: 'Dashboard admin role and verified MFA required',
      }),
      {
        status: 403,
        headers: { 'content-type': 'application/json' },
      },
    )
  }

  if (!session.scopes.includes('dashboard:access')) {
    return new Response(
      JSON.stringify({ error: 'Dashboard scope required: dashboard:access' }),
      {
        status: 403,
        headers: { 'content-type': 'application/json' },
      },
    )
  }

  if (requiredScope && !session.scopes.includes(requiredScope)) {
    return new Response(
      JSON.stringify({ error: `Dashboard scope required: ${requiredScope}` }),
      {
        status: 403,
        headers: { 'content-type': 'application/json' },
      },
    )
  }

  return null
}

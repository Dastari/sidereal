import * as React from 'react'
import { redirect } from '@tanstack/react-router'
import { apiDelete, apiGet, apiPost } from '@/lib/api/client'

export type DashboardSessionStatus = {
  authenticated: boolean
  configured: boolean
  accountId: string | null
  email: string | null
  roles: Array<string>
  scopes: Array<string>
  mfaVerified: boolean
  mfaRequired?: boolean
  challengeId?: string
  challengeType?: string
  expiresInS?: number
}

export type DashboardBootstrapStatus = {
  required: boolean
  configured: boolean
  error?: string
}

export type AccountCharacterSummary = {
  playerEntityId: string
  displayName: string
  createdAtEpochS: number
  status: string
}

export type AccountCharactersPayload = {
  characters: Array<AccountCharacterSummary>
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

let dashboardSessionSnapshot: DashboardSessionStatus | null = null
let dashboardSessionReadyPromise: Promise<DashboardSessionStatus> | null = null
const dashboardSessionListeners = new Set<
  (session: DashboardSessionStatus | null) => void
>()

function publishDashboardSession(nextSession: DashboardSessionStatus | null) {
  dashboardSessionSnapshot = nextSession
  for (const listener of dashboardSessionListeners) {
    listener(nextSession)
  }
}

export function getDashboardSessionSnapshot() {
  return dashboardSessionSnapshot
}

export function hasDashboardAdminAccess(
  status: DashboardSessionStatus | null,
): boolean {
  return (
    status?.authenticated === true &&
    status.mfaVerified &&
    status.scopes.includes('dashboard:access') &&
    status.roles.some((role) =>
      ['admin', 'dev_tool', 'developer'].includes(role.toLowerCase()),
    )
  )
}

export function hasDashboardAdminIdentity(
  status: DashboardSessionStatus | null,
): status is DashboardSessionStatus & { authenticated: true } {
  return (
    status?.authenticated === true &&
    status.scopes.includes('dashboard:access') &&
    status.roles.some((role) =>
      ['admin', 'dev_tool', 'developer'].includes(role.toLowerCase()),
    )
  )
}

export function isDashboardAdminRoute(pathname: string): boolean {
  const normalizedPathname = pathname.split(/[?#]/, 1)[0] || '/'
  return normalizedPathname !== '/'
}

export function onDashboardSessionChange(
  listener: (session: DashboardSessionStatus | null) => void,
) {
  dashboardSessionListeners.add(listener)
  listener(dashboardSessionSnapshot)

  return () => {
    dashboardSessionListeners.delete(listener)
  }
}

export async function refreshDashboardSessionStatus() {
  const next = await apiGet<DashboardSessionStatus>('/api/dashboard-session')
  publishDashboardSession(next)
  return next
}

export async function ensureDashboardSessionReady() {
  if (typeof window === 'undefined') {
    return null
  }

  if (dashboardSessionReadyPromise) {
    return dashboardSessionReadyPromise
  }

  dashboardSessionReadyPromise = refreshDashboardSessionStatus()

  try {
    return await dashboardSessionReadyPromise
  } catch {
    const fallback: DashboardSessionStatus = {
      authenticated: false,
      configured: false,
      accountId: null,
      email: null,
      roles: [],
      scopes: [],
      mfaVerified: false,
    }
    publishDashboardSession(fallback)
    return fallback
  } finally {
    dashboardSessionReadyPromise = null
  }
}

export async function loadDashboardBootstrapStatus() {
  return apiGet<DashboardBootstrapStatus>('/api/bootstrap')
}

export async function loadAccountCharacters() {
  return apiGet<AccountCharactersPayload>('/api/account/characters')
}

export async function createAccountCharacter(displayName: string) {
  return apiPost<AccountCharacterSummary>('/api/account/characters', {
    displayName,
  })
}

export async function deleteAccountCharacter(playerEntityId: string) {
  return apiDelete<{ accepted: boolean }>(
    `/api/account/characters/${encodeURIComponent(playerEntityId)}`,
  )
}

export async function resetAccountCharacter(playerEntityId: string) {
  return apiPost<AccountCharacterSummary>(
    `/api/account/characters/${encodeURIComponent(playerEntityId)}/reset`,
  )
}

export async function enrollAccountTotp() {
  return apiPost<DashboardTotpEnrollment>('/api/account/mfa/totp/enroll')
}

export async function verifyAccountTotpEnrollment(
  enrollmentId: string,
  code: string,
) {
  const next = await apiPost<DashboardSessionStatus>(
    '/api/account/mfa/totp/verify',
    { enrollmentId, code },
  )
  publishDashboardSession(next)
  return next
}

export async function requireDashboardRoute(
  redirectTo: string,
  pathname = redirectTo,
) {
  const status = await ensureDashboardSessionReady()
  if (status?.authenticated !== true) {
    throw redirect({
      to: '/login',
      search: {
        redirect: redirectTo,
      },
    })
  }

  if (!isDashboardAdminRoute(pathname)) {
    return
  }

  if (hasDashboardAdminIdentity(status) && status.mfaVerified !== true) {
    throw redirect({
      to: '/mfa-setup',
      search: {
        redirect: redirectTo,
      },
    })
  }

  if (hasDashboardAdminAccess(status)) {
    return
  }

  throw redirect({ to: '/' })
}

export async function loginDashboard(
  email: string,
  password: string,
): Promise<DashboardSessionStatus> {
  const next = await apiPost<DashboardSessionStatus>('/api/dashboard-session', {
    email,
    password,
  })
  publishDashboardSession(next)
  return next
}

export async function registerDashboard(
  email: string,
  password: string,
): Promise<DashboardSessionStatus> {
  const next = await apiPost<DashboardSessionStatus>('/api/dashboard-session', {
    mode: 'register',
    email,
    password,
  })
  publishDashboardSession(next)
  return next
}

export async function setupFirstDashboardAdmin(
  email: string,
  password: string,
  setupToken: string,
): Promise<DashboardSessionStatus> {
  const next = await apiPost<DashboardSessionStatus>('/api/bootstrap', {
    email,
    password,
    setupToken,
  })
  publishDashboardSession(next)
  return next
}

export async function completeDashboardMfa(
  challengeId: string,
  code: string,
): Promise<DashboardSessionStatus> {
  const next = await apiPost<DashboardSessionStatus>('/api/dashboard-session', {
    challenge_id: challengeId,
    code,
  })
  publishDashboardSession(next)
  return next
}

export async function logoutDashboard() {
  const next = await apiDelete<DashboardSessionStatus>('/api/dashboard-session')
  publishDashboardSession(next)
  return next
}

export function useDashboardSession() {
  const [status, setStatus] = React.useState<DashboardSessionStatus | null>(
    dashboardSessionSnapshot,
  )

  React.useEffect(() => onDashboardSessionChange(setStatus), [])

  return status
}

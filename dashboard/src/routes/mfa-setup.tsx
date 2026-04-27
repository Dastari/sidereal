import * as React from 'react'
import { createFileRoute, redirect, useNavigate } from '@tanstack/react-router'
import type { DashboardTotpEnrollment } from '@/lib/dashboard-auth'
import {
  enrollAccountTotp,
  ensureDashboardSessionReady,
  hasDashboardAdminAccess,
  hasDashboardAdminIdentity,
  verifyAccountTotpEnrollment,
} from '@/lib/dashboard-auth'
import { normalizeCode } from '@/components/auth/TotpCodeInput'
import { TotpSetupPanel } from '@/components/auth/TotpSetupPanel'

type MfaSetupSearch = {
  redirect?: string
}

export const Route = createFileRoute('/mfa-setup')({
  validateSearch: (search: Record<string, unknown>): MfaSetupSearch => ({
    redirect: typeof search.redirect === 'string' ? search.redirect : '/',
  }),
  beforeLoad: async () => {
    if (typeof window === 'undefined') return
    const status = await ensureDashboardSessionReady()
    if (status?.authenticated !== true) {
      throw redirect({ to: '/login', search: { redirect: '/' } })
    }
    if (!hasDashboardAdminIdentity(status)) {
      throw redirect({ to: '/' })
    }
    if (hasDashboardAdminAccess(status)) {
      throw redirect({ to: '/' })
    }
  },
  component: MfaSetupPage,
})

function MfaSetupPage() {
  const navigate = useNavigate()
  const search = Route.useSearch()
  const redirectTo =
    search.redirect && search.redirect.startsWith('/') ? search.redirect : '/'
  const [enrollment, setEnrollment] =
    React.useState<DashboardTotpEnrollment | null>(null)
  const [code, setCode] = React.useState('')
  const [pending, setPending] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  const startEnrollment = React.useCallback(async () => {
    setPending(true)
    setError(null)
    try {
      setEnrollment(await enrollAccountTotp())
      setCode('')
    } catch (enrollError) {
      setError(
        enrollError instanceof Error
          ? enrollError.message
          : 'Failed to start authenticator setup.',
      )
    } finally {
      setPending(false)
    }
  }, [])

  React.useEffect(() => {
    void ensureDashboardSessionReady().then((status) => {
      if (!hasDashboardAdminIdentity(status)) {
        void navigate({ to: '/' })
        return
      }
      if (hasDashboardAdminAccess(status)) {
        void navigate({ to: redirectTo })
        return
      }
      void startEnrollment()
    })
  }, [navigate, redirectTo, startEnrollment])

  const verifyEnrollment = React.useCallback(
    async (completedCode?: string) => {
      if (!enrollment) return
      const normalizedCode = normalizeCode(completedCode ?? code)
      if (normalizedCode.length !== 6) return
      setPending(true)
      setError(null)
      try {
        const status = await verifyAccountTotpEnrollment(
          enrollment.enrollmentId,
          normalizedCode,
        )
        if (!hasDashboardAdminAccess(status)) {
          setError(
            'Authenticator was verified, but dashboard access was not granted.',
          )
          return
        }
        await navigate({ to: redirectTo })
      } catch (verifyError) {
        setError(
          verifyError instanceof Error
            ? verifyError.message
            : 'Failed to verify authenticator code.',
        )
      } finally {
        setPending(false)
      }
    },
    [code, enrollment, navigate, redirectTo],
  )

  return (
    <main className="grid-shell flex min-h-screen items-center justify-center p-6 text-foreground">
      <TotpSetupPanel
        enrollment={enrollment}
        code={code}
        onCodeChange={setCode}
        onRegenerate={() => void startEnrollment()}
        onVerify={(completedCode) => void verifyEnrollment(completedCode)}
        pending={pending}
        error={error}
      />
    </main>
  )
}

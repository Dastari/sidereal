import * as React from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { KeyRound, UserPlus } from 'lucide-react'
import {
  completeDashboardMfa,
  ensureDashboardSessionReady,
  hasDashboardAdminAccess,
  hasDashboardAdminIdentity,
  isDashboardAdminRoute,
  loadDashboardBootstrapStatus,
  loginDashboard,
  registerDashboard,
} from '@/lib/dashboard-auth'
import { TotpCodeInput, normalizeCode } from '@/components/auth/TotpCodeInput'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

type LoginSearch = {
  redirect?: string
}

export const Route = createFileRoute('/login')({
  validateSearch: (search: Record<string, unknown>): LoginSearch => ({
    redirect: typeof search.redirect === 'string' ? search.redirect : '/',
  }),
  component: LoginPage,
})

function LoginPage() {
  const navigate = useNavigate()
  const search = Route.useSearch()
  const redirectTo =
    search.redirect && search.redirect.startsWith('/') ? search.redirect : '/'
  const [mode, setMode] = React.useState<'login' | 'register'>('login')
  const [email, setEmail] = React.useState('')
  const [password, setPassword] = React.useState('')
  const [totpCode, setTotpCode] = React.useState('')
  const [challengeId, setChallengeId] = React.useState<string | null>(null)
  const [pending, setPending] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const [accessNotice, setAccessNotice] = React.useState<string | null>(null)

  const routeAfterAuth = React.useCallback(
    (status: Awaited<ReturnType<typeof ensureDashboardSessionReady>>) => {
      if (!isDashboardAdminRoute(redirectTo)) {
        return redirectTo
      }
      return hasDashboardAdminAccess(status) ? redirectTo : '/'
    },
    [redirectTo],
  )

  React.useEffect(() => {
    const checkSession = async () => {
      const status = await ensureDashboardSessionReady()
      if (status?.authenticated !== true) {
        return
      }
      if (
        isDashboardAdminRoute(redirectTo) &&
        hasDashboardAdminIdentity(status) &&
        status.mfaVerified !== true
      ) {
        await navigate({ to: '/mfa-setup', search: { redirect: redirectTo } })
        return
      }
      await navigate({ to: routeAfterAuth(status) })
    }

    void loadDashboardBootstrapStatus()
      .then((bootstrapStatus) => {
        if (bootstrapStatus.required) {
          void navigate({ to: '/setup', search: { redirect: redirectTo } })
          return
        }
        void checkSession()
      })
      .catch(() => {
        void checkSession()
      })
  }, [navigate, redirectTo, routeAfterAuth])

  const finishAuthenticated = React.useCallback(async () => {
    const status = await ensureDashboardSessionReady()
    if (isDashboardAdminRoute(redirectTo) && !hasDashboardAdminAccess(status)) {
      if (hasDashboardAdminIdentity(status) && status.mfaVerified !== true) {
        await navigate({ to: '/mfa-setup', search: { redirect: redirectTo } })
        return
      }
      setAccessNotice(
        'This account can use My Account. Dashboard tools require an admin or dev account, verified MFA, and the dashboard:access scope.',
      )
      await navigate({ to: '/' })
      return
    }
    await navigate({ to: routeAfterAuth(status) })
  }, [navigate, redirectTo, routeAfterAuth])

  const submitPassword = React.useCallback(async () => {
    setPending(true)
    setError(null)
    try {
      const status =
        mode === 'register'
          ? await registerDashboard(email, password)
          : await loginDashboard(email, password)
      if (status.mfaRequired && status.challengeId) {
        setChallengeId(status.challengeId)
        setTotpCode('')
        return
      }
      await finishAuthenticated()
    } catch (loginError) {
      setError(
        loginError instanceof Error
          ? loginError.message
          : mode === 'register'
            ? 'Registration failed.'
            : 'Login failed.',
      )
    } finally {
      setPending(false)
    }
  }, [email, finishAuthenticated, mode, password])

  const submitTotp = React.useCallback(
    async (completedCode?: string) => {
      if (!challengeId) return
      const normalizedCode = normalizeCode(completedCode ?? totpCode)
      if (normalizedCode.length !== 6) return
      setPending(true)
      setError(null)
      try {
        await completeDashboardMfa(challengeId, normalizedCode)
        await finishAuthenticated()
      } catch (mfaError) {
        setError(
          mfaError instanceof Error
            ? mfaError.message
            : 'Authenticator verification failed.',
        )
      } finally {
        setPending(false)
      }
    },
    [challengeId, finishAuthenticated, totpCode],
  )

  return (
    <main className="grid-shell flex min-h-screen items-center justify-center p-6 text-foreground">
      <section className="grid-panel w-full max-w-md space-y-5 border bg-card/88 p-6 shadow-[0_0_34px_color-mix(in_oklch,var(--glow)_20%,transparent)]">
        <div className="space-y-1">
          <div className="grid-title grid-text-glow text-lg font-semibold text-primary">
            Sidereal Control Surface
          </div>
          <p className="text-sm text-muted-foreground">
            Use your gateway game account to access dashboard tools.
          </p>
        </div>

        {error ? (
          <Alert variant="destructive">
            <AlertTitle>Authentication failed</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        {accessNotice ? (
          <Alert variant="warning">
            <AlertTitle>Dashboard access required</AlertTitle>
            <AlertDescription>{accessNotice}</AlertDescription>
          </Alert>
        ) : null}

        {challengeId ? (
          <div className="space-y-4">
            <div className="space-y-2">
              <Label id="dashboard-totp-code-label">Authenticator code</Label>
              <TotpCodeInput
                id="dashboard-totp-code"
                value={totpCode}
                onChange={setTotpCode}
                onComplete={(completedCode) => void submitTotp(completedCode)}
                disabled={pending}
                aria-label="Dashboard authenticator code"
              />
            </div>
            <div className="flex gap-2">
              <Button
                type="button"
                className="flex-1"
                disabled={pending || normalizeCode(totpCode).length !== 6}
                onClick={() => void submitTotp()}
              >
                <KeyRound className="h-4 w-4" />
                Verify
              </Button>
              <Button
                type="button"
                variant="outline"
                disabled={pending}
                onClick={() => setChallengeId(null)}
              >
                Back
              </Button>
            </div>
          </div>
        ) : (
          <Tabs
            value={mode}
            onValueChange={(value) => setMode(value as 'login' | 'register')}
          >
            <TabsList className="grid w-full grid-cols-2">
              <TabsTrigger value="login">Login</TabsTrigger>
              <TabsTrigger value="register">Register</TabsTrigger>
            </TabsList>
            <TabsContent value={mode} className="space-y-4 pt-2">
              <div className="space-y-2">
                <Label htmlFor="dashboard-email">Email</Label>
                <Input
                  id="dashboard-email"
                  type="email"
                  autoComplete="email"
                  value={email}
                  onChange={(event) => setEmail(event.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="dashboard-password">Password</Label>
                <Input
                  id="dashboard-password"
                  type="password"
                  autoComplete={
                    mode === 'register' ? 'new-password' : 'current-password'
                  }
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  onKeyDown={(event) => {
                    if (
                      event.key === 'Enter' &&
                      email.trim().length > 0 &&
                      password.trim().length > 0
                    ) {
                      event.preventDefault()
                      void submitPassword()
                    }
                  }}
                />
              </div>
              <Button
                type="button"
                className="w-full"
                disabled={
                  pending ||
                  email.trim().length === 0 ||
                  password.trim().length === 0
                }
                onClick={() => void submitPassword()}
              >
                {mode === 'register' ? (
                  <UserPlus className="h-4 w-4" />
                ) : (
                  <KeyRound className="h-4 w-4" />
                )}
                {mode === 'register' ? 'Create account' : 'Login'}
              </Button>
            </TabsContent>
          </Tabs>
        )}
      </section>
    </main>
  )
}

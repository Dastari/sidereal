import * as React from 'react'
import { createFileRoute, redirect, useNavigate } from '@tanstack/react-router'
import { ShieldCheck, UserPlus } from 'lucide-react'
import {
  hasDashboardAdminAccess,
  loadDashboardBootstrapStatus,
  setupFirstDashboardAdmin,
} from '@/lib/dashboard-auth'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

type SetupSearch = {
  redirect?: string
}

export const Route = createFileRoute('/setup')({
  validateSearch: (search: Record<string, unknown>): SetupSearch => ({
    redirect: typeof search.redirect === 'string' ? search.redirect : '/',
  }),
  beforeLoad: async () => {
    if (typeof window === 'undefined') return
    let status: Awaited<ReturnType<typeof loadDashboardBootstrapStatus>>
    try {
      status = await loadDashboardBootstrapStatus()
    } catch {
      return
    }
    if (!status.required) {
      throw redirect({ to: '/login' })
    }
  },
  component: SetupPage,
})

function SetupPage() {
  const navigate = useNavigate()
  const search = Route.useSearch()
  const redirectTo =
    search.redirect && search.redirect.startsWith('/') ? search.redirect : '/'
  const [email, setEmail] = React.useState('')
  const [password, setPassword] = React.useState('')
  const [setupToken, setSetupToken] = React.useState('')
  const [configured, setConfigured] = React.useState(true)
  const [pending, setPending] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  React.useEffect(() => {
    void loadDashboardBootstrapStatus()
      .then((status) => {
        setConfigured(status.configured)
        if (!status.required) {
          void navigate({ to: '/login', search: { redirect: redirectTo } })
        }
      })
      .catch((statusError) => {
        setConfigured(false)
        setError(
          statusError instanceof Error
            ? statusError.message
            : 'Failed to load setup status.',
        )
      })
  }, [navigate, redirectTo])

  const handleSetup = React.useCallback(async () => {
    setPending(true)
    setError(null)
    try {
      const status = await setupFirstDashboardAdmin(email, password, setupToken)
      if (!hasDashboardAdminAccess(status)) {
        setError(
          'First administrator was created, but the returned session does not have dashboard access.',
        )
        return
      }
      await navigate({ to: redirectTo })
    } catch (setupError) {
      setError(
        setupError instanceof Error
          ? setupError.message
          : 'Failed to create first administrator.',
      )
    } finally {
      setPending(false)
    }
  }, [email, navigate, password, redirectTo, setupToken])

  return (
    <main className="grid-shell flex min-h-screen items-center justify-center p-6 text-foreground">
      <section className="grid-panel w-full max-w-md space-y-5 border bg-card/88 p-6 shadow-[0_0_34px_color-mix(in_oklch,var(--glow)_20%,transparent)]">
        <div className="space-y-1">
          <div className="grid-title grid-text-glow flex items-center gap-2 text-lg font-semibold text-primary">
            <ShieldCheck className="h-5 w-5" />
            Sidereal Initial Setup
          </div>
          <p className="text-sm text-muted-foreground">
            Create the first gateway administrator for this database.
          </p>
        </div>

        {!configured ? (
          <Alert variant="warning">
            <AlertTitle>Setup token required</AlertTitle>
            <AlertDescription>
              Set <code>GATEWAY_BOOTSTRAP_TOKEN</code> on the gateway and{' '}
              <code>SIDEREAL_DASHBOARD_SESSION_SECRET</code> on the dashboard
              before completing setup.
            </AlertDescription>
          </Alert>
        ) : null}

        {error ? (
          <Alert variant="destructive">
            <AlertTitle>Setup failed</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        <div className="space-y-2">
          <Label htmlFor="setup-email">Admin email</Label>
          <Input
            id="setup-email"
            type="email"
            autoComplete="email"
            value={email}
            disabled={pending || !configured}
            onChange={(event) => setEmail(event.target.value)}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="setup-password">Admin password</Label>
          <Input
            id="setup-password"
            type="password"
            autoComplete="new-password"
            value={password}
            disabled={pending || !configured}
            onChange={(event) => setPassword(event.target.value)}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="setup-token">Setup token</Label>
          <Input
            id="setup-token"
            type="password"
            autoComplete="one-time-code"
            value={setupToken}
            disabled={pending || !configured}
            onChange={(event) => setSetupToken(event.target.value)}
            onKeyDown={(event) => {
              if (
                event.key === 'Enter' &&
                email.trim().length > 0 &&
                password.trim().length >= 12 &&
                setupToken.trim().length > 0
              ) {
                event.preventDefault()
                void handleSetup()
              }
            }}
          />
        </div>

        <Button
          type="button"
          className="w-full"
          disabled={
            pending ||
            !configured ||
            email.trim().length === 0 ||
            password.trim().length < 12 ||
            setupToken.trim().length === 0
          }
          onClick={() => void handleSetup()}
        >
          <UserPlus className="h-4 w-4" />
          Create first administrator
        </Button>
      </section>
    </main>
  )
}

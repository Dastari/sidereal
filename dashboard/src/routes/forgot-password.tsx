import * as React from 'react'
import { Link, createFileRoute } from '@tanstack/react-router'
import { KeyRound, Mail } from 'lucide-react'
import { apiPost } from '@/lib/api/client'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

export const Route = createFileRoute('/forgot-password')({
  component: ForgotPasswordPage,
})

function ForgotPasswordPage() {
  const [email, setEmail] = React.useState('')
  const [resetToken, setResetToken] = React.useState('')
  const [newPassword, setNewPassword] = React.useState('')
  const [pending, setPending] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const [status, setStatus] = React.useState<string | null>(null)

  const requestReset = React.useCallback(async () => {
    setPending(true)
    setError(null)
    setStatus(null)
    try {
      await apiPost<{ accepted: boolean }>('/api/password-reset', { email })
      setStatus('If that account exists, a password reset email has been sent.')
    } catch (requestError) {
      setError(
        requestError instanceof Error
          ? requestError.message
          : 'Failed to request password reset.',
      )
    } finally {
      setPending(false)
    }
  }, [email])

  const confirmReset = React.useCallback(async () => {
    setPending(true)
    setError(null)
    setStatus(null)
    try {
      await apiPost<{ accepted: boolean }>('/api/password-reset/confirm', {
        resetToken,
        newPassword,
      })
      setStatus('Password updated. You can now log in with the new password.')
      setResetToken('')
      setNewPassword('')
    } catch (confirmError) {
      setError(
        confirmError instanceof Error
          ? confirmError.message
          : 'Failed to confirm password reset.',
      )
    } finally {
      setPending(false)
    }
  }, [newPassword, resetToken])

  return (
    <main className="grid-shell flex min-h-screen items-center justify-center p-6 text-foreground">
      <section className="grid-panel w-full max-w-md space-y-5 border bg-card/88 p-6 shadow-[0_0_34px_color-mix(in_oklch,var(--glow)_20%,transparent)]">
        <div className="space-y-1">
          <div className="grid-title grid-text-glow text-lg font-semibold text-primary">
            Password Reset
          </div>
          <p className="text-sm text-muted-foreground">
            Request a reset email, then paste the token from that email to set a
            new password.
          </p>
        </div>

        {error ? (
          <Alert variant="destructive">
            <AlertTitle>Password reset failed</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        {status ? (
          <Alert>
            <AlertTitle>Password reset</AlertTitle>
            <AlertDescription>{status}</AlertDescription>
          </Alert>
        ) : null}

        <Tabs defaultValue="request">
          <TabsList className="grid w-full grid-cols-2">
            <TabsTrigger value="request">Request</TabsTrigger>
            <TabsTrigger value="confirm">Confirm</TabsTrigger>
          </TabsList>
          <TabsContent value="request" className="space-y-4 pt-2">
            <div className="space-y-2">
              <Label htmlFor="reset-email">Email</Label>
              <Input
                id="reset-email"
                type="email"
                autoComplete="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
              />
            </div>
            <Button
              type="button"
              className="w-full"
              disabled={pending || email.trim().length === 0}
              onClick={() => void requestReset()}
            >
              <Mail className="h-4 w-4" />
              Send Reset Email
            </Button>
          </TabsContent>
          <TabsContent value="confirm" className="space-y-4 pt-2">
            <div className="space-y-2">
              <Label htmlFor="reset-token">Reset Token</Label>
              <Input
                id="reset-token"
                autoComplete="one-time-code"
                value={resetToken}
                onChange={(event) => setResetToken(event.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="new-password">New Password</Label>
              <Input
                id="new-password"
                type="password"
                autoComplete="new-password"
                value={newPassword}
                onChange={(event) => setNewPassword(event.target.value)}
              />
            </div>
            <Button
              type="button"
              className="w-full"
              disabled={
                pending ||
                resetToken.trim().length === 0 ||
                newPassword.trim().length < 12
              }
              onClick={() => void confirmReset()}
            >
              <KeyRound className="h-4 w-4" />
              Update Password
            </Button>
          </TabsContent>
        </Tabs>

        <div className="text-right text-sm">
          <Link to="/login" className="text-primary hover:underline">
            Back to login
          </Link>
        </div>
      </section>
    </main>
  )
}

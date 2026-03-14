import * as React from 'react'
import { LockKeyhole, LockKeyholeOpen, LogOut } from 'lucide-react'
import { apiDelete, apiGet, apiPost } from '@/lib/api/client'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'

type DashboardSessionStatus = {
  authenticated: boolean
  configured: boolean
}

export function DashboardAdminAccess() {
  const [status, setStatus] = React.useState<DashboardSessionStatus | null>(
    null,
  )
  const [dialogOpen, setDialogOpen] = React.useState(false)
  const [password, setPassword] = React.useState('')
  const [pending, setPending] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  const refreshStatus = React.useCallback(async () => {
    const next = await apiGet<DashboardSessionStatus>('/api/dashboard-session')
    setStatus(next)
  }, [])

  React.useEffect(() => {
    void refreshStatus().catch((sessionError: unknown) => {
      setError(
        sessionError instanceof Error
          ? sessionError.message
          : 'Failed to load admin session status.',
      )
    })
  }, [refreshStatus])

  const handleUnlock = React.useCallback(async () => {
    setPending(true)
    setError(null)
    try {
      await apiPost<{ authenticated: boolean }>('/api/dashboard-session', {
        password,
      })
      setPassword('')
      setDialogOpen(false)
      await refreshStatus()
    } catch (sessionError) {
      setError(
        sessionError instanceof Error
          ? sessionError.message
          : 'Failed to unlock dashboard admin actions.',
      )
    } finally {
      setPending(false)
    }
  }, [password, refreshStatus])

  const handleLogout = React.useCallback(async () => {
    setPending(true)
    setError(null)
    try {
      await apiDelete<{ authenticated: boolean }>('/api/dashboard-session')
      await refreshStatus()
    } catch (sessionError) {
      setError(
        sessionError instanceof Error
          ? sessionError.message
          : 'Failed to clear dashboard admin session.',
      )
    } finally {
      setPending(false)
    }
  }, [refreshStatus])

  const configured = status?.configured ?? false
  const authenticated = status?.authenticated ?? false

  return (
    <>
      <div className="flex items-center gap-2">
        {error ? (
          <div className="hidden max-w-sm text-xs text-destructive xl:block">
            {error}
          </div>
        ) : null}
        {authenticated ? (
          <Button
            variant="ghost"
            size="sm"
            disabled={pending}
            onClick={() => void handleLogout()}
          >
            <LockKeyholeOpen className="h-4 w-4" />
            Admin unlocked
          </Button>
        ) : (
          <Button
            variant="outline"
            size="sm"
            disabled={pending || !configured}
            onClick={() => setDialogOpen(true)}
          >
            <LockKeyhole className="h-4 w-4" />
            {configured ? 'Unlock admin' : 'Admin unavailable'}
          </Button>
        )}
        {authenticated ? (
          <Button
            variant="ghost"
            size="icon"
            disabled={pending}
            onClick={() => void handleLogout()}
          >
            <LogOut className="h-4 w-4" />
            <span className="sr-only">Lock admin actions</span>
          </Button>
        ) : null}
      </div>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Unlock admin mutations</DialogTitle>
            <DialogDescription>
              Enter the dashboard admin password to enable privileged actions
              for this browser session.
            </DialogDescription>
          </DialogHeader>

          {!configured ? (
            <Alert variant="warning">
              <AlertTitle>Admin auth not configured</AlertTitle>
              <AlertDescription>
                Set `SIDEREAL_DASHBOARD_ADMIN_PASSWORD` on the server before
                using mutation routes.
              </AlertDescription>
            </Alert>
          ) : null}

          {error ? (
            <Alert variant="destructive">
              <AlertTitle>Unlock failed</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          <div className="space-y-2">
            <Label htmlFor="dashboard-admin-password">Admin password</Label>
            <Input
              id="dashboard-admin-password"
              type="password"
              value={password}
              disabled={pending || !configured}
              onChange={(event) => setPassword(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && password.trim().length > 0) {
                  event.preventDefault()
                  void handleUnlock()
                }
              }}
            />
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setDialogOpen(false)}
              disabled={pending}
            >
              Cancel
            </Button>
            <Button
              onClick={() => void handleUnlock()}
              disabled={pending || !configured || password.trim().length === 0}
            >
              Unlock
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

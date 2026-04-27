import * as React from 'react'
import { LogOut, ShieldAlert, ShieldCheck } from 'lucide-react'
import { useNavigate } from '@tanstack/react-router'
import {
  hasDashboardAdminAccess,
  hasDashboardAdminIdentity,
  logoutDashboard,
  refreshDashboardSessionStatus,
  useDashboardSession,
} from '@/lib/dashboard-auth'
import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'

export function DashboardAdminAccess() {
  const navigate = useNavigate()
  const status = useDashboardSession()
  const [pending, setPending] = React.useState(false)

  React.useEffect(() => {
    void refreshDashboardSessionStatus().catch(() => undefined)
  }, [])

  const handleLogout = React.useCallback(async () => {
    setPending(true)
    try {
      await logoutDashboard()
      await navigate({ to: '/login', search: { redirect: '/' } })
    } finally {
      setPending(false)
    }
  }, [navigate])

  const adminReady = hasDashboardAdminAccess(status)
  const adminIdentity = hasDashboardAdminIdentity(status)
  const title = adminReady
    ? 'Admin session active'
    : adminIdentity
      ? 'Admin MFA required'
      : 'Account session active'
  const description = adminReady
    ? `Scopes: ${status?.scopes.join(', ') || 'none'}`
    : adminIdentity
      ? 'Dashboard tools require verified MFA before admin routes unlock.'
      : 'Signed-in users can manage their own account characters here.'

  return (
    <div className="flex items-center gap-2">
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant={adminReady ? 'ghost' : 'outline'}
            size="sm"
            disabled
            className="disabled:opacity-100"
          >
            {adminReady ? (
              <ShieldCheck className="h-4 w-4 text-success" />
            ) : (
              <ShieldAlert className="h-4 w-4 text-warning" />
            )}
            <span className="max-w-44 truncate">
              {status?.email ?? 'Checking session'}
            </span>
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          <div className="font-medium">{title}</div>
          <div className="max-w-64 text-xs text-muted-foreground">
            {description}
          </div>
        </TooltipContent>
      </Tooltip>

      <Button
        variant="ghost"
        size="icon"
        disabled={pending}
        onClick={() => void handleLogout()}
      >
        <LogOut className="h-4 w-4" />
        <span className="sr-only">Log out dashboard session</span>
      </Button>
    </div>
  )
}

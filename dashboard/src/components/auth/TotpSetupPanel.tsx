import { KeyRound, RefreshCcw, ShieldCheck } from 'lucide-react'
import type { DashboardTotpEnrollment } from '@/lib/dashboard-auth'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { HUDFrame } from '@/components/ui/hud-frame'
import { Label } from '@/components/ui/label'
import { cn } from '@/lib/utils'
import { TotpCodeInput, normalizeCode } from '@/components/auth/TotpCodeInput'

type TotpSetupPanelProps = {
  enrollment: DashboardTotpEnrollment | null
  code: string
  onCodeChange: (value: string) => void
  onRegenerate: () => void
  onVerify: (code?: string) => void
  pending?: boolean
  error?: string | null
  className?: string
}

export function TotpSetupPanel({
  enrollment,
  code,
  onCodeChange,
  onRegenerate,
  onVerify,
  pending = false,
  error = null,
  className,
}: TotpSetupPanelProps) {
  const normalizedCode = normalizeCode(code)
  const canVerify =
    Boolean(enrollment) && !pending && normalizedCode.length === 6

  return (
    <section
      className={cn(
        'grid-panel grid w-full max-w-5xl gap-6 border bg-card/88 p-6 shadow-[0_0_34px_color-mix(in_oklch,var(--glow)_20%,transparent)] lg:grid-cols-[minmax(300px,420px)_minmax(0,1fr)]',
        className,
      )}
    >
      <div className="space-y-5">
        <div className="space-y-2">
          <div className="grid-title grid-text-glow flex items-center gap-2 text-lg font-semibold text-primary">
            <ShieldCheck className="h-5 w-5" />
            Authenticator Setup
          </div>
          <p className="max-w-md text-sm text-muted-foreground">
            Dashboard administrator access requires a verified authenticator.
          </p>
        </div>

        <ThemedQrCode qrSvg={enrollment?.qrSvg ?? null} pending={pending} />

        <Button
          type="button"
          variant="outline"
          className="w-full"
          disabled={pending}
          onClick={onRegenerate}
        >
          <RefreshCcw className="h-4 w-4" />
          Generate New QR
        </Button>
      </div>

      <div className="space-y-5">
        {error ? (
          <Alert variant="destructive">
            <AlertTitle>MFA setup failed</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        <div className="space-y-2">
          <Label>Manual secret</Label>
          <HUDFrame className="min-h-12 bg-background/55 px-3 py-2">
            <div className="break-all font-mono text-base font-semibold tracking-[0.08em] text-foreground">
              {enrollment?.manualSecret ??
                'Generate a QR code to show the manual secret.'}
            </div>
          </HUDFrame>
        </div>

        <div className="space-y-2">
          <Label id="totp-setup-code-label">Authenticator code</Label>
          <TotpCodeInput
            id="totp-setup-code"
            value={code}
            onChange={onCodeChange}
            onComplete={onVerify}
            disabled={!enrollment || pending}
            aria-label="Authenticator setup code"
          />
        </div>

        <Button
          type="button"
          className="w-full"
          disabled={!canVerify}
          onClick={() => onVerify()}
        >
          <KeyRound className="h-4 w-4" />
          Verify and Continue
        </Button>
      </div>
    </section>
  )
}

function ThemedQrCode({
  qrSvg,
  pending,
}: {
  qrSvg: string | null
  pending: boolean
}) {
  if (!qrSvg) {
    return (
      <HUDFrame className="flex aspect-square min-h-72 items-center justify-center border-dashed bg-background/50 p-5 text-sm text-muted-foreground">
        {pending ? 'Generating QR code...' : 'Authenticator QR pending'}
      </HUDFrame>
    )
  }

  return (
    <HUDFrame className="aspect-square bg-background/70 p-5">
      <div
        className={cn(
          'mx-auto flex size-full max-h-80 max-w-80 items-center justify-center border border-primary/45 bg-background p-5 shadow-[inset_0_0_28px_color-mix(in_oklch,var(--glow)_12%,transparent)]',
          '[&_svg]:size-full [&_svg]:bg-transparent',
          '[&_svg_rect]:fill-background',
          '[&_svg_path]:fill-primary [&_svg_path]:stroke-primary',
        )}
        dangerouslySetInnerHTML={{ __html: qrSvg }}
      />
    </HUDFrame>
  )
}

import * as React from 'react'
import { AlertTriangle } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Spinner } from '@/components/ui/spinner'
import { bootGameClientWasm } from '@/lib/game-client-wasm'

export function GameClientPage() {
  const [status, setStatus] = React.useState<'booting' | 'ready' | 'error'>(
    'booting',
  )
  const [message, setMessage] = React.useState(
    'Loading browser runtime and binding the dashboard canvas.',
  )

  React.useEffect(() => {
    let cancelled = false

    void bootGameClientWasm()
      .then(() => {
        if (!cancelled) {
          setStatus('ready')
          setMessage(
            'The browser game client runtime has started inside this dashboard route.',
          )
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setStatus('error')
          setMessage(
            error instanceof Error
              ? error.message
              : 'The browser game client failed to start.',
          )
        }
      })

    return () => {
      cancelled = true
    }
  }, [])

  return (
    <div className="relative flex h-full min-h-0 flex-col overflow-hidden bg-background">
      <div className="game-client-stage relative flex-1 overflow-hidden bg-[radial-gradient(circle_at_top,_rgba(96,165,250,0.10),_transparent_34%),linear-gradient(180deg,_rgba(4,8,18,1),_rgba(6,10,18,1))]">
        <canvas
          id="sidereal-game-client-canvas"
          className="block h-full w-full"
        />

        {status !== 'ready' ? (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-[linear-gradient(180deg,rgba(3,7,14,0.78),rgba(5,10,18,0.52))] p-6 text-center">
            <Card className="w-full max-w-md border-border/70 bg-card/84 shadow-[0_18px_60px_rgba(0,0,0,0.35)] backdrop-blur-xl">
              <CardHeader>
                <CardTitle className="flex items-center justify-center gap-2 text-base">
                  {status === 'booting' ? (
                    <Spinner className="h-5 w-5 text-primary" />
                  ) : (
                    <AlertTriangle className="h-5 w-5 text-destructive" />
                  )}
                  {status === 'booting'
                    ? 'Starting Sidereal browser client'
                    : 'Client startup blocked'}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-sm text-muted-foreground">
                  {status === 'booting'
                    ? 'The dashboard is loading the generated JS/WASM client wrapper and attaching Bevy to the full route canvas.'
                    : message}
                </p>
              </CardContent>
            </Card>
          </div>
        ) : null}
      </div>
    </div>
  )
}

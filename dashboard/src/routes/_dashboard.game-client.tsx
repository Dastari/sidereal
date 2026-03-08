import * as React from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { AlertTriangle, LoaderCircle } from 'lucide-react'
import { bootGameClientWasm } from '@/lib/game-client-wasm'

export const Route = createFileRoute('/_dashboard/game-client')({
  component: GameClientPage,
})

function GameClientPage() {
  const [status, setStatus] = React.useState<
    'booting' | 'ready' | 'error'
  >('booting')
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
            <div className="max-w-md space-y-3 rounded-2xl border border-border/70 bg-card/84 p-6 shadow-[0_18px_60px_rgba(0,0,0,0.35)] backdrop-blur-xl">
              <div className="flex justify-center">
                {status === 'booting' ? (
                  <LoaderCircle className="h-8 w-8 animate-spin text-primary" />
                ) : (
                  <AlertTriangle className="h-8 w-8 text-destructive" />
                )}
              </div>
              <div className="text-base font-medium text-foreground">
                {status === 'booting'
                  ? 'Starting Sidereal browser client'
                  : 'Client startup blocked'}
              </div>
              <p className="text-sm text-muted-foreground">
                {status === 'booting'
                  ? 'The dashboard is loading the generated JS/WASM client wrapper and attaching Bevy to the full route canvas.'
                  : message}
              </p>
            </div>
          </div>
        ) : null}
      </div>
    </div>
  )
}

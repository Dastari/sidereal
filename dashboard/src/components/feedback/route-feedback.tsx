import { AlertTriangle, RefreshCw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'

export function RoutePendingState({
  title,
  description,
}: {
  title: string
  description: string
}) {
  return (
    <div className="flex h-full min-h-[24rem] items-center justify-center p-6">
      <Card className="w-full max-w-xl border-border/80 bg-card/85">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Spinner className="h-4 w-4 text-primary" />
            {title}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-sm text-muted-foreground">{description}</p>
          <div className="space-y-2">
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-10/12" />
            <Skeleton className="h-24 w-full rounded-xl" />
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

export function RouteErrorState({
  title,
  error,
  onRetry,
}: {
  title: string
  error: unknown
  onRetry?: () => void
}) {
  const message =
    error instanceof Error ? error.message : 'The route failed to load.'

  return (
    <div className="flex h-full min-h-[24rem] items-center justify-center p-6">
      <Card className="w-full max-w-xl border-border/80 bg-card/85">
        <CardHeader>
          <CardTitle className="text-base">{title}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <Alert variant="destructive">
            <AlertTitle className="flex items-center gap-2">
              <AlertTriangle className="h-4 w-4" />
              Route error
            </AlertTitle>
            <AlertDescription>{message}</AlertDescription>
          </Alert>
          {onRetry ? (
            <div className="flex justify-end">
              <Button variant="outline" onClick={onRetry}>
                <RefreshCw className="h-4 w-4" />
                Retry
              </Button>
            </div>
          ) : null}
        </CardContent>
      </Card>
    </div>
  )
}

export function RouteNotFoundState({
  title,
  description,
}: {
  title: string
  description: string
}) {
  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-6">
      <Card className="w-full max-w-xl border-border/80 bg-card/90">
        <CardHeader>
          <CardTitle>{title}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">{description}</p>
        </CardContent>
      </Card>
    </div>
  )
}

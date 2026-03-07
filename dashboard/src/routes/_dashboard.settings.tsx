import { createFileRoute } from '@tanstack/react-router'
import { Settings } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'

export const Route = createFileRoute('/_dashboard/settings')({
  component: SettingsPlaceholderPage,
})

function SettingsPlaceholderPage() {
  return (
    <div className="flex h-full items-center justify-center p-6">
      <Card className="w-full max-w-3xl border-border/80 bg-card/85 backdrop-blur">
        <CardHeader>
          <Badge variant="outline" className="w-fit">
            Placeholder
          </Badge>
          <CardTitle className="mt-3 flex items-center gap-2 text-2xl">
            <Settings className="h-5 w-5 text-primary" />
            Settings
          </CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground">
          This route is reserved for dashboard configuration. Future environment
          settings, BRP defaults, asset paths, and tool preferences should live
          here instead of accumulating ad-hoc controls inside unrelated pages.
        </CardContent>
      </Card>
    </div>
  )
}

import { createFileRoute } from '@tanstack/react-router'
import { FileCode2 } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'

export const Route = createFileRoute('/_dashboard/script-editor')({
  component: ScriptEditorPlaceholderPage,
})

function ScriptEditorPlaceholderPage() {
  return (
    <div className="flex h-full items-center justify-center p-6">
      <Card className="w-full max-w-3xl border-border/80 bg-card/85 backdrop-blur">
        <CardHeader>
          <Badge variant="outline" className="w-fit">
            Placeholder
          </Badge>
          <CardTitle className="mt-3 flex items-center gap-2 text-2xl">
            <FileCode2 className="h-5 w-5 text-primary" />
            Script Editor
          </CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground">
          This route is reserved for a future in-game script editor. The shared
          shell, top navigation, and side rail are already in place so the
          eventual editor can slot into the same routed layout without another
          shell rewrite.
        </CardContent>
      </Card>
    </div>
  )
}

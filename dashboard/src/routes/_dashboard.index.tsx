import { createFileRoute } from '@tanstack/react-router'
import { Activity, Boxes, Database, Orbit, Sparkles, Wifi } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'

export const Route = createFileRoute('/_dashboard/')({
  component: DashboardOverviewPage,
})

const overviewCards = [
  {
    label: 'Database',
    value: 'AGE + Postgres',
    note: 'Persisted graph and table-backed runtime data',
    icon: Database,
  },
  {
    label: 'Game World',
    value: 'BRP Ready',
    note: 'Server and client BRP routes available from the routed shell',
    icon: Orbit,
  },
  {
    label: 'Shaders',
    value: 'Workshop',
    note: 'WGSL library, live preview, diagnostics, and metadata',
    icon: Sparkles,
  },
  {
    label: 'Endpoints',
    value: 'Monitoring',
    note: 'Intended home for service health and endpoint checks',
    icon: Wifi,
  },
]

function DashboardOverviewPage() {
  return (
    <div className="flex h-full flex-col overflow-auto p-6 bg-background">
      <div className="max-w-5xl space-y-6">
        <div className="space-y-2">
          <Badge variant="outline">Overview</Badge>
          <h1 className="text-3xl font-semibold tracking-tight text-foreground">
            Dashboard
          </h1>
          <p className="max-w-3xl text-sm text-muted-foreground">
            Statistics and endpoint health live here. The other major tools are
            now routed independently so deep links, selection slugs, and tool
            state can evolve without growing one monolithic page.
          </p>
        </div>

        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {overviewCards.map((card) => {
            const Icon = card.icon
            return (
              <Card
                key={card.label}
                className="border-border/80 bg-card/85 backdrop-blur"
              >
                <CardHeader className="flex flex-row items-start justify-between space-y-0">
                  <div className="space-y-1">
                    <div className="text-xs uppercase tracking-[0.16em] text-muted-foreground">
                      {card.label}
                    </div>
                    <CardTitle className="text-lg">{card.value}</CardTitle>
                  </div>
                  <Icon className="h-5 w-5 text-primary" />
                </CardHeader>
                <CardContent className="text-sm text-muted-foreground">
                  {card.note}
                </CardContent>
              </Card>
            )
          })}
        </div>

        <Card className="border-border/80 bg-card/80 backdrop-blur">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Activity className="h-4 w-4 text-primary" />
              Planned Health Surface
            </CardTitle>
          </CardHeader>
          <CardContent className="grid gap-3 md:grid-cols-3">
            <StatTile label="Shard sim" value="Pending wiring" />
            <StatTile label="Gateway APIs" value="Pending wiring" />
            <StatTile label="Asset delivery" value="Pending wiring" />
          </CardContent>
        </Card>

        <Card className="border-border/80 bg-card/80 backdrop-blur">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Boxes className="h-4 w-4 text-primary" />
              Route Strategy
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-2 text-sm text-muted-foreground">
            <p>
              `Database` and `Game World` now have separate route boundaries so
              persistent graph exploration and live BRP operations stop sharing
              one local state tree.
            </p>
            <p>
              URL state should favor slugs for durable selections and `nuqs`
              query params for view controls, filters, and panel sizes.
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

function StatTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-border/70 bg-background/50 p-4">
      <div className="text-xs uppercase tracking-[0.16em] text-muted-foreground">
        {label}
      </div>
      <div className="mt-2 text-sm font-medium text-foreground">{value}</div>
    </div>
  )
}

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
    <div className="flex h-full flex-col overflow-auto bg-background p-6">
      <div className="max-w-5xl space-y-6">
        <div className="space-y-2">
          <Badge variant="outline">Overview</Badge>
          <h1 className="grid-title text-3xl font-semibold tracking-tight text-foreground">
            Dashboard
          </h1>
          <p className="max-w-3xl text-sm text-muted-foreground">
            Statistics and endpoint health live here. The other major tools are
            now routed independently so deep links, selection slugs, and tool
            state can evolve without growing one monolithic page.
          </p>
        </div>

        <div className="grid auto-rows-[minmax(168px,_1fr)] gap-4 md:grid-cols-6">
          {overviewCards.map((card) => {
            const Icon = card.icon
            const isPrimary = card.label === 'Game World'
            return (
              <Card
                key={card.label}
                className={
                  isPrimary
                    ? 'md:col-span-3 md:row-span-2'
                    : 'md:col-span-3 xl:col-span-2'
                }
              >
                <CardHeader className="flex flex-row items-start justify-between space-y-0">
                  <div className="space-y-1">
                    <div className="grid-title text-xs text-muted-foreground">
                      {card.label}
                    </div>
                    <CardTitle className="text-lg">{card.value}</CardTitle>
                  </div>
                  <div className="rounded-lg border border-border/60 bg-background/50 p-2">
                    <Icon className="h-5 w-5 text-primary" />
                  </div>
                </CardHeader>
                <CardContent className="flex h-full flex-col justify-between gap-4 text-sm text-muted-foreground">
                  {card.note}
                  <div className="text-[10px] uppercase tracking-[0.18em] text-primary/80">
                    {isPrimary ? 'Live diagnostics lane' : 'Planned module'}
                  </div>
                </CardContent>
              </Card>
            )
          })}
        </div>

        <div className="grid gap-4 md:grid-cols-5">
          <Card className="md:col-span-3">
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

          <Card className="md:col-span-2">
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <Boxes className="h-4 w-4 text-primary" />
                Route Strategy
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-2 text-sm text-muted-foreground">
              <p>
                `Database` and `Game World` now have separate route boundaries
                so persistent graph exploration and live BRP operations stop
                sharing one local state tree.
              </p>
              <p>
                URL state should favor slugs for durable selections and `nuqs`
                query params for view controls, filters, and panel sizes.
              </p>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}

function StatTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid-panel rounded-xl border border-border/70 bg-background/50 p-4">
      <div className="grid-title text-xs text-muted-foreground">{label}</div>
      <div className="mt-2 text-sm font-medium text-foreground">{value}</div>
    </div>
  )
}

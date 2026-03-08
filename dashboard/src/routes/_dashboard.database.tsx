import { Outlet, createFileRoute, useNavigate } from '@tanstack/react-router'
import { parseAsString, parseAsStringLiteral, useQueryStates } from 'nuqs'
import { Database, RefreshCw, Table2, Users } from 'lucide-react'
import { AppLayout } from '@/components/layout/AppLayout'
import { Toolbar } from '@/components/sidebar/Toolbar'
import { ExplorerWorkspace } from '@/features/explorer/ExplorerWorkspace'
import { AccountsPanel } from '@/features/database/AccountsPanel'
import { TablesPanel } from '@/features/database/TablesPanel'
import { useDatabaseAdminData } from '@/features/database/useDatabaseAdminData'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'

export const Route = createFileRoute('/_dashboard/database')({
  component: DatabaseRouteLayout,
})

type DatabaseSection = 'entities' | 'accounts' | 'tables'
type AccountSortKey = 'email' | 'characters' | 'created'
type TableSortKey = 'name' | 'rows' | 'schema'

const DATABASE_BRIDGE_TAB = {
  id: 'database',
  label: 'Database',
  port: 0,
  kind: 'server' as const,
}

function DatabaseRouteLayout() {
  return <Outlet />
}

function DatabaseToolbar({
  activeSection,
  summary,
  loading,
  onRefresh,
}: {
  activeSection: DatabaseSection
  summary: {
    accountCount: number
    characterCount: number
    tableCount: number
    scriptDocumentCount: number
  }
  loading: boolean
  onRefresh: () => Promise<void>
}) {
  const navigate = useNavigate()

  return (
    <>
      <Tabs
        value={activeSection}
        onValueChange={(value) => {
          if (value === 'accounts') {
            void navigate({ to: '/database/accounts' })
            return
          }
          if (value === 'tables') {
            void navigate({ to: '/database/tables' })
            return
          }
          void navigate({ to: '/database' })
        }}
      >
        <TabsList className="h-8">
          <TabsTrigger value="entities" className="gap-2">
            <Database className="h-3.5 w-3.5" />
            Entities
          </TabsTrigger>
          <TabsTrigger value="accounts" className="gap-2">
            <Users className="h-3.5 w-3.5" />
            Accounts
          </TabsTrigger>
          <TabsTrigger value="tables" className="gap-2">
            <Table2 className="h-3.5 w-3.5" />
            Tables
          </TabsTrigger>
        </TabsList>
      </Tabs>
      <div className="ml-auto flex flex-wrap items-center gap-2">
        <SummaryBadge label="Accounts" value={summary.accountCount} />
        <SummaryBadge label="Characters" value={summary.characterCount} />
        <SummaryBadge label="Tables" value={summary.tableCount} />
        <SummaryBadge label="Scripts" value={summary.scriptDocumentCount} />
        <Button
          variant="outline"
          size="sm"
          onClick={() => void onRefresh()}
          disabled={loading}
        >
          <RefreshCw className="h-4 w-4" />
          Refresh
        </Button>
      </div>
    </>
  )
}

export function DatabaseEntitiesPage({
  selectedEntityGuid,
}: {
  selectedEntityGuid: string | null
}) {
  const navigate = useNavigate()
  const { data, loading, error, refresh } = useDatabaseAdminData()

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1">
        <ExplorerWorkspace
          scope="database"
          selectedEntityGuid={selectedEntityGuid}
          onSelectedEntityGuidChange={(entityGuid) => {
            void navigate({
              to: entityGuid ? '/database/$entityGuid' : '/database',
              params: entityGuid ? { entityGuid } : {},
              search: (prev) => prev,
              replace: true,
            })
          }}
          toolbarContent={
            <DatabaseToolbar
              activeSection="entities"
              summary={data.summary}
              loading={loading}
              onRefresh={refresh}
            />
          }
        />
      </div>
      {error ? (
        <div className="border-t border-destructive/40 bg-destructive/8 px-4 py-2 text-sm text-destructive">
          {error}
        </div>
      ) : null}
    </div>
  )
}

export function DatabaseAccountsPage() {
  const { data, loading, error, refresh, renameCharacter, requestPasswordReset } =
    useDatabaseAdminData()
  const [routeState, setRouteState] = useQueryStates({
    search: parseAsString.withDefault(''),
    sort: parseAsStringLiteral(['email', 'characters', 'created'] as const).withDefault(
      'email',
    ),
  })

  return (
    <div className="flex h-full min-h-0 flex-col">
      <AppLayout
        header={
          <Toolbar
            sourceMode="database"
            onSourceModeChange={() => {}}
            brpTabs={[DATABASE_BRIDGE_TAB]}
            activeBrpTabId={DATABASE_BRIDGE_TAB.id}
            onActiveBrpTabIdChange={() => {}}
            onAddClientTab={() => {}}
            showDataSourceTabs={false}
            showDatabaseTab={false}
          >
            <DatabaseToolbar
              activeSection="accounts"
              summary={data.summary}
              loading={loading}
              onRefresh={refresh}
            />
          </Toolbar>
        }
      >
        <AccountsPanel
          accounts={data.accounts}
          loading={loading}
          search={routeState.search}
          sortKey={routeState.sort as AccountSortKey}
          onSearchChange={(value) => {
            void setRouteState({ search: value })
          }}
          onSortKeyChange={(value) => {
            void setRouteState({ sort: value })
          }}
          onRequestPasswordReset={(account) =>
            requestPasswordReset(account.accountId)
          }
          onRenameCharacter={renameCharacter}
        />
      </AppLayout>

      {error ? (
        <div className="border-t border-destructive/40 bg-destructive/8 px-4 py-2 text-sm text-destructive">
          {error}
        </div>
      ) : null}
    </div>
  )
}

export function DatabaseTablesPage() {
  const { data, loading, error, refresh } = useDatabaseAdminData()
  const [routeState, setRouteState] = useQueryStates({
    search: parseAsString.withDefault(''),
    sort: parseAsStringLiteral(['name', 'rows', 'schema'] as const).withDefault(
      'schema',
    ),
  })

  return (
    <div className="flex h-full min-h-0 flex-col">
      <AppLayout
        header={
          <Toolbar
            sourceMode="database"
            onSourceModeChange={() => {}}
            brpTabs={[DATABASE_BRIDGE_TAB]}
            activeBrpTabId={DATABASE_BRIDGE_TAB.id}
            onActiveBrpTabIdChange={() => {}}
            onAddClientTab={() => {}}
            showDataSourceTabs={false}
            showDatabaseTab={false}
          >
            <DatabaseToolbar
              activeSection="tables"
              summary={data.summary}
              loading={loading}
              onRefresh={refresh}
            />
          </Toolbar>
        }
      >
        <TablesPanel
          tables={data.tables}
          scriptDocuments={data.scriptDocuments}
          loading={loading}
          search={routeState.search}
          sortKey={routeState.sort as TableSortKey}
          onSearchChange={(value) => {
            void setRouteState({ search: value })
          }}
          onSortKeyChange={(value) => {
            void setRouteState({ sort: value })
          }}
        />
      </AppLayout>

      {error ? (
        <div className="border-t border-destructive/40 bg-destructive/8 px-4 py-2 text-sm text-destructive">
          {error}
        </div>
      ) : null}
    </div>
  )
}

function SummaryBadge({ label, value }: { label: string; value: number }) {
  return (
    <Badge variant="secondary" className="gap-2">
      <span className="text-muted-foreground">{label}</span>
      <span className="tabular-nums text-foreground">{value}</span>
    </Badge>
  )
}

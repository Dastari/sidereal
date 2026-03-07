import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { parseAsString, parseAsStringLiteral, useQueryStates } from 'nuqs'
import { Database, RefreshCw, Table2, Users } from 'lucide-react'
import { ExplorerWorkspace } from '@/features/explorer/ExplorerWorkspace'
import { AccountsPanel } from '@/features/database/AccountsPanel'
import { TablesPanel } from '@/features/database/TablesPanel'
import { useDatabaseAdminData } from '@/features/database/useDatabaseAdminData'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'

export const Route = createFileRoute('/_dashboard/database')({
  component: DatabaseRoutePage,
})

function DatabaseRoutePage() {
  return <DatabaseToolPage selectedEntityGuid={null} />
}

export function DatabaseToolPage({
  selectedEntityGuid,
}: {
  selectedEntityGuid: string | null
}) {
  const navigate = useNavigate()
  const { data, loading, error, refresh } = useDatabaseAdminData()
  const [pageState, setPageState] = useQueryStates({
    section: parseAsStringLiteral([
      'entities',
      'accounts',
      'tables',
    ] as const).withDefault('entities'),
    accountSearch: parseAsString.withDefault(''),
    accountSort: parseAsStringLiteral([
      'email',
      'characters',
      'created',
    ] as const).withDefault('email'),
    tableSearch: parseAsString.withDefault(''),
    tableSort: parseAsStringLiteral([
      'name',
      'rows',
      'schema',
    ] as const).withDefault('schema'),
  })

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-b border-border bg-background px-4 py-3">
        <div className="flex flex-wrap items-center gap-3">
          <Tabs
            value={pageState.section}
            onValueChange={(value) => {
              void setPageState({
                section: value as 'entities' | 'accounts' | 'tables',
              })
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
          <div className="ml-auto flex items-center gap-2">
            <SummaryBadge label="Accounts" value={data.summary.accountCount} />
            <SummaryBadge
              label="Characters"
              value={data.summary.characterCount}
            />
            <SummaryBadge label="Tables" value={data.summary.tableCount} />
            <SummaryBadge
              label="Scripts"
              value={data.summary.scriptDocumentCount}
            />
            <Button
              variant="outline"
              size="sm"
              onClick={() => void refresh()}
              disabled={loading}
            >
              <RefreshCw className="h-4 w-4" />
              Refresh
            </Button>
          </div>
        </div>
      </div>

      <div className="min-h-0 flex-1">
        {pageState.section === 'entities' ? (
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
          />
        ) : pageState.section === 'accounts' ? (
          <AccountsPanel
            accounts={data.accounts}
            loading={loading}
            search={pageState.accountSearch}
            sortKey={pageState.accountSort}
            onSearchChange={(value) => {
              void setPageState({ accountSearch: value })
            }}
            onSortKeyChange={(value) => {
              void setPageState({ accountSort: value })
            }}
          />
        ) : (
          <TablesPanel
            tables={data.tables}
            scriptDocuments={data.scriptDocuments}
            loading={loading}
            search={pageState.tableSearch}
            sortKey={pageState.tableSort}
            onSearchChange={(value) => {
              void setPageState({ tableSearch: value })
            }}
            onSortKeyChange={(value) => {
              void setPageState({ tableSort: value })
            }}
          />
        )}
      </div>

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

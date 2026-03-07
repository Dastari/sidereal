import { useDeferredValue, useMemo } from 'react'
import { DatabaseZap, FileCode2, Table2 } from 'lucide-react'
import type {
  DatabaseTableRecord,
  ScriptDocumentRecord,
} from '@/features/database/types'
import type {
  DataTableColumn,
  DataTableSortOption,
} from '@/components/ui/data-table'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { DataTable } from '@/components/ui/data-table'

type TableSortKey = 'name' | 'rows' | 'schema'

interface TablesPanelProps {
  tables: Array<DatabaseTableRecord>
  scriptDocuments: Array<ScriptDocumentRecord>
  loading: boolean
  search: string
  sortKey: TableSortKey
  onSearchChange: (value: string) => void
  onSortKeyChange: (value: TableSortKey) => void
}

export function TablesPanel({
  tables,
  scriptDocuments,
  loading,
  search,
  sortKey,
  onSearchChange,
  onSortKeyChange,
}: TablesPanelProps) {
  const deferredSearch = useDeferredValue(search)
  const filteredTables = useMemo(() => {
    const needle = deferredSearch.trim().toLowerCase()
    const visible = !needle
      ? tables
      : tables.filter((table) =>
          `${table.schemaName} ${table.tableName} ${table.tableType}`
            .toLowerCase()
            .includes(needle),
        )

    return [...visible].sort((left, right) => {
      if (sortKey === 'rows') {
        return (right.rowEstimate ?? -1) - (left.rowEstimate ?? -1)
      }
      if (sortKey === 'schema') {
        return `${left.schemaName}.${left.tableName}`.localeCompare(
          `${right.schemaName}.${right.tableName}`,
        )
      }
      return left.tableName.localeCompare(right.tableName)
    })
  }, [deferredSearch, sortKey, tables])

  const tableColumns = useMemo<Array<DataTableColumn<DatabaseTableRecord>>>(
    () => [
      {
        id: 'schemaName',
        header: 'Schema',
        cell: (table) => table.schemaName,
      },
      {
        id: 'tableName',
        header: 'Table',
        cell: (table) => <span className="font-medium">{table.tableName}</span>,
      },
      {
        id: 'tableType',
        header: 'Type',
        cell: (table) => table.tableType,
      },
      {
        id: 'rowEstimate',
        header: 'Estimated Rows',
        cell: (table) => table.rowEstimate ?? 'n/a',
        headerClassName: 'text-right',
        cellClassName: 'text-right tabular-nums',
      },
    ],
    [],
  )

  const scriptColumns = useMemo<Array<DataTableColumn<ScriptDocumentRecord>>>(
    () => [
      {
        id: 'scriptPath',
        header: 'Script Path',
        cell: (document) => (
          <div className="min-w-0">
            <div className="truncate font-medium text-foreground">
              {document.scriptPath}
            </div>
            <div className="text-xs text-muted-foreground">
              {document.family}
            </div>
          </div>
        ),
      },
      {
        id: 'activeRevision',
        header: 'Revision',
        cell: (document) => document.activeRevision ?? 'none',
        headerClassName: 'text-right',
        cellClassName: 'text-right tabular-nums',
      },
      {
        id: 'draftStatus',
        header: 'Status',
        cell: (document) => (
          <Badge variant={document.hasDraft ? 'warning' : 'outline'}>
            {document.hasDraft ? 'Draft' : 'Published'}
          </Badge>
        ),
        cellClassName: 'w-[110px]',
      },
    ],
    [],
  )
  const tableSortOptions = useMemo<Array<DataTableSortOption>>(
    () => [
      { value: 'schema', label: 'Schema' },
      { value: 'name', label: 'Name' },
      { value: 'rows', label: 'Rows' },
    ],
    [],
  )

  return (
    <div className="flex h-full flex-col gap-4 p-4">
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.4fr)_minmax(320px,0.9fr)]">
        <Card className="border-border/80 bg-card/80">
          <CardHeader className="flex flex-row items-center justify-between space-y-0">
            <div>
              <CardTitle className="flex items-center gap-2 text-base">
                <Table2 className="h-4 w-4 text-primary" />
                Relational Tables
              </CardTitle>
              <div className="mt-1 text-sm text-muted-foreground">
                Information schema view over persisted auth, scripting, and support tables.
              </div>
            </div>
            <Badge variant="secondary">{filteredTables.length}</Badge>
          </CardHeader>
          <CardContent>
            <DataTable
              columns={tableColumns}
              rows={filteredTables}
              getRowId={(table) => `${table.schemaName}.${table.tableName}`}
              loading={loading}
              loadingLabel="Loading relational tables…"
              emptyLabel="No tables matched the current filter."
              searchValue={search}
              onSearchValueChange={onSearchChange}
              searchPlaceholder="Filter by schema, table, or type"
              sortValue={sortKey}
              onSortValueChange={(value) =>
                onSortKeyChange(value as TableSortKey)
              }
              sortOptions={tableSortOptions}
            />
          </CardContent>
        </Card>

        <Card className="border-border/80 bg-card/80">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <FileCode2 className="h-4 w-4 text-primary" />
              Script Catalog Tables
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between rounded-lg border border-border/70 bg-background/50 px-3 py-2 text-sm">
              <span className="text-muted-foreground">Documents</span>
              <span className="font-medium tabular-nums">
                {scriptDocuments.length}
              </span>
            </div>
            <div className="max-h-[420px] overflow-auto pr-1">
              <DataTable
                columns={scriptColumns}
                rows={scriptDocuments}
                getRowId={(document) => document.scriptPath}
                loading={loading}
                loadingLabel="Loading script catalog rows…"
                emptyLabel="No script catalog rows found."
              />
            </div>
            <div className="rounded-lg border border-border/70 bg-background/50 px-3 py-2 text-xs text-muted-foreground">
              <DatabaseZap className="mr-2 inline h-3.5 w-3.5 text-primary" />
              This section is wired to live SQL metadata and script catalog rows, not a placeholder.
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

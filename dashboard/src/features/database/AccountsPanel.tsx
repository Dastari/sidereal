import { useDeferredValue, useMemo } from 'react'
import { Users } from 'lucide-react'
import type { DatabaseAccountRecord } from '@/features/database/types'
import type {
  DataTableColumn,
  DataTableSortOption,
} from '@/components/ui/data-table'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { DataTable } from '@/components/ui/data-table'

type AccountSortKey = 'email' | 'characters' | 'created'

interface AccountsPanelProps {
  accounts: Array<DatabaseAccountRecord>
  loading: boolean
  search: string
  sortKey: AccountSortKey
  onSearchChange: (value: string) => void
  onSortKeyChange: (value: AccountSortKey) => void
}

export function AccountsPanel({
  accounts,
  loading,
  search,
  sortKey,
  onSearchChange,
  onSortKeyChange,
}: AccountsPanelProps) {
  const deferredSearch = useDeferredValue(search)
  const filteredAccounts = useMemo(() => {
    const needle = deferredSearch.trim().toLowerCase()
    const visible = !needle
      ? accounts
      : accounts.filter((account) =>
          `${account.email} ${account.accountId} ${account.primaryPlayerEntityId}`
            .toLowerCase()
            .includes(needle),
        )

    return [...visible].sort((left, right) => {
      if (sortKey === 'characters') {
        return right.characterCount - left.characterCount
      }
      if (sortKey === 'created') {
        return right.createdAtEpochS - left.createdAtEpochS
      }
      return left.email.localeCompare(right.email)
    })
  }, [accounts, deferredSearch, sortKey])

  const columns = useMemo<Array<DataTableColumn<DatabaseAccountRecord>>>(
    () => [
      {
        id: 'email',
        header: 'Email',
        cell: (account) => (
          <span className="font-medium">{account.email}</span>
        ),
      },
      {
        id: 'accountId',
        header: 'Account ID',
        cell: (account) => account.accountId,
        cellClassName: 'font-mono text-xs',
      },
      {
        id: 'primaryPlayerEntityId',
        header: 'Primary Player',
        cell: (account) => account.primaryPlayerEntityId,
        cellClassName: 'font-mono text-xs',
      },
      {
        id: 'characterCount',
        header: 'Characters',
        cell: (account) => account.characterCount,
        headerClassName: 'text-right',
        cellClassName: 'text-right tabular-nums',
      },
      {
        id: 'createdAtEpochS',
        header: 'Created',
        cell: (account) =>
          new Date(account.createdAtEpochS * 1000).toLocaleString(),
        cellClassName: 'text-muted-foreground',
      },
    ],
    [],
  )
  const sortOptions = useMemo<Array<DataTableSortOption>>(
    () => [
      { value: 'email', label: 'Email' },
      { value: 'characters', label: 'Characters' },
      { value: 'created', label: 'Created' },
    ],
    [],
  )

  return (
    <div className="flex h-full flex-col gap-4 p-4">
      <Card className="border-border/80 bg-card/80">
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <div>
            <CardTitle className="flex items-center gap-2 text-base">
              <Users className="h-4 w-4 text-primary" />
              Accounts
            </CardTitle>
            <div className="mt-1 text-sm text-muted-foreground">
              Auth accounts and character ownership in the current database.
            </div>
          </div>
          <Badge variant="secondary">{filteredAccounts.length}</Badge>
        </CardHeader>
        <CardContent>
          <DataTable
            columns={columns}
            rows={filteredAccounts}
            getRowId={(account) => account.accountId}
            loading={loading}
            loadingLabel="Loading account records…"
            emptyLabel="No accounts matched the current filter."
            searchValue={search}
            onSearchValueChange={onSearchChange}
            searchPlaceholder="Filter by email, account ID, or player entity ID"
            sortValue={sortKey}
            onSortValueChange={(value) =>
              onSortKeyChange(value as AccountSortKey)
            }
            sortOptions={sortOptions}
          />
        </CardContent>
      </Card>
    </div>
  )
}

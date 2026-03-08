import { useDeferredValue, useMemo, useState } from 'react'
import { KeyRound, MoreHorizontal, PencilLine, Trash2, Users, X } from 'lucide-react'
import type { DatabaseAccountRecord } from '@/features/database/types'
import type {
  DataTableColumn,
  DataTableSortState,
} from '@/components/ui/data-table'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { DataTable } from '@/components/ui/data-table'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

type AccountSortKey = 'email' | 'characters' | 'created'

interface AccountsPanelProps {
  accounts: Array<DatabaseAccountRecord>
  loading: boolean
  search: string
  sortKey: AccountSortKey
  onSearchChange: (value: string) => void
  onSortKeyChange: (value: AccountSortKey) => void
  onRequestPasswordReset: (
    account: DatabaseAccountRecord,
  ) => Promise<{ accepted: boolean; resetToken: string | null }>
  onRenameCharacter: (
    playerEntityId: string,
    displayName: string,
  ) => Promise<void>
}

export function AccountsPanel({
  accounts,
  loading,
  search,
  sortKey,
  onSearchChange,
  onSortKeyChange,
  onRequestPasswordReset,
  onRenameCharacter,
}: AccountsPanelProps) {
  const [pendingPasswordResetByAccountId, setPendingPasswordResetByAccountId] =
    useState<Record<string, boolean>>({})
  const [pendingRenameByCharacterId, setPendingRenameByCharacterId] = useState<
    Record<string, boolean>
  >({})
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const deferredSearch = useDeferredValue(search)
  const filteredAccounts = useMemo(() => {
    const needle = deferredSearch.trim().toLowerCase()
    const visible = !needle
      ? accounts
      : accounts.filter((account) =>
          `${account.email} ${account.accountId} ${account.primaryPlayerEntityId} ${account.characters
            .map((character) => `${character.playerEntityId} ${character.displayName ?? ''}`)
            .join(' ')}`
            .toLowerCase()
            .includes(needle),
        )

    return visible
  }, [accounts, deferredSearch])

  const columns = useMemo<Array<DataTableColumn<DatabaseAccountRecord>>>(
    () => [
      {
        id: 'email',
        header: 'Email',
        sortable: true,
        sortAccessor: (account) => account.email,
        minWidth: 220,
        cell: (account) => <span className="font-medium">{account.email}</span>,
      },
      {
        id: 'accountId',
        header: 'Account ID',
        sortable: true,
        sortAccessor: (account) => account.accountId,
        minWidth: 280,
        cell: (account) => account.accountId,
        cellClassName: 'font-mono text-xs',
      },
      {
        id: 'primaryPlayerEntityId',
        header: 'Primary Player',
        sortable: true,
        sortAccessor: (account) => account.primaryPlayerEntityId,
        minWidth: 280,
        cell: (account) => account.primaryPlayerEntityId,
        cellClassName: 'font-mono text-xs',
      },
      {
        id: 'characterCount',
        header: 'Characters',
        sortable: true,
        sortAccessor: (account) => account.characterCount,
        minWidth: 320,
        cell: (account) => (
          <div className="flex flex-col items-start gap-1">
            <span className="tabular-nums text-sm">{account.characterCount}</span>
            <div className="flex flex-wrap gap-1">
              {account.characters.map((character) => {
                const pending = pendingRenameByCharacterId[character.playerEntityId] === true
                return (
                  <Badge
                    key={character.playerEntityId}
                    variant="outline"
                    className="gap-1 font-mono text-[11px]"
                  >
                    <span className="max-w-[18rem] truncate font-sans">
                      {character.displayName ?? '(unnamed)'}
                    </span>
                    <span>{character.playerEntityId.slice(0, 8)}</span>
                    <button
                      type="button"
                      className="rounded p-0.5 transition-colors hover:bg-accent disabled:opacity-50"
                      disabled={pending}
                      onClick={() => {
                        const currentName = character.displayName ?? ''
                        const nextName = window.prompt(
                          'Rename character display name',
                          currentName,
                        )
                        if (
                          nextName === null ||
                          nextName.trim() === currentName.trim()
                        ) {
                          return
                        }
                        setError(null)
                        setMessage(null)
                        setPendingRenameByCharacterId((current) => ({
                          ...current,
                          [character.playerEntityId]: true,
                        }))
                        void onRenameCharacter(character.playerEntityId, nextName.trim())
                          .then(() => {
                            setMessage(
                              `Renamed ${character.playerEntityId.slice(0, 8)} to "${nextName.trim()}".`,
                            )
                          })
                          .catch((renameError: unknown) => {
                            setError(
                              renameError instanceof Error
                                ? renameError.message
                                : 'Failed to rename character',
                            )
                          })
                          .finally(() => {
                            setPendingRenameByCharacterId((current) => ({
                              ...current,
                              [character.playerEntityId]: false,
                            }))
                          })
                      }}
                      title="Rename character"
                    >
                      <PencilLine className="h-3 w-3" />
                    </button>
                  </Badge>
                )
              })}
            </div>
          </div>
        ),
      },
      {
        id: 'createdAtEpochS',
        header: 'Created',
        sortable: true,
        sortAccessor: (account) => account.createdAtEpochS,
        minWidth: 180,
        cell: (account) =>
          new Date(account.createdAtEpochS * 1000).toLocaleString(),
        cellClassName: 'text-muted-foreground',
      },
      {
        id: 'actions',
        header: 'Actions',
        sortable: false,
        enableHiding: false,
        width: 96,
        cell: (account) => {
          const pending = pendingPasswordResetByAccountId[account.accountId] === true
          return (
            <div className="flex justify-end">
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="ghost" size="icon" className="h-8 w-8" disabled={pending}>
                    <MoreHorizontal className="h-4 w-4" />
                    <span className="sr-only">Open actions</span>
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-44">
                  <DropdownMenuItem
                    disabled={pending}
                    onSelect={() => {
                      setError(null)
                      setMessage(null)
                      setPendingPasswordResetByAccountId((current) => ({
                        ...current,
                        [account.accountId]: true,
                      }))
                      void onRequestPasswordReset(account)
                        .then((result) => {
                          setMessage(
                            result.resetToken
                              ? `Password reset token for ${account.email}: ${result.resetToken}`
                              : `Password reset requested for ${account.email}.`,
                          )
                        })
                        .catch((requestError: unknown) => {
                          setError(
                            requestError instanceof Error
                              ? requestError.message
                              : 'Failed to request password reset',
                          )
                        })
                        .finally(() => {
                          setPendingPasswordResetByAccountId((current) => ({
                            ...current,
                            [account.accountId]: false,
                          }))
                        })
                    }}
                  >
                    <KeyRound className="mr-2 h-4 w-4" />
                    Reset password
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          )
        },
        headerClassName: 'text-right',
      },
    ],
    [
      onRenameCharacter,
      onRequestPasswordReset,
      pendingPasswordResetByAccountId,
      pendingRenameByCharacterId,
    ],
  )

  const defaultSortState = useMemo<DataTableSortState>(() => {
    if (sortKey === 'characters') {
      return { columnId: 'characterCount', direction: 'desc' }
    }
    if (sortKey === 'created') {
      return { columnId: 'createdAtEpochS', direction: 'desc' }
    }
    return { columnId: 'email', direction: 'asc' }
  }, [sortKey])

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
          {message ? (
            <div className="mb-3 rounded-md border border-border/80 bg-muted/40 px-3 py-2 text-xs">
              {message}
            </div>
          ) : null}
          {error ? (
            <div className="mb-3 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {error}
            </div>
          ) : null}
          <DataTable
            columns={columns}
            rows={filteredAccounts}
            getRowId={(account) => account.accountId}
            loading={loading}
            loadingLabel="Loading account records..."
            emptyLabel="No accounts matched the current filter."
            searchValue={search}
            onSearchValueChange={onSearchChange}
            searchPlaceholder="Filter by email, account ID, or player entity ID"
            defaultSortState={defaultSortState}
            onSortStateChange={(value) => {
              if (!value) return
              if (value.columnId === 'characterCount') onSortKeyChange('characters')
              else if (value.columnId === 'createdAtEpochS') onSortKeyChange('created')
              else onSortKeyChange('email')
            }}
            paginationMode="pagination"
            defaultPageSize={20}
            pageSizeOptions={[10, 20, 50]}
            selectionMode="multiple"
            actionBar={({ selectedRows, clearSelection }) => (
              <div className="flex items-center gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  disabled={selectedRows.length === 0}
                  onClick={() => {
                    setError(
                      selectedRows.length > 0
                        ? `Delete hook ready for ${selectedRows.length} selected accounts. Wire this to your delete API.`
                        : null,
                    )
                  }}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                  Delete selected
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  disabled={selectedRows.length === 0}
                  onClick={clearSelection}
                >
                  <X className="h-3.5 w-3.5" />
                  Clear
                </Button>
              </div>
            )}
          />
        </CardContent>
      </Card>
    </div>
  )
}

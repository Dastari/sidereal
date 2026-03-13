import { useDeferredValue, useEffect, useMemo, useState } from 'react'
import {
  KeyRound,
  MoreHorizontal,
  PencilLine,
  Trash2,
  Users,
  X,
} from 'lucide-react'
import type {
  DatabaseAccountRecord,
  DatabaseCharacterRecord,
} from '@/features/database/types'
import type {
  DataTableColumn,
  DataTableSortState,
} from '@/components/ui/data-table'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { DataTable } from '@/components/ui/data-table'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

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
  ) => Promise<{ accepted: boolean }>
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
  const [renameDialogOpen, setRenameDialogOpen] = useState(false)
  const [renameCharacter, setRenameCharacter] =
    useState<DatabaseCharacterRecord | null>(null)
  const [renameValue, setRenameValue] = useState('')

  useEffect(() => {
    if (!renameCharacter) {
      setRenameValue('')
      return
    }
    setRenameValue(renameCharacter.displayName ?? '')
  }, [renameCharacter])

  const deferredSearch = useDeferredValue(search)
  const filteredAccounts = useMemo(() => {
    const needle = deferredSearch.trim().toLowerCase()
    return !needle
      ? accounts
      : accounts.filter((account) =>
          `${account.email} ${account.accountId} ${account.primaryPlayerEntityId} ${account.characters
            .map(
              (character) =>
                `${character.playerEntityId} ${character.displayName ?? ''}`,
            )
            .join(' ')}`
            .toLowerCase()
            .includes(needle),
        )
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
            <span className="tabular-nums text-sm">
              {account.characterCount}
            </span>
            <div className="flex flex-wrap gap-1">
              {account.characters.map((character) => {
                const pending =
                  pendingRenameByCharacterId[character.playerEntityId] === true
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
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      className="h-5 w-5 rounded p-0 text-muted-foreground hover:bg-accent"
                      disabled={pending}
                      onClick={() => {
                        setError(null)
                        setMessage(null)
                        setRenameCharacter(character)
                        setRenameDialogOpen(true)
                      }}
                      title="Rename character"
                    >
                      <PencilLine className="h-3 w-3" />
                    </Button>
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
          const pending =
            pendingPasswordResetByAccountId[account.accountId] === true
          return (
            <div className="flex justify-end">
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-8 w-8"
                    disabled={pending}
                  >
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
                        .then(() => {
                          setMessage(
                            `Password reset requested for ${account.email}. The raw token is no longer returned to the browser.`,
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

  const submitRename = async () => {
    if (!renameCharacter) {
      return
    }
    const nextName = renameValue.trim()
    const currentName = (renameCharacter.displayName ?? '').trim()
    if (nextName.length < 2 || nextName.length > 64) {
      setError('Display name must be between 2 and 64 characters.')
      return
    }
    if (nextName === currentName) {
      setRenameDialogOpen(false)
      return
    }

    setError(null)
    setMessage(null)
    setPendingRenameByCharacterId((current) => ({
      ...current,
      [renameCharacter.playerEntityId]: true,
    }))

    try {
      await onRenameCharacter(renameCharacter.playerEntityId, nextName)
      setMessage(
        `Renamed ${renameCharacter.playerEntityId.slice(0, 8)} to "${nextName}".`,
      )
      setRenameDialogOpen(false)
      setRenameCharacter(null)
    } catch (renameError) {
      setError(
        renameError instanceof Error
          ? renameError.message
          : 'Failed to rename character',
      )
    } finally {
      setPendingRenameByCharacterId((current) => ({
        ...current,
        [renameCharacter.playerEntityId]: false,
      }))
    }
  }

  return (
    <>
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
              <Alert variant="success" className="mb-3">
                <AlertTitle>Action complete</AlertTitle>
                <AlertDescription>{message}</AlertDescription>
              </Alert>
            ) : null}
            {error ? (
              <Alert variant="destructive" className="mb-3">
                <AlertTitle>Action failed</AlertTitle>
                <AlertDescription>{error}</AlertDescription>
              </Alert>
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
                if (value.columnId === 'characterCount')
                  onSortKeyChange('characters')
                else if (value.columnId === 'createdAtEpochS')
                  onSortKeyChange('created')
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

      <Dialog
        open={renameDialogOpen}
        onOpenChange={(open) => {
          setRenameDialogOpen(open)
          if (!open) {
            setRenameCharacter(null)
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Rename character</DialogTitle>
            <DialogDescription>
              Update the persisted display name for this player entity.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="character-display-name">Display name</Label>
            <Input
              id="character-display-name"
              value={renameValue}
              onChange={(event) => setRenameValue(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault()
                  void submitRename()
                }
              }}
            />
            {renameCharacter ? (
              <p className="text-xs text-muted-foreground">
                Player entity:{' '}
                <span className="font-mono">
                  {renameCharacter.playerEntityId}
                </span>
              </p>
            ) : null}
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setRenameDialogOpen(false)
                setRenameCharacter(null)
              }}
            >
              Cancel
            </Button>
            <Button
              disabled={renameCharacter == null}
              onClick={() => void submitRename()}
            >
              Save name
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

import { useDeferredValue, useEffect, useMemo, useState } from 'react'
import {
  KeyRound,
  MoreHorizontal,
  PencilLine,
  ShieldCheck,
  ShieldOff,
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

type AccountSortKey = 'email' | 'characters' | 'mfa' | 'created'

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

function accountSearchText(account: DatabaseAccountRecord) {
  return [
    account.email,
    account.accountId,
    account.primaryPlayerEntityId,
    account.mfaTotpEnabled ? 'mfa enabled totp authenticator' : 'mfa disabled',
    ...account.characters.flatMap((character) => [
      character.playerEntityId,
      character.displayName ?? '',
      character.status,
    ]),
  ].join(' ')
}

function formatEpoch(epochS: number | null) {
  if (!epochS) return 'n/a'
  return new Date(epochS * 1000).toLocaleString()
}

function CharacterList({
  account,
  pendingRenameByCharacterId,
  onRename,
}: {
  account: DatabaseAccountRecord
  pendingRenameByCharacterId: Record<string, boolean>
  onRename: (character: DatabaseCharacterRecord) => void
}) {
  if (account.characters.length === 0) {
    return <span className="text-sm text-muted-foreground">No characters</span>
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="text-xs text-muted-foreground">
        {account.characterCount} character
        {account.characterCount === 1 ? '' : 's'}
      </div>
      <div className="grid gap-2">
        {account.characters.map((character) => {
          const pending =
            pendingRenameByCharacterId[character.playerEntityId] === true
          return (
            <div
              key={character.playerEntityId}
              className="grid gap-1 rounded-md border border-border/70 bg-background/45 px-2.5 py-2"
            >
              <div className="flex min-w-0 items-center gap-2">
                <span className="truncate font-medium">
                  {character.displayName ?? '(unnamed)'}
                </span>
                <Badge
                  variant={
                    character.status === 'active' ? 'secondary' : 'outline'
                  }
                  className="shrink-0 text-[10px]"
                >
                  {character.status}
                </Badge>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  className="ml-auto h-6 w-6 rounded p-0 text-muted-foreground hover:bg-accent"
                  disabled={pending}
                  onClick={() => onRename(character)}
                  title="Rename character"
                >
                  <PencilLine className="h-3 w-3" />
                </Button>
              </div>
              <div className="grid gap-1 text-[11px] text-muted-foreground md:grid-cols-[minmax(0,1fr)_auto]">
                <span className="truncate font-mono">
                  {character.playerEntityId}
                </span>
                <span className="tabular-nums">
                  Created {formatEpoch(character.createdAtEpochS)}
                </span>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
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
          accountSearchText(account).toLowerCase().includes(needle),
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
        cell: (account) => (
          <div className="min-w-0">
            <div className="truncate font-medium">{account.email}</div>
            <div className="truncate font-mono text-xs text-muted-foreground">
              {account.accountId}
            </div>
          </div>
        ),
      },
      {
        id: 'accountId',
        header: 'Account ID',
        sortable: true,
        sortAccessor: (account) => account.accountId,
        minWidth: 280,
        defaultVisible: false,
        cell: (account) => account.accountId,
        cellClassName: 'font-mono text-xs',
      },
      {
        id: 'primaryPlayerEntityId',
        header: 'Legacy Player',
        sortable: true,
        sortAccessor: (account) => account.primaryPlayerEntityId,
        minWidth: 280,
        defaultVisible: false,
        cell: (account) => account.primaryPlayerEntityId || 'none',
        cellClassName: 'font-mono text-xs',
      },
      {
        id: 'mfa',
        header: 'MFA',
        sortable: true,
        sortAccessor: (account) => (account.mfaTotpEnabled ? 1 : 0),
        minWidth: 170,
        cell: (account) => (
          <div className="flex flex-col items-start gap-1">
            <Badge
              variant={account.mfaTotpEnabled ? 'success' : 'outline'}
              className="gap-1"
            >
              {account.mfaTotpEnabled ? (
                <ShieldCheck className="h-3.5 w-3.5" />
              ) : (
                <ShieldOff className="h-3.5 w-3.5" />
              )}
              {account.mfaTotpEnabled ? 'TOTP enabled' : 'Not enrolled'}
            </Badge>
            {account.mfaTotpEnabled ? (
              <span className="text-[11px] text-muted-foreground">
                Verified {formatEpoch(account.mfaVerifiedAtEpochS)}
              </span>
            ) : null}
          </div>
        ),
      },
      {
        id: 'characterCount',
        header: 'Characters',
        sortable: true,
        sortAccessor: (account) => account.characterCount,
        minWidth: 460,
        cell: (account) => (
          <CharacterList
            account={account}
            pendingRenameByCharacterId={pendingRenameByCharacterId}
            onRename={(character) => {
              setError(null)
              setMessage(null)
              setRenameCharacter(character)
              setRenameDialogOpen(true)
            }}
          />
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
    if (sortKey === 'mfa') {
      return { columnId: 'mfa', direction: 'desc' }
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
              getSearchText={(account) => accountSearchText(account)}
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
                else if (value.columnId === 'mfa') onSortKeyChange('mfa')
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

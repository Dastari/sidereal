import * as React from 'react'
import { createFileRoute } from '@tanstack/react-router'
import {
  LogIn,
  Plus,
  RefreshCcw,
  ShieldCheck,
  Trash2,
  UserRound,
} from 'lucide-react'
import type { AccountCharacterSummary } from '@/lib/dashboard-auth'
import {
  createAccountCharacter,
  deleteAccountCharacter,
  loadAccountCharacters,
  resetAccountCharacter,
  useDashboardSession,
} from '@/lib/dashboard-auth'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { HUDFrame } from '@/components/ui/hud-frame'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { cn } from '@/lib/utils'

export const Route = createFileRoute('/_dashboard/')({
  component: AccountOverviewPage,
})

type PendingConfirmation =
  | { kind: 'delete'; character: AccountCharacterSummary }
  | { kind: 'reset'; character: AccountCharacterSummary }

function AccountOverviewPage() {
  const session = useDashboardSession()
  const [characters, setCharacters] = React.useState<
    Array<AccountCharacterSummary>
  >([])
  const [selectedId, setSelectedId] = React.useState<string | null>(null)
  const [loading, setLoading] = React.useState(true)
  const [busyId, setBusyId] = React.useState<string | null>(null)
  const [error, setError] = React.useState<string | null>(null)
  const [createOpen, setCreateOpen] = React.useState(false)
  const [newDisplayName, setNewDisplayName] = React.useState('')
  const [confirmation, setConfirmation] =
    React.useState<PendingConfirmation | null>(null)

  const selectedCharacter = React.useMemo(() => {
    if (selectedId) {
      const match = characters.find(
        (character) => character.playerEntityId === selectedId,
      )
      if (match) return match
    }
    return characters.length > 0 ? characters[0] : null
  }, [characters, selectedId])
  const selectedPlayerEntityId = selectedCharacter
    ? selectedCharacter.playerEntityId
    : null

  React.useEffect(() => {
    if (!selectedId && characters[0]) {
      setSelectedId(characters[0].playerEntityId)
    }
    if (
      selectedId &&
      !characters.some((character) => character.playerEntityId === selectedId)
    ) {
      setSelectedId(characters[0]?.playerEntityId ?? null)
    }
  }, [characters, selectedId])

  const refreshCharacters = React.useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const payload = await loadAccountCharacters()
      setCharacters(payload.characters)
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : 'Failed to load characters.',
      )
    } finally {
      setLoading(false)
    }
  }, [])

  React.useEffect(() => {
    void refreshCharacters()
  }, [refreshCharacters])

  const createCharacter = React.useCallback(async () => {
    setError(null)
    try {
      const character = await createAccountCharacter(newDisplayName)
      setCharacters((current) => [...current, character])
      setSelectedId(character.playerEntityId)
      setNewDisplayName('')
      setCreateOpen(false)
    } catch (createError) {
      setError(
        createError instanceof Error
          ? createError.message
          : 'Failed to create character.',
      )
    }
  }, [newDisplayName])

  const confirmAction = React.useCallback(async () => {
    if (!confirmation) return
    setBusyId(confirmation.character.playerEntityId)
    setError(null)
    try {
      if (confirmation.kind === 'delete') {
        await deleteAccountCharacter(confirmation.character.playerEntityId)
        setCharacters((current) =>
          current.filter(
            (character) =>
              character.playerEntityId !==
              confirmation.character.playerEntityId,
          ),
        )
      } else {
        const character = await resetAccountCharacter(
          confirmation.character.playerEntityId,
        )
        setCharacters((current) =>
          current.map((existing) =>
            existing.playerEntityId === character.playerEntityId
              ? character
              : existing,
          ),
        )
      }
    } catch (actionError) {
      setError(
        actionError instanceof Error
          ? actionError.message
          : `Failed to ${confirmation.kind} character.`,
      )
    } finally {
      setBusyId(null)
    }
  }, [confirmation])

  return (
    <div className="relative flex h-full overflow-hidden bg-background text-foreground">
      <div
        aria-hidden="true"
        className="absolute inset-0 bg-[radial-gradient(circle_at_42%_18%,color-mix(in_oklch,var(--primary)_16%,transparent),transparent_34%),linear-gradient(140deg,color-mix(in_oklch,var(--background)_92%,black),var(--background))]"
      />
      <div
        aria-hidden="true"
        className="absolute inset-x-0 bottom-0 h-40 border-t border-border/30 bg-[linear-gradient(180deg,transparent,color-mix(in_oklch,var(--card)_80%,black))]"
      />

      <div className="relative z-[1] grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_360px] gap-5 p-5 max-lg:grid-cols-1 max-lg:overflow-auto">
        <section className="relative min-h-[560px] overflow-hidden border border-border/70 bg-card/35">
          <div className="absolute left-5 top-5 z-[2] space-y-2">
            <Badge variant="outline" className="bg-background/70">
              My Account
            </Badge>
            <h1 className="grid-title grid-text-glow text-2xl font-semibold text-primary">
              Character Select
            </h1>
          </div>

          {selectedCharacter ? (
            <div className="flex h-full min-h-[560px] flex-col items-center justify-end px-6 pb-8 pt-20">
              <div className="relative flex min-h-0 w-full flex-1 items-end justify-center">
                <div className="absolute bottom-14 h-14 w-[min(520px,76vw)] border border-primary/20 bg-primary/10 blur-2xl" />
                <img
                  src="/icons/ship.svg"
                  alt=""
                  className="relative z-[1] h-[min(38vh,320px)] w-[min(62vw,520px)] object-contain opacity-95 drop-shadow-[0_0_34px_color-mix(in_oklch,var(--glow)_42%,transparent)]"
                />
              </div>

              <div className="mb-3 text-center">
                <div className="grid-title text-3xl font-semibold text-primary">
                  {selectedCharacter.displayName}
                </div>
                <div className="mt-1 font-mono text-xs text-muted-foreground">
                  {selectedCharacter.playerEntityId}
                </div>
              </div>

              <div className="flex flex-wrap justify-center gap-2">
                <Button disabled>
                  <LogIn className="h-4 w-4" />
                  Enter World
                </Button>
                <Button
                  variant="outline"
                  disabled={busyId === selectedCharacter.playerEntityId}
                  onClick={() =>
                    setConfirmation({
                      kind: 'reset',
                      character: selectedCharacter,
                    })
                  }
                >
                  <RefreshCcw className="h-4 w-4" />
                  Reset
                </Button>
                <Button
                  variant="destructive"
                  disabled={busyId === selectedCharacter.playerEntityId}
                  onClick={() =>
                    setConfirmation({
                      kind: 'delete',
                      character: selectedCharacter,
                    })
                  }
                >
                  <Trash2 className="h-4 w-4" />
                  Delete
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex h-full min-h-[560px] flex-col items-center justify-center px-6 text-center">
              <UserRound className="h-14 w-14 text-primary" />
              <div className="mt-4 grid-title text-xl text-foreground">
                No Characters
              </div>
              <p className="mt-2 max-w-sm text-sm text-muted-foreground">
                Create a character to seed a fresh player entity and starter
                ship for this account.
              </p>
              <Button className="mt-5" onClick={() => setCreateOpen(true)}>
                <Plus className="h-4 w-4" />
                Create Character
              </Button>
            </div>
          )}
        </section>

        <aside className="flex min-h-0 flex-col gap-4">
          <HUDFrame label="Account" className="p-4">
            <div className="flex items-start gap-3">
              <div className="flex h-10 w-10 items-center justify-center border border-primary/40 bg-primary/10 text-primary">
                <ShieldCheck className="h-5 w-5" />
              </div>
              <div className="min-w-0">
                <div className="grid-title text-sm text-foreground">
                  {session?.email ?? 'Signed in account'}
                </div>
                <div className="mt-1 text-xs text-muted-foreground">
                  {session?.mfaVerified ? 'MFA verified' : 'MFA not verified'}
                </div>
              </div>
            </div>
          </HUDFrame>

          {error ? (
            <Alert variant="destructive">
              <AlertTitle>Character action failed</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          <HUDFrame label="Characters" className="min-h-0 flex-1 p-3">
            <div className="mb-3 flex items-center gap-2">
              <Button
                className="flex-1"
                onClick={() => setCreateOpen(true)}
                disabled={loading}
              >
                <Plus className="h-4 w-4" />
                Create New
              </Button>
              <Button
                variant="outline"
                size="icon"
                onClick={() => void refreshCharacters()}
                disabled={loading}
                aria-label="Refresh characters"
              >
                {loading ? (
                  <Spinner className="h-4 w-4" />
                ) : (
                  <RefreshCcw className="h-4 w-4" />
                )}
              </Button>
            </div>

            <div className="min-h-0 space-y-2 overflow-auto pr-1">
              {characters.map((character) => (
                <button
                  key={character.playerEntityId}
                  type="button"
                  className={cn(
                    'group flex w-full items-center gap-3 border border-border/60 bg-background/45 p-2 text-left transition-[background-color,border-color,box-shadow]',
                    selectedPlayerEntityId === character.playerEntityId
                      ? 'border-primary/70 bg-primary/12 shadow-[0_0_20px_color-mix(in_oklch,var(--glow)_24%,transparent)]'
                      : 'hover:border-primary/40 hover:bg-secondary/35',
                  )}
                  onClick={() => setSelectedId(character.playerEntityId)}
                >
                  <div className="h-12 w-12 border border-border/70 bg-card/80 p-2">
                    <img
                      src="/icons/ship.svg"
                      alt=""
                      className="h-full w-full object-contain opacity-90"
                    />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="grid-title truncate text-sm font-semibold text-foreground">
                      {character.displayName}
                    </div>
                    <div className="mt-0.5 text-xs text-muted-foreground">
                      Starter corvette
                    </div>
                  </div>
                  <Badge variant="outline" className="shrink-0">
                    {character.status}
                  </Badge>
                </button>
              ))}

              {!loading && characters.length === 0 ? (
                <div className="border border-dashed border-border/80 p-4 text-sm text-muted-foreground">
                  No account characters yet.
                </div>
              ) : null}
            </div>
          </HUDFrame>
        </aside>
      </div>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create Character</DialogTitle>
            <DialogDescription>
              Character creation seeds account ownership and starter-world graph
              records through the gateway.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="character-display-name">Display name</Label>
            <Input
              id="character-display-name"
              value={newDisplayName}
              onChange={(event) => setNewDisplayName(event.target.value)}
              onKeyDown={(event) => {
                if (
                  event.key === 'Enter' &&
                  newDisplayName.trim().length >= 2
                ) {
                  event.preventDefault()
                  void createCharacter()
                }
              }}
            />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={() => void createCharacter()}
              disabled={newDisplayName.trim().length < 2}
            >
              <Plus className="h-4 w-4" />
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={confirmation !== null}
        onOpenChange={(open) => {
          if (!open) setConfirmation(null)
        }}
        title={
          confirmation?.kind === 'reset'
            ? 'Reset character?'
            : 'Delete character?'
        }
        description={
          confirmation?.kind === 'reset'
            ? `Reset ${confirmation.character.displayName} to a fresh starter-world state.`
            : `Delete ${confirmation?.character.displayName ?? 'this character'} from this account.`
        }
        confirmText={confirmation?.kind === 'reset' ? 'Reset' : 'Delete'}
        variant={confirmation?.kind === 'delete' ? 'destructive' : 'default'}
        onConfirm={confirmAction}
      />
    </div>
  )
}

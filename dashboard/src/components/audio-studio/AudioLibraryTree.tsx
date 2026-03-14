import * as React from 'react'
import {
  ChevronDown,
  ChevronRight,
  FolderTree,
  Music4,
  Radio,
} from 'lucide-react'
import type { AudioStudioCueEntry } from '@/features/audio-studio/types'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { cn } from '@/lib/utils'

interface AudioLibraryTreeProps {
  entries: Array<AudioStudioCueEntry>
  selectedSoundId: string | null
  search: string
  onSelect: (soundId: string) => void
}

type ProfileGroup = {
  profileId: string
  kind: string
  entries: Array<AudioStudioCueEntry>
}

export function AudioLibraryTree({
  entries,
  selectedSoundId,
  search,
  onSelect,
}: AudioLibraryTreeProps) {
  const [openKinds, setOpenKinds] = React.useState<Record<string, boolean>>({})
  const [openProfiles, setOpenProfiles] = React.useState<Record<string, boolean>>({})

  const grouped = React.useMemo(() => {
    const needle = search.trim().toLowerCase()
    const filtered = !needle
      ? entries
      : entries.filter((entry) =>
          [
            entry.displayName,
            entry.profileId,
            entry.cueId,
            entry.kind,
            entry.playbackKind,
            entry.routeBus ?? '',
            entry.clipAssetId ?? '',
            entry.asset?.sourcePath ?? '',
          ]
            .join(' ')
            .toLowerCase()
            .includes(needle),
        )

    const kindMap = new Map<string, Map<string, ProfileGroup>>()
    for (const entry of filtered) {
      const profileGroups =
        kindMap.get(entry.kind) ?? new Map<string, ProfileGroup>()
      if (!kindMap.has(entry.kind)) {
        kindMap.set(entry.kind, profileGroups)
      }
      const profileGroup = profileGroups.get(entry.profileId) ?? {
        profileId: entry.profileId,
        kind: entry.kind,
        entries: [],
      }
      if (!profileGroups.has(entry.profileId)) {
        profileGroups.set(entry.profileId, profileGroup)
      }
      profileGroup.entries.push(entry)
    }

    return Array.from(kindMap.entries())
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([kind, profiles]) => ({
        kind,
        profiles: Array.from(profiles.values())
          .sort((left, right) => left.profileId.localeCompare(right.profileId))
          .map((profile) => ({
            ...profile,
            entries: [...profile.entries].sort((left, right) =>
              left.cueId.localeCompare(right.cueId),
            ),
          })),
      }))
  }, [entries, search])

  const isKindOpen = React.useCallback(
    (kind: string) => openKinds[kind] ?? true,
    [openKinds],
  )
  const isProfileOpen = React.useCallback(
    (profileId: string) => openProfiles[profileId] ?? true,
    [openProfiles],
  )

  return (
    <ScrollArea className="h-full">
      <div className="space-y-3 p-4">
        <div className="grid-hud-frame relative overflow-hidden border border-border/70 bg-card/55 px-3 py-2">
          <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.22em] text-primary/90">
            <FolderTree className="h-4 w-4" />
            Audio Registry
            <Badge variant="secondary" className="ml-auto">
              {entries.length}
            </Badge>
          </div>
          <div className="mt-1 text-xs text-muted-foreground">
            Music, SFX, loops, and authored profile cues.
          </div>
        </div>

        {grouped.map(({ kind, profiles }) => {
          const kindOpen = isKindOpen(kind)
          return (
            <Collapsible
              key={kind}
              open={kindOpen}
              onOpenChange={() =>
                setOpenKinds((prev) => ({ ...prev, [kind]: !(prev[kind] ?? true) }))
              }
            >
              <CollapsibleTrigger className="flex w-full items-center gap-2 px-2 py-1.5 text-left text-sm font-semibold text-foreground/90 transition-colors hover:bg-secondary/30">
                {kindOpen ? (
                  <ChevronDown className="h-4 w-4 text-muted-foreground" />
                ) : (
                  <ChevronRight className="h-4 w-4 text-muted-foreground" />
                )}
                {kind === 'music' ? (
                  <Music4 className="h-4 w-4 text-primary" />
                ) : (
                  <Radio className="h-4 w-4 text-primary" />
                )}
                <span className="capitalize">{kind}</span>
                <Badge variant="outline" className="ml-auto">
                  {profiles.reduce((count, profile) => count + profile.entries.length, 0)}
                </Badge>
              </CollapsibleTrigger>
              <CollapsibleContent>
                <div className="ml-3 border-l border-border/50 pl-3">
                  {profiles.map((profile) => {
                    const profileOpen = isProfileOpen(profile.profileId)
                    return (
                      <Collapsible
                        key={profile.profileId}
                        open={profileOpen}
                        onOpenChange={() =>
                          setOpenProfiles((prev) => ({
                            ...prev,
                            [profile.profileId]: !(prev[profile.profileId] ?? true),
                          }))
                        }
                      >
                        <CollapsibleTrigger className="flex w-full items-center gap-2 px-2 py-1.5 text-left text-sm text-foreground/85 transition-colors hover:bg-secondary/20">
                          {profileOpen ? (
                            <ChevronDown className="h-4 w-4 text-muted-foreground" />
                          ) : (
                            <ChevronRight className="h-4 w-4 text-muted-foreground" />
                          )}
                          <span className="truncate font-mono text-[12px]">
                            {profile.profileId}
                          </span>
                          <Badge variant="secondary" className="ml-auto">
                            {profile.entries.length}
                          </Badge>
                        </CollapsibleTrigger>
                        <CollapsibleContent>
                          <div className="ml-3 space-y-1 border-l border-border/40 pl-3">
                            {profile.entries.map((entry) => (
                              <Button
                                key={entry.soundId}
                                type="button"
                                variant="ghost"
                                className={cn(
                                  'h-auto w-full justify-start px-3 py-2 text-left shadow-none',
                                  entry.soundId === selectedSoundId
                                    ? 'bg-primary/12 text-primary'
                                    : 'text-foreground/85 hover:bg-secondary/20',
                                )}
                                onClick={() => onSelect(entry.soundId)}
                              >
                                <div className="flex min-w-0 flex-1 items-start gap-2">
                                  <div className="mt-1 h-2 w-2 shrink-0 bg-primary/90" />
                                  <div className="min-w-0 flex-1">
                                    <div className="truncate text-sm font-medium">
                                      {entry.cueId}
                                    </div>
                                    <div className="truncate text-[11px] uppercase tracking-[0.16em] text-muted-foreground">
                                      {entry.playbackKind}
                                      {entry.asset ? ` / ${entry.asset.contentType}` : ''}
                                    </div>
                                  </div>
                                </div>
                              </Button>
                            ))}
                          </div>
                        </CollapsibleContent>
                      </Collapsible>
                    )
                  })}
                </div>
              </CollapsibleContent>
            </Collapsible>
          )
        })}

        {grouped.length === 0 ? (
          <div className="px-2 py-8 text-center text-sm text-muted-foreground">
            No audio cues matched the current search.
          </div>
        ) : null}
      </div>
    </ScrollArea>
  )
}

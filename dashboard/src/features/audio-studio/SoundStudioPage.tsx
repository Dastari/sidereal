import * as React from 'react'
import { RadioTower, Search, Volume2 } from 'lucide-react'
import type {
  AudioStudioCatalog,
  AudioStudioCueEntry,
  AudioStudioMarkerDraft,
  AudioStudioMarkers,
} from '@/features/audio-studio/types'
import {
  formatAudioStudioMarkerDraft,
  mergeAudioStudioMarkers,
  parseAudioStudioMarkerDraft,
  resolveEditableAudioStudioMarkers,
} from '@/features/audio-studio/types'
import { AudioLibraryTree } from '@/components/audio-studio/AudioLibraryTree'
import { AudioProfileEditor } from '@/components/audio-studio/AudioProfileEditor'
import { AudioWaveformPlayer } from '@/components/audio-studio/AudioWaveformPlayer'
import {
  AppLayout,
  Panel,
  PanelContent,
  PanelHeader,
} from '@/components/layout/AppLayout'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { useSessionStorageNumber } from '@/hooks/use-session-storage-number'

const DEFAULT_SOUND_STUDIO_SIDEBAR_WIDTH = 300
const DEFAULT_SOUND_STUDIO_DETAIL_WIDTH = 360

function normalizeSelectedSoundId(
  value: string | null | undefined,
): string | null {
  if (!value) {
    return null
  }
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

function hasAnyMarkers(markers: AudioStudioMarkers): boolean {
  return Object.values(markers).some((value) => value !== null)
}

function resolveMarkersSource(
  entry: AudioStudioCueEntry,
): AudioStudioCueEntry['markersSource'] {
  if (hasAnyMarkers(entry.profileMarkers)) {
    return 'profile'
  }
  if (hasAnyMarkers(entry.clipDefaultMarkers)) {
    return 'clip_defaults'
  }
  return 'unconfigured'
}

function mergeMarkers(
  profileMarkers: AudioStudioMarkers,
  clipDefaultMarkers: AudioStudioMarkers,
): AudioStudioMarkers {
  return mergeAudioStudioMarkers(profileMarkers, clipDefaultMarkers)
}

export interface SoundStudioPageProps {
  initialData: AudioStudioCatalog
  selectedSoundId?: string | null
  onSelectedSoundIdChange?: (soundId: string | null) => void
}

export function SoundStudioPage({
  initialData,
  selectedSoundId = null,
  onSelectedSoundIdChange,
}: SoundStudioPageProps) {
  const [entries, setEntries] = React.useState(initialData.entries)
  const [search, setSearch] = React.useState('')
  const [playheadSeconds, setPlayheadSeconds] = React.useState(0)
  const [activeSoundId, setActiveSoundId] = React.useState<string | null>(() =>
    normalizeSelectedSoundId(selectedSoundId),
  )
  const [draftMarkers, setDraftMarkers] =
    React.useState<AudioStudioMarkerDraft>(
      formatAudioStudioMarkerDraft(resolveEditableAudioStudioMarkers(null)),
    )
  const [sidebarWidth, setSidebarWidth] = useSessionStorageNumber(
    'dashboard:sound-studio:sidebar-width',
    DEFAULT_SOUND_STUDIO_SIDEBAR_WIDTH,
  )
  const [detailPanelWidth, setDetailPanelWidth] = useSessionStorageNumber(
    'dashboard:sound-studio:detail-panel-width',
    DEFAULT_SOUND_STUDIO_DETAIL_WIDTH,
  )

  React.useEffect(() => {
    setEntries(initialData.entries)
  }, [initialData])

  React.useEffect(() => {
    setActiveSoundId(normalizeSelectedSoundId(selectedSoundId))
  }, [selectedSoundId])

  const selectedEntry =
    entries.find((entry) => entry.soundId === activeSoundId) ?? null
  const parsedDraftMarkers = React.useMemo(
    () => parseAudioStudioMarkerDraft(draftMarkers),
    [draftMarkers],
  )
  const previewMarkers = React.useMemo(() => {
    if (!selectedEntry) {
      return parsedDraftMarkers
    }
    if (selectedEntry.markersSource === 'clip_defaults') {
      return mergeAudioStudioMarkers(
        selectedEntry.profileMarkers,
        parsedDraftMarkers,
      )
    }
    return mergeAudioStudioMarkers(
      parsedDraftMarkers,
      selectedEntry.clipDefaultMarkers,
    )
  }, [parsedDraftMarkers, selectedEntry])

  React.useEffect(() => {
    setDraftMarkers(
      formatAudioStudioMarkerDraft(
        resolveEditableAudioStudioMarkers(selectedEntry),
      ),
    )
  }, [selectedEntry?.soundId])

  return (
    <AppLayout
      sidebarWidth={sidebarWidth}
      detailPanelWidth={detailPanelWidth}
      onSidebarResize={setSidebarWidth}
      onDetailPanelResize={setDetailPanelWidth}
      header={
        <div className="flex items-center gap-4 px-5 py-3">
          <div className="min-w-0">
            <div className="font-display text-lg uppercase tracking-[0.22em] text-primary">
              Sound Studio
            </div>
            <div className="text-xs uppercase tracking-[0.16em] text-muted-foreground">
              Registry browsing, stereo waveform preview, and Lua marker
              editing.
            </div>
          </div>
          <div className="ml-auto flex flex-wrap items-center gap-2">
            <Badge variant="outline">{initialData.summary.cueCount} cues</Badge>
            <Badge variant="outline">
              {initialData.summary.profileCount} profiles
            </Badge>
            <Badge variant="outline">
              {initialData.summary.musicCount} music
            </Badge>
            <Badge variant="outline">{initialData.summary.sfxCount} sfx</Badge>
          </div>
        </div>
      }
      sidebar={
        <Panel>
          <PanelHeader>
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <RadioTower className="h-4 w-4 text-primary" />
                <div className="text-[11px] uppercase tracking-[0.22em] text-primary/90">
                  Audio Explorer
                </div>
              </div>
              <div className="relative">
                <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder="Search profiles, cues, assets..."
                  className="pl-9"
                />
              </div>
            </div>
          </PanelHeader>
          <PanelContent>
            <AudioLibraryTree
              entries={entries}
              selectedSoundId={activeSoundId}
              search={search}
              onSelect={(soundId) => {
                setActiveSoundId(soundId)
                onSelectedSoundIdChange?.(soundId)
              }}
            />
          </PanelContent>
        </Panel>
      }
      detailPanel={
        <AudioProfileEditor
          entry={selectedEntry}
          playheadSeconds={playheadSeconds}
          draft={draftMarkers}
          onDraftChange={setDraftMarkers}
          onEntryChange={(nextEntry) => {
            setEntries((previous) =>
              previous.map((candidate) => {
                if (candidate.soundId === nextEntry.soundId) {
                  return nextEntry
                }
                if (
                  nextEntry.clipAssetId &&
                  candidate.clipAssetId === nextEntry.clipAssetId &&
                  candidate.markersSource !== 'profile'
                ) {
                  const nextCandidate: AudioStudioCueEntry = {
                    ...candidate,
                    clipDefaultMarkers: nextEntry.clipDefaultMarkers,
                    effectiveMarkers: mergeMarkers(
                      candidate.profileMarkers,
                      nextEntry.clipDefaultMarkers,
                    ),
                  }
                  return {
                    ...nextCandidate,
                    markersSource: resolveMarkersSource(nextCandidate),
                  }
                }
                return candidate
              }),
            )
            setDraftMarkers(
              formatAudioStudioMarkerDraft(
                resolveEditableAudioStudioMarkers(nextEntry),
              ),
            )
          }}
        />
      }
    >
      <div className="flex min-h-0 flex-1 flex-col">
        <div className="flex items-center gap-3 border-b border-border/60 px-4 py-2 text-xs uppercase tracking-[0.16em] text-muted-foreground">
          <Volume2 className="h-4 w-4 text-primary" />
          {selectedEntry ? (
            <>
              <span>{selectedEntry.profileId}</span>
              <span>/</span>
              <span>{selectedEntry.cueId}</span>
            </>
          ) : (
            <span>Select a cue to begin previewing audio</span>
          )}
        </div>
        <AudioWaveformPlayer
          entry={selectedEntry}
          markerValues={previewMarkers}
          draftMarkerValues={parsedDraftMarkers}
          onDraftMarkerValuesChange={(nextMarkers) => {
            setDraftMarkers(formatAudioStudioMarkerDraft(nextMarkers))
          }}
          onPlayheadChange={setPlayheadSeconds}
        />
      </div>
    </AppLayout>
  )
}

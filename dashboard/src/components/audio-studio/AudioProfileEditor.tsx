import * as React from 'react'
import { RotateCcw, Save, Target, Trash2 } from 'lucide-react'
import type {
  AudioStudioCueEntry,
  AudioStudioMarkerDraft,
  AudioStudioMarkerKey,
} from '@/features/audio-studio/types'
import {
  audioStudioMarkerKeys,
  formatAudioStudioMarkerDraft,
  formatAudioStudioMarkerValue,
  parseAudioStudioMarkerDraft,
  resolveEditableAudioStudioMarkers,
} from '@/features/audio-studio/types'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ButtonGroup } from '@/components/ui/button-group'
import { HUDFrame } from '@/components/ui/hud-frame'
import { Input } from '@/components/ui/input'

interface AudioProfileEditorProps {
  entry: AudioStudioCueEntry | null
  playheadSeconds: number
  draft: AudioStudioMarkerDraft
  onDraftChange: (draft: AudioStudioMarkerDraft) => void
  onEntryChange: (entry: AudioStudioCueEntry) => void
}

const markerLabels: Record<AudioStudioMarkerKey, string> = {
  intro_start_s: 'Intro Start',
  loop_start_s: 'Loop Start',
  loop_end_s: 'Loop End',
  outro_start_s: 'Outro Start',
  clip_end_s: 'Clip End',
}

export function AudioProfileEditor({
  entry,
  playheadSeconds,
  draft,
  onDraftChange,
  onEntryChange,
}: AudioProfileEditorProps) {
  const [errorText, setErrorText] = React.useState<string | null>(null)
  const [statusText, setStatusText] = React.useState<string | null>(null)
  const [isSaving, setIsSaving] = React.useState(false)

  React.useEffect(() => {
    setErrorText(null)
    setStatusText(null)
  }, [entry])

  const editableMarkers = resolveEditableAudioStudioMarkers(entry)

  return (
    <HUDFrame
      label="Profile"
      className="flex min-h-0 flex-1 flex-col grow h-full"
    >
      <div className="border-b border-border/60 px-4 py-3">
        <div className="flex items-start gap-2">
          <div className="min-w-0">
            <div className="font-display text-lg uppercase tracking-[0.18em] text-primary">
              Audio Profile
            </div>
            <div className="truncate font-mono text-xs text-muted-foreground">
              {entry?.profileId ?? 'Select a cue to edit its authored markers'}
            </div>
          </div>
          {entry ? (
            <Badge variant="secondary" className="ml-auto">
              {entry.markersSource === 'clip_defaults'
                ? 'Clip Defaults'
                : entry.markersSource === 'profile'
                  ? 'Cue Override'
                  : 'Unconfigured'}
            </Badge>
          ) : null}
        </div>
        {entry ? (
          <div className="mt-3 flex flex-wrap gap-2 text-[11px] uppercase tracking-[0.16em] text-muted-foreground">
            <Badge variant="outline">{entry.kind}</Badge>
            <Badge variant="outline">{entry.playbackKind}</Badge>
            {entry.routeBus ? (
              <Badge variant="outline">{entry.routeBus}</Badge>
            ) : null}
            {entry.spatialMode ? (
              <Badge variant="outline">{entry.spatialMode}</Badge>
            ) : null}
          </div>
        ) : null}
      </div>

      <div className="min-h-0 flex-1 space-y-4 overflow-auto px-4 py-4">
        {entry ? (
          <>
            <div className="space-y-1 text-xs text-muted-foreground">
              <div className="font-mono text-[11px] uppercase tracking-[0.18em] text-foreground/85">
                Clip Asset
              </div>
              <div className="font-mono text-[12px]">
                {entry.clipAssetId ?? 'No clip asset assigned'}
              </div>
              <div>{entry.asset?.sourcePath ?? 'No source path resolved'}</div>
              {entry.markersSource === 'clip_defaults' ? (
                <div className="text-warning">
                  Saving updates the shared clip defaults for this asset.
                </div>
              ) : entry.markersSource === 'unconfigured' ? (
                <div className="text-primary/85">
                  Saving creates cue-local marker values in Lua playback
                  settings.
                </div>
              ) : null}
            </div>

            <div className="space-y-3">
              {audioStudioMarkerKeys.map((key) => (
                <div
                  key={key}
                  className="grid grid-cols-[minmax(0,1fr)_120px] gap-2 border border-border/55 bg-card/45 p-3"
                >
                  <div className="space-y-1">
                    <div className="text-[11px] uppercase tracking-[0.18em] text-primary/90">
                      {markerLabels[key]}
                    </div>
                    <Input
                      type="number"
                      inputMode="decimal"
                      step="0.0001"
                      min="0"
                      value={draft[key]}
                      onChange={(event) => {
                        const next = event.target.value
                        onDraftChange({ ...draft, [key]: next })
                      }}
                      onBlur={() => {
                        const next = draft[key].trim()
                        if (next.length === 0) {
                          return
                        }
                        const parsed = Number(next)
                        if (!Number.isFinite(parsed)) {
                          return
                        }
                        onDraftChange({
                          ...draft,
                          [key]: formatAudioStudioMarkerValue(parsed),
                        })
                      }}
                      placeholder={
                        editableMarkers[key] === null
                          ? 'Unset'
                          : formatAudioStudioMarkerValue(editableMarkers[key])
                      }
                    />
                  </div>

                  <div className="flex flex-col gap-2">
                    <Button
                      type="button"
                      variant="secondary"
                      className="justify-start"
                      onClick={() =>
                        onDraftChange({
                          ...draft,
                          [key]: formatAudioStudioMarkerValue(playheadSeconds),
                        })
                      }
                    >
                      <Target className="h-4 w-4" />
                      Use Playhead
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      className="justify-start"
                      onClick={() =>
                        onDraftChange({
                          ...draft,
                          [key]: '',
                        })
                      }
                    >
                      <Trash2 className="h-4 w-4" />
                      Clear
                    </Button>
                  </div>
                </div>
              ))}
            </div>

            <div className="space-y-2">
              <ButtonGroup>
                <Button
                  type="button"
                  disabled={isSaving}
                  onClick={() => {
                    const nextMarkers = parseAudioStudioMarkerDraft(draft)
                    setIsSaving(true)
                    setErrorText(null)
                    setStatusText(null)

                    void (async () => {
                      try {
                        const response = await fetch(
                          `/api/audio-cues/${encodeURIComponent(entry.soundId)}`,
                          {
                            method: 'POST',
                            headers: {
                              'content-type': 'application/json',
                            },
                            body: JSON.stringify(nextMarkers),
                          },
                        )
                        const payload = (await response
                          .json()
                          .catch(() => ({}))) as {
                          error?: string
                          entry?: AudioStudioCueEntry
                        }
                        if (!response.ok || !payload.entry) {
                          throw new Error(
                            payload.error ?? 'Failed to save audio markers',
                          )
                        }
                        onEntryChange(payload.entry)
                        onDraftChange(
                          formatAudioStudioMarkerDraft(
                            resolveEditableAudioStudioMarkers(payload.entry),
                          ),
                        )
                        setStatusText('Saved audio marker settings to Lua.')
                      } catch (error) {
                        setErrorText(
                          error instanceof Error
                            ? error.message
                            : 'Failed to save audio markers',
                        )
                      } finally {
                        setIsSaving(false)
                      }
                    })()
                  }}
                >
                  <Save className="h-4 w-4" />
                  {isSaving ? 'Saving...' : 'Save Markers'}
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  disabled={isSaving}
                  onClick={() => {
                    onDraftChange(formatAudioStudioMarkerDraft(editableMarkers))
                    setErrorText(null)
                    setStatusText('Reset edits to the current authored values.')
                  }}
                >
                  <RotateCcw className="h-4 w-4" />
                  Reset
                </Button>
              </ButtonGroup>

              {statusText ? (
                <div className="text-xs text-primary/85">{statusText}</div>
              ) : null}
              {errorText ? (
                <div className="text-xs text-destructive">{errorText}</div>
              ) : null}
            </div>
          </>
        ) : (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            Select a sound cue to inspect and edit its marker profile.
          </div>
        )}
      </div>
    </HUDFrame>
  )
}

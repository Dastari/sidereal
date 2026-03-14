export const audioStudioMarkerKeys = [
  'intro_start_s',
  'loop_start_s',
  'loop_end_s',
  'outro_start_s',
  'clip_end_s',
] as const

export type AudioStudioMarkerKey =
  | 'intro_start_s'
  | 'loop_start_s'
  | 'loop_end_s'
  | 'outro_start_s'
  | 'clip_end_s'

export type AudioStudioMarkers = {
  intro_start_s: number | null
  loop_start_s: number | null
  loop_end_s: number | null
  outro_start_s: number | null
  clip_end_s: number | null
}

export type AudioStudioMarkerDraft = Record<AudioStudioMarkerKey, string>

export type AudioStudioMarkersSource =
  | 'profile'
  | 'clip_defaults'
  | 'unconfigured'

export type AudioStudioAssetEntry = {
  assetId: string
  sourcePath: string
  contentType: string
  bootstrapRequired: boolean
  startupRequired: boolean
  byteLength: number | null
  fileExists: boolean
}

export type AudioStudioCueEntry = {
  soundId: string
  profileId: string
  cueId: string
  kind: string
  playbackKind: string
  displayName: string
  clipAssetId: string | null
  asset: AudioStudioAssetEntry | null
  routeBus: string | null
  spatialMode: string | null
  markersSource: AudioStudioMarkersSource
  profileMarkers: AudioStudioMarkers
  clipDefaultMarkers: AudioStudioMarkers
  effectiveMarkers: AudioStudioMarkers
}

export type AudioStudioCatalog = {
  entries: Array<AudioStudioCueEntry>
  summary: {
    cueCount: number
    musicCount: number
    sfxCount: number
    profileCount: number
  }
  audioRegistryPath: string
  assetRegistryPath: string
  loadedAt: string
}

export function encodeSoundId(profileId: string, cueId: string): string {
  return `${profileId}~${cueId}`
}

export function decodeSoundId(
  soundId: string,
): { profileId: string; cueId: string } | null {
  const separatorIndex = soundId.lastIndexOf('~')
  if (separatorIndex <= 0 || separatorIndex >= soundId.length - 1) {
    return null
  }
  return {
    profileId: soundId.slice(0, separatorIndex),
    cueId: soundId.slice(separatorIndex + 1),
  }
}

export function emptyAudioStudioMarkers(): AudioStudioMarkers {
  return {
    intro_start_s: null,
    loop_start_s: null,
    loop_end_s: null,
    outro_start_s: null,
    clip_end_s: null,
  }
}

export function roundAudioStudioMarkerValue(value: number): number {
  return Math.round(value * 10000) / 10000
}

export function formatAudioStudioMarkerValue(value: number): string {
  return roundAudioStudioMarkerValue(value).toFixed(4)
}

export function mergeAudioStudioMarkers(
  preferred: AudioStudioMarkers,
  fallback: AudioStudioMarkers,
): AudioStudioMarkers {
  return {
    intro_start_s: preferred.intro_start_s ?? fallback.intro_start_s,
    loop_start_s: preferred.loop_start_s ?? fallback.loop_start_s,
    loop_end_s: preferred.loop_end_s ?? fallback.loop_end_s,
    outro_start_s: preferred.outro_start_s ?? fallback.outro_start_s,
    clip_end_s: preferred.clip_end_s ?? fallback.clip_end_s,
  }
}

export function resolveEditableAudioStudioMarkers(
  entry: AudioStudioCueEntry | null,
): AudioStudioMarkers {
  if (!entry) {
    return emptyAudioStudioMarkers()
  }
  if (entry.markersSource === 'profile') {
    return entry.profileMarkers
  }
  if (entry.markersSource === 'clip_defaults') {
    return entry.clipDefaultMarkers
  }
  return emptyAudioStudioMarkers()
}

export function formatAudioStudioMarkerDraft(
  markers: AudioStudioMarkers,
): AudioStudioMarkerDraft {
  return {
    intro_start_s:
      markers.intro_start_s === null
        ? ''
        : formatAudioStudioMarkerValue(markers.intro_start_s),
    loop_start_s:
      markers.loop_start_s === null
        ? ''
        : formatAudioStudioMarkerValue(markers.loop_start_s),
    loop_end_s:
      markers.loop_end_s === null
        ? ''
        : formatAudioStudioMarkerValue(markers.loop_end_s),
    outro_start_s:
      markers.outro_start_s === null
        ? ''
        : formatAudioStudioMarkerValue(markers.outro_start_s),
    clip_end_s:
      markers.clip_end_s === null
        ? ''
        : formatAudioStudioMarkerValue(markers.clip_end_s),
  }
}

export function parseAudioStudioMarkerDraft(
  draft: AudioStudioMarkerDraft,
): AudioStudioMarkers {
  const parsed = emptyAudioStudioMarkers()
  for (const key of audioStudioMarkerKeys) {
    const value = draft[key].trim()
    if (value.length === 0) {
      parsed[key] = null
      continue
    }
    const next = Number(value)
    parsed[key] = Number.isFinite(next) ? roundAudioStudioMarkerValue(next) : null
  }
  return parsed
}

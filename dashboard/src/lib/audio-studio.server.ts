import path from 'node:path'
import { promises as fs } from 'node:fs'
import type {
  AudioStudioAssetEntry,
  AudioStudioCatalog,
  AudioStudioCueEntry,
  AudioStudioMarkers,
  AudioStudioMarkersSource,
} from '@/features/audio-studio/types'
import {
  audioStudioMarkerKeys,
  decodeSoundId,
  emptyAudioStudioMarkers,
  encodeSoundId,
} from '@/features/audio-studio/types'

type AudioStudioPaths = {
  repoRoot: string
  dataRoot: string
  audioRegistryPath: string
  assetRegistryPath: string
}

type TableBounds = {
  start: number
  end: number
  bodyStart: number
  bodyEnd: number
}

type ParsedAssetRegistryEntry = {
  assetId: string
  sourcePath: string
  contentType: string
  bootstrapRequired: boolean
  startupRequired: boolean
}

type ParsedClipDefinition = {
  clipAssetId: string
  defaultsBounds: TableBounds | null
  defaultMarkers: AudioStudioMarkers
}

type ParsedCueDefinition = {
  cueId: string
  playbackBounds: TableBounds | null
  playbackKind: string
  clipAssetId: string | null
  routeBus: string | null
  spatialMode: string | null
  profileMarkers: AudioStudioMarkers
}

type ParsedProfileDefinition = {
  profileId: string
  kind: string
  cues: Map<string, ParsedCueDefinition>
}

type ParsedAudioRegistry = {
  clips: Map<string, ParsedClipDefinition>
  profiles: Map<string, ParsedProfileDefinition>
}

const AUDIO_REGISTRY_RELATIVE_PATH = 'data/scripts/audio/registry.lua'
const ASSET_REGISTRY_RELATIVE_PATH = 'data/scripts/assets/registry.lua'

function createEmptyMarkers(): AudioStudioMarkers {
  return emptyAudioStudioMarkers()
}

function hasAnyMarkers(markers: AudioStudioMarkers): boolean {
  return audioStudioMarkerKeys.some((key) => markers[key] !== null)
}

function mergeMarkers(
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

function prettifySegment(value: string): string {
  return value
    .split(/[._-]+/)
    .filter((segment) => segment.length > 0)
    .map((segment) => segment[0].toUpperCase() + segment.slice(1))
    .join(' ')
}

function buildCueDisplayName(profileId: string, cueId: string): string {
  const tail = profileId.split('.').at(-1) ?? profileId
  return `${prettifySegment(tail)} / ${prettifySegment(cueId)}`
}

async function pathExists(targetPath: string): Promise<boolean> {
  try {
    await fs.access(targetPath)
    return true
  } catch {
    return false
  }
}

async function resolveAudioStudioPaths(): Promise<AudioStudioPaths> {
  const candidates = [
    process.env.SIDEREAL_REPO_ROOT?.trim(),
    process.cwd(),
    path.resolve(process.cwd(), '..'),
  ].filter((value): value is string => Boolean(value && value.length > 0))

  for (const candidate of candidates) {
    const repoRoot = path.resolve(candidate)
    const audioRegistryPath = path.join(repoRoot, AUDIO_REGISTRY_RELATIVE_PATH)
    if (await pathExists(audioRegistryPath)) {
      return {
        repoRoot,
        dataRoot: path.join(repoRoot, 'data'),
        audioRegistryPath,
        assetRegistryPath: path.join(repoRoot, ASSET_REGISTRY_RELATIVE_PATH),
      }
    }
  }

  throw new Error('Unable to locate data/scripts/audio/registry.lua')
}

function isIdentifierStart(value: string | undefined): boolean {
  return value !== undefined && /[A-Za-z_]/.test(value)
}

function isIdentifierPart(value: string | undefined): boolean {
  return value !== undefined && /[A-Za-z0-9_]/.test(value)
}

function skipWhitespace(source: string, index: number, max: number): number {
  let cursor = index
  while (cursor < max && /\s/.test(source[cursor] ?? '')) {
    cursor += 1
  }
  return cursor
}

function skipStringLiteral(source: string, index: number, quote: string): number {
  let cursor = index + 1
  while (cursor < source.length) {
    const char = source[cursor]
    if (char === '\\') {
      cursor += 2
      continue
    }
    if (char === quote) {
      return cursor + 1
    }
    cursor += 1
  }
  return source.length
}

function findTableBoundsFromBrace(source: string, braceIndex: number): TableBounds | null {
  if (source[braceIndex] !== '{') {
    return null
  }

  let depth = 0
  let cursor = braceIndex
  while (cursor < source.length) {
    const char = source[cursor]
    if (char === '"' || char === "'") {
      cursor = skipStringLiteral(source, cursor, char)
      continue
    }
    if (char === '{') {
      depth += 1
    } else if (char === '}') {
      depth -= 1
      if (depth === 0) {
        return {
          start: braceIndex,
          end: cursor,
          bodyStart: braceIndex + 1,
          bodyEnd: cursor,
        }
      }
    }
    cursor += 1
  }

  return null
}

function findAssignmentTableBounds(
  source: string,
  assignment: string,
): TableBounds | null {
  const assignmentIndex = source.indexOf(assignment)
  if (assignmentIndex === -1) {
    return null
  }
  const braceIndex = source.indexOf('{', assignmentIndex)
  if (braceIndex === -1) {
    return null
  }
  return findTableBoundsFromBrace(source, braceIndex)
}

function splitTopLevelListEntryBounds(
  source: string,
  parentBounds: TableBounds,
): Array<TableBounds> {
  const entries: Array<TableBounds> = []
  let cursor = parentBounds.bodyStart

  while (cursor < parentBounds.bodyEnd) {
    const char = source[cursor]
    if (char === '"' || char === "'") {
      cursor = skipStringLiteral(source, cursor, char)
      continue
    }
    if (char === '{') {
      const bounds = findTableBoundsFromBrace(source, cursor)
      if (!bounds) {
        break
      }
      entries.push(bounds)
      cursor = bounds.end + 1
      continue
    }
    cursor += 1
  }

  return entries
}

function advancePastTopLevelValue(
  source: string,
  valueStart: number,
  max: number,
): number {
  const char = source[valueStart]
  if (char === '"' || char === "'") {
    return skipStringLiteral(source, valueStart, char)
  }
  if (char === '{') {
    const bounds = findTableBoundsFromBrace(source, valueStart)
    return bounds ? bounds.end + 1 : max
  }

  let cursor = valueStart
  while (cursor < max) {
    const current = source[cursor]
    if (current === ',' || current === '\n') {
      return cursor + 1
    }
    cursor += 1
  }
  return max
}

function findTopLevelAssignmentTableBounds(
  source: string,
  parentBounds: TableBounds,
  key: string,
): TableBounds | null {
  let cursor = parentBounds.bodyStart

  while (cursor < parentBounds.bodyEnd) {
    const char = source[cursor]
    if (char === '"' || char === "'") {
      cursor = skipStringLiteral(source, cursor, char)
      continue
    }
    if (!isIdentifierStart(char)) {
      cursor += 1
      continue
    }

    const keyStart = cursor
    cursor += 1
    while (cursor < parentBounds.bodyEnd && isIdentifierPart(source[cursor])) {
      cursor += 1
    }
    const identifier = source.slice(keyStart, cursor)
    const afterIdentifier = skipWhitespace(source, cursor, parentBounds.bodyEnd)
    if (source[afterIdentifier] !== '=') {
      continue
    }
    const valueStart = skipWhitespace(source, afterIdentifier + 1, parentBounds.bodyEnd)
    if (identifier === key && source[valueStart] === '{') {
      return findTableBoundsFromBrace(source, valueStart)
    }
    cursor = advancePastTopLevelValue(source, valueStart, parentBounds.bodyEnd)
  }

  return null
}

function findTopLevelKeyedTableEntries(
  source: string,
  parentBounds: TableBounds,
): Array<{ key: string; bounds: TableBounds }> {
  const entries: Array<{ key: string; bounds: TableBounds }> = []
  let cursor = parentBounds.bodyStart

  while (cursor < parentBounds.bodyEnd) {
    const char = source[cursor]
    if (char === '"' || char === "'") {
      cursor = skipStringLiteral(source, cursor, char)
      continue
    }
    if (!isIdentifierStart(char)) {
      cursor += 1
      continue
    }

    const keyStart = cursor
    cursor += 1
    while (cursor < parentBounds.bodyEnd && isIdentifierPart(source[cursor])) {
      cursor += 1
    }
    const key = source.slice(keyStart, cursor)
    const afterIdentifier = skipWhitespace(source, cursor, parentBounds.bodyEnd)
    if (source[afterIdentifier] !== '=') {
      continue
    }

    const valueStart = skipWhitespace(source, afterIdentifier + 1, parentBounds.bodyEnd)
    if (source[valueStart] !== '{') {
      cursor = advancePastTopLevelValue(source, valueStart, parentBounds.bodyEnd)
      continue
    }

    const bounds = findTableBoundsFromBrace(source, valueStart)
    if (!bounds) {
      break
    }
    entries.push({ key, bounds })
    cursor = bounds.end + 1
  }

  return entries
}

function parseQuotedStringField(block: string, key: string): string | null {
  return block.match(new RegExp(`${key}\\s*=\\s*"([^"]+)"`))?.[1] ?? null
}

function parseBooleanField(block: string, key: string): boolean {
  return block.match(new RegExp(`${key}\\s*=\\s*(true|false)`))?.[1] === 'true'
}

function parseNumberField(block: string, key: string): number | null {
  const raw = block.match(new RegExp(`${key}\\s*=\\s*(-?\\d+(?:\\.\\d+)?)`))?.[1]
  if (!raw) {
    return null
  }
  const value = Number(raw)
  return Number.isFinite(value) ? value : null
}

function parseMarkersFromBlock(block: string): AudioStudioMarkers {
  return {
    intro_start_s: parseNumberField(block, 'intro_start_s'),
    loop_start_s: parseNumberField(block, 'loop_start_s'),
    loop_end_s: parseNumberField(block, 'loop_end_s'),
    outro_start_s: parseNumberField(block, 'outro_start_s'),
    clip_end_s: parseNumberField(block, 'clip_end_s'),
  }
}

function parseAudioAssetRegistryEntries(source: string): Array<ParsedAssetRegistryEntry> {
  const assetsBounds = findAssignmentTableBounds(source, 'AssetRegistry.assets')
  if (!assetsBounds) {
    return []
  }

  return splitTopLevelListEntryBounds(source, assetsBounds)
    .map((bounds) => source.slice(bounds.start, bounds.end + 1))
    .map((block) => ({
      assetId: parseQuotedStringField(block, 'asset_id'),
      sourcePath: parseQuotedStringField(block, 'source_path'),
      contentType: parseQuotedStringField(block, 'content_type') ?? 'application/octet-stream',
      bootstrapRequired: parseBooleanField(block, 'bootstrap_required'),
      startupRequired: parseBooleanField(block, 'startup_required'),
    }))
    .filter(
      (entry): entry is ParsedAssetRegistryEntry =>
        Boolean(
          entry.assetId &&
            entry.sourcePath &&
            (entry.assetId.startsWith('audio.') ||
              entry.contentType.startsWith('audio/')),
        ),
    )
}

function parseAudioRegistry(source: string): ParsedAudioRegistry {
  const clips = new Map<string, ParsedClipDefinition>()
  const profiles = new Map<string, ParsedProfileDefinition>()

  const clipsBounds = findAssignmentTableBounds(source, 'AudioRegistry.clips')
  if (clipsBounds) {
    for (const clipBounds of splitTopLevelListEntryBounds(source, clipsBounds)) {
      const clipBlock = source.slice(clipBounds.start, clipBounds.end + 1)
      const clipAssetId = parseQuotedStringField(clipBlock, 'clip_asset_id')
      if (!clipAssetId) {
        continue
      }
      const defaultsBounds = findTopLevelAssignmentTableBounds(source, clipBounds, 'defaults')
      const defaultMarkers = defaultsBounds
        ? parseMarkersFromBlock(source.slice(defaultsBounds.start, defaultsBounds.end + 1))
        : createEmptyMarkers()
      clips.set(clipAssetId, {
        clipAssetId,
        defaultsBounds,
        defaultMarkers,
      })
    }
  }

  const profilesBounds = findAssignmentTableBounds(source, 'AudioRegistry.profiles')
  if (profilesBounds) {
    for (const profileBounds of splitTopLevelListEntryBounds(source, profilesBounds)) {
      const profileBlock = source.slice(profileBounds.start, profileBounds.end + 1)
      const profileId = parseQuotedStringField(profileBlock, 'profile_id')
      if (!profileId) {
        continue
      }
      const kind = parseQuotedStringField(profileBlock, 'kind') ?? 'unknown'
      const cuesBounds = findTopLevelAssignmentTableBounds(source, profileBounds, 'cues')
      const cues = new Map<string, ParsedCueDefinition>()
      if (cuesBounds) {
        for (const cueEntry of findTopLevelKeyedTableEntries(source, cuesBounds)) {
          const playbackBounds = findTopLevelAssignmentTableBounds(
            source,
            cueEntry.bounds,
            'playback',
          )
          const routeBounds = findTopLevelAssignmentTableBounds(source, cueEntry.bounds, 'route')
          const spatialBounds = findTopLevelAssignmentTableBounds(
            source,
            cueEntry.bounds,
            'spatial',
          )
          const playbackBlock = playbackBounds
            ? source.slice(playbackBounds.start, playbackBounds.end + 1)
            : ''
          const routeBlock = routeBounds
            ? source.slice(routeBounds.start, routeBounds.end + 1)
            : ''
          const spatialBlock = spatialBounds
            ? source.slice(spatialBounds.start, spatialBounds.end + 1)
            : ''

          cues.set(cueEntry.key, {
            cueId: cueEntry.key,
            playbackBounds,
            playbackKind: parseQuotedStringField(playbackBlock, 'kind') ?? 'unknown',
            clipAssetId: parseQuotedStringField(playbackBlock, 'clip_asset_id'),
            routeBus: parseQuotedStringField(routeBlock, 'bus'),
            spatialMode: parseQuotedStringField(spatialBlock, 'mode'),
            profileMarkers: parseMarkersFromBlock(playbackBlock),
          })
        }
      }
      profiles.set(profileId, {
        profileId,
        kind,
        cues,
      })
    }
  }

  return { clips, profiles }
}

function resolveMarkersSource(
  profileMarkers: AudioStudioMarkers,
  clipDefaultMarkers: AudioStudioMarkers,
): AudioStudioMarkersSource {
  if (hasAnyMarkers(profileMarkers)) {
    return 'profile'
  }
  if (hasAnyMarkers(clipDefaultMarkers)) {
    return 'clip_defaults'
  }
  return 'unconfigured'
}

async function buildAssetIndex(
  paths: AudioStudioPaths,
): Promise<Map<string, AudioStudioAssetEntry>> {
  if (!(await pathExists(paths.assetRegistryPath))) {
    return new Map()
  }

  const assetRegistrySource = await fs.readFile(paths.assetRegistryPath, 'utf8')
  const parsedEntries = parseAudioAssetRegistryEntries(assetRegistrySource)
  const assets = await Promise.all(
    parsedEntries.map(async (entry) => {
      const assetPath = path.join(paths.dataRoot, entry.sourcePath)
      const fileExists = await pathExists(assetPath)
      const stat = fileExists ? await fs.stat(assetPath) : null
      const asset: AudioStudioAssetEntry = {
        assetId: entry.assetId,
        sourcePath: entry.sourcePath,
        contentType: entry.contentType,
        bootstrapRequired: entry.bootstrapRequired,
        startupRequired: entry.startupRequired,
        byteLength: stat?.size ?? null,
        fileExists,
      }
      return [entry.assetId, asset] as const
    }),
  )

  return new Map(assets)
}

async function loadParsedAudioStudioState(paths: AudioStudioPaths): Promise<{
  source: string
  parsedRegistry: ParsedAudioRegistry
  assetIndex: Map<string, AudioStudioAssetEntry>
}> {
  const [source, assetIndex] = await Promise.all([
    fs.readFile(paths.audioRegistryPath, 'utf8'),
    buildAssetIndex(paths),
  ])

  return {
    source,
    parsedRegistry: parseAudioRegistry(source),
    assetIndex,
  }
}

function buildCatalogEntries(
  parsedRegistry: ParsedAudioRegistry,
  assetIndex: Map<string, AudioStudioAssetEntry>,
): Array<AudioStudioCueEntry> {
  const entries: Array<AudioStudioCueEntry> = []

  for (const profile of parsedRegistry.profiles.values()) {
    for (const cue of profile.cues.values()) {
      const clipDefaults =
        cue.clipAssetId !== null
          ? parsedRegistry.clips.get(cue.clipAssetId)?.defaultMarkers ?? createEmptyMarkers()
          : createEmptyMarkers()
      const markersSource = resolveMarkersSource(cue.profileMarkers, clipDefaults)
      const effectiveMarkers = mergeMarkers(cue.profileMarkers, clipDefaults)
      const asset = cue.clipAssetId ? assetIndex.get(cue.clipAssetId) ?? null : null

      entries.push({
        soundId: encodeSoundId(profile.profileId, cue.cueId),
        profileId: profile.profileId,
        cueId: cue.cueId,
        kind: profile.kind,
        playbackKind: cue.playbackKind,
        displayName: buildCueDisplayName(profile.profileId, cue.cueId),
        clipAssetId: cue.clipAssetId,
        asset,
        routeBus: cue.routeBus,
        spatialMode: cue.spatialMode,
        markersSource,
        profileMarkers: cue.profileMarkers,
        clipDefaultMarkers: clipDefaults,
        effectiveMarkers,
      })
    }
  }

  entries.sort((left, right) => {
    const kindOrder = left.kind.localeCompare(right.kind)
    if (kindOrder !== 0) {
      return kindOrder
    }
    const profileOrder = left.profileId.localeCompare(right.profileId)
    if (profileOrder !== 0) {
      return profileOrder
    }
    return left.cueId.localeCompare(right.cueId)
  })

  return entries
}

export async function loadAudioStudioCatalog(): Promise<AudioStudioCatalog> {
  const paths = await resolveAudioStudioPaths()
  const { parsedRegistry, assetIndex } = await loadParsedAudioStudioState(paths)
  const entries = buildCatalogEntries(parsedRegistry, assetIndex)

  return {
    entries,
    summary: {
      cueCount: entries.length,
      musicCount: entries.filter((entry) => entry.kind === 'music').length,
      sfxCount: entries.filter((entry) => entry.kind !== 'music').length,
      profileCount: new Set(entries.map((entry) => entry.profileId)).size,
    },
    audioRegistryPath: path
      .relative(paths.repoRoot, paths.audioRegistryPath)
      .replace(/\\/g, '/'),
    assetRegistryPath: path
      .relative(paths.repoRoot, paths.assetRegistryPath)
      .replace(/\\/g, '/'),
    loadedAt: new Date().toISOString(),
  }
}

async function loadCatalogEntryOrThrow(soundId: string): Promise<{
  entry: AudioStudioCueEntry
  paths: AudioStudioPaths
}> {
  const paths = await resolveAudioStudioPaths()
  const catalog = await loadAudioStudioCatalog()
  const entry = catalog.entries.find((candidate) => candidate.soundId === soundId)
  if (!entry) {
    throw new Error('Audio cue not found')
  }
  return { entry, paths }
}

export async function loadAudioCueAssetBytes(soundId: string): Promise<{
  entry: AudioStudioCueEntry
  bytes: Buffer
  contentType: string
}> {
  const { entry, paths } = await loadCatalogEntryOrThrow(soundId)
  if (!entry.asset) {
    throw new Error('Selected cue does not resolve to an audio asset')
  }

  const assetPath = path.resolve(paths.dataRoot, entry.asset.sourcePath)
  const safePrefix = `${paths.dataRoot}${path.sep}`
  if (!assetPath.startsWith(safePrefix) && assetPath !== paths.dataRoot) {
    throw new Error('Resolved asset path is outside the data root')
  }
  const bytes = await fs.readFile(assetPath)
  return {
    entry,
    bytes,
    contentType: entry.asset.contentType,
  }
}

function getLineIndent(source: string, index: number): string {
  const lineStart = source.lastIndexOf('\n', index - 1) + 1
  let cursor = lineStart
  while (cursor < source.length && (source[cursor] === ' ' || source[cursor] === '\t')) {
    cursor += 1
  }
  return source.slice(lineStart, cursor)
}

function trimBoundaryBlankLines(lines: Array<string>): Array<string> {
  let start = 0
  let end = lines.length
  while (start < end && lines[start]?.trim() === '') {
    start += 1
  }
  while (end > start && lines[end - 1]?.trim() === '') {
    end -= 1
  }
  return lines.slice(start, end)
}

function formatLuaNumber(value: number): string {
  const normalized = value.toFixed(4).replace(/\.?0+$/, '')
  return normalized.includes('.') ? normalized : `${normalized}.0`
}

function replaceMarkersInTableBlock(
  source: string,
  tableBounds: TableBounds,
  markers: AudioStudioMarkers,
): string {
  const existingInner = source.slice(tableBounds.bodyStart, tableBounds.bodyEnd)
  const markerPattern = new RegExp(
    `^\\s*(?:${audioStudioMarkerKeys.join('|')})\\s*=\\s*[^\\n]*,\\s*$`,
    'gm',
  )
  const remainingLines = trimBoundaryBlankLines(
    existingInner
      .replace(markerPattern, '')
      .split('\n')
      .map((line) => line.replace(/[ \t]+$/g, '')),
  )

  const tableIndent = getLineIndent(source, tableBounds.start)
  const fieldIndent = `${tableIndent}  `
  const markerLines = audioStudioMarkerKeys.flatMap((key) => {
    const value = markers[key]
    if (value === null) {
      return []
    }
    return [`${fieldIndent}${key} = ${formatLuaNumber(value)},`]
  })
  const nextLines = [...remainingLines, ...markerLines]
  const nextInner =
    nextLines.length === 0
      ? '\n'
      : `\n${nextLines.join('\n')}\n${tableIndent}`

  return `${source.slice(0, tableBounds.bodyStart)}${nextInner}${source.slice(
    tableBounds.bodyEnd,
  )}`
}

function sanitizeMarkerValues(markers: AudioStudioMarkers): AudioStudioMarkers {
  const sanitized = createEmptyMarkers()
  for (const key of audioStudioMarkerKeys) {
    const value = markers[key]
    sanitized[key] =
      typeof value === 'number' && Number.isFinite(value) ? Number(value) : null
  }
  return sanitized
}

export async function saveAudioCueMarkers(
  soundId: string,
  markers: AudioStudioMarkers,
): Promise<AudioStudioCueEntry> {
  const decoded = decodeSoundId(soundId)
  if (!decoded) {
    throw new Error('Invalid sound id')
  }

  const paths = await resolveAudioStudioPaths()
  const { source, parsedRegistry, assetIndex } = await loadParsedAudioStudioState(paths)
  const profile = parsedRegistry.profiles.get(decoded.profileId)
  const cue = profile?.cues.get(decoded.cueId)
  if (!profile || !cue) {
    throw new Error('Audio cue not found')
  }

  const clip = cue.clipAssetId ? parsedRegistry.clips.get(cue.clipAssetId) ?? null : null
  const currentSource = resolveMarkersSource(
    cue.profileMarkers,
    clip?.defaultMarkers ?? createEmptyMarkers(),
  )
  const targetBounds =
    currentSource === 'clip_defaults' && clip?.defaultsBounds
      ? clip.defaultsBounds
      : cue.playbackBounds
  if (!targetBounds) {
    throw new Error('Selected cue does not have an editable playback table')
  }

  const nextSource = replaceMarkersInTableBlock(
    source,
    targetBounds,
    sanitizeMarkerValues(markers),
  )
  await fs.writeFile(paths.audioRegistryPath, nextSource, 'utf8')

  const refreshedRegistry = parseAudioRegistry(nextSource)
  const refreshedEntries = buildCatalogEntries(refreshedRegistry, assetIndex)
  const refreshedEntry = refreshedEntries.find((entry) => entry.soundId === soundId)
  if (!refreshedEntry) {
    throw new Error('Failed to reload saved audio cue')
  }
  return refreshedEntry
}

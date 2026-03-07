import { performance } from 'node:perf_hooks'
import path from 'node:path'
import { promises as fs } from 'node:fs'

export type ShaderCatalogEntry = {
  shaderId: string
  filename: string
  shaderClass: 'fullscreen' | 'sprite' | 'effect' | 'unknown'
  assetId: string | null
  shaderRole: string | null
  bootstrapRequired: boolean | null
  dependencies: string[]
  sourcePath: string
  cachePath: string | null
  sourceExists: boolean
  cacheExists: boolean
  byteLength: number
  updatedAt: string
}

export type ShaderFileRecord = {
  entry: ShaderCatalogEntry
  source: string
  elapsedMs: number
}

export type ShaderUploadResult = {
  entry: ShaderCatalogEntry
  elapsedMs: number
}

type ShaderPaths = {
  repoRoot: string
  sourceRoot: string
  cacheRoot: string
  assetRegistryPath: string
}

type AssetRegistryEntry = {
  assetId: string
  sourcePath: string
  shaderRole: string | null
  bootstrapRequired: boolean
  dependencies: string[]
}

const WGSL_EXTENSION = '.wgsl'

function encodeShaderId(relativePath: string): string {
  return Buffer.from(relativePath, 'utf8').toString('base64url')
}

function decodeShaderId(shaderId: string): string | null {
  try {
    return Buffer.from(shaderId, 'base64url').toString('utf8')
  } catch {
    return null
  }
}

function classifyShader(filename: string): ShaderCatalogEntry['shaderClass'] {
  const lower = filename.toLowerCase()
  if (
    lower.includes('starfield') ||
    lower.includes('background') ||
    lower.includes('planet')
  ) {
    return 'fullscreen'
  }
  if (
    lower.includes('sprite') ||
    lower.includes('asteroid') ||
    lower.includes('thruster')
  ) {
    return 'sprite'
  }
  if (lower.includes('impact') || lower.includes('overlay')) {
    return 'effect'
  }
  return 'unknown'
}

async function pathExists(targetPath: string): Promise<boolean> {
  try {
    await fs.access(targetPath)
    return true
  } catch {
    return false
  }
}

async function resolveShaderPaths(): Promise<ShaderPaths> {
  const candidates = [
    process.env.SIDEREAL_REPO_ROOT?.trim(),
    process.cwd(),
    path.resolve(process.cwd(), '..'),
  ].filter((value): value is string => Boolean(value && value.length > 0))

  for (const candidate of candidates) {
    const repoRoot = path.resolve(candidate)
    const sourceRoot = path.join(repoRoot, 'data', 'shaders')
    if (await pathExists(sourceRoot)) {
      return {
        repoRoot,
        sourceRoot,
        cacheRoot: path.join(repoRoot, 'data', 'cache_stream', 'shaders'),
        assetRegistryPath: path.join(repoRoot, 'data', 'scripts', 'assets', 'registry.lua'),
      }
    }
  }

  throw new Error('Unable to locate data/shaders for shader workbench')
}

function validateRelativeShaderPath(relativePath: string): string {
  const normalized = relativePath.replace(/\\/g, '/')
  if (!normalized.endsWith(WGSL_EXTENSION)) {
    throw new Error('Shader path must end in .wgsl')
  }
  if (
    normalized.startsWith('/') ||
    normalized.includes('../') ||
    normalized.includes('..\\') ||
    normalized.includes('\0')
  ) {
    throw new Error('Shader path is not allowed')
  }
  return normalized
}

function sanitizeUploadFilename(filename: string): string {
  const trimmed = filename.trim()
  if (trimmed.length === 0) {
    throw new Error('filename is required')
  }
  const basename = path.basename(trimmed).replace(/[^A-Za-z0-9._-]/g, '_')
  if (!basename.endsWith(WGSL_EXTENSION)) {
    throw new Error('Only .wgsl uploads are supported')
  }
  return basename
}

function extractLuaQuotedStrings(raw: string): string[] {
  const matches = raw.match(/"([^"]+)"/g) ?? []
  return matches.map((match) => match.slice(1, -1))
}

export function parseAssetRegistryEntries(source: string): AssetRegistryEntry[] {
  const assetsMarker = 'AssetRegistry.assets'
  const markerIndex = source.indexOf(assetsMarker)
  if (markerIndex === -1) {
    return []
  }

  const listStart = source.indexOf('{', markerIndex)
  if (listStart === -1) {
    return []
  }

  let depth = 0
  let listEnd = -1
  for (let index = listStart; index < source.length; index += 1) {
    const char = source[index]
    if (char === '{') {
      depth += 1
    } else if (char === '}') {
      depth -= 1
      if (depth === 0) {
        listEnd = index
        break
      }
    }
  }

  if (listEnd === -1) {
    return []
  }

  const listBody = source.slice(listStart + 1, listEnd)
  const entries: AssetRegistryEntry[] = []
  let entryStart = -1
  depth = 0

  for (let index = 0; index < listBody.length; index += 1) {
    const char = listBody[index]
    if (char === '{') {
      if (depth === 0) {
        entryStart = index
      }
      depth += 1
    } else if (char === '}') {
      depth -= 1
      if (depth === 0 && entryStart !== -1) {
        const block = listBody.slice(entryStart, index + 1)
        const assetId = block.match(/asset_id\s*=\s*"([^"]+)"/)?.[1] ?? null
        const sourcePath = block.match(/source_path\s*=\s*"([^"]+)"/)?.[1] ?? null
        if (assetId && sourcePath) {
          const dependencyBlock =
            block.match(/dependencies\s*=\s*\{([\s\S]*?)\}/)?.[1] ?? ''
          entries.push({
            assetId,
            sourcePath,
            shaderRole: block.match(/shader_role\s*=\s*"([^"]+)"/)?.[1] ?? null,
            bootstrapRequired:
              block.match(/bootstrap_required\s*=\s*(true|false)/)?.[1] === 'true',
            dependencies: extractLuaQuotedStrings(dependencyBlock),
          })
        }
        entryStart = -1
      }
    }
  }

  return entries
}

async function loadAssetRegistryIndex(
  paths: ShaderPaths,
): Promise<Map<string, AssetRegistryEntry>> {
  if (!(await pathExists(paths.assetRegistryPath))) {
    return new Map()
  }

  const registrySource = await fs.readFile(paths.assetRegistryPath, 'utf8')
  const entries = parseAssetRegistryEntries(registrySource)
  return new Map(entries.map((entry) => [entry.sourcePath, entry]))
}

async function buildCatalogEntry(
  paths: ShaderPaths,
  registry: Map<string, AssetRegistryEntry>,
  relativePath: string,
): Promise<ShaderCatalogEntry> {
  const normalized = validateRelativeShaderPath(relativePath)
  const sourcePath = path.join(paths.sourceRoot, normalized)
  const cachePath = path.join(paths.cacheRoot, normalized)
  const [sourceExists, cacheExists] = await Promise.all([
    pathExists(sourcePath),
    pathExists(cachePath),
  ])
  const stat = sourceExists ? await fs.stat(sourcePath) : null
  const registryEntry = registry.get(`shaders/${normalized}`) ?? null
  return {
    shaderId: encodeShaderId(normalized),
    filename: path.basename(normalized),
    shaderClass: classifyShader(normalized),
    assetId: registryEntry?.assetId ?? null,
    shaderRole: registryEntry?.shaderRole ?? null,
    bootstrapRequired: registryEntry?.bootstrapRequired ?? null,
    dependencies: registryEntry?.dependencies ?? [],
    sourcePath: path.relative(paths.repoRoot, sourcePath).replace(/\\/g, '/'),
    cachePath: cacheExists
      ? path.relative(paths.repoRoot, cachePath).replace(/\\/g, '/')
      : null,
    sourceExists,
    cacheExists,
    byteLength: stat?.size ?? 0,
    updatedAt: stat?.mtime.toISOString() ?? new Date(0).toISOString(),
  }
}

export async function listShaderCatalog(): Promise<{
  shaders: Array<ShaderCatalogEntry>
  elapsedMs: number
}> {
  const startedAt = performance.now()
  const paths = await resolveShaderPaths()
  const registry = await loadAssetRegistryIndex(paths)
  const filenames = (await fs.readdir(paths.sourceRoot, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && entry.name.endsWith(WGSL_EXTENSION))
    .map((entry) => entry.name)
    .sort((left, right) => left.localeCompare(right))

  const shaders = await Promise.all(
    filenames.map((relativePath) => buildCatalogEntry(paths, registry, relativePath)),
  )

  return {
    shaders,
    elapsedMs: Number((performance.now() - startedAt).toFixed(2)),
  }
}

export async function loadShaderFile(shaderId: string): Promise<ShaderFileRecord> {
  const startedAt = performance.now()
  const decoded = decodeShaderId(shaderId)
  if (!decoded) {
    throw new Error('Invalid shader id')
  }
  const paths = await resolveShaderPaths()
  const registry = await loadAssetRegistryIndex(paths)
  const relativePath = validateRelativeShaderPath(decoded)
  const sourcePath = path.join(paths.sourceRoot, relativePath)
  const source = await fs.readFile(sourcePath, 'utf8')
  const entry = await buildCatalogEntry(paths, registry, relativePath)
  return {
    entry,
    source,
    elapsedMs: Number((performance.now() - startedAt).toFixed(2)),
  }
}

export async function uploadShaderFile(
  filename: string,
  source: string,
): Promise<ShaderUploadResult> {
  const startedAt = performance.now()
  const safeFilename = sanitizeUploadFilename(filename)
  if (source.trim().length === 0) {
    throw new Error('source is required')
  }
  const paths = await resolveShaderPaths()
  const registry = await loadAssetRegistryIndex(paths)
  const sourcePath = path.join(paths.sourceRoot, safeFilename)
  const cachePath = path.join(paths.cacheRoot, safeFilename)

  await fs.mkdir(path.dirname(sourcePath), { recursive: true })
  await fs.mkdir(path.dirname(cachePath), { recursive: true })
  await fs.writeFile(sourcePath, source, 'utf8')
  await fs.writeFile(cachePath, source, 'utf8')

  return {
    entry: await buildCatalogEntry(paths, registry, safeFilename),
    elapsedMs: Number((performance.now() - startedAt).toFixed(2)),
  }
}

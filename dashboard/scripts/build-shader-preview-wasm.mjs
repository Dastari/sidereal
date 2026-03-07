import { spawnSync } from 'node:child_process'
import { mkdirSync, rmSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const dashboardDir = path.resolve(scriptDir, '..')
const repoRoot = path.resolve(dashboardDir, '..')
const outDir = path.join(
  dashboardDir,
  'public',
  'wasm',
  'sidereal-shader-preview',
)

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: 'inherit',
    ...options,
  })
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

rmSync(outDir, { recursive: true, force: true })
mkdirSync(outDir, { recursive: true })

run('cargo', [
  'build',
  '-p',
  'sidereal-shader-preview',
  '--target',
  'wasm32-unknown-unknown',
])

run('wasm-bindgen', [
  '--target',
  'web',
  '--out-dir',
  outDir,
  path.join(
    repoRoot,
    'target',
    'wasm32-unknown-unknown',
    'debug',
    'sidereal_shader_preview.wasm',
  ),
])

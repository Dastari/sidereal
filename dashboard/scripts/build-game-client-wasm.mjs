import { spawnSync } from 'node:child_process'
import { mkdirSync, rmSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const dashboardDir = path.resolve(scriptDir, '..')
const repoRoot = path.resolve(dashboardDir, '..')
const outDir = path.join(dashboardDir, 'public', 'wasm', 'sidereal-client')
const profile = process.env.SIDEREAL_CLIENT_WASM_PROFILE?.trim() || 'release'

function cargoProfileArgs(profileName) {
  if (profileName === 'debug' || profileName === 'dev') {
    return []
  }
  if (profileName === 'release') {
    return ['--release']
  }
  return ['--profile', profileName]
}

function cargoProfileDir(profileName) {
  if (profileName === 'debug' || profileName === 'dev') {
    return 'debug'
  }
  return profileName
}

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
  'rustc',
  '-p',
  'sidereal-client',
  '--lib',
  '--target',
  'wasm32-unknown-unknown',
  ...cargoProfileArgs(profile),
  '--crate-type',
  'cdylib',
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
    cargoProfileDir(profile),
    'sidereal_client.wasm',
  ),
])

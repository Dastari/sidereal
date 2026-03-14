import os from 'node:os'
import path from 'node:path'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { afterEach, describe, expect, it } from 'vitest'
import {
  loadAudioStudioCatalog,
  saveAudioCueMarkers,
} from '@/lib/audio-studio.server'

const AUDIO_REGISTRY_FIXTURE = `local AudioRegistry = {}

AudioRegistry.clips = {
  {
    clip_asset_id = "audio.sfx.weapon.ballistic_fire",
    defaults = {
      intro_start_s = 0.0,
      loop_start_s = 1.0,
      loop_end_s = 2.0,
      outro_start_s = 2.0,
      clip_end_s = 4.0,
    },
  },
}

AudioRegistry.profiles = {
  {
    profile_id = "weapon.ballistic_gatling",
    kind = "weapon",
    cues = {
      fire = {
        playback = {
          kind = "segmented_loop",
          clip_asset_id = "audio.sfx.weapon.ballistic_fire",
        },
        route = {
          bus = "sfx",
        },
        spatial = {
          mode = "world_2d",
        },
      },
    },
  },
  {
    profile_id = "music.menu.standard",
    kind = "music",
    cues = {
      main = {
        playback = {
          kind = "loop",
          clip_asset_id = "audio.music.menu_loop",
        },
        route = {
          bus = "music",
        },
        spatial = {
          mode = "screen_nonpositional",
        },
      },
    },
  },
}

return AudioRegistry
`

const ASSET_REGISTRY_FIXTURE = `local AssetRegistry = {}

AssetRegistry.assets = {
  {
    asset_id = "audio.sfx.weapon.ballistic_fire",
    source_path = "audio/sfx/ballistic_fire.ogg",
    content_type = "audio/ogg",
    dependencies = {},
    bootstrap_required = true,
    startup_required = false,
  },
  {
    asset_id = "audio.music.menu_loop",
    source_path = "music/menu-loop.ogg",
    content_type = "audio/ogg",
    dependencies = {},
    bootstrap_required = true,
    startup_required = false,
  },
}

return AssetRegistry
`

const createdRoots: Array<string> = []
const originalRepoRoot = process.env.SIDEREAL_REPO_ROOT

async function createFixtureRepo(): Promise<string> {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), 'sidereal-audio-studio-'))
  createdRoots.push(repoRoot)

  await mkdir(path.join(repoRoot, 'data', 'scripts', 'audio'), { recursive: true })
  await mkdir(path.join(repoRoot, 'data', 'scripts', 'assets'), { recursive: true })
  await mkdir(path.join(repoRoot, 'data', 'audio', 'sfx'), { recursive: true })
  await mkdir(path.join(repoRoot, 'data', 'music'), { recursive: true })

  await Promise.all([
    writeFile(
      path.join(repoRoot, 'data', 'scripts', 'audio', 'registry.lua'),
      AUDIO_REGISTRY_FIXTURE,
      'utf8',
    ),
    writeFile(
      path.join(repoRoot, 'data', 'scripts', 'assets', 'registry.lua'),
      ASSET_REGISTRY_FIXTURE,
      'utf8',
    ),
    writeFile(
      path.join(repoRoot, 'data', 'audio', 'sfx', 'ballistic_fire.ogg'),
      new Uint8Array([0, 1, 2, 3]),
    ),
    writeFile(
      path.join(repoRoot, 'data', 'music', 'menu-loop.ogg'),
      new Uint8Array([4, 5, 6, 7]),
    ),
  ])

  process.env.SIDEREAL_REPO_ROOT = repoRoot
  return repoRoot
}

afterEach(async () => {
  process.env.SIDEREAL_REPO_ROOT = originalRepoRoot
  await Promise.all(
    createdRoots.splice(0).map(async (root) => rm(root, { recursive: true, force: true })),
  )
})

describe('audio-studio server helpers', () => {
  it('loads audio cues from the authored Lua registries', async () => {
    await createFixtureRepo()

    const catalog = await loadAudioStudioCatalog()

    expect(catalog.summary.cueCount).toBe(2)
    expect(catalog.summary.musicCount).toBe(1)
    expect(catalog.summary.sfxCount).toBe(1)

    const ballistic = catalog.entries.find(
      (entry) => entry.soundId === 'weapon.ballistic_gatling~fire',
    )
    expect(ballistic).toBeTruthy()
    expect(ballistic?.markersSource).toBe('clip_defaults')
    expect(ballistic?.effectiveMarkers.loop_start_s).toBe(1)

    const music = catalog.entries.find(
      (entry) => entry.soundId === 'music.menu.standard~main',
    )
    expect(music?.markersSource).toBe('unconfigured')
    expect(music?.asset?.sourcePath).toBe('music/menu-loop.ogg')
  })

  it('writes marker updates back to the correct Lua table', async () => {
    const repoRoot = await createFixtureRepo()

    const updatedDefaults = await saveAudioCueMarkers('weapon.ballistic_gatling~fire', {
      intro_start_s: 0.25,
      loop_start_s: 1.5,
      loop_end_s: 2.75,
      outro_start_s: 3.1,
      clip_end_s: 4.2,
    })
    expect(updatedDefaults.markersSource).toBe('clip_defaults')
    expect(updatedDefaults.clipDefaultMarkers.loop_start_s).toBe(1.5)

    const updatedCue = await saveAudioCueMarkers('music.menu.standard~main', {
      intro_start_s: 0.5,
      loop_start_s: 1.25,
      loop_end_s: 8,
      outro_start_s: 8,
      clip_end_s: 10,
    })
    expect(updatedCue.markersSource).toBe('profile')
    expect(updatedCue.profileMarkers.loop_end_s).toBe(8)

    const registrySource = await readFile(
      path.join(repoRoot, 'data', 'scripts', 'audio', 'registry.lua'),
      'utf8',
    )
    expect(registrySource).toContain('loop_start_s = 1.5,')
    expect(registrySource).toContain('clip_end_s = 4.2,')
    expect(registrySource).toContain('loop_end_s = 8.0,')
    expect(registrySource).toContain('clip_end_s = 10.0,')
  })
})

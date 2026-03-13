# Audio Runtime Implementation Plan

Status: Proposed implementation plan
Date: 2026-03-13
Owners: client runtime + gameplay + asset delivery + scripting
Primary references:
- `docs/features/audio_runtime_contract.md`
- `docs/features/asset_delivery_contract.md`
- `docs/features/visibility_replication_contract.md`
- `docs/plans/robust_weapons_combat_audio_system_plan.md`
- `docs/decisions/dr-0007_entity_variant_framework.md`
- `docs/decisions/dr-0019_lua_asset_registry_and_gateway_http_delivery.md`

2026-03-13 status note:
1. This plan assumes the feature direction in `docs/features/audio_runtime_contract.md`.
2. This plan makes one explicit long-term recommendation: use a Sidereal-owned audio runtime over direct Kira integration, not Bevy's built-in audio and not a thin `bevy_kira_audio` wrapper.
3. The goal is a no-compromise audio stack that can scale from menu UI sounds to combat loops, dialog, environmental processing, and future higher-fidelity spatial propagation.

## 1. Executive Decision

### 1.1 Best long-term backend choice

For Sidereal's long-term target, the best solution is:

1. keep Bevy for ECS, transforms, scheduling, and app lifecycle,
2. keep Lua as the authoritative source for audio content/profile/mixer definitions,
3. build a Sidereal-owned audio runtime layer,
4. integrate Kira directly as the initial mixer/effects/playback backend,
5. preserve a backend abstraction so native and browser backends can diverge internally without changing authored behavior.

This is the best long-term choice because:

1. Bevy 0.18 audio is not a full mixer/effects system.
2. Kira exposes the mixer/effect/track/spatial primitives Sidereal actually needs.
3. direct Kira integration gives explicit control over:
   - sub-tracks,
   - send tracks,
   - reverb and filters,
   - loop regions,
   - runtime parameter control,
   - byte-backed sound construction from the existing asset cache.
4. a Sidereal-owned layer keeps audio definitions data-driven and avoids hard-coding gameplay logic into whichever backend crate is used.

### 1.2 Why not Bevy audio

Bevy audio remains too limited for the target system:

1. one global volume resource instead of real category buses,
2. no built-in mixer graph or send tracks,
3. no built-in master effect chain,
4. only simple spatial panning/attenuation,
5. no first-class segmented loop controller for weapon intro/loop/outro behavior.

### 1.3 Why not `bevy_kira_audio` as the primary architecture

`bevy_kira_audio` is useful reference material and can be a fallback spike tool, but it should not be the final architecture.

Reason:

1. Sidereal already has a custom asset-delivery and byte-backed runtime model.
2. Sidereal needs authoritative bootstrap/hot-reload/catalog control that is more explicit than an out-of-the-box Bevy asset-driven plugin path.
3. Sidereal wants a custom mixer graph, custom environment processing, and custom event-to-cue flow that should remain owned by project code.
4. Direct Kira integration avoids fighting an extra abstraction layer later.

This is an inference from the documented surface area of `bevy_kira_audio` versus Kira itself. Kira's direct docs expose the mixer and effect graph more clearly than the Bevy wrapper surface does.

## 2. Runtime Audio Format Decision

### 2.1 Published runtime format

Use `OGG Vorbis` as the default published runtime format.

Recommendation:

1. convert `ballistic_fire.mp3` -> `ballistic_fire.ogg`
2. convert `explosion1.mp3` -> `explosion1.ogg`

### 2.2 Why OGG over WAV

OGG is the better runtime format for Sidereal because:

1. much smaller payloads than WAV,
2. better for authenticated HTTP delivery and local cache footprint,
3. better for large music/dialog libraries,
4. good general support in Rust audio decoders/backends,
5. perfectly adequate quality for game SFX, dialog, and music.

WAV should be treated as an authoring/mastering format, not the default shipped format.

Use WAV only when:

1. the content pipeline needs an uncompressed master for editing,
2. profiling proves a specific tiny hot-path sound absolutely needs it,
3. a backend-specific decoder issue forces a temporary exception.

### 2.3 Project policy recommendation

1. Store mastering/source assets outside the published runtime set in uncompressed form if desired.
2. Publish OGG to the authoritative asset registry and runtime cache by default.
3. Add `audio/ogg` as the canonical first implementation format.
4. Do not make MP3 the long-term project standard.

## 3. High-Level Architecture

The system should be split into six layers.

### 3.1 Layer A: content authoring

Lua authoring files:

1. `data/scripts/assets/registry.lua`
   - payload files and delivery metadata
2. `data/scripts/audio/registry.lua`
   - buses, sends, effect presets, environments, and playback profiles
3. existing bundle/entity Lua registries
   - which entities/components reference which audio profile IDs

### 3.2 Layer B: shared schema and validation

Add a shared crate, recommended name:

1. `crates/sidereal-audio`

Responsibilities:

1. strongly typed audio catalog schema,
2. profile schema validation,
3. mixer graph schema validation,
4. effect preset schema validation,
5. profile lookup/query helpers,
6. serializable compiled catalog types for bootstrap delivery.

This crate should not depend on Bevy runtime audio playback.

### 3.3 Layer C: server-side catalog build

Gateway/server-side runtime:

1. load Lua audio registry,
2. validate against `sidereal-audio` schema,
3. compile to an `AudioCatalog`,
4. assign `catalog_version`,
5. deliver the catalog to clients during bootstrap,
6. send invalidation when it changes.

### 3.4 Layer D: client-side catalog and asset resolution

Client runtime:

1. bootstrap downloads the audio catalog alongside the asset manifest,
2. resolves logical profile IDs to logical asset IDs,
3. resolves logical asset IDs to cached asset bytes through the existing asset manager,
4. converts cached audio bytes into Kira sound data,
5. keeps fail-soft fallback behavior when assets or profiles are missing.

### 3.5 Layer E: client runtime audio engine

Sidereal-owned runtime responsibilities:

1. instantiate mixer graph,
2. maintain bus state and effect handles,
3. track active emitters and active audio instances,
4. bind listener position,
5. apply settings changes,
6. process authoritative presentation events into cue playback.

### 3.6 Layer F: gameplay/presentation integration

Gameplay and presentation systems do not play files directly.

Instead they:

1. expose logical profile IDs on components,
2. emit authoritative gameplay/presentation events,
3. let the audio runtime resolve those IDs into playback actions.

## 4. Sidereal Audio Runtime Modules

Recommended module layout:

1. `crates/sidereal-audio`
   - schema/types/validation
2. `crates/sidereal-audio-catalog` optional if catalog build grows large
   - only if the build/decode layer becomes large enough to justify separation
3. `bins/sidereal-client/src/runtime/audio/`
   - `mod.rs`
   - `catalog.rs`
   - `settings.rs`
   - `backend.rs`
   - `backend_kira.rs`
   - `listener.rs`
   - `emitters.rs`
   - `events.rs`
   - `music.rs`
   - `ui.rs`
   - `debug.rs`

Do not continue growing the current single-file `runtime/audio.rs` once real implementation starts.

## 5. Mixer Graph Design

### 5.1 Fixed logical graph

The top-level graph should be stable in Rust, while Lua configures parameters and preset chains.

Recommended graph:

```text
master
  |- music
  |- sfx
  |- dialog
  |- ui
  |- ambient

send tracks:
  |- world_reverb
  |- radio_fx
  |- pause_muffle
```

Rules:

1. every playable cue routes to exactly one primary bus,
2. every bus routes to `master`,
3. send tracks are optional per cue/profile,
4. environment state can modify send levels and selected effect parameters.

### 5.2 Why fixed graph plus data-driven parameters

Do not expose a fully arbitrary Lua-authored graph in the first implementation.

Use a fixed graph with Lua-authored definitions because:

1. easier to validate,
2. easier to support on both native and WASM,
3. easier to debug and hot reload,
4. still flexible enough for a game-scale mix architecture.

Lua should define:

1. bus defaults,
2. effect chains on supported buses,
3. send defaults,
4. ducking rules,
5. environment presets,
6. per-profile routing overrides.

### 5.3 Ducking model

Avoid depending on backend sidechain features.

Implement ducking in Sidereal runtime logic:

1. when dialog is active, tween `music` and optionally `sfx` bus gain down,
2. when pause/map/scanner mode is active, tween the desired buses and master filters,
3. when the state ends, tween back to authored defaults.

This keeps behavior deterministic and backend-portable.

## 6. Effect Model

### 6.1 Supported effect chain types

Initial supported effect DSL should cover:

1. `volume_control`
2. `filter`
3. `eq_filter`
4. `reverb`
5. `delay`
6. `compressor`
7. `distortion`
8. `panning_control`

These map cleanly to Kira's documented built-in effect modules.

### 6.2 Effect authoring rule

Lua defines declarative effect instances and parameter defaults.

Rust implements:

1. validation,
2. translation into backend effect builders,
3. runtime parameter tweening,
4. fallback behavior when a backend cannot support a requested effect.

Lua must not contain arbitrary DSP code.

### 6.3 Environment processing

Environment processing should be modeled as:

1. send-level presets,
2. per-bus filter presets,
3. optional master filter overrides,
4. optional dialog/music treatment overrides.

Examples:

1. `open_space`
   - almost dry,
   - wide stereo,
   - minimal reverb send
2. `station_interior`
   - stronger `world_reverb`,
   - reduced high frequencies on distant emitters
3. `scanner_mode`
   - low-pass on `master`,
   - dialog untouched or lightly filtered,
   - music attenuated
4. `menu_pause`
   - low-pass or gain reduction on world buses,
   - UI bus unaffected

## 7. Lua Catalog Design

### 7.1 Audio catalog top-level schema

Recommended top-level layout:

```lua
return {
  schema_version = 1,
  buses = {
    { bus_id = "music", parent = "master", default_volume_db = -4.0 },
    { bus_id = "sfx", parent = "master", default_volume_db = 0.0 },
    { bus_id = "dialog", parent = "master", default_volume_db = 0.0 },
    { bus_id = "ui", parent = "master", default_volume_db = -3.0 },
    { bus_id = "ambient", parent = "master", default_volume_db = -6.0 },
  },
  sends = {
    {
      send_id = "world_reverb",
      effects = {
        { kind = "reverb", mix = 0.22, damping = 0.48, room_size = 0.65 },
      },
    },
    {
      send_id = "radio_fx",
      effects = {
        { kind = "filter", mode = "band_pass", cutoff_hz = 2200.0, q = 0.85 },
        { kind = "distortion", drive = 0.05 },
      },
    },
  },
  environments = {
    {
      environment_id = "open_space",
      bus_overrides = {},
      send_level_db = { world_reverb = -20.0 },
    },
    {
      environment_id = "station_interior",
      send_level_db = { world_reverb = -8.0 },
      bus_effect_overrides = {
        sfx = {
          { kind = "filter", mode = "low_pass", cutoff_hz = 14000.0, q = 0.71 },
        },
      },
    },
  },
  concurrency_groups = {
    { group_id = "weapon_loop_per_emitter", max_instances = 1, scope = "emitter_slot" },
    { group_id = "asteroid_explosion", max_instances = 24, scope = "profile_global" },
  },
  profiles = {
    -- examples follow below
  },
}
```

### 7.2 Cue-oriented profile model

Use cue-oriented profiles, not "one asset per sound" logic.

Each profile can expose multiple cues:

1. `start`
2. `loop`
3. `stop`
4. `impact`
5. `idle`
6. `boost`
7. `collision_light`
8. `collision_heavy`
9. `line`
10. `hover`
11. `click`

This keeps the component side small and the Lua side expressive.

### 7.3 Example profiles

#### Weapon: ballistic gatling

```lua
{
  profile_id = "weapon.ballistic_gatling",
  kind = "weapon",
  cues = {
    fire = {
      playback = {
        kind = "segmented_loop",
        clip_asset_id = "audio.sfx.weapon.ballistic_fire",
        intro_start_s = 0.0,
        loop_start_s = 1.0,
        loop_end_s = 2.0,
        outro_start_s = 2.0,
        clip_end_s = 4.0,
      },
      route = {
        bus = "sfx",
        sends = {
          { send_id = "world_reverb", level_db = -12.0 },
        },
      },
      spatial = {
        mode = "world_2d",
        min_distance_m = 5.0,
        max_distance_m = 220.0,
        rolloff = "logarithmic",
        pan_strength = 1.0,
        distance_lowpass = { enabled = true, near_hz = 18000.0, far_hz = 6000.0 },
      },
      concurrency = {
        group_id = "weapon_loop_per_emitter",
        steal = "restart",
      },
    },
    dry_fire = {
      playback = {
        kind = "one_shot",
        variants = {
          { clip_asset_id = "audio.sfx.weapon.dry_fire_01", weight = 1.0 },
        },
      },
      route = { bus = "sfx" },
      spatial = { mode = "world_2d", min_distance_m = 3.0, max_distance_m = 80.0 },
    },
  },
}
```

#### Weapon: beam laser

```lua
{
  profile_id = "weapon.beam_laser_mk1",
  kind = "weapon",
  cues = {
    start = {
      playback = {
        kind = "one_shot",
        clip_asset_id = "audio.sfx.weapon.beam_start_01",
      },
      route = { bus = "sfx" },
      spatial = { mode = "world_2d", min_distance_m = 6.0, max_distance_m = 180.0 },
    },
    sustain = {
      playback = {
        kind = "loop",
        clip_asset_id = "audio.sfx.weapon.beam_loop_01",
      },
      route = {
        bus = "sfx",
        sends = {
          { send_id = "world_reverb", level_db = -15.0 },
        },
      },
      spatial = { mode = "world_2d", min_distance_m = 6.0, max_distance_m = 180.0 },
    },
    stop = {
      playback = {
        kind = "one_shot",
        clip_asset_id = "audio.sfx.weapon.beam_stop_01",
      },
      route = { bus = "sfx" },
      spatial = { mode = "world_2d", min_distance_m = 6.0, max_distance_m = 180.0 },
    },
    impact = {
      playback = {
        kind = "one_shot",
        clip_asset_id = "audio.sfx.weapon.beam_impact_01",
      },
      route = { bus = "sfx" },
      spatial = { mode = "world_2d", min_distance_m = 4.0, max_distance_m = 120.0 },
    },
  },
}
```

#### Entity loop: thrusters

```lua
{
  profile_id = "entity.ship.thruster_standard",
  kind = "emitter_set",
  cues = {
    idle = {
      playback = { kind = "loop", clip_asset_id = "audio.sfx.ship.thruster_idle_01" },
      route = { bus = "ambient" },
      spatial = { mode = "world_2d", min_distance_m = 10.0, max_distance_m = 260.0 },
    },
    boost = {
      playback = { kind = "loop", clip_asset_id = "audio.sfx.ship.thruster_boost_01" },
      route = { bus = "ambient" },
      spatial = { mode = "world_2d", min_distance_m = 12.0, max_distance_m = 320.0 },
    },
  },
}
```

#### Entity event: collision

```lua
{
  profile_id = "entity.hull.collision_metal",
  kind = "collision",
  cues = {
    light = {
      playback = {
        kind = "one_shot",
        variants = {
          { clip_asset_id = "audio.sfx.collision.metal_light_01", weight = 1.0 },
          { clip_asset_id = "audio.sfx.collision.metal_light_02", weight = 1.0 },
        },
      },
      route = { bus = "sfx" },
      spatial = { mode = "world_2d", min_distance_m = 3.0, max_distance_m = 100.0 },
    },
    heavy = {
      playback = {
        kind = "one_shot",
        variants = {
          { clip_asset_id = "audio.sfx.collision.metal_heavy_01", weight = 1.0 },
        },
      },
      route = { bus = "sfx" },
      spatial = { mode = "world_2d", min_distance_m = 5.0, max_distance_m = 170.0 },
    },
  },
}
```

#### Dialog

```lua
{
  profile_id = "dialog.faction_trader.standard_radio",
  kind = "dialog",
  cues = {
    line = {
      playback = { kind = "one_shot", clip_asset_id = "audio.dialog.trader.line_001" },
      route = {
        bus = "dialog",
        sends = {
          { send_id = "radio_fx", level_db = 0.0 },
        },
      },
      spatial = { mode = "screen_nonpositional" },
      ducking = { music_db = -8.0, sfx_db = -4.0, tween_ms = 120 },
    },
  },
}
```

#### Music

```lua
{
  profile_id = "music.exploration.outer_belt",
  kind = "music",
  cues = {
    main = {
      playback = {
        kind = "stream_or_static_loop",
        clip_asset_id = "audio.music.outer_belt_theme",
        loop_region = { start_s = 12.0, end_s = 98.5 },
      },
      route = { bus = "music" },
      spatial = { mode = "screen_nonpositional" },
    },
  },
}
```

#### UI

```lua
{
  profile_id = "ui.menu.standard",
  kind = "ui",
  cues = {
    hover = {
      playback = { kind = "one_shot", clip_asset_id = "audio.ui.hover_01" },
      route = { bus = "ui" },
      spatial = { mode = "screen_nonpositional" },
    },
    click = {
      playback = { kind = "one_shot", clip_asset_id = "audio.ui.click_01" },
      route = { bus = "ui" },
      spatial = { mode = "screen_nonpositional" },
    },
  },
}
```

## 8. Component and Entity Design

### 8.1 Persisted/replicated gameplay-facing IDs

Audio should live on gameplay entities as logical profile IDs, not as raw asset IDs and not as backend playback handles.

Recommended shared components:

1. `AudioProfileBindingSet`
   - generic profile-slot bindings for any entity with continuous or cue-based audio
2. existing `Destructible.destruction_profile_id`
   - continue to use for destruction presentation
3. future `WeaponPresentationProfileId` or transitional `weapon_audio_profile_id`
4. future `CollisionAudioMaterialId`
   - for material-style collision resolution if needed
5. future `DialogVoiceProfileId`
   - when NPC or speaker entities need reusable voice presentation definitions

### 8.2 Generic binding component

Preferred flexible component:

```rust
#[sidereal_component_macros::sidereal_component(
    kind = "audio_profile_binding_set",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AudioProfileBindingSet {
    pub bindings: Vec<AudioProfileBinding>,
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq)]
pub struct AudioProfileBinding {
    pub slot: String,
    pub profile_id: String,
}
```

Example bindings:

1. weapon entity:
   - `slot = "fire_primary", profile_id = "weapon.ballistic_gatling"`
2. ship entity:
   - `slot = "thruster", profile_id = "entity.ship.thruster_standard"`
3. station entity:
   - `slot = "ambient_loop", profile_id = "entity.station.ambient_hum_large"`

### 8.3 Why a generic binding set is the right base

This gives flexibility without exploding the number of special-purpose components.

It supports:

1. weapons,
2. thrusters,
3. ambient emitters,
4. collision profiles,
5. scripted one-off emitters,
6. future VFX/SFX variant swaps.

Domain-specific components remain valid when they already exist and already own the right concept:

1. `Destructible.destruction_profile_id` stays,
2. weapon presentation can graduate to a dedicated component once the weapon refactor is further along.

### 8.4 Client-only runtime components

Do not persist or replicate backend state.

Recommended client-only components:

```rust
#[derive(Component)]
pub struct ActiveAudioEmitter {
    pub owner_entity: Entity,
    pub slot: String,
    pub profile_id: String,
}

#[derive(Component)]
pub struct ActiveLoopCue {
    pub cue_id: String,
    pub instance_id: AudioInstanceId,
}

#[derive(Component)]
pub struct AudioListenerAnchor;
```

## 9. Resource Design

### 9.1 Shared settings resource

```rust
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    pub master_volume_db: f32,
    pub buses: HashMap<String, AudioBusSettings>,
    pub pause_behavior: AudioPauseBehavior,
    pub captions_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioBusSettings {
    pub volume_db: f32,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AudioPauseBehavior {
    KeepPlaying,
    DuckWorldOnly,
    PauseWorldKeepUi,
    PauseAll,
}
```

### 9.2 Catalog resource

```rust
#[derive(Resource, Debug, Clone)]
pub struct AudioCatalogState {
    pub catalog_version: String,
    pub catalog: Arc<AudioCatalog>,
}
```

### 9.3 Runtime resource

```rust
#[derive(Resource)]
pub struct AudioRuntimeState {
    pub backend: Box<dyn SiderealAudioBackend>,
    pub buses: HashMap<String, AudioBusRuntime>,
    pub sends: HashMap<String, AudioSendRuntime>,
    pub active_instances: SlotMap<AudioInstanceId, ActiveAudioInstance>,
    pub active_loops_by_entity_slot: HashMap<(Entity, String), AudioInstanceId>,
    pub current_environment_id: String,
}
```

### 9.4 Backend trait

```rust
pub trait SiderealAudioBackend: Send + Sync {
    fn apply_bus_gain(&mut self, bus_id: &str, db: f32, tween: AudioTween);
    fn apply_send_level(&mut self, send_id: &str, db: f32, tween: AudioTween);
    fn apply_effect_param(
        &mut self,
        target: AudioEffectTarget,
        param: &str,
        value: AudioParamValue,
        tween: AudioTween,
    );
    fn play_one_shot(&mut self, request: OneShotRequest) -> Result<AudioInstanceId, AudioError>;
    fn play_loop(&mut self, request: LoopRequest) -> Result<AudioInstanceId, AudioError>;
    fn update_spatial(&mut self, instance_id: AudioInstanceId, spatial: SpatialRuntimeParams);
    fn stop_instance(&mut self, instance_id: AudioInstanceId, tween: AudioTween);
    fn pause_instance(&mut self, instance_id: AudioInstanceId, tween: AudioTween);
    fn resume_instance(&mut self, instance_id: AudioInstanceId, tween: AudioTween);
}
```

This is the seam that lets native and browser backends differ later without changing Lua content or gameplay-side IDs.

## 10. Asset and Catalog Delivery

### 10.1 Raw audio assets

Audio assets still use the normal authoritative asset registry and payload route.

Required asset contract changes:

1. ensure `audio/ogg` is a first-class content type in runtime helpers,
2. if MP3 remains temporarily supported, add `audio/mpeg` explicitly,
3. add byte-backed audio decode path in the client runtime,
4. extend required-asset and lazy-fetch logic so audio can be resolved like visual assets.

### 10.2 Audio catalog delivery

The audio catalog is not an asset payload and should not be treated like an opaque sound file.

Recommended delivery path:

1. gateway/bootstrap JSON includes:
   - `audio_catalog_version`,
   - compiled `audio_catalog` payload,
   - or a small URL to fetch the compiled catalog over authenticated HTTP if it becomes large
2. replication sends audio catalog invalidation/version changes similarly to asset catalog invalidation
3. client hot reloads the catalog and applies safe runtime graph/profile refreshes

### 10.3 Why not store all profile logic in ordinary entity components

Because that would:

1. duplicate authored data across many entities,
2. make content changes require mass entity migrations,
3. bloat replication/persistence,
4. reduce reusability for UI/dialog/music where no world entity owns the content.

## 11. Event and Networking Model

### 11.1 Principle

Server drives truth; client resolves presentation.

### 11.2 Event shape

There are two acceptable ways to reach the final model.

#### Option A: enrich existing domain messages

Examples:

1. `ServerWeaponFiredMessage` gains `weapon_entity_id` or `presentation_profile_id`
2. `ServerBeamStateMessage` carries `beam_profile_id`
3. `ServerEntityDestructionMessage` keeps `destruction_profile_id`
4. collision/dialog/music events gain similar logical profile hooks

#### Option B: add a generic presentation cue lane

Example:

```rust
pub struct ServerPresentationCueMessage {
    pub emitter_entity_id: Option<String>,
    pub origin_xy: Option<[f32; 2]>,
    pub profile_id: String,
    pub cue_id: String,
    pub lifecycle: PresentationCueLifecycle,
    pub seed: u64,
}
```

Recommendation:

1. use Option A first for combat and destruction,
2. only add Option B when multiple systems need a truly generic server-authored cue lane.

### 11.3 Looped cues over the network

For segmented or sustained sounds:

1. do not infer start/stop only from gaps between repeated fire events,
2. send explicit lifecycle where required:
   - `Start`
   - `Sustain`
   - `Stop`
3. or reuse authoritative beam/presentation state messages with clear transitions.

This is mandatory for:

1. gatling/minigun loops,
2. beam laser sustain loops,
3. long thruster boost states,
4. engine overload loops,
5. radio transmission channels that open/close.

## 12. How Each Audio Family Should Attach

### 12.1 Ballistic weapons

Attachment:

1. weapon entity profile binding
2. authoritative fire lifecycle message
3. client loop controller with segmented or multi-clip fallback

### 12.2 Beam weapons

Attachment:

1. weapon entity profile binding
2. beam start/update/stop events
3. separate cue IDs for `start`, `sustain`, `stop`, `impact`

### 12.3 Thrusters and engine hum

Attachment:

1. world entity `AudioProfileBindingSet` slot on the ship/root entity
2. client runtime reads movement/thrust state locally from replicated components
3. no need to spam network play messages when the state is already replicated

### 12.4 Collision effects

Attachment:

1. collision audio profile on material/entity/profile,
2. client resolves impact strength and selects `light`/`heavy` cue,
3. keep authoritative collision state server-side for gameplay, but client may still derive purely presentational impact intensity from replicated motion plus collision messages if appropriate.

### 12.5 Dialog

Attachment:

1. dialog event or dialog line message carries `profile_id` and `cue_id = "line"`,
2. world speech may optionally be positional,
3. radio/mission dialog usually routes to `dialog` bus with `radio_fx` send,
4. captions/subtitles should hook to the same event model.

### 12.6 Background music

Attachment:

1. client-local state machine driven by authored music states,
2. world/mission/zone state chooses a logical music profile ID,
3. no world entity required unless diegetic music sources become a real feature later.

### 12.7 Menu and UI

Attachment:

1. client-local UI events reference Lua-authored profile IDs,
2. no replication required,
3. route to `ui` bus,
4. never let world ducking or environment processing unintentionally mangle the `ui` bus.

## 13. Kira Integration Notes

### 13.1 Sound loading

Use byte-backed decode from the existing asset cache, not file-path assumptions.

Target flow:

1. fetch cached bytes for `asset_id`,
2. decode into `StaticSoundData` from a `Cursor<Vec<u8>>`,
3. for native desktop long music/dialog, optionally use `StreamingSoundData` when backed by a real file or stream source,
4. for WASM, keep static byte-backed decode as the portable baseline,
5. add a browser-specific streaming path later if static music proves too heavy.

### 13.2 Loop regions

Kira already exposes loop regions and runtime loop-region control. This should be the basis for:

1. ballistic segmented loops,
2. beam sustain loops,
3. music loop points,
4. ambient loops with seamless loop segments.

### 13.3 Spatial tracks

Use Kira spatial tracks for world-positioned emitters.

Keep in mind:

1. top-down 2D still needs its own world-units-to-audio scale policy,
2. listener follows camera anchor,
3. future higher-fidelity propagation can be layered later if desired.

## 14. Suggested Rust API Surface

### 14.1 Shared schema types

Recommended types in `crates/sidereal-audio`:

```rust
pub struct AudioCatalog {
    pub buses: Vec<AudioBusDefinition>,
    pub sends: Vec<AudioSendDefinition>,
    pub environments: Vec<AudioEnvironmentDefinition>,
    pub concurrency_groups: Vec<AudioConcurrencyGroup>,
    pub profiles: Vec<AudioProfileDefinition>,
}

pub struct AudioProfileDefinition {
    pub profile_id: String,
    pub kind: AudioProfileKind,
    pub cues: Vec<AudioCueDefinition>,
}

pub struct AudioCueDefinition {
    pub cue_id: String,
    pub playback: AudioPlaybackDefinition,
    pub route: AudioRouteDefinition,
    pub spatial: AudioSpatialDefinition,
    pub concurrency: Option<AudioConcurrencyDefinition>,
}
```

### 14.2 Client playback request types

```rust
pub struct OneShotRequest {
    pub profile_id: String,
    pub cue_id: String,
    pub bus_id: String,
    pub emitter_entity: Option<Entity>,
    pub world_position: Option<Vec2>,
    pub seed: u64,
}

pub struct LoopRequest {
    pub profile_id: String,
    pub cue_id: String,
    pub bus_id: String,
    pub emitter_entity: Option<Entity>,
    pub world_position: Option<Vec2>,
    pub mode: LoopPlaybackMode,
}

pub enum LoopPlaybackMode {
    SeamlessLoop,
    SegmentedIntroLoopOutro,
}
```

## 15. Implementation Phases

### Phase 0: decision and scaffolding

1. Approve direct Kira integration direction.
2. Add `crates/sidereal-audio`.
3. Define schema and validation tests.
4. Decide canonical published runtime format as OGG.
5. Extend asset-runtime helpers for `audio/ogg`.

### Phase 1: catalog and bootstrap

1. Add `data/scripts/audio/registry.lua`.
2. Build server-side audio catalog loader and validator.
3. Deliver audio catalog during bootstrap.
4. Add client-side audio catalog resource and hot-reload invalidation.

### Phase 2: mixer runtime

1. Add `AudioSettings`.
2. Add Kira backend integration.
3. Build fixed mixer graph:
   - master
   - music
   - sfx
   - dialog
   - ui
   - ambient
   - send tracks
4. Apply Lua-authored bus/effect defaults.

### Phase 3: asset decode and backend playback

1. Implement byte-backed cached audio decode.
2. Add one-shot playback.
3. Add loop playback.
4. Add segmented intro/loop/outro controller.
5. Add listener binding and world spatial updates.

### Phase 4: first content slice

1. Register converted OGG assets for:
   - ballistic fire
   - asteroid explosion
2. Add Lua audio profiles for both.
3. Hook gatling fire lifecycle to audio.
4. Hook asteroid destruction profile to explosion SFX.

### Phase 5: broader gameplay families

1. Beam weapon cues.
2. Thruster/engine loops.
3. Collision cues.
4. UI cue library.
5. Music state machine.
6. Dialog playback and ducking.

### Phase 6: environment and polish

1. Environment presets.
2. Distance low-pass curves.
3. Pause/map/scanner filters.
4. Debug overlay and metrics.
5. Browser-specific music streaming work if needed.

## 16. Test Plan

### 16.1 Unit tests

1. Lua audio registry decode/validation.
2. cue/profile lookup.
3. concurrency policy decisions.
4. segmented loop state machine transitions.
5. bus/effect preset validation.

### 16.2 Integration tests

1. bootstrap audio catalog delivery.
2. runtime missing-asset fallback.
3. hot reload invalidation.
4. combat audio visibility filtering.
5. destruction audio cue routing.

### 16.3 Client validation

1. native compile and playback smoke tests,
2. WASM compile and byte-backed decode tests,
3. multi-client visibility validation,
4. segmented gatling loop correctness,
5. dialog ducking behavior,
6. environment preset transition behavior.

## 17. Acceptance Criteria

The audio system should not be considered complete until:

1. no gameplay audio uses hardcoded file names or asset paths,
2. all runtime audio content resolves from Lua-authored profile IDs,
3. category bus controls work live at runtime,
4. master-stage filtering affects all buses together,
5. world spatial emitters update with entity/camera transforms,
6. gatling-style intro/loop/outro cues behave correctly under authoritative network state,
7. destruction profiles resolve both VFX and SFX coherently,
8. dialog, UI, music, and world SFX all coexist without architecture forks,
9. native and WASM preserve the same authored semantics,
10. missing profiles/assets fail soft with telemetry and visible diagnostics.

## 18. Immediate Action Items

If work begins from this plan, the first concrete tasks should be:

1. convert `data/audio/sfx/ballistic_fire.mp3` and `data/audio/sfx/explosion1.mp3` to OGG,
2. add `audio/ogg` support plumbing wherever missing,
3. create `data/scripts/audio/registry.lua`,
4. add `crates/sidereal-audio`,
5. implement bootstrap audio catalog delivery,
6. replace the current menu-loop stopgap with the new runtime once the mixer exists.

## 19. Source References

Primary upstream references used for the backend choice:

1. Bevy 0.18 audio docs:
   - `https://docs.rs/bevy_audio/0.18.0/bevy_audio/`
2. Bevy spatial audio example:
   - `https://bevy.org/examples/audio/spatial-audio-2d/`
3. Kira overview and mixer docs:
   - `https://docs.rs/kira/latest/kira/`
   - `https://docs.rs/kira/latest/kira/track/`
4. Kira effect modules:
   - `https://docs.rs/kira/latest/src/kira/effect.rs.html`
5. Kira sound model and platform note:
   - `https://docs.rs/kira/latest/kira/sound/`
   - `https://docs.rs/kira/latest/src/kira/sound.rs.html`
6. `bevy_kira_audio` docs for wrapper capability comparison:
   - `https://docs.rs/crate/bevy_kira_audio/latest`

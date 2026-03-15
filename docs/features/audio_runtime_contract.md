# Audio Runtime Contract

Status: Proposed feature contract
Date: 2026-03-13
Owners: client runtime + gameplay + asset delivery
Primary architecture references:
- `docs/sidereal_design_document.md`
- `docs/features/asset_delivery_contract.md`
- `docs/features/visibility_replication_contract.md`
- `docs/plans/robust_weapons_combat_audio_system_plan.md`
- `docs/plans/audio_runtime_implementation_plan_2026-03-13.md`
- `docs/decisions/dr-0007_entity_variant_framework.md`
- `docs/decisions/dr-0019_lua_asset_registry_and_gateway_http_delivery.md`

2026-03-13 status note:
1. This document defines the target audio authoring and runtime contract to drive later implementation planning.
2. It is intentionally more detailed than the high-level audio section in `docs/plans/robust_weapons_combat_audio_system_plan.md`.
3. The corresponding implementation plan now lives in `docs/plans/audio_runtime_implementation_plan_2026-03-13.md`.
4. Until implementation lands, this document should be treated as the preferred design direction for new audio work.

2026-03-13 implementation update:
1. Gateway bootstrap now delivers the authored `audio/registry.lua` catalog and an `audio_catalog_version` alongside the asset bootstrap manifest.
2. Client runtime now owns a split `runtime/audio/` module tree with catalog/settings/backend/system ownership instead of the old embedded menu-music file.
3. Native client playback now runs through a Kira-backed Sidereal audio runtime with authored bus graph setup, authored menu/world music playback, gatling segmented-loop playback, asteroid destruction one-shots, and listener sync.
4. `BallisticWeapon.fire_audio_profile_id` is now the canonical gameplay hook for weapon-fire audio authoring, and `ServerWeaponFiredMessage` now carries `weapon_guid` so remote clients can resolve the correct authored profile.

2026-03-14 implementation update:
1. `audio/registry.lua` now supports an optional top-level `clips` section for clip-level playback defaults authored in Lua.
2. Clip defaults are merged into cue playback at registry load time before validation and catalog delivery.
3. Cue-local playback fields remain authoritative when present; clip defaults only fill missing playback markers.

2026-03-15 implementation update:
1. Startup-required audio assets now flow through the dedicated startup manifest/public startup asset lane before `Auth`.
2. Client applies a startup-safe audio catalog subset during `StartupLoading`, allowing authored menu/login audio profiles to resolve before authenticated world bootstrap.

## 1. Purpose

Define how Sidereal should author, deliver, replicate, mix, spatialize, and play game audio under the existing server-authoritative and Lua-authored asset model.

This document is the answer to:

1. where sound files should live,
2. where logical sound/profile metadata should live,
3. which gameplay/runtime objects should reference audio,
4. how audio should be triggered over the network,
5. what Bevy can and cannot do for us directly,
6. what a durable Sidereal audio runtime must support beyond the initial weapon/explosion slice.

Authoring precedence:

1. Raw payload delivery metadata remains in `data/scripts/assets/registry.lua`.
2. Audio clip defaults and audio cue/profile semantics live in `data/scripts/audio/registry.lua`.
3. Cue-local playback fields override clip defaults when both are present.

## 2. Current Baseline

Current repository state on 2026-03-13:

1. Client runtime only has a small embedded menu-music path in `bins/sidereal-client/src/runtime/audio.rs`.
2. There is no general-purpose runtime audio system for streamed audio assets, bus control, spatial emitters, or gameplay SFX.
3. Combat replication currently sends:
   - `ServerWeaponFiredMessage` for tracer visuals,
   - `ServerEntityDestructionMessage` for destruction effects.
4. Runtime asset delivery already supports generic `audio/*` cache directories, but current helper extension mapping only lists `audio/ogg` and `audio/wav`; `audio/mpeg` is not yet explicitly covered.
5. Gameplay currently stores `destruction_profile_id` on `Destructible`, which is the natural hook for destruction presentation expansion.

Implication:

1. Sidereal does not yet have an audio authoring contract.
2. Audio integration should not be done as ad hoc `AudioPlayer` spawns wired directly to hardcoded file paths.
3. The first real implementation should build the reusable contract, not only attach one-off sounds.

## 3. Non-Negotiable Constraints

The following rules align with existing architecture and are intended to be enforceable once implementation starts:

1. Audio playback is client presentation only. The server never streams audio bytes and never delegates gameplay authority to client audio state.
2. Asset payload bytes continue to flow only through authenticated gateway HTTP `/assets/<asset_guid>` delivery under `docs/features/asset_delivery_contract.md`.
3. Rust runtime code must not hardcode gameplay audio asset IDs, filenames, or source paths.
4. Replicated/persisted gameplay data may carry logical profile IDs or logical asset IDs only.
5. Native and WASM must preserve the same authored audio semantics even if backend adapters differ internally.
6. Spatial audio, distance attenuation, category mixing, and missing-asset fallback must fail soft and must not crash gameplay.
7. Audio triggering must follow authority flow:
   - client input -> shard sim -> authoritative combat/destruction result -> routed presentation message/state -> client playback.

## 4. What Bevy 0.18 Gives Us

Bevy 0.18 is a usable foundation for basic playback, but it is not a full game-audio system.

Confirmed baseline capabilities:

1. `AudioPlayer` + `PlaybackSettings` can play one-shot and looped sounds.
2. `PlaybackSettings` supports:
   - initial volume,
   - speed,
   - paused/muted startup,
   - spatial flag,
   - start position,
   - duration.
3. Spatial playback exists via `SpatialListener` and `SpatialAudioSink`.
4. Per-sound sink control exists through `AudioSink` and `SpatialAudioSink`.

Confirmed limitations relevant to Sidereal:

1. Spatial audio is simple stereo panning, not HRTF or a full 3D renderer.
2. Bevy exposes one `GlobalVolume`, and changing it does not affect already playing audio.
3. Bevy does not provide built-in category buses/submixes for `sfx`, `dialog`, `music`, and `ui`.
4. Bevy does not provide a built-in effect graph for track-level/master reverb, low-pass, or other post-mix filters.
5. Bevy loop support is clip-wide or region-limited through `start_position` + `duration`, but it does not provide a first-class segmented intro/loop/outro weapon controller.

Conclusion:

1. Bevy asset decoding/playback primitives are useful.
2. Bevy alone is not sufficient for Sidereal's desired category mixing plus master-filter pipeline.
3. Sidereal should build a thin audio runtime abstraction with explicit backend capability requirements instead of directly coupling gameplay systems to Bevy audio primitives.

## 5. Backend Direction

Recommended direction:

1. Keep Bevy for ECS integration, transforms, and generic asset ownership.
2. Put Sidereal-specific playback behind a dedicated runtime layer, for example:
   - `AudioSettings` resource for user-configurable values,
   - `AudioMixerState` resource for runtime mixer state and backend handles,
   - `AudioListenerState` resource or component binding,
   - spawned client presentation entities for active world emitters/loops.
3. Require the runtime backend to support:
   - per-category volume and mute,
   - a master stage affecting all categories together,
   - spatial emitters tied to world transforms,
   - reverb/filter sends or equivalent environment processing,
   - one-shot playback,
   - looped playback,
   - segmented intro/loop/outro weapon playback,
   - runtime parameter changes for volume/filter/send amounts.

Pragmatic recommendation:

1. Treat Bevy built-in audio as insufficient for the final target.
2. Plan for a backend abstraction that can use a mixer/effect-capable backend.
3. Kira is the most promising currently documented candidate because it exposes tracks, effects, send tracks, and spatial tracks.
4. Do not hard-bind Sidereal to a native-only backend without a native plus WASM parity spike first.

## 6. Authoring Model

Audio authoring should be split into two layers:

### 6.1 Raw asset layer

Raw sound files live in the data tree under normal source content paths, for example:

1. `data/audio/sfx/ballistic_fire.mp3`
2. `data/audio/sfx/explosion1.mp3`
3. future `data/audio/music/...`
4. future `data/audio/dialog/...`

These files are payloads only. They are not the authoritative gameplay-facing definition.

### 6.2 Logical asset registry layer

Every playable audio payload must be declared in the authoritative Lua asset registry under `data/scripts/assets/registry.lua`.

Example target entries:

```lua
{
  asset_id = "audio.sfx.weapon.ballistic_fire",
  source_path = "audio/sfx/ballistic_fire.mp3",
  content_type = "audio/mpeg",
  dependencies = {},
  bootstrap_required = false,
},
{
  asset_id = "audio.sfx.explosion.asteroid.01",
  source_path = "audio/sfx/explosion1.mp3",
  content_type = "audio/mpeg",
  dependencies = {},
  bootstrap_required = false,
},
```

Rule:

1. The asset registry remains responsible for payload identity, checksum, and delivery metadata only.
2. The asset registry must not become the place where gameplay playback semantics such as loop markers, attenuation curves, or bus routing are authored.

### 6.3 Audio profile registry layer

Sidereal should add a dedicated script-authored audio profile registry, recommended path:

1. `data/scripts/audio/registry.lua`

This registry should define logical playback profiles that reference asset IDs and contain playback semantics. Profile data belongs here because:

1. gameplay components should not duplicate timing/mix/attenuation fields on every entity,
2. raw asset metadata is too low-level,
3. profile reuse is needed across many entities and effects,
4. the project already prefers logical profile IDs and data-driven content.

Recommended initial schema groups:

1. `weapon_profiles`
2. `destruction_profiles`
3. `ui_profiles`
4. `music_profiles`
5. `dialog_profiles`
6. `environment_profiles`

## 7. Where Audio References Should Live

### 7.1 Weapons

Weapon audio should be associated with the weapon, not with client-local code and not inferred from hardcoded weapon names.

Preferred long-term shape:

1. weapon entity carries a logical presentation/audio profile reference,
2. script-authored weapon audio profile resolves to:
   - asset IDs,
   - loop behavior,
   - bus/category,
   - spatial settings,
   - concurrency rules.

Recommended model:

1. final direction: a weapon-side logical profile such as `WeaponPresentationProfileId` or `WeaponAudioProfileId`,
2. transitional acceptable path: add `weapon_audio_profile_id` to `BallisticWeapon` while the broader weapon-family refactor is still in flight.

What should not happen:

1. `BallisticWeapon` storing raw file paths,
2. direct Rust match statements like `"Ballistic Gatling" => ballistic_fire.mp3`,
3. client code choosing audio by archetype name alone.

### 7.2 Destruction and explosions

Asteroid explosion audio should not be a bespoke ad hoc `DestructionSound` unless there is a proven need to decouple audio from the rest of destruction presentation.

Recommended direction:

1. Keep `Destructible.destruction_profile_id`.
2. Expand the meaning of that ID so it references a script-authored destruction presentation profile that can include both:
   - visual effect selection,
   - audio playback selection.

Why:

1. the existing gameplay hook already exists,
2. the same destruction event triggers both VFX and SFX,
3. destruction presentation usually wants one coherent authored profile.

Only split this later if there is a real design need to mix-and-match destruction VFX and SFX independently.

### 7.3 Global settings

Bus volumes, mute toggles, master filters, and user preferences should live in resources, not world entities.

Recommended split:

1. `AudioSettings` resource:
   - persisted user preferences,
   - bus volumes,
   - mute toggles,
   - output mode options,
   - accessibility options.
2. `AudioMixerState` resource:
   - runtime backend handles,
   - current bus gain state,
   - master effect state,
   - environment profile state,
   - debug telemetry.

Reason:

1. these are global/session-local concerns,
2. they do not need entity identity,
3. Bevy settings-style resources are the correct shape here.

### 7.4 Active world sounds

Active positional sounds should be represented by ECS entities/components in the client runtime.

Reason:

1. world sounds need transforms and lifetimes,
2. active loop emitters benefit from normal ECS inspection/debugging,
3. attaching emitters to world entities or presentation proxies keeps spatial updates explicit.

Recommended client-only components/resources:

1. `ActiveAudioEmitter`
2. `AudioEmitterProfileResolved`
3. `LoopedAudioInstance`
4. `AudioListenerAnchor`
5. `AudioEmitterOcclusionState` or `AudioEmitterEnvironmentState` later if needed

## 8. Playback Profile Model

Recommended minimum profile schema:

```lua
return {
  schema_version = 1,
  weapon_profiles = {
    {
      profile_id = "weapon.ballistic_gatling.fire",
      category = "sfx",
      playback = {
        kind = "segmented_loop",
        clip_asset_id = "audio.sfx.weapon.ballistic_fire",
        intro_start_s = 0.0,
        loop_start_s = 1.0,
        loop_end_s = 2.0,
        outro_start_s = 2.0,
        clip_end_s = 4.0,
      },
      spatial = {
        min_distance_m = 6.0,
        max_distance_m = 220.0,
        rolloff = "logarithmic",
        doppler = false,
      },
      mixer = {
        send_to_environment = 0.15,
        lowpass_by_distance = true,
      },
      concurrency = {
        scope = "per_emitter",
        max_instances = 1,
        steal = "oldest",
      },
    },
  },
  destruction_profiles = {
    {
      profile_id = "destruction.asteroid.default",
      category = "sfx",
      one_shot_variants = {
        "audio.sfx.explosion.asteroid.01",
      },
      spatial = {
        min_distance_m = 10.0,
        max_distance_m = 300.0,
        rolloff = "logarithmic",
      },
      mixer = {
        send_to_environment = 0.25,
      },
    },
  },
}
```

### 8.1 One-shot playback

Use for:

1. explosions,
2. UI clicks,
3. pickup sounds,
4. short weapon shots that do not need loop state.

Fields:

1. `clip_asset_id` or `variants[]`
2. optional pitch/volume variance
3. category/bus
4. spatial parameters if positional
5. concurrency controls

### 8.2 Segmented loop playback

Use for:

1. gatlings,
2. miniguns,
3. spool-up beam weapons,
4. engine hum or other held-state loops.

Canonical authored fields:

1. `intro_start_s`
2. `loop_start_s`
3. `loop_end_s`
4. `outro_start_s`
5. `clip_end_s`

Rules:

1. These markers belong in the audio profile registry, not in gameplay components.
2. Embedded file metadata markers may be supported later, but they must not be the only authoritative source in the first implementation.
3. If a target backend cannot provide sample-accurate segmented looping, the fallback authoring path is separate clips:
   - `intro_clip_asset_id`
   - `loop_clip_asset_id`
   - `outro_clip_asset_id`

### 8.3 Why markers belong in the profile and not the asset entry

1. Asset registry entries are generic delivery metadata.
2. The same clip could theoretically be reused by different profiles with different gain/send settings.
3. The current asset pipeline does not parse audio authoring metadata from payload files.
4. Script-authored timing fields are easier to validate, diff, and override.

## 9. Mixer and Channel Model

Sidereal should ship with these logical buses:

1. `master`
2. `sfx`
3. `dialog`
4. `music`
5. `ui`
6. optional `ambient`

Required user-facing controls:

1. per-bus volume scalar
2. per-bus mute toggle
3. master volume scalar
4. output pause/focus-loss behavior
5. settings persistence

Required runtime capabilities:

1. any playing instance must resolve to exactly one primary category bus,
2. all category buses must flow through a master stage,
3. master stage must support global filter/effect application that affects the entire mixed result,
4. category stages should support future ducking, for example dialog ducking music and SFX.

Recommendation:

1. Store authored category on the profile.
2. Apply user settings from `AudioSettings`.
3. Drive actual mixer handles from `AudioMixerState`.

## 10. Spatial Audio and Environment Processing

### 10.1 Listener binding

For the current top-down client, the primary listener should follow the gameplay camera anchor, not a random world entity.

Reason:

1. the player hears the scene from the presented view,
2. top-down camera offset matters,
3. it keeps audio aligned with what is visually on screen.

Future flexibility:

1. cockpit/cutscene modes may bind listener differently,
2. spectator/debug camera can override listener binding.

### 10.2 Positional falloff

The audio runtime must support at least:

1. distance attenuation,
2. stereo pan/spatial placement,
3. max-distance culling,
4. optional distance-based low-pass filtering for far-away muffling.

### 10.3 Reverb and environment

Sidereal still needs environment processing even in a space setting, because gameplay presentation can legitimately apply stylized or location-specific processing:

1. station interiors or docking bays,
2. cockpit/comms treatment,
3. nebula or anomaly zones,
4. warp or hyperspace transition effects,
5. map/paused/slow-time presentation filters,
6. underwater-like muffling equivalents for damaged systems or scanner mode.

Recommended model:

1. environment/audio-zone profile authored in scripts,
2. client runtime resolves the active environment for the listener or emitter,
3. profile drives:
   - reverb send amount,
   - master low-pass/high-pass style filters,
   - wet/dry mix,
   - optional music/dialog treatment.

Important distinction:

1. category buses are logical mix groups,
2. environment processing is a separate layer that can affect one or more buses or the master output.

## 11. Networking and Authority

### 11.1 General rule

Network traffic should carry authoritative presentation events and IDs, not commands like "play this file now" and never raw audio bytes.

### 11.2 Weapon fire

Current `ServerWeaponFiredMessage` is enough for tracers but not enough for robust weapon audio.

Audio-capable weapon presentation needs at least one of:

1. a `weapon_entity_id` that the client can resolve to an audio/presentation profile,
2. a compact `weapon_audio_profile_id` or `weapon_presentation_profile_id` hint in the event payload,
3. a more generic weapon presentation lifecycle message.

For sustained or segmented weapon audio, do not rely solely on per-shot timing gaps to infer stop state under jitter.

Recommended eventual contract for held-fire families:

1. explicit start/active/stop lifecycle signaling for sustained presentation,
2. or a generic authoritative weapon presentation state channel that also serves beam visuals.

### 11.3 Destruction

Current `ServerEntityDestructionMessage { destruction_profile_id, origin_xy, ... }` is the correct pattern.

Recommended direction:

1. keep destruction events as single authoritative presentation triggers,
2. resolve both explosion VFX and SFX from the profile on the client,
3. do not send extra sound-only destruction messages.

### 11.4 Visibility rules

Audio playback follows the same visibility-delivery contract as the presentation event that spawned it.

Rules:

1. if a client should not receive the event, it should not receive the audio trigger,
2. owner/public/faction visibility still applies,
3. no `NetworkTarget::All` broadcast for final combat audio delivery,
4. client-local attenuation may still suppress or cull far-away audio after legal delivery.

## 12. Concurrency, Voice Limits, and Repetition Control

A real game-audio system needs anti-spam controls from the start.

Required profile-level controls:

1. `max_instances`
2. `concurrency_scope`:
   - global,
   - per_category,
   - per_profile,
   - per_emitter,
   - per_owner,
3. `steal_policy`:
   - oldest,
   - quietest,
   - furthest,
   - reject_new,
4. `min_retrigger_interval_ms`
5. `priority`

Examples:

1. Gatling loop should usually be `max_instances = 1` per emitter.
2. Asteroid explosions should allow multiple simultaneous emitters but still voice-cap by distance and priority.
3. UI clicks should not flood the mix if a user drags rapidly over controls.

## 13. Immediate Adoption for the First Two Sounds

First target content:

1. `data/audio/sfx/ballistic_fire.mp3`
2. `data/audio/sfx/explosion1.mp3`

Recommended immediate mapping:

1. register both as logical assets in `data/scripts/assets/registry.lua`,
2. add script-authored audio profiles:
   - `weapon.ballistic_gatling.fire`
   - `destruction.asteroid.default`
3. associate the gatling profile with the weapon entity/profile,
4. resolve asteroid destruction audio through `destruction_profile_id`,
5. treat both as runtime-optional assets unless design chooses to make nearby-combat SFX bootstrap-required for first-load UX.

Required supporting contract update:

1. add explicit `audio/mpeg` handling to shared asset-runtime metadata helpers and validation expectations if MP3 remains the chosen source format.

## 14. Additional Audio Features a Top-Down Space ARPG Should Plan For

Beyond the first two sounds, the system should be able to grow into:

1. engine/thruster idle and boost loops,
2. shield hit and shield-break sounds,
3. missile launch, flyby, and detonation,
4. UI clicks, warnings, and confirmations,
5. music states and combat stingers,
6. faction or mission radio/dialog channels,
7. loot, pickup, and quest progression cues,
8. hazard-zone ambience and anomaly soundscapes,
9. camera/map-mode filter transitions,
10. damage-state or low-health master filtering,
11. subtitle/caption hooks for dialog-critical events,
12. debug tools for active emitters, bus levels, missing assets, and voice stealing.

## 15. Validation and Testing Expectations

When implementation starts, minimum validation should include:

1. unit tests for profile decoding/validation,
2. asset-registry coverage for audio content types,
3. client tests for fallback behavior when audio assets are missing,
4. native and WASM compile parity,
5. loop-transition tests for segmented weapon audio,
6. multi-client visibility tests proving audio triggers only arrive with permitted events,
7. manual verification of distance attenuation and bus volume controls,
8. telemetry for active voices, dropped voices, and missing-profile resolution failures.

## 16. Recommended Next Planning Questions

This contract intentionally leaves a few planning decisions to the implementation plan:

1. choose the concrete backend strategy:
   - Bevy-only MVP,
   - Kira-backed runtime abstraction,
   - another backend behind the same Sidereal interface.
2. choose whether initial segmented weapon loops use:
   - single-clip markers,
   - multi-clip intro/loop/outro,
   - both with backend fallback.
3. choose whether first implementation adds a generic weapon presentation profile or a temporary `weapon_audio_profile_id` on `BallisticWeapon`.
4. decide which initial sounds are bootstrap-required versus lazy runtime-optional.

## 17. External References

These upstream references informed the capability assessment above:

1. Bevy 0.18 audio example and API docs:
   - `https://bevy.org/examples/audio/spatial-audio-2d/`
   - `https://docs.rs/bevy_audio/0.18.0/bevy_audio/`
2. Kira mixer/effect/spatial track docs:
   - `https://docs.rs/kira/latest/kira/track/`
3. `bevy_kira_audio` channel and spatial plugin docs:
   - `https://docs.rs/bevy_kira_audio/latest/bevy_kira_audio/`

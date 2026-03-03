# Lighting Model and Dynamic Space Events Plan

Status: Proposed implementation plan  
Date: 2026-03-03  
Owners: client rendering + gameplay runtime + asset streaming

Primary references:
- `docs/sidereal_design_document.md`
- `docs/features/asset_delivery_contract.md`
- `docs/features/thruster_plumes_afterburner_plan.md`
- `docs/features/visibility_replication_contract.md`

## 1. Goals

Define one consistent lighting model for Sidereal (top-down action RPG) that supports:

1. Global illumination style ambient response.
2. Directional/key light from suns/stars.
3. Dynamic emissive lighting from thrusters, weapons, impacts, and particles.
4. Normal/bump-map based ship/asteroid lighting and shadow cues.
5. Flashy neon combat readability (lasers, guns, plasma effects).
6. Background lightning and other sci-fi dynamic space events.
7. Native + WASM parity with streamed shader/material assets.

## 2. Non-Negotiable Runtime Rules

1. Authority remains one-way: gameplay simulation and visibility are server-authoritative; lighting is presentation unless explicitly declared gameplay-relevant.
2. Lighting systems must never write authoritative motion/gameplay state.
3. Dynamic light/particle instances are not replicated one-by-one.
4. Replicate compact event/state inputs only; derive render detail client-side.
5. Use logical `asset_id` references, never raw runtime file paths in gameplay components.
6. Keep native/WASM behavior aligned; no gameplay-lighting forks by target.

## 3. Consistent Lighting Model

Use a layered 2.5D lighting stack:

1. `L0` Background radiance: fullscreen shaders (starfield/nebula/background) output ambient sky radiance and event flashes.
2. `L1` Stellar key light: one or more directional/area sun contributions affecting world sprites/normal maps.
3. `L2` Local dynamic lights: short-lived point/cone lights from thrusters, weapon fire, impacts, explosions, EMP arcs.
4. `L3` Emissive/bloom composite: neon highlights and glow shaping for combat readability.

All world objects sample the same lighting inputs (ambient + stellar + local) so visuals stay coherent.

## 4. Global Illumination Strategy (Top-Down Friendly)

Do not attempt heavyweight real-time ray-traced GI for v1.

Recommended approach:

1. Low-resolution light accumulation buffer (screen-space or world-tiled).
2. Ambient probe grid per camera region:
   - seeded from background radiance + nebula color fields,
   - slowly time-smoothed,
   - sampled by lit sprite/material shaders.
3. Optional emissive injection pass:
   - strong local emitters (explosions, large plasma bursts) inject temporary bounce tint into nearby probes.

Result: GI-like cohesion at MMO-friendly cost.

## 5. Sun/Star Lighting

### 5.1 Data model

Introduce a public replicated environment state (shard-level):

1. `EnvironmentLightingState`
   - primary light direction (world-space)
   - primary light color/intensity
   - optional secondary stellar fill
   - ambient baseline color/intensity

### 5.2 Runtime behavior

1. Lighting direction is stable over short intervals; blend over time for transitions.
2. World materials use this as key light source for normals/shadows.
3. Sun occlusion for 2D top-down uses simplified height-shadow approximation (see Section 6).

## 6. Normal/Bump Maps and Shadow Cues

### 6.1 Surface shading model

Add optional per-entity material metadata:

1. `NormalMapAssetId` (optional)
2. `HeightForShadowM` (optional scalar or profile id)
3. `MaterialLightingProfileId` (optional preset)

### 6.2 Shadow model

For top-down readability and performance:

1. Use projected contact/drop shadows with light-direction offset.
2. Modulate shadow softness/length by `HeightForShadowM` and sun elevation proxy.
3. Avoid expensive per-pixel shadow maps in v1.

This provides strong depth cues for ships/modules/asteroids while staying performant.

## 7. Thrusters, Weapons, and Particle Lighting

### 7.1 Dynamic emitters

Extend visual effect systems with a shared emitter contract:

1. `LocalLightEmitter`
   - color, intensity, radius
   - shape (`point`, `cone`, `capsule`)
   - falloff curve
   - ttl/fade policy

Sources:

1. Thruster plumes (already present) emit cone/point lights.
2. Weapon fire emits short pulse lights.
3. Beam/laser weapons emit line-segment light proxies.
4. Explosions and hit sparks emit burst lights.

### 7.2 Neon combat look

Establish palette bands and intensity rails:

1. Faction/player-safe base hues.
2. High-saturation accent hues for weapon classes.
3. Controlled bloom threshold to prevent full-screen washout.
4. Per-effect max luminance caps to protect readability.

### 7.3 Particle lighting

1. Particles receive lighting from ambient + local light buffer.
2. Selected particles (muzzle flare/explosion core) also emit light.
3. Distant LOD: disable particle-emissive lights first, keep only major emitters.

## 8. Background Lightning and Dynamic Sci-Fi Space Events

## 8.1 Lightning (thunder/lightning style in space backdrop)

Implement as environment event-driven backdrop modulation:

1. `BackgroundStormEvent`
   - event type (`ion_storm`, `nebula_lightning`, `solar_arc`)
   - region seed / spatial mask
   - flash cadence profile
   - color/intensity curve
   - duration
2. Backdrop shaders sample active storm events and add:
   - cloud/nebula arc flashes,
   - directional sheet-light pulses,
   - horizon glow ramps.

Note: no atmospheric thunder requirement; optional low-frequency rumble SFX can still be used stylistically.

## 8.2 Common dynamic space events (v1 list)

1. Solar flare pulses (global warm key-light spikes).
2. Nebula ion lightning cells (regional blue/violet flashes).
3. Plasma wind gusts (ambient hue drift + particle streaks).
4. Debris electrostatic storms (local spark arcs).
5. Gravitational shear lensing waves (distortion + subtle light bend).
6. Pulsar sweep bands (periodic directional color/intensity sweep).

## 8.3 Authority and replication

1. Cosmetic-only events can be deterministic client-generated from shard seed + time window.
2. Gameplay-relevant events (if later added) must be server-authored and replicated as compact event state.
3. Keep event payload compact and cadence-bounded.

## 9. Asset/Shader Contract Additions

Add logical asset IDs (examples):

1. `lighting_world_lit_sprite_wgsl`
2. `lighting_light_accumulation_wgsl`
3. `lighting_bloom_composite_wgsl`
4. `lighting_background_storm_wgsl`
5. `textures.lut.neon_palette_png`
6. `textures.noise.storm_cells_png`
7. `textures.normals.ship_default_png`

Require source/cache parity:

1. `data/shaders/*` source and `data/cache_stream/shaders/*` streamed cache paths must stay aligned in the same change.

## 10. Performance and LOD Policy

1. Hard budget dynamic lights per camera zone (for example near/mid/far tiers).
2. Cluster or tile lights for cheap accumulation.
3. Distant entities use reduced lighting model:
   - no normal-map detail,
   - no particle-emissive lights,
   - reduced bloom/emissive contribution.
4. Expose runtime tuning env vars for light count, probe resolution, bloom quality.

## 11. Implementation Phases

### Phase A: Foundation

1. Add shared lighting data components/resources (`EnvironmentLightingState`, emitter contract).
2. Add shader/material scaffolding and streamed asset IDs.
3. Integrate ambient + stellar key-light into existing world sprite path.

### Phase B: Surface Lighting

1. Add optional normal/bump map support for ships and asteroids.
2. Add projected top-down shadow cues from stellar direction.

### Phase C: Combat Emissive Stack

1. Convert thruster/weapon/explosion visuals to shared emitter pipeline.
2. Add neon bloom composite and clamp policy.
3. Add LOD throttling and budgets.

### Phase D: Space Events

1. Implement background lightning storm event model.
2. Add initial dynamic event catalog (solar flare, ion storm, pulsar sweep).
3. Integrate optional event SFX hooks.

### Phase E: Hardening

1. Profiling and fallback tiers for low-end GPUs.
2. Cross-target parity verification (native + wasm webgpu).
3. Multiplayer session sanity checks for synchronized event timing where required.

## 12. Testing Plan

Unit tests:

1. Lighting state blending and clamping.
2. Emitter lifecycle (spawn/fade/despawn) determinism.
3. LOD budget selection stability.

Integration tests:

1. Replicated environment lighting state reaches clients correctly.
2. Cosmetic event generation is deterministic for same shard seed/time window.
3. Gameplay unaffected when lighting assets are missing (fail-soft placeholders).

Build/quality gates:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo check --workspace`
4. `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
5. `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`

## 13. Open Decisions

1. Should environment lighting state live as shard-global resource only, or also as replicated ECS singleton entity for tooling visibility?
2. Should storm events be fully server-scheduled or hybrid deterministic (server keyframes + client interpolation)?
3. What is the first strict visual style guide for neon palettes and bloom limits to keep combat legible?
4. Which event types, if any, become gameplay-affecting in v1 versus cosmetic-only?

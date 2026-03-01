# Generic Fullscreen Shader System Plan (Project-Specific, Bevy 0.18)

## Goal

Implement a clean fullscreen shader architecture that works with existing server-authoritative fullscreen layers, streamed WGSL assets, and client BRP editing, while removing current legacy coupling in material uniforms.

Primary outcomes:

- Shared per-frame world motion data is computed once and reused by fullscreen shaders.
- Shader-specific parameters (for example `Density`) are not packed into unrelated uniforms.
- `starfield` and `space_background` use a consistent uniform contract.
- Source and streamed shader paths stay in lockstep (`data/shaders/*` and `data/cache_stream/shaders/*`).
- Existing behavior remains compatible with `Material2d` (no custom render pipeline required).

## Current State Inventory

### Client rendering and material wiring

- `bins/sidereal-client/src/native/backdrop.rs`
  - `StarfieldMaterial` currently exposes only `viewport_time`, `drift_intensity`, `velocity_dir`.
  - `velocity_dir.w` is currently overloaded as density scale.
  - `SpaceBackgroundMaterial` uses a different layout (`viewport_time`, `colors`, `motion`), not the shared world schema.
- `bins/sidereal-client/src/native/mod.rs`
  - Registers both `Material2d` plugins.
  - Schedules `update_starfield_material_system` then `update_space_background_material_system` in `Last`.
- `bins/sidereal-client/src/native/resources.rs`
  - Holds motion resources (`StarfieldMotionState`, `CameraMotionState`) that already contain the world inputs we should centralize.
- `bins/sidereal-client/src/native/visuals.rs`
  - Builds fullscreen renderables from replicated `FullscreenLayer` entities.
  - Injects client-local `Density(0.05)` for starfield renderables.

### Shader and stream paths

- Source shaders: `data/shaders/starfield.wgsl`, `data/shaders/space_background.wgsl`.
- Stream/cache shaders: `data/cache_stream/shaders/starfield.wgsl`, `data/cache_stream/shaders/space_background.wgsl`.
- `bins/sidereal-client/src/native/shaders.rs` and `bins/sidereal-client/src/native/assets.rs` load/reload these streamed shaders and treat both as critical bootstrap assets.
- `bins/sidereal-replication/src/replication/assets.rs` and `crates/sidereal-asset-runtime/src/lib.rs` define both shader IDs as streamable assets.

### Authority and visibility context

- `FullscreenLayer` is persist+replicate+public (`crates/sidereal-game/src/components/fullscreen_layer.rs`), so layer entities are server-authoritative.
- `bins/sidereal-replication/src/replication/visibility.rs` applies delivery policy. No shader-specific behavior is in this system today.
- `Density` currently exists as a local gameplay component (`replicate = false`, `persist = false`) in `crates/sidereal-game/src/components/density.rs`.

## Target Architecture

## 1) Shared world data resource (single writer per frame)

Add a new client resource in `bins/sidereal-client/src/native/resources.rs`:

- `FullscreenExternalWorldData`
  - `viewport_time: Vec4` (`xy` viewport, `z` elapsed time, `w` reserved or global warp baseline)
  - `drift_intensity: Vec4` (`xy` travel drift, `z` intensity, `w` alpha)
  - `velocity_dir: Vec4` (`xy` heading, `z` zoom scale, `w` reserved)

Important: `velocity_dir.w` becomes reserved only. Density must no longer use this slot.

## 2) Material schema alignment

### Starfield

Update `StarfieldMaterial` in `backdrop.rs` to include:

- `viewport_time` (binding 0)
- `drift_intensity` (binding 1)
- `velocity_dir` (binding 2)
- `starfield_params` (binding 3), with `x = density`

### Space background

Update `SpaceBackgroundMaterial` to align to the same first three bindings:

- `viewport_time` (binding 0)
- `drift_intensity` (binding 1)
- `velocity_dir` (binding 2)
- optional `space_bg_params` (binding 3) for future tuning (can be zero-initialized now)

This removes `colors` and `motion` as legacy fields.

## 3) System decomposition

Split update logic into two phases in `backdrop.rs` (or a new `native/fullscreen.rs` module):

- `compute_fullscreen_external_world_system`
  - reads window, `Time`, `CameraMotionState`, `StarfieldMotionState`, controlled-entity velocity/projection context
  - writes `FullscreenExternalWorldData`
- material apply systems
  - `apply_starfield_material_system`
  - `apply_space_background_material_system`
  - copy shared world resource into each material, then apply per-shader params

Scheduling target:

- Keep in `Last` after camera/control lock systems (same intent as current order).
- Run compute once, then starfield/background apply systems.

## 4) Shader uniform contract

Both WGSL files must share the first three bindings:

- `@group(2) @binding(0) viewport_time`
- `@group(2) @binding(1) drift_intensity`
- `@group(2) @binding(2) velocity_dir`

Starfield adds binding 3 (`starfield_params`) and reads density from `starfield_params.x`.

Legacy contract to remove:

- Any use of `velocity_dir.w` as density in `starfield.wgsl`.
- `colors` and `motion` uniforms in `space_background.wgsl`.

## Detailed Implementation Phases

## Phase 0 - Baseline and safety

1. Confirm current build compiles before migration.
2. Capture baseline screenshot/video for starfield + background in native client.
3. Record current BRP behavior for `Density` edits on client entity.

## Phase 1 - Add shared world resource

Files:

- `bins/sidereal-client/src/native/resources.rs`
- `bins/sidereal-client/src/native/mod.rs`

Tasks:

1. Define `FullscreenExternalWorldData` resource with sensible defaults.
2. Initialize resource in app startup (native path).
3. Keep existing motion resources intact (no behavior change yet).

## Phase 2 - Refactor systems (no WGSL change yet)

Files:

- `bins/sidereal-client/src/native/backdrop.rs`
- optionally split into `bins/sidereal-client/src/native/fullscreen.rs`

Tasks:

1. Extract world-input computation from `update_starfield_material_system` into `compute_fullscreen_external_world_system`.
2. Update starfield apply system to read `FullscreenExternalWorldData`.
3. Keep temporary compatibility mapping while starfield WGSL is not switched (short-lived transitional state allowed in this phase only).
4. Update `mod.rs` schedule ordering:
   - compute shared data
   - apply starfield
   - apply space background

## Phase 3 - Material schema migration

Files:

- `bins/sidereal-client/src/native/backdrop.rs`

Tasks:

1. Add `starfield_params: Vec4` to `StarfieldMaterial`.
2. Remove density usage from `velocity_dir.w` population.
3. Replace `SpaceBackgroundMaterial` fields from (`colors`, `motion`) to shared-world schema (+ optional `space_bg_params`).
4. Update defaults to maintain current visual baseline as closely as possible.

## Phase 4 - Shader conversion and parity update

Files (must be changed together):

- `data/shaders/starfield.wgsl`
- `data/cache_stream/shaders/starfield.wgsl`
- `data/shaders/space_background.wgsl`
- `data/cache_stream/shaders/space_background.wgsl`

Tasks:

1. Starfield WGSL:
   - add binding 3 `starfield_params`
   - replace `density_scale = clamp(velocity_dir.w, ...)` with `density_scale = clamp(starfield_params.x, ...)`
2. Space background WGSL:
   - replace `colors` + `motion` usage with shared uniforms
   - move any previous color tuning into constants or `space_bg_params` (if added)
3. Keep visual result approximately equivalent (especially drift response and speed influence).
4. Verify streamed reload still works (client `reload_streamed_shaders` path).

## Phase 5 - Legacy code removal

Remove all legacy assumptions after migration:

- `velocity_dir.w` carrying density (Rust and WGSL comments/code).
- `SpaceBackgroundMaterial.colors` and `SpaceBackgroundMaterial.motion`.
- Old placeholder shader uniform declarations in `native/shaders.rs` that still reference removed bindings.
- Any compatibility glue introduced in Phase 2.

Also clean comments so they match final contract.

## Phase 6 - BRP parameter editing path

Files:

- `bins/sidereal-client/src/native/visuals.rs` (where `Density` is inserted)
- `bins/sidereal-client/src/native/backdrop.rs`

Tasks:

1. Keep `Density` as shader-specific component read by starfield apply system.
2. Ensure default injected value remains explicit (`Density(0.05)` or updated project default).
3. Confirm BRP update of `Density` updates `starfield_params.x` next frame.

Decision note:

- Keep `Density` non-replicated for now (client-local tuning). If synchronized per-layer tuning is desired later, that requires a new replicated fullscreen params component and visibility policy review.

## Phase 7 - Visibility/replication validation (no behavior change expected)

Files to validate:

- `bins/sidereal-replication/src/replication/visibility.rs`
- `crates/sidereal-game/src/components/fullscreen_layer.rs`

Checklist:

1. Fullscreen layers still replicate as public entities.
2. No visibility regression for fallback vs authoritative layer handoff.
3. No new replicated shader-param component is introduced accidentally without policy decision.

## Phase 8 - Docs and tests

Documentation updates:

- Update this plan to "implemented" state with final schema.
- If visibility/replication policy changes, update:
  - `docs/features/visibility_replication_contract.md`
  - `docs/decision_register.md` (and feature DR file if needed)

Validation commands (minimum):

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace`
- `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
- `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`

Manual checks:

- Launch client, verify both fullscreen layers render.
- Verify shader streaming hot-reload updates visuals.
- Verify BRP `Density` edits change starfield density in real time.
- Verify no "despawn unknown entity" spam related to fullscreen layers during connect/disconnect.

## File-by-File Change Map

Core code:

- `bins/sidereal-client/src/native/resources.rs` (new shared world data resource)
- `bins/sidereal-client/src/native/backdrop.rs` (material schema + systems)
- `bins/sidereal-client/src/native/mod.rs` (system ordering/init)
- `bins/sidereal-client/src/native/shaders.rs` (placeholder schema parity)
- `bins/sidereal-client/src/native/visuals.rs` (density default and layer build assumptions)

Shaders:

- `data/shaders/starfield.wgsl`
- `data/cache_stream/shaders/starfield.wgsl`
- `data/shaders/space_background.wgsl`
- `data/cache_stream/shaders/space_background.wgsl`

Validation-only (likely no edits unless policy changes):

- `bins/sidereal-replication/src/replication/visibility.rs`
- `bins/sidereal-replication/src/replication/assets.rs`
- `crates/sidereal-asset-runtime/src/lib.rs`

## Legacy Removal Checklist (Explicit)

- [ ] Remove density-from-`velocity_dir.w` in Rust and WGSL.
- [ ] Remove `colors`/`motion` uniforms and CPU writers for space background.
- [ ] Remove stale comments describing old packed semantics.
- [ ] Ensure placeholder WGSL matches final bind layout.
- [ ] Ensure source/cache shader copies are schema-identical.

## Risks and Mitigations

- Visual drift mismatch after uniform migration:
  - Mitigate with side-by-side baseline capture and tunable constants in params vec4.
- Streamed shader/source divergence:
  - Mitigate by updating both paths in same change and validating checksum stream.
- Hidden schedule regressions:
  - Mitigate by keeping compute/apply in `Last` after camera lock chain.
- BRP edits not reflecting:
  - Mitigate by asserting material update system always reads current component each frame (or with proper `Changed<T>` split plus full world-data update path).

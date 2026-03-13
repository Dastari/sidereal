# Thruster Plumes and Afterburner Plan

Status: In progress (baseline corvette implementation landed)  
Date: 2026-03-03  
Owners: gameplay runtime + client rendering + asset streaming

Primary references:
- `docs/sidereal_design_document.md`
- `docs/features/asset_delivery_contract.md`
- `docs/component_authoring_guide.md`

## Implementation Status Notes (2026-03-03)

Baseline implementation now includes:

1. Corvette topology updated to a single center-aft engine hardpoint + module.
2. Hardpoint local rotation persisted alongside offset and applied in hierarchy reconstruction.
3. Afterburner actions (`AfterburnerOn`/`AfterburnerOff`) wired from `Shift` hold input.
4. Engine `AfterburnerCapability` + hull `AfterburnerState` integrated into authoritative flight simulation.
5. Initial client thruster plume visual attached to engine modules and driven by authoritative throttle + afterburner state.

Implemented in this baseline:

1. `max_afterburner_velocity_mps` now raises the forward thrust governor while afterburner is active (engine-thrust-only cap behavior; no hard velocity clamp).

## Implementation Status Notes (2026-03-13)

1. `ThrusterPlumeShaderSettings` now replicates with public visibility rather than owner-only visibility.
2. Thruster plume attachment/update now keys off public `EntityLabels` + `MountedOn` instead of the owner-only `Engine` marker, so plume rendering does not disappear when the winning visual lane lacks that gameplay marker.
3. Native impact: authored plume settings now stay available on the actual rendered lane instead of relying on owner-only state surviving duplicate/lane selection.
4. WASM impact: no architecture split; the same replicated plume settings now apply to browser clients.

## Implementation Status Notes (2026-03-13, later plume fix)

1. Thruster plume drive state now resolves against the mounted ship UUID when `FlightComputer` lives on a mounted module entity instead of the ship root.
2. This fixes the local corvette/rocinante case where the plume updater could fail to find throttle/afterburner state even though the engine module and plume settings were present and public.
3. Native impact: plumes now resume reacting to authoritative throttle/afterburner state on modular ships rather than staying effectively invisible because the lookup never matched.
4. WASM impact: no architecture split; the same mounted-module resolution logic applies to browser clients.

## 1. Goal

Add visually responsive spaceship thruster plumes that:

1. Scale with authoritative thrust state.
2. Support an afterburner mode with stronger/intense visuals.
3. Expose tunable parameters (color, falloff, noise, flicker, bloom strength, length/width, etc.).
4. Stay native+WASM compatible and aligned with streamed shader contracts.

## 2. Shader vs Particle System

Recommendation for Sidereal now: **shader-first, optional particles later**.

Why shader-first is the better baseline:

1. Plumes are continuous effects tied directly to thrust scalars; shaders model that naturally with low CPU overhead.
2. Existing client already supports streamed WGSL shader assets and fallback handling.
3. MMO scenes can contain many ships; pure particle emitters for all engines can become CPU-heavy and bandwidth-heavy if overused.
4. Shader params are easy to drive from replicated gameplay state (or derived local render state) without spawning/despawning many entities.

When particles are still useful:

1. Short burst accents (ignition pops, sputter, shock diamonds, afterburner crackle).
2. Damage/failure sparks.
3. Very close camera hero effects.

Direction: **Hybrid**.
1. Core plume = shader quads/meshes.
2. Optional particle accent layer = local visual only.

## 3. Runtime Model

Authoritative flow remains one-way:

1. Input intent updates authoritative flight state (`FlightComputer`, engine output).
2. Server sim determines thrust/reverse/brake/afterburner state.
3. Replication delivers required state to clients.
4. Client render maps that state to plume shader uniforms.

No client-authoritative physics changes from plume effects.

## 4. Data Model (Proposed)

Add gameplay/render config components in `crates/sidereal-game/src/components/`:

1. `ThrusterVisualConfig` (`persist=true`, `replicate=true`)
   - `plume_shader_asset_id: String` (logical asset id)
   - `base_color_rgb: Vec3`
   - `hot_color_rgb: Vec3`
   - `afterburner_color_rgb: Vec3`
   - `base_length_m`, `max_length_m`
   - `base_width_m`, `max_width_m`
   - `flicker_hz`, `noise_strength`
   - `intensity_scale`
2. `AfterburnerCapability` (`persist=true`, `replicate=true`)
   - `enabled: bool`
   - `multiplier: f32`
   - `fuel_burn_multiplier: f32`
   - `heat_per_s` (future hook)
3. `AfterburnerState` (`persist=true`, `replicate=true`)
   - `active: bool`
   - `activation_alpha` (0..1 smoothed for visuals)

Notes:

1. Keep one primary component per file and use `#[sidereal_component(...)]`.
2. Replicated fields should be minimal and stable; pure render-only ephemeral values can remain client-local.

## 5. Action/Input Additions (Proposed)

Add explicit afterburner intent actions:

1. `AfterburnerOn`
2. `AfterburnerOff`

Example default mapping:

1. Hold `LeftShift` for afterburner.
2. Keep movement/thrust input unchanged.

Server flight systems enforce constraints:

1. Fuel availability.
2. Cooldown/heat policy (if configured).
3. Cap multiplier and safety limits.

## 6. Rendering Architecture

### 6.1 Per-thruster render entity

For each engine module/hardpoint, spawn/attach a child render entity:

1. Local transform anchored at exhaust nozzle.
2. 2D quad mesh + custom plume material (WGSL fragment shader).
3. Optional additive blend and soft alpha edge.

### 6.2 Shader uniforms

Drive uniforms every frame from replicated/derived state:

1. `thrust_alpha` (0..1)
2. `afterburner_alpha` (0..1)
3. `time_s`
4. color ramp inputs (`base/hot/afterburner`)
5. geometry scaling (`length`, `width`)
6. flicker/noise controls

### 6.3 Streamed shader path parity

When adding plume shader assets:

1. Add source shader under `data/shaders/`.
2. Add streamed cache counterpart under `data/cache_stream/shaders/` via existing asset flow.
3. Keep runtime source and streamed cache schema/path parity in the same change.

Do not introduce path divergence between source and streamed cache variants.

## 7. Visual Behavior Spec (Initial)

Base plume response:

1. `thrust_alpha = clamp(abs(throttle), 0, 1)`.
2. length/width interpolate from base to max using `thrust_alpha`.
3. color shifts from `base_color` toward `hot_color` as thrust rises.

Afterburner response:

1. When active, blend toward `afterburner_color`.
2. Increase length/width and intensity by capability multipliers.
3. Add high-frequency flicker/noise band.
4. Optional lens flare/bloom contribution in post chain (future).

Reverse thrust/brake:

1. Either render a distinct reverse plume profile or reduce forward plume and switch direction.
2. Keep this data-driven in config, not hardcoded ship-class branches.

## 8. Networking and Performance

1. Do not replicate particle instances.
2. Replicate only compact thrust/afterburner state already needed for gameplay correctness.
3. Compute per-frame visual interpolation on client.
4. Keep plume rendering batched/material-instance efficient.
5. Use LOD:
   - distant ships use cheaper plume variant (fewer noise ops),
   - very distant ships clamp to minimal billboard glow.

## 9. Asset and Tooling Requirements

Add logical asset IDs for plume shaders/textures in catalog pipeline:

1. `thruster_plume_wgsl`
2. optional `thruster_noise_lut_png`
3. optional `afterburner_mask_png`

All references should use logical `asset_id` identifiers, never raw disk paths in gameplay components.

## 10. WASM/Native Parity Notes

1. Use shared gameplay and render systems; no feature-flag split by target.
2. Any platform branch must stay at transport boundary only (`cfg(target_arch = "wasm32")`).
3. Verify `bevy/webgpu` build path supports plume shader bindings.

## 11. Testing Plan

Unit tests (`crates/sidereal-game`):

1. Afterburner action toggles state deterministically.
2. Fuel burn multiplier applies only while afterburner is active.
3. Thrust-to-plume scalar mapping clamps and remains stable.

Integration tests (`bins/sidereal-replication`):

1. Authenticated client input toggles afterburner on controlled entity only.
2. State replicates to other clients without authority leaks.
3. No control spoofing regression.

Client checks:

1. Native build with plume shader assets.
2. WASM build with `bevy/webgpu`.
3. Visual sanity in two-client session (local predicted + remote interpolated ships).

## 12. Implementation Sequence

1. Introduce new components/actions and register via macro path.
2. Extend flight system with afterburner authoritative logic.
3. Add plume material + shader and streamed asset registration.
4. Attach thruster render children to engine modules/hardpoints.
5. Drive shader uniforms from replicated motion/thrust state.
6. Add optional local-only particle accents.
7. Validate quality gates and cross-target builds.

## 13. Open Decisions

1. Should afterburner be fuel-only constrained, or also heat/cooldown constrained in v1?
2. Should remote clients see full color customization, or a sanitized palette tier?
3. Should afterburner be a hold action only, or toggle mode too?

# DR-0029: Runtime Shader Family Taxonomy and Lua Authoring Model

Status: Accepted direction
Date: 2026-03-07
Owners: client rendering + scripting + asset streaming

Status note (2026-03-12):

1. `RuntimeEffectMaterial` now has an initial explosion billboard variant in addition to thruster plume, impact spark, and tracer usage.
2. Explosion visuals still use the same fixed Rust-owned effect family ABI; no new standalone explosion material type was introduced.

Status note (2026-04-28):

1. Native client runtime shader reloads now preflight streamed WGSL with Naga and validate every declared `@group(2)` resource against the fixed Rust-owned material-family ABI before installing it into a Bevy shader handle.
2. Invalid streamed shader updates are rejected without replacing the currently installed shader. If there is no previous shader for that slot, the client installs the family fallback shader instead.
3. This preserves live shader editing while preventing material binding-layout mismatches from reaching wgpu pipeline creation, where they are fatal by default. WASM uses the same shared validation path when streamed shader overrides are enabled; existing browser-safe fullscreen fallbacks remain unchanged.

Primary references:
- `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
- `docs/plans/dynamic_runtime_shader_material_plan.md`
- `docs/features/asset_delivery_contract.md`
- `docs/features/scripting_support.md`
- `docs/sidereal_design_document.md`

## 1. Decision Summary

Sidereal should not model rendering as one Rust material type per visual effect. The correct long-term model is:

1. a small fixed set of Rust-owned shader/material families,
2. Lua-authored visual instances, pass stacks, asset references, and parameter payloads,
3. a bounded number of shader assets within each family,
4. catalog-validated runtime delivery and fallback.

The engine boundary is:

1. Rust owns material families, ABI contracts, validation, replication, and runtime execution.
2. Lua owns composition, pass selection, shader asset IDs, parameter payloads, texture bindings, and per-entity/per-layer visual intent.

## 2. Why This Decision Exists

The project is moving toward a more generic engine/runtime where game-specific content is increasingly Lua-authored rather than hardcoded in Rust.

The wrong model is:

1. one Rust `Material2d` type per gameplay effect,
2. ad hoc content-specific marker components,
3. per-effect runtime branching spread across client systems,
4. Rust-side shader IDs and composition rules that duplicate what Lua content should define.

That does not scale once the project accumulates:

1. planets,
2. starfields,
3. background layers,
4. thrusters,
5. projectiles,
6. impacts,
7. explosions,
8. shockwaves,
9. smoke plumes,
10. shields,
11. selection/radar/tactical overlays,
12. post-process effects.

## 3. Accepted Family Taxonomy

The target runtime family taxonomy is:

### 3.1 Fullscreen background family

Purpose:

1. starfield
2. nebula
3. static/scrolling space background
4. future background composites

Current expected fixed schemas:

1. `StarfieldMaterial`
2. `SpaceBackgroundMaterial`

Decision:

1. keep these as acceptable fixed schemas for now,
2. continue driving their instances and layer composition from Lua-authored render-layer definitions.

### 3.2 World sprite family

Purpose:

1. generic lit sprites
2. pixel sprites
3. asteroid sprite path
4. simple world-object shaders
5. damage/palette variants where they fit a shared sprite ABI

Target:

1. one generic world-sprite family,
2. shader-family dispatch centralized in runtime shader registry,
3. Lua-authored asset/shader selection.

Implementation progress:

1. Runtime shader installation for the world-sprite family is now driven by catalog/cache shader bytes plus family emergency fallback, not per-content compiled-in WGSL sources.
2. The client runtime shader registry now resolves streamed shader bytes by authoritative `shader_asset_id` entries rather than role/singleton lookup metadata.
3. Tactical/default presentation fallbacks are no longer hardcoded to a ship icon in the client; default tactical icon selection is authored through replicated `TacticalPresentationDefaults`.

### 3.3 Planet visual family

Purpose:

1. planet body
2. planet clouds
3. planet rings
4. atmospheric rim
5. possibly star bodies if the sphere-style ABI remains compatible

Decision:

1. collapse `PlanetBodyMaterial`, `PlanetCloudMaterial`, and `PlanetRingMaterial` into one `PlanetVisualMaterial` family.
2. use one shared Rust uniform ABI for planet-style spherical visuals.
3. distinguish sub-modes in shader/runtime payload by pass kind / flags, not by separate Rust material types.
4. keep atmosphere rim inside the same planet family rather than treating it as a separate Rust material type.

Implementation progress:

1. Canonical runtime shader asset is now `planet_visual_wgsl`.
2. Lua-authored planet visual stacks now reference `planet_visual_wgsl` for body, cloud, and ring passes.
3. The client now uses one `PlanetVisualMaterial` for body, cloud, and ring passes.
4. The obsolete `PlanetBodyMaterial` / `PlanetCloudMaterial` / `PlanetRingMaterial` trio and their separate WGSL files are removed.

Reason:

1. those current materials already share the same Rust-side uniform schema conceptually,
2. their separation is primarily a Bevy type-static shader binding artifact,
3. atmosphere rim is also part of the same authored planet visual stack and should share that contract,
4. this is the cleanest next material-family collapse target.

### 3.4 Effect family

Purpose:

1. thruster plumes
2. engine glow
3. impact sparks
4. explosions
5. smoke plumes
6. shockwaves
7. projectile trails
8. beams
9. shields
10. selection highlights
11. radar sweep style world-space effects

Decision:

1. define a generic `RuntimeEffectMaterial` family rather than adding one Rust material per effect.
2. treat it as one engine family with a small number of deliberate ABI variants, not one giant catch-all schema.
3. new effect work should target this family once its ABI is defined.

Accepted effect ABI variants (directional):

1. `EffectBillboard`
   - thrusters
   - smoke
   - sparks
   - small explosions
2. `EffectBeamTrail`
   - beams
   - projectile trails
   - streaks
3. `EffectField`
   - shields
   - shockwaves
   - radial pulses
   - selection/radar rings

Shared runtime packing direction:

1. `effect_kind`
2. `time / age / lifetime`
3. `params_a`
4. `params_b`
5. `params_c`
6. `color_a`
7. `color_b`
8. `color_c`
9. optional world-lighting block
10. optional texture bindings

Authoring rule:

1. Lua should author named, validated parameters for each effect variant.
2. Rust should pack those validated parameters into internal `params_a` / `params_b` / `params_c` style uniform blocks.
3. Lua must not author arbitrary untyped parameter bags.

Migration order inside this family:

1. `ThrusterPlumeMaterial`
2. `WeaponImpactSparkMaterial`
3. future explosions/smoke/shockwave effects

Implementation progress:

1. Canonical effect-family shader asset is now `runtime_effect_wgsl`.
2. `EffectBillboard` is implemented for thruster plume and weapon impact spark.
3. `EffectBillboard` now also includes an initial explosion burst variant.
4. `EffectBeamTrail` is implemented for weapon tracers.
5. `EffectField` remains a planned base ABI category only; there is no hardcoded example-specific field consumer in the client.
6. Tactical overlay remains outside this family as a separate fixed schema.
7. `ThrusterPlumeMaterial` and `WeaponImpactSparkMaterial` are now collapsed into `RuntimeEffectMaterial`.
8. Weapon tracer visuals no longer use a legacy sprite path; they now use the `RuntimeEffectMaterial` beam/trail variant.

### 3.5 Screen overlay family

Purpose:

1. tactical map overlay
2. scanning overlays
3. HUD-space radar effects
4. future view-scoped full-screen overlays that are not background/post-process

Current accepted state:

1. tactical overlay orchestration is now on a generic runtime screen-overlay pass path,
2. `TacticalMapOverlayMaterial` may remain a fixed schema for now.

Decision:

1. tactical overlay is acceptable as a retained fixed schema until a broader shared screen-overlay ABI is justified.

### 3.6 Post-process family

Purpose:

1. bloom
2. distortion
3. vignette
4. color grading
5. future world-view post effects

Decision:

1. continue toward Lua-authored ordered post-process stacks,
2. keep the family bounded and separate from fullscreen background composition.

## 4. What Lua Must Own

Lua/content authoring should own:

1. render-layer definitions
2. render-layer rules
3. render-layer overrides
4. post-process stacks
5. world visual stacks
6. shader asset IDs
7. params asset IDs or validated inline params
8. texture bindings by logical asset ID
9. pass ordering, scale multipliers, depth bias, enabled flags
10. content presets and effect spawn templates

Lua/content authoring should prefer:

1. named validated parameters per family/variant,
2. not raw engine-packed `params_a` / `params_b` / `params_c` fields.

Lua must not own:

1. Bevy material type registration
2. bind group layout definitions
3. raw render graph wiring
4. unvalidated direct ECS render internals
5. arbitrary shader ABI shape changes at runtime

## 5. What Rust Must Own

Rust/engine runtime owns:

1. family ABI definitions
2. validation of authored shader/material compatibility
3. replication/persistence contracts for visual components
4. shader registry and family classification
5. fail-soft fallback behavior
6. budgeting and compile/load observability
7. camera/view composition rules

## 6. Current Keep / Migrate Split

### 6.1 Keep for now

The following fixed schemas are acceptable to keep for now:

1. `StarfieldMaterial`
2. `SpaceBackgroundMaterial`
3. `TacticalMapOverlayMaterial` (provisionally accepted)

### 6.2 Migrate

The following are current migration targets:

1. future billboard effects not yet moved onto `RuntimeEffectMaterial`
2. future beam/trail effect schemas beyond weapon tracers
3. field/radial effect schemas

Priority order:

1. planet trio collapse is complete,
2. the first effect-family ABI slice (`EffectBillboard`) is complete,
3. the second effect-family ABI slice (`EffectBeamTrail`) is started and weapon tracers are migrated,
4. next: define the field ABI variant,
5. add future effect shaders into those shared families instead of new schemas.

## 7. Migration Order

### Step 1: Planet family collapse

1. add a unified `planet_visual.wgsl` path,
2. replace `PlanetBodyMaterial` / `PlanetCloudMaterial` / `PlanetRingMaterial` with one `PlanetVisualMaterial`,
3. preserve existing authored `RuntimeWorldVisualStack` pass model,
4. use pass kind / uniform flags to select body/cloud/ring/atmosphere-rim behavior.

Status: Completed on 2026-03-07

### Step 2: Generic effect family definition

1. document and implement `RuntimeEffectMaterial` as one family with a small number of intentional ABI variants,
2. keep it broad enough for thrusters, sparks, explosions, smoke, shockwaves, trails, and shields,
3. keep the variants small enough that validation stays clean and WGSL branching stays understandable,
4. avoid creating one-off new Rust material schemas for those effects.

### Step 3: Effect family migration

1. move `ThrusterPlumeMaterial` to the effect family,
2. move `WeaponImpactSparkMaterial` to the effect family,
3. add new effect shaders through the same family instead of new schemas.

Status: First slice completed on 2026-03-07 via `RuntimeEffectMaterial` + `runtime_effect.wgsl` for `EffectBillboard`.

Status: Second slice partially completed on 2026-03-07 via `RuntimeEffectMaterial` + `runtime_effect.wgsl` for `EffectBeamTrail`, with weapon tracers migrated.

### Step 4: Reassess tactical overlay

1. keep `TacticalMapOverlayMaterial` if its uniform ABI remains unusually specific,
2. only collapse it later if a real shared screen-overlay ABI emerges.

## 8. Consequences

Positive:

1. fewer Rust material types
2. cleaner Lua/Rust ownership split
3. better scalability as new effects are added
4. less content-specific orchestration in client systems
5. more explicit, documentable runtime family ABI
6. better author ergonomics because Lua-facing parameters remain named and validated

Negative:

1. requires deliberate family ABI design up front
2. some WGSL files will become broader/more mode-driven
3. not every material type can disappear because Bevy `Material2d` remains type-static
4. effect-family sub-variants still need careful discipline to avoid becoming an incoherent catch-all

## 9. Explicit Non-Goal

This decision does not require forcing every visual into one universal shader/material.

The correct target is:

1. a small number of stable families,
2. not one family per effect,
3. and not one universal material that becomes an unmaintainable catch-all.

## 10. Follow-up

1. Implement planet family collapse first.
2. Add a dedicated effect-family ABI doc once Step 2 begins.
3. Keep `docs/plans/dynamic_runtime_shader_material_plan.md` aligned with actual migration progress.

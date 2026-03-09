# Sun Prism Post-Process Plan

Status: Proposed
Date: 2026-03-09
Owners: client rendering + scripting + asset streaming

Primary references:
- `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
- `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`
- `docs/plans/dynamic_runtime_shader_material_plan.md`
- `docs/plans/rendering_optimization_pass_plan.md`
- `bins/sidereal-client/src/native/backdrop.rs`
- `bins/sidereal-client/src/native/lighting.rs`

## 1. Goal

Add a camera-scoped fullscreen post-process effect that produces a circular rainbow/prism arc driven by a nearby local star body ("sun") in the current scene.

The target visual is:

1. screen-space and fullscreen-scoped,
2. positioned relative to the active local sun's screen direction,
3. ring/arc based rather than sprite based,
4. soft, spectral, and atmospheric rather than a HUD decal,
5. independent from the sun body's own planet shader.

This effect should behave like a world post-process pass, not a world entity visual stack and not a modification of the sun planet shader itself.

## 2. Why It Belongs In Post-Process

This effect is view-dependent. Its shape depends on:

1. the camera,
2. the active sun position relative to the camera,
3. screen-space angle and distance from the sun,
4. perceived atmospheric/prismatic lens behavior.

That matches the existing `post_process` phase contract from DR-0027 better than:

1. a planet child-pass,
2. a fullscreen background layer,
3. a world-space billboard attached to the sun.

Using the post-process phase keeps the effect:

1. camera-scoped,
2. ordered after world composition,
3. easy to disable or stack,
4. unaffected by world-entity culling rules.

## 3. Current Constraints

The current runtime already supports authored `RuntimePostProcessStack` data, but the active client implementation still routes post-process execution through a limited set of fixed fullscreen material families in `bins/sidereal-client/src/native/backdrop.rs`.

That means the practical implementation path is:

1. add one new fixed Rust-owned fullscreen/post-process material schema for the sun prism effect,
2. expose it through a new shader slot and shader asset ID,
3. let Lua/authored data enable it through `RuntimePostProcessStack`,
4. continue treating it as an incremental exception under the DR-0027/DR-0029 migration model, similar to the existing retained fullscreen schemas.

This avoids inventing a parallel rendering path while still respecting the current material-family reality.

## 4. High-Level Runtime Model

### 4.1 Effect source

The effect is driven by the nearest relevant local star body already available to the client through replicated `PlanetBodyShaderSettings` with `body_kind = 1`.

The client should derive:

1. active sun world position,
2. sun apparent screen position,
3. sun screen-space radius or apparent angular size proxy,
4. distance/strength falloff,
5. whether the sun is far enough inside or outside the view bounds to justify the prism arc.

### 4.2 Effect destination

The effect is rendered as a `post_process` fullscreen pass on the existing post-process render layer/camera path.

The shader should receive:

1. viewport size,
2. time,
3. active sun screen position,
4. apparent sun radius,
5. prism intensity,
6. ring radius,
7. ring thickness,
8. chromatic spread,
9. soft-noise breakup controls,
10. enable/fade controls.

### 4.3 Initial visual behavior

The first implementation should aim for:

1. one primary rainbow arc opposite or offset from the sun direction,
2. a circular/partial circular spectral ring,
3. soft spectral banding with red outer / blue inner behavior,
4. subtle shimmer/noise breakup,
5. optional ghosting streaks later, not in v1.

## 5. Proposed Data Model

### 5.1 New settings component

Add a new replicated/persisted settings component in `crates/sidereal-game/src/components/`:

`SunPrismPostProcessSettings`

Recommended fields:

1. `enabled: bool`
2. `intensity: f32`
3. `arc_radius: f32`
4. `arc_thickness: f32`
5. `chromatic_spread: f32`
6. `edge_softness: f32`
7. `noise_strength: f32`
8. `noise_scale: f32`
9. `sun_influence_radius: f32`
10. `offscreen_bias: f32`
11. `min_sun_intensity: f32`
12. `max_screen_alpha: f32`

Reason:

1. authored tuning belongs in shared replicated component data,
2. the post-process stack should decide pass composition,
3. this component should decide effect behavior.

### 5.2 Authoring path

Author the effect from Lua world/bootstrap data by:

1. adding a small bundle or direct world-init record for `SunPrismPostProcessSettings`,
2. adding a `RuntimePostProcessStack` pass using a dedicated shader asset ID such as `sun_prism_post_wgsl`.

This keeps:

1. pass composition in `RuntimePostProcessStack`,
2. effect settings in a dedicated component,
3. shader bytes in `data/shaders/`.

## 6. Client Implementation Plan

### Phase 1: Shader/material ABI

Files:

1. `data/shaders/sun_prism_post.wgsl`
2. `bins/sidereal-client/src/native/shaders.rs`
3. `bins/sidereal-client/src/native/backdrop.rs`

Work:

1. Add a new runtime fullscreen/post-process shader slot and handle for `sun_prism_post_wgsl`.
2. Add a new fixed material schema, likely `SunPrismPostProcessMaterial`.
3. Extend fullscreen/post-process material selection so `RuntimePostProcessStack` can resolve this shader asset ID.
4. Keep source/cache shader path parity in the same change.

Shader contract:

1. fullscreen quad input,
2. no dependency on UI layers,
3. transparent compositing over the world result,
4. spectral ring generated from one derived sun source.

### Phase 2: Sun source derivation

Files:

1. `bins/sidereal-client/src/native/backdrop.rs`
2. `bins/sidereal-client/src/native/lighting.rs`
3. `bins/sidereal-client/src/native/components.rs` or `resources.rs` if a shared resource is cleaner

Work:

1. Add a client-only derived resource for the active post-process sun source, for example `SunPrismSourceState`.
2. Select from replicated star bodies (`PlanetBodyShaderSettings.body_kind == 1`).
3. Project the chosen star into screen space using the gameplay camera.
4. Derive apparent radius from world size + camera zoom/path already used by planet visuals.
5. Fade out when:
   - no star is present,
   - the star is too small,
   - the star is too centered for the desired effect,
   - the authored settings disable the pass.

Selection rule for v1:

1. choose the nearest visible or most screen-relevant star,
2. do not attempt multi-star blending yet,
3. expose future multi-star support as follow-up work.

### Phase 3: Material update system

Files:

1. `bins/sidereal-client/src/native/backdrop.rs`
2. `bins/sidereal-client/src/native/plugins.rs`

Work:

1. Add a material update system similar to the existing fullscreen material update systems.
2. Feed viewport, time, and `SunPrismSourceState` into the new material each frame.
3. Bind the authored `SunPrismPostProcessSettings` values into uniforms.
4. Ensure the pass executes after world composition and before UI composition.

Important:

1. reuse existing fullscreen/post-process renderables where possible,
2. do not introduce fresh per-frame mesh/material churn,
3. align with `docs/plans/rendering_optimization_pass_plan.md`.

### Phase 4: Script/bootstrap wiring

Files:

1. `data/scripts/world/world_init.lua`
2. possibly `data/scripts/bundles/starter/` if this becomes a reusable bundle

Work:

1. Add default authored settings for the prism effect.
2. Add a `RuntimePostProcessStack` pass referencing `sun_prism_post_wgsl`.
3. Keep the effect easy to disable or tune from world bootstrap data.

## 7. Shader Design Notes

The shader should not sample the sun sprite/body directly. It should derive the arc from screen-space geometry relative to the active sun.

Recommended v1 visual model:

1. compute vector from current pixel to sun screen position,
2. derive ring center/radius for the prism arc,
3. shape one or more soft annular bands,
4. map band coordinate to spectral color,
5. add subtle turbulent breakup and atmospheric feathering,
6. alpha-composite lightly over the final scene color.

Important guardrails:

1. avoid a perfectly uniform rainbow decal,
2. avoid hard circular UI-like outlines,
3. avoid requiring an atmosphere simulation on all worlds,
4. keep the effect subtle enough that stars still dominate visually.

## 8. Recommended Increment Scope

### In scope for v1

1. single-star driven effect,
2. one authored post-process pass,
3. one fixed material schema,
4. one settings component,
5. one fullscreen/post-process shader,
6. native validation first.

### Out of scope for v1

1. true physical refraction or volumetric scattering,
2. multiple simultaneous rainbow sources,
3. occlusion against planets/ships,
4. UI-space or tactical-map variants,
5. generic arbitrary post-process family migration.

## 9. Testing and Validation Plan

### Automated

1. unit tests for `SunPrismPostProcessSettings` default parsing/serde roundtrip,
2. validation coverage for authored `RuntimePostProcessStack` entries using the new shader asset ID,
3. client compile checks for native and, if touched by shared code paths, WASM compile impact review.

### Manual

1. verify no effect when no star exists,
2. verify smooth prism arc when one star is near the edge of view,
3. verify fade behavior as camera pans and zooms,
4. verify effect does not tint UI,
5. verify multiple stars do not produce unstable source selection in v1,
6. verify post-process ordering with existing backdrop/fullscreen passes.

## 10. Documentation Updates Required During Implementation

When implementation begins, update:

1. `docs/features/scripting_support.md`
2. `docs/plans/dynamic_runtime_shader_material_plan.md`
3. `AGENTS.md` only if contributor rules change

If the fixed-schema exception becomes a lasting architectural decision rather than an incremental bridge, add or update a decision detail doc under `docs/decisions/`.

## 11. Open Questions

1. Should the prism arc appear opposite the sun direction like a rainbow analogue, or more like an internal lens/prism halo closer to the sun? The reference implies the former.
2. Should source selection use nearest star, brightest apparent star, or most on-screen star? For v1, "most screen-relevant visible star" is likely the least surprising.
3. Should the effect be tied only to stars with `body_kind = 1`, or should future authored bodies be allowed to opt in explicitly?
4. Should the pass be globally authored once per world, or attached to camera-context entities later when camera profiles become more formal?

## 12. Recommended First Implementation Order

1. Add `SunPrismPostProcessSettings` component and tests.
2. Add `sun_prism_post.wgsl` and runtime shader slot.
3. Add `SunPrismPostProcessMaterial` and post-process material routing.
4. Add client derived sun-source resource and projection logic.
5. Add material update system.
6. Author one default `RuntimePostProcessStack` pass in `world_init.lua`.
7. Validate native runtime behavior and document WASM impact.

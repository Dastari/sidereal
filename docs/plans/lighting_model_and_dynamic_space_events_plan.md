# Lighting Model and Dynamic Space Events Plan

Status: Active implementation contract and phased plan
Date: 2026-03-06
Owners: client rendering + gameplay runtime + asset/shader authoring

Update note (2026-03-10):
- The core lighting direction in this document remains current: Sidereal still uses a client-derived shared world-lighting resource consumed by `Material2d` world shaders rather than Bevy 3D PBR as the primary gameplay-world lighting model.
- This is no longer a speculative plan in several areas. `EnvironmentLightingState`, `WorldLightingState`, shared lighting uniforms, star-position-derived primary light resolution, and thruster-seeded local emitters are implemented in the native client.
- The document's oldest "immediate next steps" list is partly stale. The remaining material gaps are now narrower: ships still need their lit-world-sprite upgrade path, local-emitter coverage should expand beyond thrusters to impacts/muzzle flashes/explosions, and backdrop radiance plus dynamic event inputs still need to feed the shared lighting state.
- Conclusion: keep this document as the active lighting contract, but treat it as an implementation-status document that needs continued dated progress notes rather than a ground-up redesign.

Update note (2026-04-27):
- Asteroid world sprites now consume generated normal-map textures in `AsteroidSpriteShaderMaterial` alongside the shared world-lighting uniform. This completes the asteroid-specific bump-lighting proof point described below without adding a separate lighting authority path. Native impact: asteroid materials use albedo + linear normal map + shared lighting. WASM impact: the material binding contract is shared and must remain in shader/cache parity for browser builds.
- Starter environment-lighting defaults now use a lower primary elevation, warmer key color, reduced ambient fill, and weaker backlight so 2D material normal response is visibly directional instead of reading as static flat fill. Native impact: asteroids and planets should show stronger lit/shadowed faces from the active star direction. WASM impact: data defaults and shared replicated component defaults changed only; no platform-specific path.

Update note (2026-04-28):
- Lighting V2 is now tracked as `system.lighting.v2` with a dedicated implementation plan in `docs/plans/lighting_v2_overhaul_plan.md` and decision record `docs/decisions/dr-0038_lighting_v2_material_contract.md`. V2 supersedes the current one-primary/one-local material ABI with top-2 stellar lights, authored stellar falloff, deep-space ambient, and top-8 dynamic local emitters. Native impact: all world-facing materials must migrate to the V2 uniform contract or document an emissive/UI exemption. WASM impact: shader source, streamed cache, and published cache must remain in V2 binding parity.

Primary references:
- `docs/sidereal_design_document.md`
- `docs/features/asset_delivery_contract.md`
- `docs/features/scripting_support.md`
- `docs/features/procedural_asteroids.md`
- `docs/features/procedural_planets.md`
- `docs/features/visibility_replication_contract.md`

## 1. Why This Must Be Baked In Early

Sidereal is no longer a simple sprite stack. The project already has:

1. fullscreen background materials,
2. streamed sprite materials,
3. procedural asteroid materials,
4. procedural planet materials,
5. thruster plume materials,
6. tactical-map overlays,
7. a growing set of authored visual payloads coming from Lua.

If lighting is added late, every one of those paths will need retrofitting with inconsistent assumptions. The correct move is to define the 2.5D lighting contract now and make all future world-facing materials conform to it.

This document is the render contract for that work.

## 2.1 Current Implementation Status

Implemented now:

1. `EnvironmentLightingState` exists as a canonical public replicated/persisted ECS component.
2. Lua owns the bootstrap/default environment-lighting payload through `environment.lighting`.
3. The client derives a shared `WorldLightingState` resource from replicated ECS state.
4. If a replicated `PlanetBodyShaderSettings` entity with `body_kind = 1` is present, the client resolves direct-light direction per rendered entity from that star's world position instead of a single shard-global direction.
5. Planets, asteroid sprite materials, and thruster plumes now consume shared world-light uniforms instead of only private shader light constants.
6. A first client-derived `LocalLightEmitter` collection path exists, seeded from thruster plume visuals.
7. One bounded dominant local-light contribution is now resolved per rendered entity and folded into planet, asteroid, ring, cloud, and plume materials.

Not done yet:

1. bounded multi-emitter accumulation is not yet injected into all world materials; current runtime resolves one dominant nearby local light per rendered entity,
2. ships still need their shared lit material upgrade path,
3. backdrop radiance is not yet folded automatically into the derived world-light state,
4. dynamic space-event descriptors are still planned, not implemented.

## 2. Scope

This lighting model covers the **world render stack** only:

1. ships and other sprite-like entities,
2. asteroids,
3. planets and moons,
4. thruster plumes,
5. weapon tracers / impacts / explosions,
6. fullscreen backdrop radiance and dynamic space events.

It does **not** make lighting authoritative gameplay state by default. Lighting is presentation unless a future gameplay feature explicitly declares otherwise.

## 3. Core Decision

Sidereal uses a **2.5D material-driven lighting model** built on Bevy `Material2d` and client-side lighting resources.

The lighting stack is:

1. `Background Radiance`
   - derived from fullscreen backdrop layers and solar-system environment state.
2. `Global Stellar Illumination`
   - one primary sun/key light plus optional low-cost secondary fill.
3. `Local Dynamic Emitters`
   - ships, thrusters, impacts, muzzle flashes, explosions, special events.
4. `Material Response`
   - each world material computes its own shading from a shared lighting contract.
5. `Post / Glow`
   - optional later phase, but fed from the same emissive model.

The key architectural rule is:

**all world materials sample the same lighting inputs, but each material remains responsible for its own surface response.**

That means:

1. backdrop shaders do not directly light ships,
2. ships do not use Bevy 3D PBR,
3. planets do not need a separate rendering architecture,
4. asteroid normals, planet normals, and ship normals all feed one shared model.

## 4. Current Render Architecture Constraints

The lighting plan must fit the architecture that already exists:

1. `BackdropCamera` renders fullscreen layer materials.
2. `GameplayCamera` renders world entities and dedicated lower-z layers such as `PLANET_BODY_RENDER_LAYER`.
3. World visuals are mostly child renderables attached below replicated ECS entities.
4. Most active world materials are `Material2d` fragment shaders.
5. Native and WASM builds must stay in lockstep.

This means the lighting model must avoid:

1. dependence on full Bevy 3D PBR mesh pipelines for ordinary gameplay objects,
2. per-object heavy shadow maps,
3. platform-specific render graph branches that only work on native.

## 5. Non-Negotiable Rules

1. Lighting must never write authoritative simulation state.
2. Dynamic lights are client-derived from replicated world state or replicated compact events, not replicated one-by-one as expensive render objects.
3. Asset/shader references remain logical `asset_id` values authored in Lua registries.
4. Any shared lighting data needed by multiple materials must be exposed through stable material uniforms/resources, not ad-hoc shader-local constants.
5. New world-facing shaders must join this lighting contract rather than invent private lighting logic.
6. Native and WASM must keep identical material inputs and high-level lighting behavior.

## 6. Lighting Layers

### 6.1 L0: Background Radiance

Fullscreen backdrop layers already establish the visual mood of the scene. They should also provide the world with low-frequency radiance information.

Background radiance is not direct illumination. It is:

1. ambient color bias,
2. soft backlight/rim bias,
3. event-driven flash tint,
4. low-frequency fill for shadows.

Sources:

1. `SpaceBackgroundShaderSettings`
2. `StarfieldShaderSettings`
3. future solar-system environment presets
4. future dynamic space events (`ion_storm`, `solar_arc`, `pulsar_sweep`)

Output contract:

1. `ambient_color_rgb`
2. `ambient_intensity`
3. `backlight_color_rgb`
4. `backlight_screen_or_world_direction`
5. `event_flash_color_rgb`
6. `event_flash_intensity`

This output should be computed once per frame into a shared lighting resource, not recomputed separately in every world material from scratch.

### 6.2 L1: Global Stellar Illumination

The world needs one coherent key light.

The correct model is:

1. one primary stellar light sourced from a replicated star body when available,
2. optional secondary fill light later,
3. no dependence on Bevy 3D `DirectionalLight` for the actual 2D material response.

The existing Bevy `DirectionalLight` can remain for any engine subsystems that still need it, but the real world-lighting contract should be a client-side lighting resource consumed by `Material2d` shaders.

Primary stellar light state:

1. `source_position_xy` when a star body exists
2. fallback `direction_xy` when no star body exists yet
3. `elevation`
4. `color_rgb`
5. `intensity`
6. `wrap`
7. `shadow_softness`

This is what ships, asteroids, and planets should use for:

1. diffuse term,
2. specular highlight direction,
3. rim shaping,
4. drop-shadow direction later,
5. night/day hemisphere response on planets.

### 6.3 L2: Local Dynamic Emitters

Local emitters are short-lived or object-bound lights near gameplay entities.

Examples:

1. ship running lights,
2. exhaust plumes,
3. afterburner plumes,
4. muzzle flashes,
5. beam cores,
6. tracer impacts,
7. explosion cores,
8. EMP arcs,
9. special environment anomalies.

These are not independent replicated render entities. They should come from a shared client-side emitter model driven by already replicated state.

Emitter shapes:

1. point
2. cone
3. capsule / line segment
4. disk / halo

Emitter fields:

1. source entity or world position
2. color
3. intensity
4. radius
5. direction
6. cone angle or segment length
7. ttl / fade
8. category / budget class

### 6.4 L3: Emissive / Glow

Emissive is not the same thing as a local light.

Examples:

1. a lava seam on a planet is emissive,
2. a bright engine core is emissive,
3. a muzzle flash both emits light and is emissive,
4. neon UI is emissive but not world-lighting.

The world material contract should separate:

1. `surface_lighting_response`
2. `emissive_output`

This allows later bloom/post to consume emissive cleanly without rewriting surface shaders again.

## 7. Shared World Lighting Contract

Every lit world material should conceptually receive the same lighting inputs:

1. `ambient_color_rgb + ambient_intensity`
2. `backlight_color_rgb + backlight_intensity`
3. `stellar_direction_xy + elevation + color + intensity`
4. `event_flash_color_rgb + intensity`
5. a bounded local-light accumulation result

Every world material should provide or derive:

1. `albedo`
2. `alpha`
3. `normal`
4. `roughness_like scalar`
5. `metallic/specular bias` if needed
6. `height/depth cue` if needed for shadow offset
7. `emissive`

This does **not** mean all materials share one Rust struct today. It means all new materials must be designed so they can accept these concepts without architectural churn.

## 8. Material-Specific Responsibilities

### 8.1 Ships and Ordinary Sprites

Ships will eventually need:

1. optional normal map asset or generated normal map,
2. optional lighting profile,
3. optional running-light emitters,
4. optional material mask for emissive windows/engines.

Recommended contract additions:

1. `NormalMapAssetId` optional
2. `MaterialLightingProfileId` optional
3. `LightEmitterProfile` optional

Ship shaders should not stay as plain unlit image shaders long-term.

### 8.2 Asteroids

Asteroids already generate:

1. albedo,
2. normal map,
3. deterministic silhouette.

So asteroids should be the first material migrated fully into the shared lighting contract. They are the easiest proof that the model works on procedural content.

### 8.3 Planets

Planets already do local procedural bump lighting in-shader.

Next phase for planets is not “invent more private lighting.” It is:

1. replace planet-local ad-hoc light constants with shared environment lighting inputs,
2. let atmosphere and rim terms react to global radiance and stellar direction,
3. optionally add separate atmosphere/rings child passes that consume the same shared lighting state.

### 8.4 Thruster Plumes

Thruster plumes should both:

1. render emissive color,
2. register local dynamic light emitters.

They are the first obvious gameplay-linked local-light source and should become the seed of the emitter system.

### 8.5 Weapon / Impact Effects

Weapon tracer and impact effects should feed:

1. emissive render output,
2. short-lived pulse emitters.

The important rule is that the client derives these from existing replicated weapon-fire and impact events. No extra authoritative light replication lane is needed.

## 9. How To Implement This With Bevy

## 9.1 Do Not Bet the Design on Bevy 3D Lights

Bevy `DirectionalLight` / `PointLight` are not the correct primary abstraction for this game's world lighting.

Reasons:

1. the world is mostly 2D `Material2d`,
2. planets are rendered as 2D spheres on quads,
3. asteroids and plumes are custom fragment shaders,
4. we need deterministic cross-material visual behavior, not partial engine-default lighting.

Bevy 3D lights can still exist for limited engine support, but the real contract should be material uniforms/resources we control.

## 9.2 Recommended Bevy Implementation Shape

Phaseable implementation:

1. Add a client-side `WorldLightingState` resource.
2. Update it each frame from:
   - current solar-system or shard environment,
   - fullscreen backdrop radiance values,
   - active dynamic space events.
3. Add a client-side `LocalLightEmitter` collection/resource updated from:
   - thruster plume visuals,
   - weapon tracer/impact systems,
   - future ship running lights.
4. Build a bounded local-light accumulation representation:
   - either screen-space low-res texture,
   - or camera-local clustered/tiled array uploaded to materials.
5. Update all lit world materials to sample:
   - `WorldLightingState`,
   - bounded local-light data.

## 9.3 Preferred v1 Data Path

For this project, the safest v1 is:

1. `WorldLightingState` resource
2. camera-local bounded array of top N local emitters
3. per-material shading using those uniforms

This is cheaper and simpler than introducing a full custom render-graph light buffer immediately.

Later, if needed:

1. move to a low-resolution light accumulation texture,
2. then keep the same logical lighting contract while changing the backend implementation.

## 10. Global Illumination Strategy

Do not attempt true GI in v1.

Instead, use a cheap GI-like approximation:

1. ambient radiance from backdrop/environment,
2. backlight/rim tint from nebula/starfield mood,
3. optional event flash injection,
4. optional very low-resolution local-light accumulation later.

This gives cohesion without full bounce-light simulation.

The term “global illumination” in Sidereal should mean:

1. scene-wide ambient and backlight consistency,
2. not physically correct recursive light transport.

## 11. Dynamic Space Events

Space events are first-class inputs to lighting.

They should not be treated as one-off shader gimmicks.

Event families:

1. `solar_flare`
2. `ion_storm`
3. `nebula_lightning`
4. `pulsar_sweep`
5. `plasma_wind`
6. `gravitational_lensing_wave`

Each event can affect one or more layers:

1. backdrop-only distortion,
2. ambient radiance tint,
3. global key-light pulse,
4. local regional emitter clusters,
5. optional audio hooks.

Recommended state model:

1. compact replicated or deterministic event descriptor,
2. client-side interpolation and visual realization,
3. shared injection into `WorldLightingState`.

## 12. Proposed Data Model

### 12.1 Environment / Stellar Lighting

Add a shared replicated public component or singleton-style entity:

`EnvironmentLightingState`

Suggested fields:

1. `primary_direction_xy`
2. `primary_elevation`
3. `primary_color_rgb`
4. `primary_intensity`
5. `ambient_color_rgb`
6. `ambient_intensity`
7. `backlight_color_rgb`
8. `backlight_intensity`
9. `event_flash_color_rgb`
10. `event_flash_intensity`

Why entity-based instead of resource-only:

1. it fits the repo's ECS/persistence model better,
2. it is inspectable in tooling/dashboard,
3. it can be replicated through the existing component pipeline.

The client can still materialize a derived non-replicated resource from it for efficient render updates.

### 12.2 Local Emitters

Add a runtime-only client component/resource family:

1. `LocalLightEmitter`
2. `ResolvedLocalLight`
3. `CameraLocalLightSet`

These should remain client-side derived state unless a future gameplay feature needs explicit replicated event descriptors.

### 12.3 Material Lighting Profiles

Add authorable content hooks for material response:

1. `MaterialLightingProfileId`
2. optional `NormalMapAssetId`
3. optional `EmissiveMaskAssetId`

This keeps shader behavior data-driven instead of hardcoded by entity type forever.

## 13. Render Layers and Depth Policy

Lighting must respect the current layered 2.5D world:

1. backdrop layer
2. planet body layer
3. ordinary world entity layer
4. UI overlay layer

Rules:

1. backdrop contributes radiance but is not lit by local ship lights,
2. planets receive global lighting and selected event flashes,
3. ordinary world entities receive global + local lighting,
4. UI is not part of world lighting.

This prevents accidental cross-layer coupling later.

## 14. Performance Policy

The lighting model must be budgeted from day one.

### 14.1 Emitter Budgets

Per camera:

1. very small always-on lights budget for ships,
2. medium transient budget for combat,
3. distant emitters culled aggressively,
4. top-N selection by importance score.

Importance score factors:

1. screen-space size,
2. intensity,
3. distance to camera,
4. category priority,
5. ownership or player-local bias.

### 14.2 LOD

First things to reduce at distance:

1. local emitters,
2. specular detail,
3. per-pixel normal influence,
4. atmosphere detail,
5. post/glow intensity.

Do not degrade:

1. silhouette readability,
2. primary key-light direction,
3. core faction/combat readability colors.

## 15. Implementation Order

### Phase 1: Contract and Shared State

1. [x] Add `EnvironmentLightingState` component/resource path.
2. [x] Define the shared world-lighting uniforms expected by lit materials.
3. [x] Stop hardcoding private light constants into new materials where avoidable.
4. [x] Document asset IDs and data ownership.

### Phase 2: First Real Consumers

1. [x] Move asteroid material onto shared lighting inputs.
2. [x] Move planet material onto shared lighting inputs.
3. [ ] Introduce a lit ship material path or compatible upgrade path for world sprites.

### Phase 3: Local Emitters

1. [x] Add client-derived local emitter system.
2. [x] Make thruster plumes emit light.
3. [ ] Make weapon impacts / muzzle flashes emit light.
4. [ ] Apply bounded local-light contribution in world materials.

### Phase 4: Shadow / Depth Cues

1. Add cheap projected contact/drop shadows for ships and asteroids.
2. Drive shadow direction from primary stellar light.
3. Tune for readability, not realism.

### Phase 5: Dynamic Space Events

1. Add compact space-event descriptors.
2. Feed them into backdrop radiance and global flashes.
3. Add selective local/regional event emitters where needed.

### Phase 6: Post / Glow

1. Add restrained bloom/emissive composite if still needed.
2. Keep brightness caps and readability constraints explicit.

## 16. Immediate Next Steps

These are the next steps that should follow from the current state of the codebase:

1. Add `EnvironmentLightingState` as a canonical public ECS component or singleton-style entity.
2. Refactor `planet_visual.wgsl` and `asteroid.wgsl` to consume shared environment-light uniforms instead of shader-private light assumptions.
3. Add a client-derived local light emitter path seeded first from thruster plumes.
4. Define the ship lighting upgrade path so ships do not remain the odd one out while planets/asteroids become lit.
5. Fold backdrop radiance and event flashes into the derived `WorldLightingState`.
6. Only after the above, add separate hero-atmosphere/rings or heavier post effects.

## 17. Testing and Validation

Unit tests:

1. lighting state clamping and blending,
2. emitter importance sorting and budgeting,
3. deterministic event-to-light translation.

Integration tests:

1. replicated environment lighting reaches clients,
2. lighting resources update cleanly during world transitions,
3. missing lighting assets fail soft without breaking gameplay visuals.

Build gates:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo check --workspace`
4. `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
5. `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`

## 18. Open Decisions

1. Should `EnvironmentLightingState` be one shard-global entity, one per solar system, or both with local override rules?
2. For v1 local lights, is a bounded uniform array enough or do we go straight to a low-resolution accumulation texture?
3. Do ship running lights belong in authoritative content data now, or only after the generic lit ship material path lands?
4. Which dynamic space events are cosmetic-only in v1, and which eventually become gameplay-affecting?

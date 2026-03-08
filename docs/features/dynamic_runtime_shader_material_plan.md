# Dynamic Runtime Shader Material Plan

Status: Proposed implementation plan
Date: 2026-03-05
Owners: scripting + replication + client rendering + asset streaming

Update note (2026-03-07):
- Layer composition, default world-layer policy, rule-based entity assignment, and render-time parallax semantics are now defined by `docs/features/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`.
- This document remains the shader/material runtime plan, but the newer DR-0027 feature doc is the source of truth for render-layer composition and migration away from fixed fullscreen layer kinds.

Implementation note (2026-03-05):
- Tactical map now uses a fullscreen shader-material path (`TacticalMapOverlayMaterial`) driven by replicated `TacticalMapUiSettings` component data (owner-visible), with shader source parity in both `data/shaders/` and `data/cache_stream/shaders/`. This is an incremental step toward Phase 2/4 generic fullscreen/post-process runtime paths.

Primary references:
- `docs/features/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
- `docs/features/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`
- `docs/features/scripting_support.md`
- `docs/features/asset_delivery_contract.md`
- `docs/sidereal_design_document.md`
- `AGENTS.md`

## 1. Objective

Enable Lua-authored content to drive all runtime shader/material usage (entity, fullscreen background, and world post-process) without adding a new Rust material type per shader and without hardcoded asset path/id maps in runtime code.

Target outcome:
1. Scripts select shader-driven visuals through intent APIs; Rust validates and replicates approved state.
2. Shader source/assets are streamed through the existing asset pipeline and resolved by catalog metadata.
3. Client compiles streamed shader assets at runtime and applies them via pre-registered generic material schemas.
4. No per-shader startup boilerplate and no hardcoded `asset_id -> path` match arms in gateway/client crates.

## 2. Problem Statement

Current runtime has multiple hardcoded shader/material references and IDs that do not scale:
- startup registration and special-case materials for starfield/background/thruster/sprite paths,
- hardcoded shader path lists and ID matchers,
- hardcoded gateway `/assets/stream/{asset_id}` resolver,
- hardcoded default streamable asset source list in `sidereal-asset-runtime`.

This conflicts with the asset-delivery contract's authoritative catalog model and blocks Lua-authored extensibility.

Key constraint:
- Bevy can compile/load new shader assets at runtime.
- Bevy cannot register brand-new Rust material schemas from network/script data at runtime.

Scalable model: **small fixed set of generic runtime material schemas + data-driven shader assets/parameters from authoritative catalog metadata**.

Current implementation note (2026-03-07):
- Runtime shader role install is now centralized through a single `RuntimeShaderSpec` table in the client and missing streamed roles degrade through shared family fallback instead of per-content missing-role logic.
- The client no longer carries compiled-in WGSL fallback sources for each named content shader; shader installation is cache/catalog driven and the only built-in fallback is one emergency shader per runtime family.
- The client runtime shader registry now resolves streamed shader bytes by authoritative `shader_asset_id`/catalog entry rather than any role/singleton dispatch metadata.
- Known fullscreen shader/material selection now resolves through that centralized shader registry instead of raw string comparisons in backdrop sync paths.
- Fullscreen and post-process composition renderables now share one generic runtime fullscreen renderable/component path instead of separate ECS marker families.
- Asteroid world-sprite specialization now resolves through the same runtime shader registry instead of raw `asteroid_wgsl` string checks in streamed visual systems.
- Material shader-handle resolution is now also centralized through the runtime shader registry instead of each material impl reaching into scattered global shader-handle constants.
- Planet, cloud, ring, and thruster child attachment state now uses one generic runtime world-visual pass model instead of dedicated `Planet*Visual*` / `ThrusterPlume*` marker components.
- Planet pass existence and pass transform metadata are now authored through replicated `RuntimeWorldVisualStack` data rather than inferred client-side from `PlanetBodyShaderSettings`.
- The remaining hardcoded area is now concentrated at the material schema boundary itself: several content families still map to dedicated Rust `Material2d` types even though shader selection, pass attachment, and readiness are increasingly runtime-driven.

Render composition note:
- Layer phases (`fullscreen_background`, `world`, `fullscreen_foreground`, `post_process`), default `main_world` assignment, rule-based layer routing, and layer parallax are defined in DR-0027 and should not be redefined inconsistently here.
- The immediate next dependency from DR-0027 is authored multi-pass visual stacks for layered content (planets/clouds/rings), followed by tactical overlay migration, and only then deeper removal of the remaining fullscreen/content shader adapters.
- Planet/cloud/ring orchestration is now on the authored visual-stack path; the next dependency is tactical overlay migration, followed by deeper removal of the remaining fullscreen/content shader adapters and dedicated material schemas.
- The remaining dedicated Rust material types are now thin runtime adapter shells with centralized shader-family resolution, rather than each type carrying bespoke shader-handle boilerplate. The unresolved boundary is still the type-static `Material2d` schema requirement itself.
- Tactical overlay orchestration now runs through a generic runtime screen-overlay pass handler rather than a tactical-specific ECS pass, leaving the tactical material schema itself as the remaining specific part.
- Tactical map default icon fallback is now authored through replicated `TacticalPresentationDefaults` data rather than a hardcoded `"map_icon_ship_svg"` client constant.
- Background shader invariant: fullscreen background layers must always render when enabled and must not rely on normal world-space culling/visibility heuristics.

Implementation update (2026-03-08):
- The client no longer installs a hardcoded table of concrete authored shader asset IDs on startup.
- Rust now owns fixed shader handle slots and family/material ABI only; active `shader_asset_id` values are assigned from replicated/authored runtime data (fullscreen layer definitions, world visual usage, and tactical UI settings) before streamed shader reload.
- The previous live mismatch `sprite_pixel_shader_wgsl` vs `sprite_pixel_effect_wgsl` is removed from the active client path.
- Space background flare texture selection now uses authored `flare_texture_asset_id` data instead of a Rust-side `flare_texture_set -> asset_id` map in the active backdrop path.
- Generated asset catalog entries now also carry optional `shader_family` metadata so the client can bind remaining family slots, including the generic runtime effect family, without falling back to exact asset-ID literals.

## 3. Architecture Contract

### 3.1 Authority and scripting boundary

Follow `docs/features/scripting_support.md`:
1. Scripts emit intent only.
2. Scripts do not directly mutate client render internals.
3. Server validates visual intent payloads and persists/replicates approved component state.

### 3.2 Runtime rendering boundary

Client ships a fixed set of generic render schemas:
1. `RuntimeSpriteShaderMaterial2d`
2. `RuntimePolygonShaderMaterial2d`
3. `RuntimeFullscreenShaderMaterial2d`
4. `RuntimePostProcessMaterial`

These are registered once at startup. All per-content shader selection comes from replicated components (`shader_asset_id`, params, textures, pass ordering), not hardcoded type/path branches.

### 3.3 Asset delivery and registration boundary

Follow `docs/features/asset_delivery_contract.md`:
1. Shader and texture artifacts are logical streamed assets resolved through authoritative catalog metadata.
2. Runtime must not depend on standalone HTTP file serving (`/assets/stream/{asset_id}` is migration-only and must be removed).
3. Runtime must not carry manually maintained built-in streamable asset lists.
4. Asset/shader usage declaration is Lua-authored (manual registration now, dynamic usage-driven registration later), but final authorization/expansion remains server-side and catalog-validated.

### 3.4 Native/WASM parity

1. Runtime shader behavior must be equivalent for native and wasm32 clients.
2. Platform-specific divergence is transport boundary only.
3. Shader fallback and validation behavior must match across targets.

## 4. Data Model (Proposed)

### 4.1 Gameplay-facing components (persistable/replicated)

Use `sidereal-game` component workflow for generic visual bindings:

1. `RuntimeShaderBinding2d`
- `shader_asset_id: String`
- `material_domain: RuntimeMaterialDomain` (`Sprite`, `Polygon`)
- `params_asset_id: Option<String>`
- `texture_bindings: Vec<RuntimeTextureBinding>`
- `render_order: i32`

2. `RuntimeFullscreenShaderLayer`
- `shader_asset_id: String`
- `layer_order: i32`
- `params_asset_id: Option<String>`
- `texture_bindings: Vec<RuntimeTextureBinding>`

Layer-routing note:
- `RuntimeFullscreenShaderLayer` should align with the generic `RuntimeRenderLayerDefinition` / phase model in DR-0027 rather than preserving fixed `starfield` / `space_background` kinds.

3. `RuntimePostProcessStack`
- ordered pass list with `shader_asset_id`, params/texture bindings, enabled flag
- applies to world render target only (UI excluded)

### 4.2 Catalog metadata additions

Extend catalog entries with:
1. `shader_domain` compatibility (`world_sprite`, `world_polygon`, `fullscreen`, `post_process`)
2. required binding signature hash
3. optional parameter schema/version
4. optional dependency asset IDs (textures/includes)
5. safety profile tag (`trusted_first_party`, `modded_sandboxed`)
6. no singleton role-dispatch metadata in the authoritative runtime path; shader selection must stay on `shader_asset_id` plus compatibility metadata

## 5. Lua Registration and Intent API

### 5.1 Asset usage declaration

Lua declares content asset usage by logical IDs.

Phase A (manual): explicit registration call during script bootstrap.

Phase B (future): dynamic usage declaration inferred from script world/archetype content with server-side normalization.

Both phases feed the same authoritative registry path; neither bypasses catalog validation.

### 5.2 Runtime visual intents

Examples:

```lua
ctx:emit_intent("set_runtime_shader", {
  entity_id = target_id,
  shader_asset_id = "shader.fx.plasma_edge",
  material_domain = "sprite",
  params = { edge_strength = 0.8 }
})

ctx:emit_intent("set_fullscreen_shader_layer", {
  layer_entity_id = layer_id,
  shader_asset_id = "shader.bg.starfield_v2",
  layer_order = -190,
  params = { speed = 1.2 }
})

ctx:emit_intent("set_post_process_stack", {
  camera_entity_id = camera_id,
  passes = {
    { shader_asset_id = "shader.post.warp", enabled = true },
    { shader_asset_id = "shader.post.tint", enabled = true }
  }
})
```

Validation rules:
1. IDs must exist in catalog and be authorized for session/content scope.
2. `material_domain` must match shader metadata.
3. params must satisfy schema/range.
4. forbidden bindings/features are rejected server-side.

## 6. Client Runtime Flow

1. Replication updates runtime shader binding/fullscreen/post-process components.
2. Client resolves referenced assets by catalog-driven cache path (no hardcoded ID map).
3. Missing/invalid assets use deterministic shared fallback; if no safe fallback exists, the renderable remains unrendered.
4. When assets arrive:
- compile/load shader,
- build/update material/pass instance,
- bind params/textures,
- atomically swap into render path.
5. On compile failure:
- keep shared family fallback,
- or keep entity/layer/pass unrendered when fallback is unavailable,
- emit telemetry,
- use persistent dialog only when policy marks failure as user-visible critical.

Hard requirement: client runtime must never crash/panic due to missing or invalid streamed assets/shaders/material payloads. The failure mode is always fail-soft rendering degradation (fallback or unrendered), with simulation/networking continuing normally.

## 7. Migration Scope for Existing Hardcoded Paths

The following runtime hardcoded references are migration targets:
1. `bins/sidereal-client/src/native/assets.rs`
2. `bins/sidereal-client/src/native/backdrop.rs`
3. `bins/sidereal-client/src/native/platform.rs`
4. `bins/sidereal-client/src/native/shaders.rs`
5. `bins/sidereal-gateway/src/api.rs`
6. `bins/sidereal-gateway/tests/api_helpers.rs`
7. `crates/sidereal-asset-runtime/src/lib.rs`
8. `crates/sidereal-game/tests/corvette.rs` (remove; not shader/asset-contract coverage)

## 8. Implementation Phases

### Phase 1: Catalog-first asset resolver migration

1. Remove hardcoded gateway asset resolver match arms.
2. Remove hardcoded `default_streamable_asset_sources()` list in runtime path.
3. Introduce authoritative catalog loader path used by gateway/replication/client runtime.
4. Keep temporary compatibility shims only behind explicit migration flags and remove quickly.

Status update (2026-03-07):
1. Gateway runtime manifest generation and asset payload resolution now use the shared `sidereal-asset-runtime` catalog/storage path.
2. Client missing-role fallback behavior is reduced to shared family fallback.
3. Fullscreen/post-process renderables now execute through one generic runtime fullscreen renderable path instead of per-content marker components.
4. Asteroid world-sprite specialization now routes through registry-driven family classification rather than raw shader-id branching.
5. Planet/thruster attachment orchestration now routes through one generic runtime world-visual pass model instead of content-specific child markers.
6. Remaining work is to remove content-specific material execution assumptions, not the basic asset manifest/payload resolver.

### Phase 2: Generic runtime materials and bindings

1. Introduce/register generic runtime materials for sprite/polygon/fullscreen/post-process domains.
2. Migrate streamed sprite shader path to generic binding.
3. Migrate fullscreen background layers off starfield/space-background-specific materials and onto the DR-0027 generic layer model.

Status update (2026-03-07):
1. Fullscreen composition and streamed world-sprite family dispatch are partially migrated to generic runtime bindings.
2. World-visual pass attachment for planets/clouds/rings/thrusters is now genericized.
3. Planet/cloud/ring pass existence is now data-authored through `RuntimeWorldVisualStack`, and the client uses one stack-driven sync path instead of separate attach systems per planet sub-family.
4. The unresolved part of Phase 2 is now the next effect-family variants rather than planet orchestration: the planet trio has collapsed into `PlanetVisualMaterial`, thruster plume + weapon impact spark have collapsed into `RuntimeEffectMaterial`, and weapon tracers now use the `RuntimeEffectMaterial` beam/trail variant. Tactical overlay and future field/radial effect families still retain separate schemas because Bevy material schemas remain type-static.

### Phase 3: Replicated component model and Lua intents

1. Add runtime shader binding/fullscreen/post-process components in `sidereal-game`.
2. Add server validation and replication of approved bindings and layer definitions/rules.
3. Add Lua intent APIs and manual asset usage registration API.

### Phase 4: Fullscreen/post-process world pipeline integration

1. Apply fullscreen layers and post-process passes from replicated data.
2. Ensure post-process executes on world render target before UI composition.
3. Enforce deterministic fallback behavior for missing/invalid passes.
4. Make fullscreen background execution explicitly non-cullable so authored background shaders always render.

### Phase 5: Dynamic registration and hardening

1. Add optional dynamic usage-driven asset registration from scripts.
2. Add churn limits and compile budgeting.
3. Add observability for compile failures, fallback rates, rejected intents.

## 9. Test Plan

1. Unit tests:
- catalog/domain/schema validation
- fallback selection logic
- post-process pass ordering validation

2. Integration tests:
- script registration + intent validation (accept/reject)
- fallback then live swap when asset arrives
- cache reuse/invalidation for shader updates
- fullscreen/post-process world-only behavior (UI unaffected)

3. Parity checks:
- `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
- `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`

4. Soak test:
- repeated shader binding/post-process stack churn across many entities without unbounded compile thrash

## 10. Non-Goals

1. Runtime creation of brand-new Rust material schemas from Lua.
2. Client-local, non-replicated shader execution paths for gameplay visuals.
3. Bypassing server catalog authorization for any runtime shader/material binding.

## 11. Open Questions

1. Parameter schema storage: embedded in catalog metadata or sidecar asset?
2. Should post-process stack be camera-scoped only, or also viewport-profile-scoped?
3. Which shader compile failures are user-visible vs telemetry-only by policy?
4. What rollout policy should gate untrusted/modded shader namespaces?

## 12. Acceptance Criteria

1. No hardcoded runtime `asset_id -> path` maps in gateway/client runtime shader/material paths.
2. Fullscreen background and starfield are represented as generic runtime shader layers, not specialized code paths.
3. World post-process stack is data-driven and excludes UI layer effects by design.
4. Server enforces authorization + schema validation for all shader/material intents.
5. Native and WASM execute equivalent behavior with deterministic fallback/unrendered fail-soft behavior and cache invalidation.

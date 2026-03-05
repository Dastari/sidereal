# Dynamic Runtime Shader Material Plan

Status: Proposed implementation plan
Date: 2026-03-05
Owners: scripting + replication + client rendering + asset streaming

Primary references:
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

3. `RuntimePostProcessStack`
- ordered pass list with `shader_asset_id`, params/texture bindings, enabled flag
- applies to world render target only (UI excluded)

### 4.2 Catalog metadata additions

Extend catalog entries with:
1. `shader_domain` compatibility (`sprite`, `polygon`, `fullscreen`, `post_process`)
2. required binding signature hash
3. optional parameter schema/version
4. optional dependency asset IDs (textures/includes)
5. safety profile tag (`trusted_first_party`, `modded_sandboxed`)

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
3. Missing/invalid assets use deterministic fallback; if no safe fallback exists, the renderable remains unrendered.
4. When assets arrive:
- compile/load shader,
- build/update material/pass instance,
- bind params/textures,
- atomically swap into render path.
5. On compile failure:
- keep fallback,
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

### Phase 2: Generic runtime materials and bindings

1. Introduce/register generic runtime materials for sprite/polygon/fullscreen/post-process domains.
2. Migrate streamed sprite shader path to generic binding.
3. Migrate fullscreen background layers off starfield/space-background-specific materials.

### Phase 3: Replicated component model and Lua intents

1. Add runtime shader binding/fullscreen/post-process components in `sidereal-game`.
2. Add server validation and replication of approved bindings.
3. Add Lua intent APIs and manual asset usage registration API.

### Phase 4: Fullscreen/post-process world pipeline integration

1. Apply fullscreen layers and post-process passes from replicated data.
2. Ensure post-process executes on world render target before UI composition.
3. Enforce deterministic fallback behavior for missing/invalid passes.

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

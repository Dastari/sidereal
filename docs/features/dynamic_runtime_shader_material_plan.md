# Dynamic Runtime Shader Material Plan

Status: Proposed implementation plan
Date: 2026-03-04
Owners: scripting + replication + client rendering + asset streaming

Primary references:
- `docs/features/scripting_support.md`
- `docs/features/asset_delivery_contract.md`
- `docs/sidereal_design_document.md`
- `AGENTS.md`

## 1. Objective

Enable Lua-authored content to use large numbers of custom shaders for 2D sprites/polygons without adding a new Rust `Material2d` type per shader.

Target outcome:
1. Scripts can select/create shader-driven visuals at runtime through intent APIs.
2. Shader source/assets are streamed through the existing asset pipeline.
3. Client compiles shader assets live and applies them via pre-registered generic material types.
4. No per-shader client startup boilerplate is required.

## 2. Problem Statement

Current client setup registers concrete Bevy material plugins at startup:
- `Material2dPlugin::<StarfieldMaterial>`
- `Material2dPlugin::<SpaceBackgroundMaterial>`
- `Material2dPlugin::<StreamedSpriteShaderMaterial>`
- `Material2dPlugin::<ThrusterPlumeMaterial>`

This is correct for Bevy type registration, but does not scale if content authors need hundreds of script-defined shader variants.

Key constraint:
- Bevy can compile/load new shader assets at runtime.
- Bevy cannot invent/register brand-new Rust `Material2d` schemas from server data at runtime.

So the scalable model is: **few generic runtime material types + many streamed shader assets and parameter payloads**.

## 3. Architecture Contract

### 3.1 Authority and scripting boundary

Follow `docs/features/scripting_support.md`:
1. Scripts emit intent only.
2. Scripts do not directly mutate client render internals.
3. Server validates shader intent payloads and persists/replicates approved component state.

### 3.2 Runtime rendering boundary

1. Client ships a fixed set of generic material schemas for 2D use cases:
- `RuntimeSpriteShaderMaterial2d`
- `RuntimePolygonShaderMaterial2d`
2. These types are registered once at startup.
3. Per-entity shader selection is data-driven by replicated components (`shader_asset_id`, parameters, textures).

### 3.3 Asset delivery boundary

Follow `docs/features/asset_delivery_contract.md`:
1. Shader source/binary artifacts are logical streamed assets.
2. No gameplay dependency on standalone HTTP file serving.
3. Cache validity uses version/hash; stale shader cache entries are replaced through normal invalidation.

### 3.4 Native/WASM parity

1. Runtime shader feature behavior must be shared across native and wasm32 clients.
2. Platform-specific transport loading is allowed only at the transport boundary.
3. Shader validation and fallback behavior must be equivalent across targets.

## 4. Data Model (Proposed)

## 4.1 Gameplay-facing components (persistable/replicated)

Use `sidereal-game` component workflow for new visual binding components:

1. `RuntimeShaderBinding2d`
- `shader_asset_id: String`
- `material_domain: RuntimeMaterialDomain` (`Sprite`, `Polygon`)
- `uniform_block_asset_id: Option<String>`
- `texture_bindings: Vec<RuntimeTextureBinding>`
- `flags: RuntimeShaderFlags`

2. `RuntimeShaderParams`
- compact validated parameter map (schema-driven numeric/vector/color values)

These components represent intent/state only; client-side render internals remain local.

## 4.2 Catalog metadata additions

Extend shader asset metadata with:
1. `material_domain` compatibility (`sprite`, `polygon`)
2. required bindings signature hash
3. optional parameter schema version
4. safety profile tag (`trusted_first_party`, `modded_sandboxed`)

## 5. Script API (Proposed)

Expose intent-style APIs in Lua, e.g.:

```lua
ctx:emit_intent("set_runtime_shader", {
  entity_id = target_id,
  shader_asset_id = "shaders/fx/plasma_edge.wgsl",
  material_domain = "sprite",
  params = {
    edge_strength = 0.8,
    glow_color = {0.3, 0.8, 1.0, 1.0}
  }
})
```

Validation rules:
1. `shader_asset_id` must exist in asset catalog and be authorized.
2. `material_domain` must match shader metadata.
3. params must conform to schema and value ranges.
4. forbidden bindings/features are rejected server-side.

## 6. Client Runtime Flow

1. Entity replication applies/updates `RuntimeShaderBinding2d`.
2. Client resolves shader asset from cache or requests streaming.
3. If shader is unavailable, use deterministic fallback material.
4. When shader arrives:
- compile in asset pipeline,
- build/update material instance,
- bind params/textures,
- swap on entity.
5. On compile failure:
- keep fallback,
- emit structured error telemetry,
- optionally surface user-facing dialog for critical failures.

## 7. Fallback and Safety

Required fail-soft behavior:
1. Missing shader asset -> fallback material, no crash.
2. Compile error -> fallback material, no crash.
3. Bad params -> clamp/reject and log.
4. Unauthorized shader request -> reject at server intent validation.

Security and abuse constraints:
1. enforce max shader source size and include depth.
2. restrict dangerous WGSL feature usage by lint profile.
3. budget compilation churn per time window to prevent denial-of-service via shader thrash.

## 8. Implementation Phases

### Phase 1: Generic material consolidation

1. Introduce and register generic runtime 2D shader material types.
2. Migrate current streamed sprite path to the generic type.
3. Keep existing specialized local-only materials (starfield/thruster/background) as-is.

### Phase 2: Replicated shader binding components

1. Add `RuntimeShaderBinding2d` + `RuntimeShaderParams` in `sidereal-game`.
2. Register persistence/hydration mappings and replication serialization.
3. Apply bindings to client render entities in shared visual systems.

### Phase 3: Script intent and validation

1. Add `set_runtime_shader` intent action.
2. Implement server-side validation against catalog metadata/schema.
3. Add integration tests for accept/reject paths.

### Phase 4: Streaming/caching and diagnostics hardening

1. Ensure shader assets follow normal manifest/chunk delivery.
2. Add compile telemetry and fallback counters.
3. Add dashboards/log summaries for compile failure rates and fallback rates.

### Phase 5: Optional authoring ergonomics

1. dashboard shader schema editor integration.
2. script-side helper wrappers/templates.
3. staged rollout controls (allowlist by environment).

## 9. Test Plan

1. Unit tests:
- schema validation
- domain compatibility checks
- fallback selection logic

2. Integration tests:
- server intent validation rejects unknown/unauthorized shader IDs
- client fallback then live swap when asset arrives
- cache reuse and invalidation for shader updates

3. Parity checks:
- `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
- `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`

4. Soak test:
- repeated shader binding changes across many entities without unbounded compile thrash

## 10. Non-Goals

1. Runtime creation of brand-new Rust `Material2d` schemas from Lua.
2. Arbitrary client-local shader execution outside server-authoritative component state.
3. Replacing current local fullscreen effects pipeline in this phase.

## 11. Open Questions

1. Should shader parameter schemas be embedded in the asset catalog or separate sidecar assets?
2. Do we need a precompiled shader cache artifact for wasm startup latency, or is on-demand compile acceptable?
3. Which compile errors require user-facing dialog vs debug telemetry only?
4. Do we allow mod-scoped shader namespaces with per-session allowlists?

## 12. Acceptance Criteria

1. New script-authored sprite/polygon shader assets can be applied without adding new client material plugin registrations.
2. Client handles missing/invalid shaders with deterministic fallback and no crash.
3. Server enforces shader authorization and schema validation on intent paths.
4. Native and wasm targets compile and run equivalent behavior.
5. Asset streaming/cache invalidation for shaders follows existing contract.

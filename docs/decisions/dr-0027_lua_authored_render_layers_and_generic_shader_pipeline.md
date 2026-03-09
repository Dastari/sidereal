# DR-0027: Lua-Authored Render Layers and Generic Shader Pipeline

Status: Accepted direction; layer/rule/runtime groundwork implemented
Date: 2026-03-07
Owners: client rendering + scripting + replication + asset streaming

Primary references:
- `docs/plans/dynamic_runtime_shader_material_plan.md`
- `docs/features/scripting_support.md`
- `docs/features/asset_delivery_contract.md`
- `docs/sidereal_design_document.md`
- `AGENTS.md`

## 1. Decision Summary

Sidereal should move from hard-coded game-specific client rendering paths toward a generic 2D render composition model authored in Lua and executed through a small fixed set of Rust-owned runtime material schemas.

Core decision:

1. Lua authors render layer definitions, layer assignment rules, and post-process stack intent as data.
2. Rust validates, persists/replicates, budgets, and executes those definitions through pre-registered generic runtime material types.
3. Normal `spawn_entity` paths do not require an explicit render layer parameter; non-fullscreen entities default to the main world layer unless a validated rule or explicit override redirects them.
4. Layer depth for world-space layers is render-time parallax/depth behavior derived from camera-relative transforms. It must not mutate authoritative simulation positions.

This document defines the target contract for:

1. fullscreen background layers,
2. world-space midground/main/foreground layers,
3. fullscreen foreground layers, and
4. camera-scoped global post-process passes.

## 2. Context

Current runtime still contains multiple space-game-specific assumptions:

1. Client startup registers named shader roles and dedicated material types for starfield, space background, planets, clouds, rings, thrusters, tactical map overlay, and impact sparks.
2. Fullscreen composition is centered on fixed layer kinds like `starfield` and `space_background`.
3. Gateway and replication world-init contracts still expect exactly two fullscreen background shader asset IDs.
4. Some client visual logic branches on concrete asset IDs and entity families (for example asteroid shader/texture handling).

Those assumptions are useful for the current Sidereal content set, but they are not the right long-term engine contract for a generic 2D runtime that should be reusable across very different games.

## 3. Goals

1. Keep the engine generic enough to support arbitrary 2D games, not only Sidereal's current space visual stack.
2. Let Lua define render composition declaratively without giving scripts raw access to Bevy render internals.
3. Preserve a small fixed Rust render ABI that supports dynamic streamed shaders and assets.
4. Make default entity spawning simple: world entities go to the default main world layer unless rules or overrides say otherwise.
5. Support dynamic world-event-driven background/foreground changes authored from scripts.
6. Support world-space layer depth/parallax without breaking authoritative simulation, replication, visibility, or selection semantics.

## 4. Non-Goals

1. Runtime generation of brand-new Rust material types from Lua or network data.
2. Script-side direct access to Bevy render graph, pipelines, bind group layouts, or ECS render internals.
3. Mutating authoritative world positions solely to achieve parallax or background depth effects.
4. Making every content effect fully generic immediately; Sidereal-specific visual modules may continue to exist during migration so long as the target architecture is preserved.

## 5. Render Composition Model

### 5.0 Current implementation status

Implemented now:

1. Shared persisted/replicated component schemas exist for:
   - `RuntimeRenderLayerDefinition`
   - `RuntimeRenderLayerRule`
   - `RuntimeRenderLayerOverride`
   - `RuntimePostProcessStack`
2. `world_init.lua` now emits Lua-authored `runtime_render_layer_definition` and `runtime_render_layer_rule` records for:
   - `bg_space_background`
   - `bg_starfield`
   - `main_world`
   - `midground_planets`
   - `planets_to_midground`
3. Gateway and replication validate authored render-layer definitions, rules, and post-process stacks before accepting script output.
4. The client builds a `RuntimeRenderLayerRegistry` resource from replicated layer/rule components and resolves world entities as:
   - explicit override,
   - highest-priority matching rule,
   - default `main_world`
5. World-space streamed visuals and planet-layer parallax now derive render transforms from the resolved layer definition without mutating authoritative world position.
6. Runtime asset queueing now treats `RuntimeRenderLayerDefinition` and `RuntimePostProcessStack` shader/params/texture references as streamable asset dependencies.
7. Fullscreen background and fullscreen foreground phases execute from authored `RuntimeRenderLayerDefinition` data.
8. Fullscreen background config entities remain replicated ECS authoring surfaces for BRP/dashboard editing, but the actual rendered fullscreen quads are client-only derived scene entities cached from the last known authored config. They are not ordinary world renderables and must not disappear because a replicated config entity is temporarily filtered out of the client world tree.
9. Camera-scoped `RuntimePostProcessStack` passes now execute as ordered fullscreen overlay passes after world composition and before UI composition, while still using the currently supported fullscreen shader families.
10. Shared persisted/replicated `RuntimeWorldVisualStack` now exists for authored multi-pass world visuals.
11. Gateway and replication now validate `RuntimeWorldVisualStack` records and expose `ctx.render:define_world_visual_stack(...)` to Lua.
12. The starter planet bundle now emits an authored `RuntimeWorldVisualStack` for body/cloud/ring passes.
13. The client planet visual path now consumes that authored stack as the source of truth for pass existence, shader-family validation, scale multipliers, and depth-bias metadata.

Still intentionally not complete:

1. fully arbitrary shader-per-layer execution through a single generic Bevy material type; current fullscreen/post-process execution still uses content adapters because `Material2d` remains type-static.
2. complete migration of all Sidereal-specific content effects (tactical overlay and remaining effect families) onto a single generic visual-stack/runtime-domain path.
3. continued reduction of the remaining dedicated Rust material-schema boundary where fixed `Material2d` types are still required.

### 5.1 Phases

Render composition is split into four phases:

1. `fullscreen_background`
2. `world`
3. `fullscreen_foreground`
4. `post_process`

Rules:

1. Only `world` layers render normal world entities.
2. `fullscreen_background` and `fullscreen_foreground` layers are not the default destination for spawned gameplay entities.
3. `post_process` is camera/view scoped and applies after world composition but before UI composition unless a later UI-post policy is explicitly introduced.
4. `fullscreen_background` layers are composition layers and must not depend on normal world-space visibility/frustum culling. If a background layer is enabled for the active camera/view, it should render.

### 5.2 Layers

A render layer is an authored definition within a phase.

Examples:

1. `bg_starfield`
2. `midground_planets`
3. `main_world`
4. `fg_damage_vignette`
5. `camera_warp_post`

Each layer defines:

1. phase,
2. material domain,
3. shader asset,
4. ordering,
5. parameter/texture bindings,
6. optional world parallax behavior,
7. optional assignment rules.

### 5.3 Default behavior

Default layer policy:

1. Non-fullscreen spawned entities enter the default `main_world` layer.
2. Fullscreen layers are authored explicitly and are not produced by generic `spawn_entity` by default.
3. Rule-based assignment may redirect matched entities away from `main_world`.
4. Explicit per-entity override has higher priority than rule-based assignment.

## 6. World Layer Assignment Model

### 6.1 Authoring principle

Layer assignment is declarative and data-driven.

Scripts should define:

1. layer definitions, and
2. assignment rules that map entities to layers by labels/archetype/component presence.

`spawn_entity` remains gameplay-centric and does not need to specify render routing in the common case.

### 6.2 Assignment precedence

The canonical precedence is:

1. explicit entity render override,
2. highest-priority matching layer rule,
3. default `main_world` layer.

### 6.3 Supported match criteria

The rule matcher should be generic and entity-oriented.

Recommended criteria:

1. `labels_any`
2. `labels_all`
3. `archetypes_any`
4. `components_all`
5. `components_any`
6. `owner_scope` or visibility scope where needed

Rules must stay bounded and deterministic. They are not arbitrary Lua callbacks that run every render frame.

### 6.4 Example

```lua
render:define_layer({
  layer_id = "midground_planets",
  phase = "world",
  domain = "world_polygon",
  shader_asset_id = "shader.planet.body",
  order = -60,
  parallax_factor = 0.18,
})

render:define_rule({
  rule_id = "planets_to_midground",
  target_layer_id = "midground_planets",
  priority = 100,
  labels_any = { "Planet" },
})
```

## 7. Layer Depth and Parallax

### 7.1 Two different concepts

This contract separates:

1. render ordering depth,
2. world parallax/depth behavior.

Render ordering depth decides draw order.

Parallax/depth behavior decides how a world-space layer responds to camera motion and therefore how "far back" it feels.

### 7.2 Hard rule

Layer depth/parallax must not rewrite authoritative entity `Position` or other simulation-space motion state.

Correct model:

1. authoritative entity position remains unchanged in ECS,
2. client render systems derive visual transform from entity position + camera state + layer parallax policy,
3. physics, replication, hydration, ownership, and visibility continue to use canonical world positions.

### 7.3 Recommended layer fields

For world-space layers, use:

1. `order: i32`
2. `parallax_factor: f32`
3. optional `screen_scale_factor: f32`
4. optional `depth_bias_z: f32`
4. optional `follow_camera_rotation: bool`

Interpretation:

1. `parallax_factor = 1.0` means normal world-space motion.
2. `parallax_factor < 1.0` means the layer appears farther back.
3. `parallax_factor > 1.0` is allowed only if intentionally used for foreground exaggeration.
4. `screen_scale_factor = 1.0` means no additional apparent-size adjustment for that layer.
5. `screen_scale_factor` changes visual size only; it must not be treated as authoritative world scale or visibility radius.

## 8. Generic Runtime Rendering Boundary

### 8.1 Fixed Rust-owned material schemas

Client runtime ships a small fixed set of material schemas:

1. `RuntimeSpriteShaderMaterial2d`
2. `RuntimePolygonShaderMaterial2d`
3. `RuntimeFullscreenShaderMaterial2d`
4. `RuntimePostProcessMaterial`

These are the only engine-owned runtime shader/material families required by the generic render path.

### 8.2 Material domains

Recommended canonical domains:

1. `world_sprite`
2. `world_polygon`
3. `fullscreen`
4. `post_process`

Layer definitions select one of those domains.

### 8.3 Script boundary

Lua can select:

1. layer definitions,
2. shader asset IDs,
3. parameter payload IDs or inline validated payloads,
4. texture binding references,
5. assignment rules,
6. camera post stacks.

Lua cannot:

1. define a new Rust material type,
2. define raw bind group layout/state,
3. bypass validation,
4. mutate client render internals directly.

### 8.4 Multi-pass world visual stacks

Layer assignment alone is not enough for layered content such as planets with clouds/rings or future shield/atmosphere effects.

For that reason the generic layer pipeline also needs a separate authored concept:

1. `RuntimeWorldVisualStack`
2. ordered pass list
3. each pass chooses:
   - visual family
   - visual kind
   - material domain
   - shader asset ID
   - optional params/textures
   - optional scale/depth bias
   - enabled flag

Implementation status:

1. `RuntimeWorldVisualStack` is now implemented as a persisted/replicated gameplay component.
2. Replication and gateway validate authored stack records server-side.
3. Lua can now emit stacks through `ctx.render:define_world_visual_stack(...)`.
4. The current planet bundle emits a `RuntimeWorldVisualStack`.
5. The client now uses that stack as the source of truth for planet pass existence and transform metadata.
6. The remaining non-generic boundary is the fixed Rust `Material2d` schema family used to execute those pass kinds.

## 9. Proposed Data Model

### 9.1 Persisted/replicated layer definition

```rust
pub enum RuntimeRenderPhase {
    FullscreenBackground,
    World,
    FullscreenForeground,
    PostProcess,
}

pub enum RuntimeMaterialDomain {
    WorldSprite,
    WorldPolygon,
    Fullscreen,
    PostProcess,
}

pub struct RuntimeRenderLayerDefinition {
    pub layer_id: String,
    pub phase: RuntimeRenderPhase,
    pub material_domain: RuntimeMaterialDomain,
    pub shader_asset_id: String,
    pub params_asset_id: Option<String>,
    pub texture_bindings: Vec<RuntimeTextureBinding>,
    pub order: i32,
    pub parallax_factor: Option<f32>,
    pub depth_bias_z: Option<f32>,
    pub enabled: bool,
}
```

### 9.2 Rule-based assignment

```rust
pub struct RuntimeRenderLayerRule {
    pub rule_id: String,
    pub target_layer_id: String,
    pub priority: i32,
    pub labels_any: Vec<String>,
    pub labels_all: Vec<String>,
    pub archetypes_any: Vec<String>,
    pub components_all: Vec<String>,
    pub components_any: Vec<String>,
    pub enabled: bool,
}
```

### 9.3 Explicit override

```rust
pub struct RuntimeRenderLayerOverride {
    pub layer_id: String,
}
```

### 9.4 Camera-scoped post stack

```rust
pub struct RuntimePostProcessPass {
    pub pass_id: String,
    pub shader_asset_id: String,
    pub params_asset_id: Option<String>,
    pub texture_bindings: Vec<RuntimeTextureBinding>,
    pub order: i32,
    pub enabled: bool,
}

pub struct RuntimePostProcessStack {
    pub passes: Vec<RuntimePostProcessPass>,
}
```

## 10. Lua Authoring API Direction

### 10.1 Layer definition

```lua
render:define_layer({
  layer_id = "bg_starfield",
  phase = "fullscreen_background",
  domain = "fullscreen",
  shader_asset_id = "shader.bg.starfield_v2",
  order = -200,
  params = {
    density = 0.08,
    tint = { 0.8, 0.9, 1.0 },
  },
})
```

### 10.2 World layer with parallax

```lua
render:define_layer({
  layer_id = "midground_planets",
  phase = "world",
  domain = "world_polygon",
  shader_asset_id = "shader.planet.body",
  order = -60,
  parallax_factor = 0.18,
  depth_bias_z = -60.0,
})
```

### 10.3 Rule definition

```lua
render:define_rule({
  rule_id = "planets_to_midground",
  target_layer_id = "midground_planets",
  priority = 100,
  labels_any = { "Planet" },
})
```

### 10.4 Dynamic event-driven update

```lua
ctx:emit_intent("set_render_layer_state", {
  layer_id = "fg_radiation_warning",
  enabled = true,
  params = {
    intensity = 0.85,
    pulse_hz = 2.2,
  },
})
```

### 10.5 Post-process stack

```lua
ctx:emit_intent("set_camera_post_process_stack", {
  camera_entity_id = camera_id,
  passes = {
    {
      pass_id = "warp",
      shader_asset_id = "shader.post.warp",
      order = 10,
      enabled = true,
    },
    {
      pass_id = "grade",
      shader_asset_id = "shader.post.grade",
      order = 20,
      enabled = true,
    },
  },
})
```

## 11. Validation Contract

Server-side validation must enforce:

1. `layer_id` uniqueness in scope.
2. `shader_asset_id` exists in authoritative catalog.
3. layer `phase` and `material_domain` are compatible.
4. shader catalog metadata declares compatibility with the requested material domain.
5. parameter payload schema/version matches catalog-declared schema.
6. assignment rules only reference known layer IDs.
7. rule matchers use only allowed component kinds/labels.
8. parallax/depth values stay within validated ranges.
9. post-process stack ordering is deterministic and bounded.
10. compile churn / hot-swap frequency is rate limited.

## 12. Asset Catalog Contract Extensions

The asset catalog should move away from `shader_role` as the primary dispatch key.

Required metadata additions:

1. `shader_domains`
2. `binding_signature_hash`
3. `parameter_schema_id` or `parameter_schema_version`
4. `dependencies`
5. `safety_profile`
6. optional `role_aliases` only as convenience metadata, not singleton engine dispatch

Reason:

1. `shader_role` works for one-off globals like "the starfield shader".
2. It does not scale to many authored layers, variants, or multiple compatible shaders in the same domain.

## 13. Migration and Conflict Resolution

### 13.1 Current conflicts with existing runtime behavior

Current codebase conflicts with this target contract in the following ways:

1. fixed fullscreen layer kinds (`starfield`, `space_background`),
2. fixed world-init fields `space_background_shader_asset_id` and `starfield_shader_asset_id`,
3. game-specific client material registrations for planets/clouds/rings/thrusters/tactical map/sparks,
4. asset-ID-specific branches such as asteroid shader/texture handling,
5. `shader_role`-driven singleton shader resolution.

### 13.2 Relationship to other docs

This document supersedes or narrows earlier assumptions as follows:

1. `docs/plans/dynamic_runtime_shader_material_plan.md`
   - still valid for generic material-schema direction,
   - superseded here for layer composition, rule-based assignment, default world layer policy, and parallax semantics.
2. `docs/features/scripting_support.md`
   - scripting authority/intents remain unchanged,
   - world-init and render authoring should migrate from fixed shader asset IDs to authored render layer definitions.
3. `docs/features/asset_delivery_contract.md`
   - asset registry authority remains unchanged,
   - shader metadata should move from singleton `shader_role` assumptions toward domain/signature/schema metadata.

## 14. Implementation Plan

### Phase 1: Contract and schema groundwork

- [x] Add runtime render-layer feature doc references to design/scripting/shader/asset docs.
- [x] Introduce shared terminology: render phase, render layer, assignment rule, post-process stack.
- [x] Add proposed component definitions in `sidereal-game` for layer definitions, rules, overrides, and post stacks.
- [x] Keep legacy Sidereal-specific components during transition where needed.

### Phase 2: Client generic layer runtime

- [~] Introduce generic runtime material plugins only for `world_sprite`, `world_polygon`, `fullscreen`, and `post_process`.
  - World sprite/world polygon content now resolves through generic layer assignment/parallax policy.
  - Fullscreen/post-process execution is data-driven, but still constrained by the current fullscreen shader adapters rather than a fully arbitrary per-layer shader runtime.
- [x] Add client-side layer registry/resource built from replicated layer-definition components.
- [x] Add world-layer render pass that resolves entity-layer assignment from override -> rule -> default.
- [x] Add render-time parallax transform derivation for world layers without changing authoritative ECS positions.
- [x] Add fullscreen background and fullscreen foreground generic layer execution paths.

### Phase 3: Server validation and replication

- [x] Add validation for layer definitions, rule definitions, overrides, and post stacks.
- [x] Add replication of approved render layer definitions and assignment rules.
- [x] Add rule compilation/caching so assignment matching is deterministic and budgeted.
- [~] Reject invalid or unauthorized shader domain/schema combinations.
  - Phase/domain validation and known-layer/component-kind validation are enforced now.
  - Full authoritative shader-domain/signature/schema authorization still depends on the broader asset-catalog metadata migration in `dynamic_runtime_shader_material_plan.md`.

### Phase 4: Lua APIs and world-init migration

- [x] Add Lua authoring APIs for `define_layer`, `define_rule`, and camera post stacks.
- [x] Migrate `world_init.lua` away from fixed `space_background_shader_asset_id` / `starfield_shader_asset_id` fields.
- [x] Replace those with authored background layer definitions in world bootstrap records.
- [x] Keep temporary migration shim only if explicitly documented and short-lived.

### Phase 5: Migrate existing hard-coded content systems

- [x] Migrate fullscreen starfield/background to generic fullscreen layers.
- [x] Migrate sprite shader binding to generic world-layer binding.
- [ ] Migrate planet visual pipeline to authored world layer definitions and generic material domains where possible.
- [ ] Migrate tactical overlay to a content/UI-specific use of generic fullscreen shader infrastructure rather than engine-special casing.
- [ ] Remove asset-ID-specific asteroid special cases or convert them to metadata-driven procedural hooks.

Planet migration note:

1. Planets cannot be represented correctly by simple entity -> layer assignment alone.
2. The generic path must first support authored multi-pass visual stacks so a single logical planet can emit:
   - body
   - cloud back
   - cloud front
   - ring back
   - ring front
3. That visual-stack concept is a prerequisite for migrating the current 2.5D planet pipeline cleanly.

### Phase 6: Post-process and hardening

- [x] Add camera-scoped post-process stack execution.
- [x] Ensure UI exclusion remains deterministic.
- [ ] Add compile budgeting, fallback telemetry, and failure throttling.
- [ ] Verify native/WASM parity and Windows build coverage.

## 14.1 Explicit Path Forward

This is the intended execution order for the next implementation passes. Do not skip ahead and try to "finish generically" in one rewrite; the remaining work is tightly coupled to the current planet/tactical/material paths.

### Step 1: Introduce authored multi-pass visual stacks

Add a first-class runtime concept for one logical entity producing multiple render passes.

Target capability:
1. one entity owns a stack of pass definitions,
2. each pass selects:
   - render phase,
   - material domain,
   - shader asset,
   - order/depth bias,
   - optional parent layer binding or explicit layer target,
   - optional per-pass parameter/texture references.

Why first:
1. planet migration is blocked on this,
2. ring/cloud front/back passes are already proving that flat entity -> layer assignment is insufficient,
3. this is the missing core abstraction, not a planet-specific exception.

Primary touch targets:
1. `crates/sidereal-game/src/components/`
2. `bins/sidereal-replication/src/replication/scripting.rs`
3. `bins/sidereal-gateway/src/auth/starter_world_scripts.rs`
4. `bins/sidereal-client/src/native/visuals.rs`
5. `docs/features/scripting_support.md`

### Step 2: Migrate planets onto the authored visual-stack model

Once visual stacks exist:
1. express planet body/cloud/ring passes as authored stack entries,
2. move the current hardcoded child-pass assembly into generic stack consumption where possible,
3. keep planet-specific shader/material families temporarily if needed, but remove hardcoded pass topology from client gameplay logic.

Required outcome:
1. planet body,
2. cloud back,
3. cloud front,
4. ring back,
5. ring front

must be expressed as data-owned pass composition rather than bespoke Rust pass orchestration.

### Step 3: Migrate tactical overlay onto the same composition model

The tactical overlay should stop being a special fullscreen-only engine path and become:
1. either a fullscreen foreground layer,
2. or a camera-scoped post-process/UI-adjacent layer using the same authored execution model.

This step should happen after visual stacks exist so tactical effects are not forced into another one-off abstraction.

### Step 4: Replace remaining fullscreen/content shader adapters

Current fullscreen/post-process execution still uses fixed adapter families for the supported fullscreen shaders.

Next migration goal:
1. reduce the remaining `shader_asset_id` / shader-family branching,
2. move toward domain/signature-driven execution,
3. preserve fail-soft behavior when shader/catalog metadata is missing or invalid,
4. ensure fullscreen background execution is explicitly non-cullable so authored background shaders always render.

This step depends on the wider catalog metadata work described in `docs/plans/dynamic_runtime_shader_material_plan.md`.

## 14.2 Stop Conditions and Non-Goals for the Next Pass

The next pass should stop when:
1. multi-pass visual stacks exist,
2. planets are driven through them,
3. tactical overlay has a clear migration path or is migrated,
4. docs/tests are updated.

It should not try to:
1. invent arbitrary runtime Bevy material schemas from Lua,
2. bypass catalog/domain validation,
3. collapse all content adapters in the same change if that obscures correctness.

## 14.3 Remaining Risks

1. Bevy `Material2d` is still a fixed Rust type boundary, so "generic shader pipeline" remains partly constrained by material ABI design.
2. Planet migration is the highest regression-risk area because it touches:
   - lighting,
   - parallax,
   - ring/cloud occlusion,
   - layered child pass ordering.
3. Tactical overlay migration can accidentally blur the line between world post-process and UI composition if phase boundaries are not kept explicit.
4. Background composition can regress silently if fullscreen layers are allowed to reuse normal visibility/culling rules; this must remain an explicit invariant in the remaining migration work.

## 15. Code Touch Targets

Expected primary touch points:

1. `crates/sidereal-game/src/components/`
2. `crates/sidereal-scripting/src/`
3. `bins/sidereal-replication/src/replication/scripting.rs`
4. `bins/sidereal-replication/src/replication/visibility.rs`
5. `bins/sidereal-client/src/native/shaders.rs`
6. `bins/sidereal-client/src/native/backdrop.rs`
7. `bins/sidereal-client/src/native/visuals.rs`
8. `bins/sidereal-client/src/native/mod.rs`
9. `bins/sidereal-client/src/native/assets.rs`
10. `bins/sidereal-gateway/src/auth/starter_world_scripts.rs`

## 16. Test Plan

### 16.1 Unit tests

- [x] Layer/rule schema validation.
- [x] Domain compatibility validation.
- [ ] Assignment precedence: override > rule > default.
- [ ] Parallax transform derivation does not mutate source world position.
- [x] Post-process ordering validation.

### 16.2 Integration tests

- [x] Lua-authored layer definitions replicate to client and render through generic paths.
- [x] Default world entities appear in `main_world` without explicit layer on spawn.
- [x] Entities with label `Planet` are redirected to `midground_planets`.
- [x] Fullscreen background layers can be added/removed dynamically by script intent.
- [x] Global post-process stack applies after world composition and excludes UI.
- [x] Invalid layer/rule/shader combinations are rejected fail-closed.

### 16.3 Regression tests against current hard-coded behavior

- [x] Migrate existing starfield/background content without special `layer_kind` branches.
- [x] Migrate world-init boot content away from fixed background shader fields.
- [ ] Remove client dependence on hard-coded shader role match arms for content-specific layers.
- [ ] Make fullscreen background layers explicitly non-cullable so background shaders always render when enabled.

### 16.4 Build parity

- [ ] `cargo check --workspace`
- [ ] `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
- [ ] `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`

## 17. Acceptance Checklist

- [x] Non-fullscreen entity spawn paths no longer need an explicit layer parameter in the common case.
- [x] Default world layer policy is implemented and documented.
- [x] Fullscreen background/foreground composition is data-driven, not limited to fixed hard-coded kinds.
- [x] World layer assignment rules can be authored in Lua and validated server-side.
- [x] Layer depth/parallax is render-derived only and does not mutate authoritative motion state.
- [~] Client generic runtime material set is limited to the fixed generic schemas.
  - The layer/rule framework is fixed-schema now.
  - Fullscreen/post-process execution still depends on the current content-specific fullscreen shader adapters until the fully generic runtime material path replaces the last Bevy `Material2d` limitations.
- [x] Sidereal-specific visuals use the generic layer system or are clearly isolated as content plugins during migration.

## 18. Open Questions

1. Should world-layer rules be evaluated only on spawn/component-change, or also on every relevant label/component mutation event?
2. Do we want a distinct `ui_fullscreen` phase later, or should UI-owned shader overlays remain outside this contract?
3. How much per-entity parameter override flexibility is safe before we need stricter churn budgets?
4. Do we want authored layer definitions to be global world state, camera-scoped state, or both with explicit scope?

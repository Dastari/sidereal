# Rendering Optimization Pass Plan

Status: Proposed implementation plan
Date: 2026-03-09
Owners: client rendering + replication + asset streaming + scripting
Primary input: `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-09.md`

## 1. Objective

Run a focused optimization pass on the 2D rendering path without violating Sidereal's core direction:

1. Lua/data remains the authoring surface for runtime visuals, shader selection, layer routing, and post-process configuration.
2. Runtime shader/material changes must not require restarting the client, replication server, or shard.
3. Dashboard/live-tweak workflows remain supported.
4. One-time hitch/rebuild cost on actual shader or binding changes is acceptable.
5. Per-frame asset/material/mesh churn for unchanged content is not acceptable.

Target outcome:

1. steady frame times on the native client,
2. lower client `Update` cost from rendering-adjacent bookkeeping,
3. lower replication visibility cost that currently degrades client smoothness indirectly,
4. preserved compatibility with `docs/plans/dynamic_runtime_shader_material_plan.md`,
5. no architectural regression against `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`.

## 2. Design Philosophy Guardrails

This pass must not optimize by walking back the project's intended runtime model.

Hard requirements:

1. Keep the fixed Rust-owned material/schema boundary. Do not introduce "runtime-generated Rust material types".
2. Keep Lua-authored `shader_asset_id`, params, texture bindings, layer definitions, and post-process stacks as the content contract.
3. Keep authoritative state server-driven. Clients may cache, interpolate, and visualize, but must not become authoritative for visual gameplay state.
4. Keep world/simulation positions authoritative and separate from render-layer parallax.
5. Keep shared gameplay/runtime logic portable across native and WASM where feasible. Native optimization work must not hard-fork the architecture.

Optimization rule:

1. change-driven invalidation is good,
2. frame-driven rebuild churn is bad.

That distinction matters for live shader workflows:

1. dashboard parameter tweak -> cheap material/uniform update,
2. shader source or shader asset swap -> one-time rebuild/rebind allowed,
3. entity spawn with authored sprite + shader -> normal runtime attach path allowed,
4. unchanged fullscreen layer or unchanged sprite visual -> no rebuild, no fresh mesh, no fresh material allocation.

## 3. Proven Hotspots To Target First

### 3.1 Critical client churn

The top priority remains the proven fullscreen/post-process churn in `bins/sidereal-client/src/native/backdrop.rs`.

Current problems:

1. `sync_fullscreen_layer_renderables_system()` allocates a fresh `Rectangle` mesh even for already-existing fullscreen renderables.
2. `sync_runtime_post_process_renderables_system()` follows the same pattern for post-process passes.
3. `attach_runtime_fullscreen_material()` clears and re-adds material state repeatedly, causing unnecessary asset churn.

Why this is first:

1. it is already proven by code inspection,
2. it can explain frame spikes and long-session slowdown by itself,
3. fixing it does not require changing the higher-level live-authoring model.

### 3.2 Client whole-world frame scans

The next tier is all the frame-polled bookkeeping:

1. `bins/sidereal-client/src/native/render_layers.rs`
2. `bins/sidereal-client/src/native/assets.rs`
3. `bins/sidereal-client/src/native/visuals.rs`

Current problems:

1. render-layer registry recompiles from cloned world state every frame,
2. per-entity render-layer assignment re-walks world entities every frame,
3. asset dependency discovery rebuilds a large candidate set every frame,
4. duplicate predicted/interpolated visual suppression scores the whole world every frame.

### 3.3 Server-side visibility cadence pressure

`bins/sidereal-replication/src/replication/visibility.rs` is not a render file, but it is part of the rendering-smoothness problem because it can destabilize update cadence.

Current problems:

1. visibility scratch maps and spatial indices are rebuilt each tick,
2. candidate sets are rebuilt per client,
3. many ownership/faction/public visibility checks are repeated even when little changed.

## 4. Version-Aware Notes

This pass should explicitly account for current engine/networking capabilities.

### 4.1 Bevy 0.18

Relevant current points from the Bevy 0.18 release/docs:

1. `FullscreenMaterial` / `FullscreenMaterialPlugin` now exist and are worth evaluating for some fullscreen paths.
2. required components are available and may reduce manual attach boilerplate in some visual setup paths.

Plan implication:

1. evaluate `FullscreenMaterial` for simple fullscreen/post-process families after the churn bug is fixed,
2. do not migrate blindly if it breaks the DR-0027 runtime layer model or the runtime shader-family boundary,
3. do not assume `FullscreenMaterial` solves dynamic shader-family swapping by itself; the type-static material boundary still exists.

### 4.2 Lightyear

Relevant current points from Lightyear docs:

1. visual/frame interpolation is a supported path and is already correctly helping smooth rendering for replicated entities,
2. interest-management concepts exist upstream, but Sidereal's visibility contract is stronger and more game-specific.

Plan implication:

1. keep `FrameInterpolationPlugin::<Transform>` and the existing `FrameInterpolate<Transform>` attach path,
2. optimize around it rather than removing it,
3. only borrow ideas from Lightyear interest management if they fit `docs/features/visibility_replication_contract.md`; do not replace Sidereal's visibility model with a generic upstream feature.

### 4.3 Avian2D

Relevant current points from Avian docs:

1. interpolation support exists in Avian,
2. Sidereal currently uses Lightyear visual interpolation in the client path.

Plan implication:

1. do not turn on Avian interpolation in parallel with Lightyear interpolation without a deliberate ownership review,
2. avoid double-smoothing and transform writer conflicts,
3. preserve the single-writer motion/render ownership rule from `AGENTS.md`.

## 5. Recommended Order Of Operations

### Phase 0: Instrumentation and baselines

Goal: make the pass measurable before changing behavior.

Work:

1. Add lightweight timing/counter instrumentation around:
   - `sync_fullscreen_layer_renderables_system`
   - `sync_runtime_post_process_renderables_system`
   - `sync_runtime_render_layer_registry_system`
   - `resolve_runtime_render_layer_assignments_system`
   - `queue_missing_catalog_assets_system`
   - `suppress_duplicate_predicted_interpolated_visuals_system`
   - `update_network_visibility`
2. Add counters for:
   - fullscreen mesh allocations,
   - fullscreen material allocations,
   - post-process pass reallocations,
   - render-layer registry rebuild count,
   - asset dependency rescan count,
   - duplicate GUID suppression group count,
   - visibility candidate-set sizes per client,
   - visibility tick duration.
3. If not already present, add a debug-only dashboard/overlay view for those counters.

Potential blockers:

1. too much logging can perturb frame time,
2. if BRP/dashboard access is already performance-sensitive, metrics publishing must be sampled rather than pushed every frame.

Acceptance criteria:

1. clear before/after counters exist for each later phase,
2. no architecture change yet,
3. no user-facing regression.

### Phase 1: Eliminate fullscreen and post-process churn

Goal: stop rebuilding unchanged fullscreen renderables every frame.

Primary files:

1. `bins/sidereal-client/src/native/backdrop.rs`
2. `bins/sidereal-client/src/native/resources.rs`
3. `bins/sidereal-client/src/native/plugins.rs`

Implementation direction:

1. Introduce a shared cached fullscreen quad mesh handle resource.
2. Split fullscreen sync into:
   - create missing renderables,
   - mutate changed renderables in place,
   - remove stale renderables.
3. Introduce change detection or cache-key comparison for fullscreen bindings.
4. Cache material handles by a stable binding key instead of re-adding every frame.

Suggested cache key:

```rust
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct RuntimeFullscreenBindingKey {
    shader_asset_id: String,
    phase: String,
    params_asset_id: Option<String>,
    texture_binding_ids: Vec<String>,
    family: RuntimeFullscreenFamily,
}
```

Suggested resource shape:

```rust
#[derive(Resource, Default)]
struct FullscreenRenderResources {
    quad_mesh: Handle<Mesh>,
    material_by_key: HashMap<RuntimeFullscreenBindingKey, RuntimeFullscreenMaterialHandle>,
}
```

Suggested sync shape:

```rust
fn sync_fullscreen_layer_renderables_system(
    mut commands: Commands,
    cache: Res<FullscreenLayerCache>,
    mut render_resources: ResMut<FullscreenRenderResources>,
    existing: Query<(Entity, &RuntimeFullscreenRenderable, Option<&RuntimeFullscreenMaterialBinding>)>,
) {
    // 1. despawn stale renderables
    // 2. spawn only missing renderables using render_resources.quad_mesh.clone()
    // 3. only replace RuntimeFullscreenMaterialBinding when the binding key changed
}
```

Important compromise:

1. material-handle reuse improves performance,
2. but some fullscreen passes legitimately need unique owned params,
3. therefore cache by stable binding content, not just `shader_asset_id`.

Potential functionality loss:

1. overly aggressive handle sharing could make live parameter edits leak between unrelated layers,
2. overly aggressive "only update on `Changed<T>`" logic could miss data changes if upstream cache/resources are mutated indirectly.

Mitigation:

1. treat authored params/texture bindings as part of the binding key,
2. add explicit invalidation events when runtime shader assignments or streamed assets change.

Acceptance criteria:

1. unchanged fullscreen layers allocate zero new meshes per frame,
2. unchanged fullscreen layers allocate zero new material assets per frame,
3. live shader swap still works with one-time rebuild,
4. dashboard parameter edits still update visible output.

### Phase 2: Make render-layer compilation and assignment incremental

Goal: stop recompiling and reassigning layer state every render frame.

Primary files:

1. `bins/sidereal-client/src/native/render_layers.rs`
2. `bins/sidereal-client/src/native/resources.rs`
3. `bins/sidereal-client/src/native/plugins.rs`

Implementation direction:

1. Rebuild `RuntimeRenderLayerRegistry` only when:
   - `RuntimeRenderLayerDefinition` changes,
   - `RuntimeRenderLayerRule` changes,
   - `RuntimePostProcessStack` changes,
   - generated component registry changes.
2. Resolve per-entity `ResolvedRuntimeRenderLayer` only when:
   - labels change,
   - override changes,
   - relevant component membership changes,
   - the registry version changes.
3. Store a registry version number and the last-applied version per entity.

Suggested resource:

```rust
#[derive(Resource, Default)]
struct RuntimeRenderLayerRegistryState {
    version: u64,
    registry: RuntimeRenderLayerRegistry,
}
```

Suggested entity marker:

```rust
#[derive(Component, Default)]
struct AppliedRenderLayerRegistryVersion(u64);
```

Possible system split:

1. `rebuild_runtime_render_layer_registry_on_change_system`
2. `mark_render_layer_resolution_dirty_system`
3. `resolve_dirty_runtime_render_layers_system`

Potential blockers:

1. detecting component membership changes for rule matching is harder than detecting plain component value changes,
2. some rule criteria depend on generated component kinds, so registry rebuild must stay coupled to generated registry updates.

Compromise:

1. do not attempt perfect minimal invalidation first,
2. start with coarse-grained registry versioning plus dirty tags,
3. later refine component-kind-specific dirtiness if profiling still justifies it.

Acceptance criteria:

1. no full cloned `Vec` rebuild every frame under stable layer state,
2. no world-wide assignment sweep every frame under stable world state,
3. layer changes from Lua/replication still propagate correctly.

### Phase 3: Replace frame-polled asset dependency discovery with a dirty queue

Goal: stop rescanning all render dependencies every frame.

Primary files:

1. `bins/sidereal-client/src/native/assets.rs`
2. `bins/sidereal-client/src/native/backdrop.rs`
3. `bins/sidereal-client/src/native/render_layers.rs`

Implementation direction:

1. Maintain a resource containing the currently required asset closure for runtime visuals.
2. Update that resource only when relevant authored/render components change.
3. Push newly required asset IDs into a queue.
4. Let the existing fetch logic consume the queue without recomputing the world-wide candidate set each frame.

Suggested resource:

```rust
#[derive(Resource, Default)]
struct RuntimeAssetDemandState {
    required_asset_ids: HashSet<String>,
    pending_queue: VecDeque<String>,
}
```

Suggested update sources:

1. `RuntimeRenderLayerDefinition`
2. `RuntimePostProcessStack`
3. `SpriteShaderAssetId`
4. `StreamedSpriteShaderAssetId`
5. `StreamedVisualAssetId`
6. runtime world visual stack/pass data

Potential blockers:

1. dependency closure expansion still requires catalog lookups,
2. asset invalidation on catalog changes or cache eviction must also mark demand dirty,
3. live shader edits coming from dashboard tooling need a reliable invalidation path.

Compromise:

1. initial version can keep one coarse "asset demand dirty" flag,
2. do not block the optimization on perfect per-entity asset dependency indexing.

Acceptance criteria:

1. stable scene with no new content -> no repeated world-wide asset dependency rebuild,
2. adding a new projectile visual or shader through Lua -> required assets enqueue automatically,
3. streamed asset arrival still unblocks the waiting visuals.

### Phase 4: Reduce material/mesh diversity without breaking live authoring

Goal: recover batching and reduce draw/setup cost while preserving dynamic content workflows.

Primary files:

1. `bins/sidereal-client/src/native/visuals.rs`
2. `bins/sidereal-client/src/native/backdrop.rs`
3. `bins/sidereal-client/src/native/shaders.rs`
4. `docs/plans/dynamic_runtime_shader_material_plan.md`

Implementation direction:

1. Keep a small fixed family ABI:
   - sprite
   - polygon
   - fullscreen
   - post-process
   - existing effect-family exceptions only where type-static Bevy constraints still require them
2. Reuse a shared quad mesh where possible instead of `meshes.add(Rectangle::new(1.0, 1.0))` per entity.
3. Reuse material handles whenever bindings are truly shareable.
4. Prefer plain `Sprite` path for content that gets little value from a custom shader.

Projectile/content workflow target:

1. Lua weapon definition selects `visual_asset_id`,
2. Lua weapon definition selects `shader_asset_id`,
3. replicated/runtime visual component references both,
4. client resolves them through existing runtime family/material ABI,
5. no restart required.

Example desired content shape:

```lua
weapon.projectile_visual = {
  visual_asset_id = "sprite.projectile.plasma_round",
  shader_asset_id = "shader.fx.plasma_edge",
  material_domain = "world_sprite",
  params = {
    glow = 0.8,
    edge_softness = 0.15,
  },
}
```

Potential compromises:

1. full batching and arbitrary per-entity live parameter freedom are in tension,
2. if every projectile instance carries unique material params, batching will still degrade,
3. best compromise is family-level sharing plus per-instance params only where gameplay/art actually needs them.

Decision point:

1. define which params are truly instance-unique,
2. push repeated/shared params into shared materials or shared resources,
3. reserve unique material instances for high-value effects.

Potential functionality loss:

1. some niche shader combinations may need to be deferred if they do not fit the approved family ABI,
2. certain "fully arbitrary shader for every effect family" expectations may need to be narrowed to "arbitrary shader asset within approved family/binding contract".

This compromise is aligned with current design philosophy and with the existing runtime shader plan.

### Phase 5: Rationalize cameras and passes

Goal: reduce pass/view overhead without regressing required composition behavior.

Primary files:

1. `bins/sidereal-client/src/native/scene_world.rs`
2. `bins/sidereal-client/src/native/scene.rs`
3. `bins/sidereal-client/src/native/backdrop.rs`

Review items:

1. backdrop camera,
2. gameplay camera,
3. debug overlay camera,
4. fullscreen foreground camera,
5. post-process camera,
6. UI overlay camera.

Implementation direction:

1. disable debug overlay camera entirely unless active,
2. evaluate whether fullscreen foreground and post-process need separate views in all cases,
3. keep UI separation if required by current composition semantics.

Potential blockers:

1. some passes may currently depend on camera layering semantics rather than an explicit compositing contract,
2. collapsing cameras too early can break fullscreen ordering or UI correctness.

Compromise:

1. treat camera collapse as a later optimization phase,
2. do not perform it before Phases 1 through 4 are measured.

Acceptance criteria:

1. no regression in fullscreen ordering,
2. no regression in tactical/UI overlays,
3. measurable reduction in extraction/render-pass overhead.

### Phase 6: Move duplicate-visual suppression closer to adoption/handoff

Goal: remove the full-world duplicate scoring pass from the steady-state frame path.

Primary files:

1. `bins/sidereal-client/src/native/visuals.rs`
2. `bins/sidereal-client/src/native/replication.rs`
3. `bins/sidereal-client/src/native/plugins.rs`

Implementation direction:

1. track presentation ownership closer to replicated entity adoption and predicted/interpolated handoff,
2. maintain a GUID -> displayed entity mapping resource,
3. only recompute on lifecycle change rather than every frame.

Potential blockers:

1. the current lifecycle may still produce short periods where multiple runtime entities exist for one GUID,
2. handoff correctness matters more than tiny visual optimization wins.

Compromise:

1. if lifecycle cleanup is too invasive for this pass, keep the existing scan behind a dirty trigger rather than every frame.

### Phase 7: Optimize replication visibility without violating the visibility contract

Goal: reduce server cadence pressure that currently contributes to render smoothness problems.

Primary files:

1. `bins/sidereal-replication/src/replication/visibility.rs`
2. `docs/features/visibility_replication_contract.md`

Implementation direction:

1. retain and reuse scratch allocations aggressively,
2. avoid rebuilding candidate sets for clients whose observer context did not materially change,
3. reuse spatial indices where entity movement/visibility source changes are bounded,
4. separate low-frequency disclosure/grid component updates from high-frequency visibility checks where possible.

Important constraint:

1. do not replace Sidereal's visibility model with Lightyear interest management,
2. do not weaken owner/public/faction visibility correctness,
3. do not break valid no-ship/no-engine observer states.

Potential blockers:

1. correctness surface is large,
2. mount-root and ownership semantics are subtle,
3. aggressive caching can easily serve stale visibility if invalidation is incomplete.

Compromise:

1. start with allocation/index reuse and observer-context dirty checks,
2. defer deeper algorithm changes until profiles prove they are necessary.

Acceptance criteria:

1. lower average and p95 visibility tick cost,
2. no replication correctness regressions,
3. smoother client update cadence under load.

## 6. Existing Files To Use As Anchors

Client render churn and composition:

1. `bins/sidereal-client/src/native/backdrop.rs`
2. `bins/sidereal-client/src/native/scene_world.rs`
3. `bins/sidereal-client/src/native/scene.rs`

Client runtime render bookkeeping:

1. `bins/sidereal-client/src/native/render_layers.rs`
2. `bins/sidereal-client/src/native/assets.rs`
3. `bins/sidereal-client/src/native/visuals.rs`
4. `bins/sidereal-client/src/native/replication.rs`
5. `bins/sidereal-client/src/native/plugins.rs`

Server-side indirect rendering pressure:

1. `bins/sidereal-replication/src/replication/visibility.rs`

Design-contract references:

1. `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-09.md`
2. `docs/plans/dynamic_runtime_shader_material_plan.md`
3. `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
4. `docs/features/visibility_replication_contract.md`
5. `AGENTS.md`

## 7. Decisions And Compromises Likely Required

### 7.1 Shared materials vs live-tweak freedom

Tradeoff:

1. more shared materials improve batching,
2. more instance-unique params improve live-edit flexibility.

Recommended compromise:

1. share by default,
2. allow unique instances only for explicitly author-requested per-instance params,
3. document which params are family-shared vs instance-owned.

### 7.2 Generic runtime family boundary vs total shader freedom

Tradeoff:

1. the project wants dynamic shader spawning and swapping,
2. Bevy still requires type-static material schemas.

Recommended compromise:

1. arbitrary shader assets are allowed only within approved family/binding ABI,
2. adding a truly new shader family remains a Rust-side engine change.

This is already consistent with the existing runtime shader plan and should stay explicit.

### 7.3 Camera simplification vs composition clarity

Tradeoff:

1. fewer cameras reduce overhead,
2. separate cameras can preserve clear composition semantics.

Recommended compromise:

1. fix churn and frame scans first,
2. only collapse cameras after measuring the simpler wins.

### 7.4 Interpolation ownership

Tradeoff:

1. Avian interpolation may offer value in some contexts,
2. Lightyear interpolation is already integrated and helping.

Recommended compromise:

1. keep the current Lightyear interpolation path,
2. do not layer Avian interpolation on top during this pass.

## 8. Things That Would Go Against Project Philosophy

Do not do these during the optimization pass:

1. remove Lua/data-driven runtime shader selection in favor of hardcoded Rust visual mappings,
2. require client or server restart for normal content shader swaps,
3. move authoritative visual/gameplay routing decisions entirely client-side,
4. break parity by introducing native-only gameplay/render contracts instead of platform-boundary differences,
5. reintroduce simulation-state writes from render/parallax systems,
6. replace the visibility contract with a simpler but less correct replication rule,
7. optimize by disabling owner/public/faction visibility semantics.

## 9. Recommended Execution Sequence

Recommended implementation order:

1. Phase 0 instrumentation
2. Phase 1 fullscreen/post-process churn fix
3. Phase 2 render-layer incrementalization
4. Phase 3 asset-demand dirty queue
5. Phase 4 material/mesh reuse and batching recovery
6. Phase 6 duplicate-visual suppression lifecycle tightening
7. Phase 5 camera/pass rationalization
8. Phase 7 replication visibility optimization

Reasoning:

1. Phase 1 is the highest-confidence win.
2. Phases 2 and 3 remove obvious steady-state CPU overhead.
3. Phase 4 is important but needs the earlier invalidation/caching work first.
4. Phase 5 should not be attempted before easier wins are measured.
5. Phase 7 is critical, but it touches correctness-heavy code and should follow cleaner client-side baselines unless the server profile shows it is already the dominant bottleneck.

## 10. Verification Plan

For each phase:

1. compare before/after counters,
2. run targeted gameplay scenarios with many entities, layered backgrounds, and post-process enabled,
3. validate live dashboard shader param edits,
4. validate runtime shader swap without restart,
5. validate new projectile visual/shader attach without restart,
6. validate no regression in predicted/interpolated smoothing.

Minimum command gates after code changes touching the client/runtime:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

Targeted runtime tests to add or expand:

1. fullscreen renderable sync only rebuilds on authored change,
2. runtime layer registry version bumps only on relevant authored changes,
3. asset-demand queue updates when new runtime shader references appear,
4. projectile visual/shader spawn path resolves without restart,
5. visibility optimization preserves owner/public/faction semantics.

## 11. Exit Criteria

This optimization pass is complete when:

1. unchanged fullscreen/post-process content produces no per-frame mesh/material churn,
2. stable scenes no longer run render-layer and asset-demand world scans every frame,
3. client frame pacing measurably improves in representative scenes,
4. replication visibility cost is reduced or shown not to be the active limiter,
5. live shader/dashboard workflows still function,
6. Lua-authored projectile and runtime visual additions still work without restart,
7. no change violates the contracts in `AGENTS.md`, DR-0027, the dynamic runtime shader plan, or the visibility contract.

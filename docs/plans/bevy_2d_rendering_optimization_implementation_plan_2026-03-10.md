# Bevy 2D Rendering Optimization Implementation Plan

Status: Proposed implementation plan
Date: 2026-03-10
Owners: client rendering + replication + asset delivery + diagnostics

Primary input:
- `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-10.md`

Guardrails:
- Preserve `DR-0027` and the Lua-authored render-layer/runtime shader direction.
- Preserve Lightyear transform interpolation and the current post-correction camera follow ordering.
- Do not introduce native-only architecture that blocks later WASM parity recovery.
- Native impact: primary optimization target.
- WASM impact: keep change-driven invalidation, render-layer resolution, and asset dependency tracking in shared client code where possible; no platform-specific render architecture fork is planned in this pass.

## 1. Objective

Reduce perceived slowness and actual frame-time cost by fixing the hot paths that the March 10 audit identified as highest ROI:

1. fullscreen/post-process renderable churn,
2. frame-polled render-layer registry and assignment rebuilds,
3. frame-polled asset dependency discovery,
4. duplicate visual arbitration scans,
5. replication visibility cadence pressure.

Success criteria:

1. unchanged fullscreen and post-process authored state causes zero new mesh allocations per frame,
2. unchanged authored render-layer state causes zero registry recompiles per frame,
3. unchanged asset references cause zero whole-world dependency rescans per frame,
4. duplicate visual suppression no longer scans all GUID-bearing world entities every frame,
5. visibility tick cost and candidate counts are measurable and reduced under the same load.

## 2. Phase Order

### Phase 0: Instrumentation and Baseline

Goal:
Create the counters and timers needed to prove each later phase helped.

Work:

1. Add lightweight timing resources or diagnostics around:
   - `sync_fullscreen_layer_renderables_system`
   - `sync_runtime_post_process_renderables_system`
   - `sync_runtime_render_layer_registry_system`
   - `resolve_runtime_render_layer_assignments_system`
   - `queue_missing_catalog_assets_system`
   - `suppress_duplicate_predicted_interpolated_visuals_system`
   - `update_network_visibility`
2. Add counters for:
   - fullscreen quad allocations,
   - fullscreen material allocations,
   - post-process quad allocations,
   - render-layer registry rebuild count,
   - render-layer assignment update count,
   - asset dependency graph rebuild count,
   - asset dependency scan count,
   - duplicate GUID group count,
   - duplicate winner swap count,
   - visibility candidate count per client,
   - visibility tick duration.
3. Expose these metrics through a debug resource and, if cheap enough, the existing debug overlay or BRP path.

Files expected:

1. `bins/sidereal-client/src/native/resources.rs`
2. `bins/sidereal-client/src/native/plugins.rs`
3. `bins/sidereal-client/src/native/backdrop.rs`
4. `bins/sidereal-client/src/native/render_layers.rs`
5. `bins/sidereal-client/src/native/assets.rs`
6. `bins/sidereal-client/src/native/visuals.rs`
7. `bins/sidereal-replication/src/replication/visibility.rs`

Acceptance criteria:

1. Before/after metrics exist for every later phase.
2. Instrumentation overhead is bounded and can be gated if needed.

### Phase 1: Eliminate Fullscreen and Post-Process Churn

Goal:
Stop recreating unchanged fullscreen/post-process renderables every frame.

Work:

1. Add a client resource holding:
   - one cached fullscreen quad mesh handle,
   - optional cached material handles keyed by stable fullscreen binding content.
2. Refactor fullscreen sync into:
   - stale removal,
   - create missing renderables,
   - mutate existing renderables only when order, phase, or binding key changes.
3. Refactor post-process sync the same way.
4. Replace `attach_runtime_fullscreen_material()` with a cache-aware path that does not clear and recreate material assets when the binding key is unchanged.

Suggested resource direction:

```rust
#[derive(Resource, Default)]
struct FullscreenRenderCache {
    quad_mesh: Handle<Mesh>,
    material_by_key: HashMap<RuntimeFullscreenBindingKey, RuntimeFullscreenMaterialHandle>,
}
```

Tests:

1. Unit tests for binding-key equality and cache invalidation.
2. Client-side tests that unchanged authored fullscreen state keeps the same mesh/material handles.

Docs:

1. Update `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-10.md` only if implementation materially changes conclusions.
2. If the runtime behavior contract changes for fullscreen authoring, update `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`.

Acceptance criteria:

1. Unchanged fullscreen state allocates zero new fullscreen meshes per frame.
2. Unchanged fullscreen state allocates zero new fullscreen materials per frame.
3. Live shader/params updates still trigger a one-time correct rebind.

### Phase 2: Make Render-Layer Compilation and Assignment Incremental

Goal:
Convert render-layer authoring from whole-world frame polling into change-driven invalidation.

Work:

1. Introduce a dirty flag or generation resource for runtime render-layer authored state.
2. Rebuild `RuntimeRenderLayerRegistry` only when relevant authored components change or are removed.
3. Track which world entities need layer reassignment because labels, overrides, or relevant match components changed.
4. Keep compiled component-ID lookup tables stable until `GeneratedComponentRegistry` changes.

Files expected:

1. `bins/sidereal-client/src/native/render_layers.rs`
2. `bins/sidereal-client/src/native/resources.rs`
3. `bins/sidereal-client/src/native/plugins.rs`

Tests:

1. Registry rebuild only occurs on relevant authored-state changes.
2. Entity assignment updates only occur when rule inputs change.
3. Default `main_world` fallback remains unchanged.

Acceptance criteria:

1. Stable frame with no authored-state changes performs zero registry rebuilds.
2. Stable frame with no relevant entity-state changes performs zero assignment rewrites.

### Phase 3: Replace Runtime Asset Discovery Polling With a Dirty Dependency Graph

Goal:
Keep authoritative runtime lazy fetch behavior while removing whole-world dependency rescans from the hot frame path.

Work:

1. Add a resource that tracks required runtime asset IDs and dependency closure.
2. Recompute that resource only when:
   - fullscreen layers change,
   - runtime render layers change,
   - post-process stacks change,
   - streamed visual asset IDs change,
   - sprite shader asset IDs change,
   - catalog generation changes.
3. Replace `queue_missing_catalog_assets_system()` world polling with queue consumption from the precomputed pending asset set.

Files expected:

1. `bins/sidereal-client/src/native/assets.rs`
2. `bins/sidereal-client/src/native/resources.rs`
3. `bins/sidereal-client/src/native/plugins.rs`

Tests:

1. New asset reference schedules fetch once.
2. Removed asset reference clears pending state correctly.
3. Dependency closure still fetches prerequisites before dependents.

Docs:

1. Update `docs/features/asset_delivery_contract.md` only if the runtime contract changes. The current intended change is implementation-only.

Acceptance criteria:

1. Stable frame with no dependency changes performs zero dependency rescans.
2. Runtime optional asset fetch semantics remain authoritative and dependency-safe.

### Phase 4: Reduce Per-Frame Presentation Arbitration and Auxiliary Overlay Work

Goal:
Remove whole-world duplicate visual winner selection from the normal frame path and trim always-on overlay work.

Work:

1. Introduce a winner-per-GUID resource updated on adoption, despawn, control handoff, and relevant marker changes.
2. Make `suppress_duplicate_predicted_interpolated_visuals_system()` consume that resource instead of rescanning the full world every frame, or eliminate the system entirely if the lifecycle becomes event-driven enough.
3. Gate debug overlay camera activation and debug overlay systems behind `DebugOverlayState.enabled`.
4. Convert `propagate_ui_overlay_layer_system()` to spawn-time or dirty-only behavior.

Files expected:

1. `bins/sidereal-client/src/native/visuals.rs`
2. `bins/sidereal-client/src/native/debug_overlay.rs`
3. `bins/sidereal-client/src/native/ui.rs`
4. `bins/sidereal-client/src/native/scene_world.rs`
5. `bins/sidereal-client/src/native/plugins.rs`

Tests:

1. Duplicate winner remains stable across handoff cases.
2. Overlay-disabled frames do not run overlay collection/update work unnecessarily.
3. UI overlay descendants still receive the right render layer when spawned or reparented.

Acceptance criteria:

1. No whole-world duplicate GUID arbitration in steady-state frames.
2. Debug overlay off means debug overlay camera and debug overlay systems are not active.

### Phase 5: Reduce Material/Mesh Diversity Where It Is Cheap To Do So

Goal:
Lower draw-call and bind-group pressure without fighting the type-static Rust material boundary.

Work:

1. Add shared unit quad mesh resources for shader-backed world visuals and effects.
2. Audit which material instances genuinely need unique per-entity data and which can share handles or uniform buckets.
3. Budget planet multi-pass visuals explicitly and avoid accidental expansion of always-on pass count.

Files expected:

1. `bins/sidereal-client/src/native/visuals.rs`
2. `bins/sidereal-client/src/native/resources.rs`
3. `bins/sidereal-client/src/native/backdrop.rs`

Tests:

1. Shared-geometry resources survive hot reload and scene reload correctly.
2. Material sharing does not leak parameters between unrelated visuals.

Acceptance criteria:

1. Shared geometry replaces repeated `Rectangle::new(1.0, 1.0)` allocations in steady-state paths.
2. Draw-call and material-instance counts drop in representative authored scenes.

### Phase 6: Optimize Replication Visibility Cadence

Goal:
Reduce server-side visibility work that destabilizes client render smoothness.

Work:

1. Persist scratch indices across ticks where safe instead of clearing and rebuilding all maps and sets every tick.
2. Move static-landmark discovery onto a lower-frequency or change-driven path separate from per-tick delivery updates.
3. Cache resolved world-layer lookup inputs used by discovered-landmark delivery adjustments.
4. Use the Phase 0 metrics to identify which part of `update_network_visibility()` dominates before refactoring the full flow.

Files expected:

1. `bins/sidereal-replication/src/replication/visibility.rs`
2. `bins/sidereal-replication/src/plugins.rs`
3. Possibly shared visibility-related resources/components if cache generations are needed

Tests:

1. Visibility authorization/delivery semantics remain unchanged.
2. Discovered landmark behavior remains correct.
3. Owner/public/faction/global render-config bypass rules remain correct.

Docs:

1. Update `docs/features/visibility_replication_contract.md` if behavior changes.
2. If only caching and scheduling change, keep the contract unchanged and note no policy change in code comments/tests.

Acceptance criteria:

1. Visibility tick duration drops under comparable entity/client load.
2. Candidate counts and delivery results remain correct.

## 3. Validation Matrix

For each implementation phase:

1. Run touched crate unit tests.
2. Run relevant client or replication integration tests when visibility/adoption flow changes.
3. Run:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo check --workspace`
4. Because this plan targets client code, also run:
   - `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
   - `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`

## 4. Rollout Order Recommendation

Recommended implementation order:

1. Phase 0
2. Phase 1
3. Phase 2
4. Phase 3
5. Re-measure
6. Phase 4
7. Re-measure
8. Phase 6
9. Phase 5 only after the earlier phases prove the remaining render-path bottleneck is material diversity rather than ECS churn

Reason:

1. Phases 1 through 3 remove proven hot-path waste.
2. Phase 4 simplifies steady-state presentation cost and overlay overhead.
3. Phase 6 addresses the largest indirect source of visual instability.
4. Phase 5 is worth doing, but only after the obvious frame-polled churn is gone.

## 5. Definition of Done

This optimization pass is complete when:

1. the Phase 0 metrics exist and are easy to compare,
2. fullscreen/post-process steady-state churn is eliminated,
3. render-layer and asset dependency work are change-driven,
4. duplicate visual arbitration is no longer a full-world per-frame scan,
5. visibility tick cost is reduced without violating the visibility contract,
6. the native client feels materially smoother under the same authored load,
7. no native-only shortcuts make later WASM parity recovery harder.

# Bevy 2D Rendering Optimization Audit Report

Status: Active
Date: 2026-03-10
Prompt source: `docs/prompts/bevy_2d_rendering_optimization_audit_prompt.md`
Scope: Native client render smoothness, render-adjacent ECS/runtime cost, replication/visibility delivery cadence, and asset/shader/material behavior that affects perceived or actual 2D rendering performance.
Limitations: Static code audit only. No live GPU captures, renderdoc traces, puffin/tracy captures, packet captures, or long-session memory graphs were available. Any GPU-bound or frame-spike conclusions not directly proven by code are labeled as inference.

Update note (2026-03-10):
- The major March 9 client churn findings are still present in the current codebase.
- The highest-ROI work is still to remove per-frame fullscreen/post-process mesh and material churn, then convert whole-world render-layer and asset-dependency polling into change-driven invalidation.
- The current render-layer direction from `DR-0027` remains correct; the problem is hot-path execution cost, not the data-driven composition model itself.

## 1. Executive Summary

The game does not read like a normal-play 2D renderer that is mainly GPU-bound. It reads like a client CPU and frame-pacing problem, amplified by replication cadence pressure and a few expensive render-path design choices.

The strongest current findings are:

1. Fullscreen and post-process renderables still allocate new quad meshes and new material assets every `Update`, even for unchanged passes. This is a live churn bug, not a tuning issue, in `bins/sidereal-client/src/native/backdrop.rs:37-168` and `bins/sidereal-client/src/native/backdrop.rs:263-444`.
2. The client still recompiles render-layer registry state and re-resolves world-layer assignments by cloning and rescanning ECS state every frame in `bins/sidereal-client/src/native/render_layers.rs:15-200`.
3. Runtime optional asset discovery still polls the entire visible render dependency surface every frame in `bins/sidereal-client/src/native/assets.rs:246-360`.
4. Duplicate predicted/interpolated visual suppression still does a full-world GUID arbitration pass every frame in `bins/sidereal-client/src/native/visuals.rs:268-360`.
5. The replication server still rebuilds scratch maps, candidate sets, landmark discovery scans, and per-client visibility decisions every tick in `bins/sidereal-replication/src/replication/visibility.rs:594-1425`, which can indirectly hurt client render smoothness by making authoritative update cadence uneven under load.

One important smoothness path is correct and should be kept:

1. The client uses Lightyear transform frame interpolation in `bins/sidereal-client/src/native/mod.rs:129-147`.
2. The runtime explicitly keeps `FrameInterpolate<Transform>` aligned for replicated world entities in `bins/sidereal-client/src/native/transforms.rs:184-223`.
3. Camera follow was intentionally moved to the post-interpolation/post-correction lane in `bins/sidereal-client/src/native/plugins.rs:480-509`.

That means the top issue is not "add interpolation." The top issue is "stop doing obviously expensive work every frame around the interpolation path."

## 2. What Most Likely Makes The Game Feel Slow

Ordered by expected impact:

1. Per-frame fullscreen/post-process mesh and material churn causes render-world churn, allocations, and likely long-tail frame spikes.
2. Client `Update` still carries too much always-on render-adjacent bookkeeping, including render-layer rebuilds, asset dependency scans, duplicate arbitration, visual attachment/update, UI propagation, and backdrop sync.
3. Replication visibility work can produce bursty or uneven authoritative delivery cadence, which makes the renderer feel worse even when average FPS is acceptable.
4. Shader-backed 2D visuals are still material-heavy and mesh-heavy enough to reduce batching materially in large scenes.
5. Multiple always-on cameras and overlay passes increase extraction/pass overhead before the scene is cheap enough to justify them.

## 3. Findings

### C1. Fullscreen and post-process renderables are still rebuilt every frame

Severity: Critical
Confidence: Proven
Primary impact: Client CPU, allocator churn, frame pacing, likely long-session slowdown

Evidence:

1. `sync_fullscreen_layer_renderables_system()` creates a new fullscreen quad handle every run and reinserts fullscreen render components for selected entities in `bins/sidereal-client/src/native/backdrop.rs:37-168`.
2. `sync_runtime_post_process_renderables_system()` creates a fresh quad for existing post-process renderables as well as new ones in `bins/sidereal-client/src/native/backdrop.rs:263-385`.
3. `attach_runtime_fullscreen_material()` always clears material bindings and allocates a fresh `Material2d` asset in `bins/sidereal-client/src/native/backdrop.rs:395-444`.

Why it matters:

1. Fullscreen entities are few, so they should be cheap.
2. The current path makes them expensive every frame even when authored state is unchanged.
3. This is exactly the kind of bug that creates "feels worse after a while" reports because the renderer is constantly asked to process changed mesh/material state that is not actually new content.

Recommendation:

1. Introduce one cached fullscreen quad handle resource.
2. Split fullscreen/post-process sync into create, mutate-in-place, and stale-removal paths.
3. Cache material handles by stable binding key and only rebuild when shader family, params asset, or texture bindings actually change.

### C2. Render-layer registry compilation and world-layer assignment are still frame-polled

Severity: High
Confidence: Proven
Primary impact: Client CPU, schedule pressure, change-detection noise

Evidence:

1. `sync_runtime_render_layer_registry_system()` clones generated component metadata, layer definitions, rules, and post-process stacks into temporary `Vec`s every run in `bins/sidereal-client/src/native/render_layers.rs:15-149`.
2. `resolve_runtime_render_layer_assignments_system()` clones per-entity labels, overrides, and current resolved state into another `Vec`, then revisits each entity to re-resolve assignments in `bins/sidereal-client/src/native/render_layers.rs:151-200`.

Why it matters:

1. `DR-0027` explicitly wants bounded, deterministic rule evaluation rather than arbitrary per-frame callbacks.
2. The current direction is architecturally correct, but the implementation is still "recompute from whole-world snapshots each frame."
3. This is likely one of the largest avoidable CPU costs in authored-heavy scenes.

Recommendation:

1. Rebuild registry state only on `Added`, `Changed`, and removed-component signals for `RuntimeRenderLayerDefinition`, `RuntimeRenderLayerRule`, and `RuntimePostProcessStack`.
2. Resolve world-layer assignments only for entities whose relevant labels/components/overrides changed.
3. Persist compiled component-ID lookups until generated registry contents change.

### C3. Runtime optional asset discovery is still a whole-world polling loop

Severity: High
Confidence: Proven
Primary impact: Client CPU, startup and hot-reload hitching, asset-fetch latency

Evidence:

1. `queue_missing_catalog_assets_system()` scans fullscreen layers, runtime render layers, post-process stacks, sprite shader asset IDs, streamed shader asset IDs, and streamed visual asset IDs every frame in `bins/sidereal-client/src/native/assets.rs:246-360`.
2. The system rebuilds a `HashSet`, expands dependency closure, then chooses a single next asset fetch candidate each run.

Why it matters:

1. The asset delivery contract says runtime fetch is lazy and authoritative, but not that dependency discovery must be frame-polled.
2. This polling shape is especially bad when world authoring state is stable but the client is just waiting for a few assets.
3. It also increases the chance that hot reloads and optional asset attachment feel hitchy because discovery work and presentation work share the same frame budget.

Recommendation:

1. Build a dirty dependency graph resource keyed by authored render-layer, post-process, and visual components.
2. Maintain a pending asset queue resource that the HTTP fetch system consumes without rescanning the world.
3. Emit explicit invalidation when catalog generation or shader assignment generation changes.

### C4. Duplicate predicted/interpolated visual arbitration still scans the whole replicated world every frame

Severity: Medium
Confidence: Proven
Primary impact: Client CPU, presentation churn, architecture complexity

Evidence:

1. `suppress_duplicate_predicted_interpolated_visuals_system()` scores every `WorldEntity` with a GUID, then performs a second pass to hide non-winners in `bins/sidereal-client/src/native/visuals.rs:268-360`.

Why it matters:

1. This is a symptom of noisy entity lifecycle and presentation handoff, not a long-term steady-state render architecture.
2. Even if the query is not the top hot path, it keeps duplicate presentation cost alive every frame.

Recommendation:

1. Move winner selection closer to replication adoption and control-handoff transitions.
2. Maintain one displayed winner per GUID class in a dedicated resource instead of rescoring the full world every frame.
3. Add counters for duplicate-GUID groups and winner swaps so the team can prove whether lifecycle work is actually improving.

### C5. Shader-backed streamed visuals and planet passes still trade batching away aggressively

Severity: High
Confidence: Strong inference
Primary impact: Client CPU draw submission, client GPU state churn

Evidence:

1. Streamed asteroid and generic shader-backed sprite paths allocate per-entity quad meshes and per-entity materials in `bins/sidereal-client/src/native/visuals.rs:618-672`.
2. Planet body/cloud/ring passes allocate separate meshes and separate materials per pass in `bins/sidereal-client/src/native/visuals.rs:955-1160`.
3. Several planet and fullscreen passes are marked `NoFrustumCulling`, which is correct for some paths but increases the cost of material-heavy passes if entity counts grow.

Why it matters:

1. Plain sprites can batch well; custom `Material2d` paths batch much less effectively when each entity or pass owns a unique material instance.
2. This does not mean the data-driven shader direction is wrong.
3. It means the remaining transitional Rust-owned material families need stronger pooling, instancing, or content-budget discipline.

Recommendation:

1. Reuse a shared unit quad mesh resource across shader-backed 2D visuals.
2. Collapse material diversity where parameters can be moved into textures, atlases, or shared uniform buckets.
3. Budget planet multi-pass visuals more explicitly and avoid treating every large body as an unconstrained multi-pass material stack.

### C6. The client still keeps too many always-on views and render-adjacent systems active

Severity: Medium
Confidence: Proven
Primary impact: Client CPU, client GPU, pass overhead

Evidence:

1. World scene spawn creates separate backdrop, gameplay, debug overlay, fullscreen foreground, and post-process cameras in `bins/sidereal-client/src/native/scene_world.rs:64-156`.
2. The debug overlay camera starts active by default in `bins/sidereal-client/src/native/scene_world.rs:115-128`.
3. UI systems still propagate `RenderLayers` to descendants every frame in `bins/sidereal-client/src/native/ui.rs:51-64`.
4. Debug overlay collection and text update systems still run every in-world frame even when the overlay is disabled, though they early-return after entering the systems in `bins/sidereal-client/src/native/plugins.rs:511-523`, `bins/sidereal-client/src/native/debug_overlay.rs:59-99`, and `bins/sidereal-client/src/native/ui.rs:131-220`.

Why it matters:

1. A few separate cameras are acceptable if the scene is otherwise cheap.
2. This scene is not otherwise cheap yet.
3. The cheapest overlay system is the one that does not enter the schedule that frame.

Recommendation:

1. Gate the debug overlay camera and debug overlay systems behind `DebugOverlayState.enabled`.
2. Revisit whether fullscreen foreground and post-process need separate always-on cameras.
3. Make UI render-layer propagation event-driven or spawn-time-only.

### C7. Startup shader reload is still coarse-grained but not the primary current problem

Severity: Medium
Confidence: Proven plus inference
Primary impact: Startup hitching, hot-reload hitching

Evidence:

1. `spawn_world_scene()` reloads streamed shaders during scene entry in `bins/sidereal-client/src/native/scene_world.rs:55-63`.
2. `reload_streamed_shaders()` reinstalls runtime shader handles from current assignments in `bins/sidereal-client/src/native/shaders.rs:569-583`.
3. Fullscreen readiness currently returns `true` even if a streamed shader asset is absent because fullscreen material handles always have a fallback installed in `bins/sidereal-client/src/native/shaders.rs:585-598`.

Why it matters:

1. Startup reload cost is real, but it is not as severe as the proven per-frame churn.
2. The current path is directionally compatible with the asset/shader contracts because fallback shaders are allowed.
3. The main remaining risk is hitching around coarse reload boundaries, not a steady-state render bottleneck.

Recommendation:

1. Keep the authored shader pipeline.
2. Add prewarm timing and reload-generation counters before attempting deeper redesign.
3. Defer larger shader-pipeline changes until per-frame churn is removed.

### C8. Server visibility work is still expensive enough to degrade render smoothness indirectly

Severity: High
Confidence: Proven
Primary impact: Authoritative update cadence, client frame pacing under load

Evidence:

1. `update_network_visibility()` clears and rebuilds scratch state each tick in `bins/sidereal-replication/src/replication/visibility.rs:670-910`.
2. It rescans replicated entities for landmark discovery per client in `bins/sidereal-replication/src/replication/visibility.rs:912-1060`.
3. It then loops all replicated entities and all live clients again to mutate `ReplicationState` in `bins/sidereal-replication/src/replication/visibility.rs:1124-1389`.

Why it matters:

1. This is not in the renderer, but it can absolutely make rendering feel bad by producing bursty delivery and frequent client-side adoption/correction churn.
2. The visibility contract is still correct: authorization then delivery then payload.
3. The implementation is doing too much repeated work per tick.

Recommendation:

1. Persist more spatial/ownership/layer-resolution caches across ticks instead of rebuilding them from scratch.
2. Separate low-frequency landmark discovery from per-tick visibility delivery.
3. Add timing, candidate-count, and visible-entity-count telemetry before changing policy.

## 4. Required Confirm / Refute Set

1. The game is GPU-bound in normal gameplay: Not proven and unlikely to be the primary bottleneck from code inspection alone.
2. The game is CPU-bound on the client in normal gameplay: Likely true.
3. The game is bottlenecked by ECS scheduling/query work more than actual draw submission: Likely true today.
4. The game is bottlenecked by replication/update churn more than rendering itself: Partly true. Replication cadence is a major indirect contributor, but there are also direct client render-path bugs.
5. The game feels slow mainly because of frame pacing/interpolation issues rather than raw frame time: Likely true in practice. The interpolation lane is present; the remaining problem is uneven surrounding work.
6. Shader/material diversity is defeating batching enough to matter: Likely true in authored-heavy scenes.
7. Too many fullscreen or post-process passes are active for the current visual payoff: Likely true.
8. Off-screen or non-visible entities are still paying too much render-related cost: Likely true, mostly via schedule work and non-cullable effect paths.
9. Asset/shader compilation or loading hitching is a meaningful source of stalls: Plausible, but secondary to the proven per-frame churn.
10. Server-side visibility/replication behavior is causing client render instability or overload: Likely true under load.
11. The current render-layer architecture is directionally correct and should be kept: Yes.
12. The current render-layer/material implementation has avoidable transitional cost that should be simplified: Yes.

## 5. End-to-End Render Flow Map

### 5.1 Asset/bootstrap to client-ready rendering

1. Client starts with streamed shader/material plugins and frame-time diagnostics in `bins/sidereal-client/src/native/mod.rs:336-363`.
2. World-scene entry reloads streamed shader handles and spawns the active camera stack in `bins/sidereal-client/src/native/scene_world.rs:55-156`.
3. Runtime asset polling discovers referenced shader, params, texture, and visual asset IDs in `bins/sidereal-client/src/native/assets.rs:246-360`.
4. Visual attach systems bind sprites, shader-backed visuals, fullscreen passes, and overlay passes during `Update` in `bins/sidereal-client/src/native/plugins.rs:324-398` and `bins/sidereal-client/src/native/plugins.rs:458-523`.

### 5.2 Replicated entity arrival to visible draw

1. Lightyear/Avian replication is configured in `bins/sidereal-client/src/native/mod.rs:126-147`.
2. Replicated entities are adopted, spatial components are bootstrapped, and transform interpolation markers are synchronized in client replication/update systems before visuals run.
3. Render-layer rules resolve world-layer assignment for world entities in `bins/sidereal-client/src/native/render_layers.rs:151-225`.
4. Visual attach systems spawn streamed visual children, shader-backed quads, and planet pass children in `bins/sidereal-client/src/native/visuals.rs:520-695` and `bins/sidereal-client/src/native/visuals.rs:955-1160`.
5. PostUpdate interpolation/correction and camera follow then drive the final rendered transforms in `bins/sidereal-client/src/native/plugins.rs:480-509`.

### 5.3 Camera-relative/world-layer transform derivation

1. Authoritative world positions remain unchanged.
2. Client render systems derive local offsets and screen-scale from resolved layer definitions when updating streamed visuals and planet passes.
3. This matches `DR-0027`: render-layer parallax is render-time behavior, not gameplay-space mutation.

### 5.4 Fullscreen background/foreground/post-process execution

1. Authored fullscreen definitions and authored post-process stacks are replicated as ECS data.
2. Client fullscreen sync systems turn that authored state into renderable fullscreen quads every frame in `bins/sidereal-client/src/native/backdrop.rs:37-168` and `bins/sidereal-client/src/native/backdrop.rs:263-385`.
3. Backdrop cameras are normalized in `sync_backdrop_camera_system()` and fullscreen quads are scaled to viewport size in `sync_backdrop_fullscreen_system()`.

### 5.5 Prediction/reconciliation/interpolation to final motion

1. Fixed-step gameplay and prediction run at 60 Hz in `bins/sidereal-client/src/native/mod.rs:147` and client prediction plugins.
2. Lightyear frame interpolation is enabled in `bins/sidereal-client/src/native/mod.rs:135`.
3. Sidereal keeps `FrameInterpolate<Transform>` markers aligned for relevant entities in `bins/sidereal-client/src/native/transforms.rs:184-223`.
4. Camera follow runs after interpolation and rollback visual correction in `bins/sidereal-client/src/native/plugins.rs:480-509`.

## 6. Performance Budget Map

This section is partly inferential.

### 6.1 Client CPU

Highest likely costs:

1. Fullscreen/post-process sync churn.
2. Render-layer registry rebuild and assignment resolution.
3. Runtime asset dependency polling.
4. Duplicate visual arbitration.
5. Visual attach/update systems for shader-backed sprites, planets, plumes, tracers, and overlays.

### 6.2 Client GPU

Highest likely costs:

1. Multiple always-on cameras and overlay passes.
2. Material-heavy fullscreen, shader-backed sprite, planet, and effect passes.
3. Alpha-blended fullscreen and effect layers.
4. Reduced batching from per-entity/per-pass material diversity.

### 6.3 Client main-thread stalls

Highest likely costs:

1. Mesh/material asset allocation churn.
2. Shader reload/install work on scene entry or hot reload.
3. Runtime asset cache/mount checks in frame-sensitive systems.

### 6.4 Server tick cost affecting visual smoothness

Highest likely costs:

1. Visibility scratch rebuilds.
2. Per-client candidate-set construction.
3. Landmark discovery rescans.
4. Per-entity x per-client visibility mutation loop.

### 6.5 Network/replication delivery cost affecting render churn

Highest likely costs:

1. Uneven delivery cadence from visibility work.
2. Duplicate presentation lifecycles during handoff/relevance changes.
3. Asset-driven visual attach churn when newly relevant entities reference not-yet-ready content.

## 7. Top Remediation Plan

### 7.1 Top 5 highest-ROI changes

1. Remove per-frame fullscreen/post-process mesh and material churn.
2. Make render-layer registry compilation and assignment incremental.
3. Replace runtime asset dependency polling with change-driven invalidation plus a pending fetch queue.
4. Reduce duplicate predicted/interpolated visual arbitration to event-driven lifecycle changes.
5. Add visibility tick telemetry, then reduce scratch rebuild and per-client rescans on the replication server.

### 7.2 Quick wins

1. Cache one shared fullscreen quad mesh.
2. Cache one shared unit quad mesh for shader-backed 2D visuals where geometry is identical.
3. Gate debug overlay camera and systems behind the enabled flag.
4. Make UI overlay layer propagation spawn-time or dirty-only.

### 7.3 Medium refactors

1. Incremental render-layer registry/resource invalidation.
2. Dirty asset-dependency graph and fetch queue.
3. Winner-per-GUID presentation registry.
4. Persisted visibility scratch indices across ticks.

### 7.4 Large architectural changes

Only after measurement:

1. Reduce material family fragmentation for world sprite/effect families.
2. Revisit pass/camera composition after client CPU hot paths are fixed.

### 7.5 Order of operations

1. Instrument first.
2. Fix fullscreen/post-process churn.
3. Remove frame-polled render-layer and asset discovery loops.
4. Simplify duplicate visual lifecycle.
5. Optimize replication visibility cadence.
6. Re-measure before attempting broader material-family redesign.

### 7.6 Before/after measurements required per major fix

1. Frame-time average and 95th/99th percentile.
2. Per-system timings for the named hotspots.
3. Mesh/material allocation counts per frame.
4. Draw-call count and active-view count.
5. Visibility tick duration, candidate count, and visible entity count per client.

## 8. Instrumentation and Profiling Gaps

Missing telemetry that should be added:

1. Per-system timers for:
   - `sync_fullscreen_layer_renderables_system`
   - `sync_runtime_post_process_renderables_system`
   - `sync_runtime_render_layer_registry_system`
   - `resolve_runtime_render_layer_assignments_system`
   - `queue_missing_catalog_assets_system`
   - `suppress_duplicate_predicted_interpolated_visuals_system`
   - `update_network_visibility`
2. Counters for fullscreen mesh allocations, fullscreen material allocations, and post-process material rebinds.
3. Counters for render-layer registry rebuild count and per-frame assignment resolution count.
4. Counters for runtime asset dependency rescan count and pending-queue size.
5. Counters for duplicate GUID groups and winner swaps.
6. Replication visibility timing plus candidate counts, queried-cell counts, and visible-entity counts per client.
7. A render-budget overlay or BRP-readable resource that exposes current active cameras, fullscreen passes, post-process passes, shader-backed sprite count, and planet pass count.
8. Startup/hot-reload shader install timing.
9. GPU-side pass timing if Bevy/WGPU diagnostics make it available.

## 9. Final Assessment

The current render architecture should not be thrown away. The authored render-layer, streamed asset, and interpolation direction is sound and matches the repo's current design docs.

The current implementation still has several hot-path execution bugs and whole-world polling loops that are large enough to dominate perceived performance. The first pass should be ruthless about removing those frame-polled rebuilds before attempting bigger visual-system redesigns.

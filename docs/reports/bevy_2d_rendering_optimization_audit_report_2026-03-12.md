# Bevy 2D Rendering Optimization Audit Report

Status: Active  
Report date: 2026-03-12  
Prompt source path: `docs/prompts/bevy_2d_rendering_optimization_audit_prompt.md`  
Supersedes: `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-10.md`  
Scope: Current client-visible render smoothness across Bevy 2D rendering, camera/layer/material/shader paths, ECS scheduling, Lightyear prediction/interpolation handoff, asset/bootstrap behavior, and replication/visibility delivery that affects perceived render performance.  
Limitations: Static code audit only. No live frame captures, GPU timings, packet captures, Tracy/Puffin captures, or RenderDoc traces were available for this pass. GPU/percentile conclusions remain inferential unless directly tied to code shape.

Update note (2026-03-12):
- The March 10 critical visibility finding is no longer accurate in its original form. The replication server now maintains a persistent visibility entity cache, persistent client-context cache, persistent spatial index, split landmark-discovery lane, and diff-based membership cache in `bins/sidereal-replication/src/replication/visibility.rs`.
- The March 10 asset-bootstrap serialization finding is also no longer accurate in its original form. Required bootstrap fetches now run with bounded parallelism (`MAX_PARALLEL_BOOTSTRAP_FETCHES = 4`) in `bins/sidereal-client/src/native/auth_net.rs`.
- Runtime lazy asset fetches are likewise no longer single-flight. They now allow up to four concurrent fetches in `bins/sidereal-client/src/native/assets.rs`.
- The project has also added render/asset/visibility telemetry hooks that materially improve auditability, especially in `bins/sidereal-client/src/native/debug_overlay.rs`, `bins/sidereal-client/src/native/resources.rs`, and `bins/sidereal-replication/src/replication/visibility.rs`.

## 1. Executive Summary

The codebase is in a materially better position than it was on 2026-03-10.

The largest wins that have already landed are:

1. Visibility is no longer doing the old full scratch rebuild plus inline landmark discovery in one monolithic hot loop.
2. Asset bootstrap and runtime lazy asset fetch are no longer strictly serialized.
3. Render-layer assignment and duplicate-visual suppression are more incremental and instrumented than they were in the previous audit.

The game still does not read like a normal-play client that is primarily GPU-bound. It still reads like a mixed frame-pacing, client-CPU, and remaining replication-cadence problem.

The current highest-leverage risks are now:

1. Client-side hot-path polling and per-frame bookkeeping in render-layer, shader-assignment, tactical-map, and nameplate systems.
2. Remaining server-side visibility membership application cost, which still evaluates each replicated entity against each relevant client after candidate narrowing.
3. Material instance pressure from planet multi-pass rendering, effect pools, and fullscreen/post-process material attachment.
4. Main-thread stalls from runtime asset fetch completion, because file writes and index saves still happen through `block_on(...)` in the polling system.
5. A still-expensive baseline pass/camera shape for a client that has not yet reduced its CPU-side world/UI overhead enough to make those extra lanes cheap.

## 2. What Changed Since 2026-03-10

### Closed or substantially reduced findings

1. `Replication visibility is still a full scratch rebuild plus per-client fanout every fixed tick`
   - No longer true in the March 10 sense.
   - Current code maintains `VisibilityEntityCache`, `VisibilityClientContextCache`, `VisibilityMembershipCache`, and `VisibilitySpatialIndex` and refreshes them incrementally in `bins/sidereal-replication/src/replication/visibility.rs:240-335`, `bins/sidereal-replication/src/replication/visibility.rs:682-913`, and `bins/sidereal-replication/src/replication/visibility.rs:1531-1632`.
   - Landmark discovery is now split into `refresh_static_landmark_discoveries()` on its own cadence in `bins/sidereal-replication/src/replication/visibility.rs:928-1064`.

2. `Asset delivery is serialized in both bootstrap and lazy runtime fetch paths`
   - No longer true.
   - Bootstrap now uses `MAX_PARALLEL_BOOTSTRAP_FETCHES = 4` and bounded parallel task submission in `bins/sidereal-client/src/native/auth_net.rs:111` and `bins/sidereal-client/src/native/auth_net.rs:402-475`.
   - Runtime asset fetch now uses `MAX_CONCURRENT_RUNTIME_ASSET_FETCHES = 4` in `bins/sidereal-client/src/native/assets.rs:100-145` and `bins/sidereal-client/src/native/assets.rs:344-449`.

### Findings that remain, but in a reduced form

1. Server-side visibility is still important for smoothness, but the remaining cost is now the final membership application lane, not the old full rebuild path.
2. Duplicate predicted/interpolated suppression still exists, but it is now dirty-guid driven instead of a naive whole-world recomputation.
3. Render-layer logic still runs every frame, but it now has change detection, targeted scans, and counters.

## 3. Current Findings

### F1. Visibility cadence improved significantly, but the final membership lane is still the top remaining server-side smoothness risk

- Severity: High
- Confidence: Proven
- Main impact: `server tick variance`, `replication cadence`, `client smoothness indirectly`
- Exact references:
  - `bins/sidereal-replication/src/replication/visibility.rs:1531-1632`
  - `bins/sidereal-replication/src/replication/visibility.rs:1636-2059`
  - `bins/sidereal-replication/src/replication/visibility.rs:2083-2228`
  - `bins/sidereal-replication/src/replication/visibility.rs:2291-2316`
- Why it matters:
  - The expensive March 10 work has been split and cached correctly.
  - The remaining hot lane still loops each replicated entity across each computed client state, performs candidate checks, authorization, delivery checks, and set-diff application.
  - That is directionally better, but it still scales with `entities * relevant_clients` after candidate narrowing.
  - The code now exposes enough metrics to validate this with real runs, which is a major improvement.
- Current disposition:
  - Keep the new architecture.
  - Focus next on reducing work inside `update_network_visibility()` rather than rethinking the whole pipeline again.
- Next fix targets:
  - Separate owner-forced/config entities from ordinary range-checked entities earlier.
  - Cache more policy-class outputs per entity so the apply loop does less branching.
  - Use the new telemetry to verify `apply_ms` versus `discovery_and_candidate_ms` in live tests before changing policy code.

### F2. Asset fetch concurrency is fixed, but runtime asset completion still risks main-thread hitches

- Severity: High
- Confidence: Proven
- Main impact: `main-thread stalls`, `asset hitching`, `late visual completion`
- Exact references:
  - `bins/sidereal-client/src/native/auth_net.rs:111`
  - `bins/sidereal-client/src/native/auth_net.rs:402-475`
  - `bins/sidereal-client/src/native/assets.rs:134`
  - `bins/sidereal-client/src/native/assets.rs:344-449`
  - `bins/sidereal-client/src/native/assets.rs:452-540`
- Why it matters:
  - The fetch side is now correctly parallelized.
  - The completion side still uses `bevy::tasks::block_on(...)` inside the frame-driven polling system for task completion, cache writes, and cache-index saves.
  - This means the game can still hitch when assets finish, even though it no longer waits for them one-by-one at the network stage.
- Current disposition:
  - The old finding is closed, but a new hitch source remains.
- Next fix targets:
  - Push cache write/index-save completion fully off the frame thread.
  - Batch or defer index saves instead of saving after every completed asset.
  - Prioritize shader/material-critical assets before secondary art if hitching is still noticeable.

### F3. Client-side hot-path polling remains the strongest likely client CPU cost

- Severity: High
- Confidence: Proven
- Main impact: `client CPU`, `frame pacing`
- Exact references:
  - `bins/sidereal-client/src/native/shaders.rs:640-760`
  - `bins/sidereal-client/src/native/render_layers.rs:20-192`
  - `bins/sidereal-client/src/native/render_layers.rs:195-278`
  - `bins/sidereal-client/src/native/assets.rs:170-237`
  - `bins/sidereal-client/src/native/plugins.rs:281-348`
- Why it matters:
  - `sync_runtime_shader_assignments_system()` still scans authored layer and sprite-shader state every `Update`.
  - `sync_runtime_render_layer_registry_system()` now early-outs correctly, but it still runs every frame and still counts authored definitions/rules/stacks.
  - `resolve_runtime_render_layer_assignments_system()` is improved and targeted, but it is still part of the always-on `Update` lane.
  - `sync_runtime_asset_dependency_state_system()` also runs every frame and must decide whether dependency inputs changed.
  - Taken together, this is still a lot of always-on CPU bookkeeping before draw submission is even the question.
- Current disposition:
  - This is now more important than the old bootstrap-serialization problem.
- Next fix targets:
  - Convert shader assignment resolution into a cached authored-state product keyed by catalog reload generation plus removal cursors.
  - Make dependency-state refresh event-driven or generation-driven instead of per-frame polling.
  - Use `RenderLayerPerfCounters` and the debug overlay counters to establish whether assignment recomputes are still frequent in normal gameplay.

### F4. Tactical map and nameplate UI still do expensive per-frame world/UI reconciliation

- Severity: High
- Confidence: Proven
- Main impact: `client CPU`, `frame pacing`, `perceived slowness`
- Exact references:
  - `bins/sidereal-client/src/native/ui.rs:521-800`
  - `bins/sidereal-client/src/native/ui.rs:847-892`
  - `bins/sidereal-client/src/native/ui.rs:1670-1773`
  - `bins/sidereal-client/src/native/ui.rs:1776-1895`
  - `bins/sidereal-client/src/native/plugins.rs:417-425`
  - `bins/sidereal-client/src/native/plugins.rs:491-495`
- Why it matters:
  - Tactical map overlay still rebuilds `existing_marker_entities`, walks all live contacts, smooths them, transforms them to screen space, and upserts SVG marker entities every frame while enabled.
  - Nameplate sync still rebuilds `winner_entities` and nameplate target maps.
  - Nameplate position updates still rebuild a per-frame `entity_data_by_entity` map and then do repeated `world_to_viewport` calls.
  - Health-bar ratio updates still walk all bars for matching targets inside the root update loop.
- Current disposition:
  - This is still one of the clearest client-side “game feels slow even if FPS is acceptable” contributors in HUD-heavy scenes.
- Next fix targets:
  - Split tactical UI into lower-frequency structural updates and per-frame interpolation-only updates.
  - Gate nameplate work harder by enable state and visible-count budget.
  - Replace the current nested nameplate health-bar update path with a direct target lookup cache.

### F5. Material instance pressure is still real, especially in planet multi-pass and fullscreen/post-process lanes

- Severity: High
- Confidence: Strong inference
- Main impact: `draw-state churn`, `batching loss`, `client CPU`, `client GPU`
- Exact references:
  - `bins/sidereal-client/src/native/visuals.rs:1396-1595`
  - `bins/sidereal-client/src/native/visuals.rs:2107-2167`
  - `bins/sidereal-client/src/native/backdrop.rs:546-621`
  - `bins/sidereal-client/src/native/debug_overlay.rs:140-187`
- Why it matters:
  - Planet visuals still allocate unique `PlanetVisualMaterial` handles per entity/pass for body, cloud back/front, and ring back/front.
  - Effect pools still allocate unique `RuntimeEffectMaterial` handles per pooled tracer/spark entity at pool creation.
  - Fullscreen/post-process attachment still creates new material handles whenever the binding changes.
  - The debug overlay now counts material populations, which is useful because this is still a plausible batching blocker.
- Current disposition:
  - Mesh sharing is already correct; material sharing is the remaining issue.
- Next fix targets:
  - Pool or bucket reusable uniform patterns for common effects.
  - Treat planet pass count and planet material count as first-class budgets.
  - Consider whether fullscreen materials can be rebound from a shared small pool instead of allocated per attachment.

### F6. The baseline camera/pass layout is still heavier than the rest of the runtime can comfortably afford

- Severity: Medium
- Confidence: Proven
- Main impact: `render extraction`, `pass overhead`, `frame pacing`
- Exact references:
  - `bins/sidereal-client/src/native/scene_world.rs:61-201`
  - `bins/sidereal-client/src/native/camera.rs:281-357`
  - `bins/sidereal-client/src/native/plugins.rs:447-465`
  - `bins/sidereal-client/src/native/mod.rs:385-387`
  - `bins/sidereal-client/src/native/debug_overlay.rs:140-187`
- Why it matters:
  - In-world scene setup still uses backdrop, planet-body, gameplay, fullscreen-foreground, post-process, UI overlay, and optional debug overlay cameras.
  - This is not necessarily wrong, but it is a high baseline for a client that still has hot-path CPU work elsewhere.
  - Present mode is still `AutoVsync`, so pacing quality depends on avoiding missed frames rather than hiding them with a raw unlocked framerate.
- Current disposition:
  - Keep the architecture for now, but budget it explicitly.
- Next fix targets:
  - Use the existing active-camera and pass counters during native profiling.
  - Collapse always-on passes only after measuring whether client CPU or GPU is actually the tighter budget.

### F7. Duplicate predicted/interpolated suppression remains a transitional tax, but it is no longer the same level of risk as before

- Severity: Medium
- Confidence: Proven
- Main impact: `client CPU`, `presentation complexity`
- Exact references:
  - `bins/sidereal-client/src/native/visuals.rs:374-679`
  - `bins/sidereal-client/src/native/replication.rs:901-967`
  - `bins/sidereal-client/src/native/ui.rs:1670-1705`
  - `bins/sidereal-client/src/native/debug_overlay.rs:182-187`
- Why it matters:
  - The code still has to pick lane winners and suppress losers.
  - That logic still influences camera follow, UI, and visual attachment behavior.
  - The good news is that it is now dirty-guid driven rather than a naive full-world reset loop.
- Current disposition:
  - Keep it for now.
  - Do not treat this as the first optimization target unless telemetry shows winner swaps are still frequent.

### F8. The interpolation and camera ordering look directionally correct and should be preserved

- Severity: Preserve
- Confidence: Proven
- Main impact: `smoothness`, `correctness`
- Exact references:
  - `bins/sidereal-client/src/native/plugins.rs:431-468`
  - `bins/sidereal-client/src/native/transforms.rs:127-317`
  - `bins/sidereal-client/src/native/camera.rs:97-275`
- Why it matters:
  - Camera follow is still deliberately scheduled after Lightyear interpolation and visual correction.
  - The client still seeds interpolation when history is missing and recovers obviously stalled interpolated transforms.
  - This remains the right lane to keep.
- Current disposition:
  - Do not “fix” this by moving camera follow earlier or by removing the interpolation safeguards.

## 4. Specific Statements: Confirmed or Refuted

1. `The game is GPU-bound in normal gameplay.`
   - Refuted by current code shape.
   - The stronger evidence still points to client CPU plus pacing issues, not raw GPU saturation.

2. `The game is CPU-bound on the client in normal gameplay.`
   - Likely true.
   - This is still the most plausible default reading of the current runtime.

3. `The game is bottlenecked by ECS scheduling/query work more than actual draw submission.`
   - Likely true.
   - Render-layer, shader-assignment, asset-dependency, tactical-map, and nameplate bookkeeping all support this.

4. `The game is bottlenecked by replication/update churn more than rendering itself.`
   - Partly true.
   - Less true than on March 10, but still meaningful because the visibility membership lane can still destabilize cadence.

5. `The game feels slow mainly because of frame pacing/interpolation issues rather than raw frame time.`
   - Partly true.
   - More precisely: pacing plus client CPU overhead plus remaining replication cadence issues.

6. `Shader/material diversity is defeating batching enough to matter.`
   - Likely true.
   - Planet passes, effects, and fullscreen lanes are still the clearest contributors.

7. `Too many fullscreen or post-process passes are active for the current visual payoff.`
   - Possibly true, but needs measurement.
   - The baseline pass count is still high enough to budget, not high enough to condemn without captures.

8. `Off-screen or non-visible entities are still paying too much render-related cost.`
   - Partly true.
   - The biggest remaining examples are tactical/nameplate UI work and intentional `NoFrustumCulling` lanes, not a blanket culling failure.

9. `Asset/shader compilation or loading hitching is a meaningful source of stalls.`
   - Likely true.
   - The network side improved, but main-thread asset completion and streamed shader reloads can still hitch.

10. `Server-side visibility/replication behavior is causing client render instability or overload.`
   - Still true, but less severe than before.

11. `The current render-layer architecture is directionally correct and should be kept.`
   - Confirmed.

12. `The current render-layer/material implementation has avoidable transitional cost that should be simplified.`
   - Confirmed.

## 5. End-to-End Render Flow Map

### 5.1 Asset/bootstrap to client-ready rendering

1. Enter-world acceptance triggers asset bootstrap submission in `auth_net`.
2. Required bootstrap assets are fetched with bounded parallelism and written into the local cache/index.
3. Asset catalog messages drive runtime dependency refresh.
4. Runtime dependency candidates feed the bounded runtime fetch queue.
5. Completed assets update the cache/index and can trigger streamed shader reload.
6. Visual attachment systems then resolve streamed visuals/material kinds against the ready catalog/cache state.

### 5.2 Replicated entity arrival to visible draw

1. Lightyear adoption creates runtime world entities.
2. Transform bootstrap systems seed or recover valid visual transforms for predicted/interpolated entities.
3. Render-layer resolution assigns the entity to the correct world layer.
4. Duplicate predicted/interpolated suppression hides losing visual copies.
5. Visual attach systems add sprites, planet passes, projectile visuals, effects, and streamed visual children.
6. Post-interpolation camera/update systems derive same-frame render transforms and parallax.
7. Bevy transform propagation and render extraction feed the active cameras/passes.

### 5.3 Camera-relative/world-layer transform derivation

1. Gameplay camera follows the selected runtime anchor after interpolation/correction.
2. `CameraMotionState` derives world, smoothed, and parallax positions.
3. Streamed world visuals and planet visuals convert world positions into camera-relative projected positions using runtime render-layer depth/scale rules.

### 5.4 Fullscreen background/foreground/post-process execution

1. World scene boot spawns dedicated backdrop, fullscreen-foreground, and post-process cameras.
2. Fullscreen layer systems ensure fullscreen renderables exist with the correct render layer and material binding.
3. Backdrop camera sync keeps those cameras/projections aligned to the active gameplay view.
4. Fullscreen quad transforms scale to the current viewport every frame.

### 5.5 Prediction/reconciliation/interpolation to final presented motion

1. Lightyear owns prediction/interpolation.
2. Sidereal adds transform bootstrap for entities that lack interpolation history and a fallback for clearly stalled interpolated transforms.
3. Camera follow intentionally samples the post-correction pose that will actually be rendered that frame.
4. Duplicate suppression and controlled-entity adoption still bridge transitional relevance/handoff cases.

## 6. Performance Budget Map

This section is partly inferential. Each inference is labeled.

### 6.1 Client CPU

1. Proven: render-layer registry/assignment maintenance.
2. Proven: runtime shader assignment scanning.
3. Proven: tactical-map marker maintenance and smoothing.
4. Proven: nameplate projection and health-bar updates.
5. Strong inference: material instance diversity increases CPU submission overhead enough to matter.

### 6.2 Client GPU

1. Proven: multiple active camera/pass lanes exist.
2. Proven: planet rendering can emit several `NoFrustumCulling` passes per body.
3. Strong inference: fullscreen/post-process and alpha-heavy effect paths are a secondary budget, not the primary bottleneck today.

### 6.3 Client main-thread stalls

1. Proven: runtime asset completion still performs `block_on(...)` write/save work in the polling system.
2. Strong inference: streamed shader reloads can still produce visible hitching when assets land.

### 6.4 Server tick cost affecting visual smoothness

1. Proven: visibility cache/index/discovery work is now much better shaped.
2. Proven: final membership application still evaluates entity-client visibility decisions each tick.
3. Strong inference: this remains the largest server-side contributor to uneven authoritative cadence.

### 6.5 Network/replication delivery cost affecting render churn

1. Proven: candidate narrowing now exists and should reduce delivery churn.
2. Proven: controlled/owner/config entities still bypass ordinary visibility filters in several cases.
3. Strong inference: remaining cadence problems now come more from apply-lane cost than from raw over-delivery.

## 7. Priority Order

If the goal is “what should be fixed first now,” the order is:

1. Measure and reduce `update_network_visibility()` apply-lane cost with the new telemetry rather than revisiting the already-landed cache/index refactor.
2. Remove main-thread asset completion stalls from `poll_runtime_asset_http_fetches_system()`.
3. Reduce always-on client polling in shader assignment, asset dependency refresh, and render-layer maintenance.
4. Cut tactical-map and nameplate per-frame reconciliation cost.
5. Budget and reduce material-instance diversity in planet/effect/fullscreen lanes.

## 8. Bottom Line

The codebase is no longer “sitting” where it was on March 10.

It is sitting in a better transitional state where two major audit findings are already fixed:

1. visibility is cached and cadence-split rather than monolithic, and
2. asset fetch is concurrent rather than serialized.

The next bottlenecks are more incremental and more honest:

1. remaining visibility membership cost,
2. client CPU bookkeeping,
3. UI/world overlay churn,
4. material diversity,
5. asset completion hitching.

That is progress. It also means the next pass should be telemetry-driven rather than architecture-panicked.

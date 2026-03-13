# Bevy 2D Rendering Optimization Audit Report

Date: 2026-03-09
Prompt source: `docs/audits/bevy_2d_rendering_optimization_audit_prompt.md`
Scope: Client rendering performance, frame pacing, render-adjacent ECS/runtime overhead, replication/visibility delivery cost, and server-side contributors to perceived render slowness.
Limitations: Static code audit only. No live GPU captures, frame captures, packet traces, tracy/puffin profiles, or long-session memory graphs were available. GPU-bound vs CPU-bound conclusions are therefore partly inferential except where the code proves churn/allocation behavior directly.

## 1. Executive Summary

The codebase already has one important smoothness improvement wired correctly: the client enables `FrameInterpolationPlugin::<Transform>` and attaches `FrameInterpolate<Transform>` during replicated entity adoption (`bins/sidereal-client/src/runtime/mod.rs:116-140`, `bins/sidereal-client/src/runtime/replication.rs:550-563`). This is a real improvement over the prior Lightyear audit state and should be kept.

The biggest proven rendering-performance problem is elsewhere: the fullscreen and post-process sync systems recreate meshes and material handles for already-existing renderables every `Update`. That is not an optimization opportunity. It is a live churn bug, and it is severe enough to explain frame-time instability, memory growth, and "it gets slower the longer it runs" behavior on its own.

Behind that, the runtime is doing too much whole-world bookkeeping every frame on the client and every fixed tick on the replication server:

1. client render-layer registry rebuild and per-entity layer resolution are clone-heavy and always-on,
2. client asset dependency discovery rescans the entire visible rendering dependency surface every frame,
3. server visibility rebuilds scratch maps/sets from the full replicated world every tick, then walks every replicated entity against every live client.

The current render-layer direction is still defensible. The current implementation is not cheap enough yet.

## 2. What Most Likely Makes The Game Feel Slow

Most likely causes, in order:

1. Proven per-frame fullscreen/post-process mesh and material churn on the client.
2. Client `Update` schedule overload from adoption, transform fallback, render-layer rebuild, visual suppression, asset queueing, visuals, lighting, UI, and audits all in the same frame path.
3. Server visibility/delivery work producing uneven authoritative update cadence under load, which the client then has to smooth over.
4. Material/mesh diversity in the 2D visual path defeating batching, especially for shader-backed sprite visuals and planet passes.
5. Too many always-on cameras/passes relative to the current visual payoff.

The codebase does not read like a pure GPU-bound 2D renderer. It reads like a client CPU/frame-pacing problem with server-side cadence pressure and a few render-path architectural costs layered on top.

## 3. Critical Findings

### Finding C1: Fullscreen and post-process renderables are rebuilt every frame instead of only on creation/change
- Severity: Critical
- Confidence: proven
- Main impact: client CPU, memory/bandwidth, frame pacing
- Why it matters:
  Existing fullscreen entities are not simply updated in place. Each `Update`, the code allocates a fresh rectangle mesh handle, clears/rebinds material components, and inserts the render bundle again for existing fullscreen entities and existing post-process passes. That creates continuous asset churn and forces avoidable render-world changes.
- Evidence:
  - `bins/sidereal-client/src/runtime/backdrop.rs:66-119`
  - `bins/sidereal-client/src/runtime/backdrop.rs:341-389`
  - `bins/sidereal-client/src/runtime/backdrop.rs:451-492`
- Details:
  `sync_fullscreen_layer_renderables_system()` calls `meshes.add(Rectangle::new(1.0, 1.0))` for already-existing renderables, then reinserts `Mesh2d`, `Transform`, `RenderLayers`, and `NoFrustumCulling`. `sync_runtime_post_process_renderables_system()` does the same. `attach_runtime_fullscreen_material()` also allocates a new `Material2d` asset each time for starfield/space background variants.
- Recommendation:
  Split these systems into:
  1. creation path for missing renderables,
  2. mutation path for changed order/visibility/material binding only,
  3. removal path for stale renderables.
  Reuse one shared fullscreen quad mesh per domain or one global unit quad handle. Reuse material handles when shader family and owned settings have not changed.
- Expected payoff:
  Very high. This should immediately reduce frame spikes, render-world churn, and long-session slowdown.
- Risk / complexity:
  Medium.
- Priority:
  must fix

## 4. Client Render Pipeline Findings

### Finding R1: The client runs too many always-on cameras and composition passes for the current workload
- Severity: High
- Confidence: proven
- Main impact: client CPU, client GPU, frame pacing
- Why it matters:
  In-world rendering currently uses separate cameras for backdrop, gameplay, debug overlay, fullscreen foreground, post-process, and UI overlay. Several are always active. Every extra view increases extraction, render graph work, visibility work, and pass overhead.
- Evidence:
  - `bins/sidereal-client/src/runtime/scene_world.rs:62-154`
  - `bins/sidereal-client/src/runtime/scene.rs:16-28`
- Details:
  The client spawns:
  1. backdrop camera,
  2. gameplay camera,
  3. debug overlay camera,
  4. fullscreen foreground camera,
  5. post-process camera,
  6. UI overlay camera.
  This may be justified eventually, but it is expensive for a game already fighting CPU-side churn.
- Recommendation:
  Audit which views truly need dedicated cameras. Likely reductions:
  1. disable debug overlay camera entirely unless overlay is active,
  2. collapse fullscreen foreground and post-process if only one effect family is active,
  3. verify whether the UI overlay camera needs to stay independent during in-world play.
- Expected payoff:
  Moderate to high, especially on weaker GPUs and during UI-heavy scenes.
- Risk / complexity:
  Medium.
- Priority:
  should fix

### Finding R2: Shader-backed 2D visuals currently trade batching away too aggressively
- Severity: High
- Confidence: strong inference
- Main impact: client CPU, client GPU
- Why it matters:
  The shader-backed sprite path creates a mesh and a unique `Material2d` instance per entity for asteroid and generic sprite shader cases. That is the opposite of a batch-friendly 2D renderer.
- Evidence:
  - `bins/sidereal-client/src/runtime/visuals.rs:553-599`
  - `bins/sidereal-client/src/runtime/backdrop.rs:458-489`
  - `bins/sidereal-client/src/runtime/mod.rs:333-340`
- Details:
  Plain sprites can batch much better. The custom shader paths create `Rectangle` meshes and new material instances for individual entities. Planet passes are likewise material-per-pass-per-entity. Even if Bevy can still optimize some parts, this path is structurally much more expensive than sprite batching.
- Recommendation:
  Reduce the number of entity-unique material instances.
  1. Prefer plain sprite path wherever shader value is low.
  2. Move repeated shared uniforms into shared resources or texture/atlas-driven data where possible.
  3. Consolidate shader families so many entities can share the same material handle and only differ by transform/texture index.
  4. Profile draw count before and after.
- Expected payoff:
  High in large scenes with many shader-backed visuals.
- Risk / complexity:
  High.
- Priority:
  should fix

### Finding R3: Fullscreen layers and post-process passes are explicitly non-cullable and always maintained through dynamic sync systems
- Severity: Medium
- Confidence: proven
- Main impact: client GPU, client CPU
- Why it matters:
  Fullscreen layers should be non-cullable, but the current implementation keeps them alive through active synchronization passes every frame. That is acceptable only if the number of fullscreen passes stays extremely small and the sync work stays cheap, which it currently does not.
- Evidence:
  - `bins/sidereal-client/src/runtime/backdrop.rs:97-118`
  - `bins/sidereal-client/src/runtime/backdrop.rs:366-388`
  - `bins/sidereal-client/src/runtime/backdrop.rs:505-539`
- Recommendation:
  Keep the non-cullable policy, but make the runtime path state-driven rather than per-frame rebinding-driven. Track dirty fullscreen/post-process authored state and only rebuild renderables on change.
- Expected payoff:
  Moderate.
- Risk / complexity:
  Medium.
- Priority:
  should fix

## 5. ECS / Schedule / Transform Findings

### Finding E1: Client `Update` is overloaded with rendering-adjacent work that should not all be frame-rate work
- Severity: High
- Confidence: proven
- Main impact: client CPU, frame pacing, architecture/maintainability
- Why it matters:
  The client performs replication adoption, transform bootstrap, asset queueing, render-layer rebuild, duplicate suppression, visual attach/update, backdrop sync, lighting sync, camera logic, tactical overlay updates, UI updates, audits, and debug text updates in the normal `Update` path. This makes frame time sensitive to too many unrelated concerns.
- Evidence:
  - `bins/sidereal-client/src/runtime/plugins.rs:139-253`
  - `bins/sidereal-client/src/runtime/plugins.rs:300-370`
  - `bins/sidereal-client/src/runtime/plugins.rs:430-514`
- Recommendation:
  Split runtime work into:
  1. event/change-driven replication adoption and registry maintenance,
  2. fixed-tick simulation/prediction-related sync,
  3. render-frame-only visual interpolation and camera tasks,
  4. low-frequency diagnostics.
  Most importantly, stop rescanning authored layer state and asset dependencies every render frame.
- Expected payoff:
  High.
- Risk / complexity:
  High.
- Priority:
  must fix

### Finding E2: Render-layer registry rebuild and layer assignment resolution rescan and clone the world every frame
- Severity: High
- Confidence: proven
- Main impact: client CPU, architecture/maintainability
- Why it matters:
  `sync_runtime_render_layer_registry_system()` clones all layer definitions, rules, and post-process stacks into `Vec`s, recompiles them, and may replace the registry resource. `resolve_runtime_render_layer_assignments_system()` then clones entity labels/overrides/current state into another `Vec` and walks every world entity again.
- Evidence:
  - `bins/sidereal-client/src/runtime/render_layers.rs:15-149`
  - `bins/sidereal-client/src/runtime/render_layers.rs:151-225`
- Recommendation:
  Make authored render-layer state incremental.
  1. Recompile the registry only on `Added`, `Changed`, and `RemovedComponents` for layer/rule/post-process entities.
  2. Resolve per-entity layer assignment only when labels, relevant components, or override data change.
  3. Cache compiled rule component-ID sets permanently until the generated registry changes.
- Expected payoff:
  High in large worlds or authored-heavy content.
- Risk / complexity:
  Medium.
- Priority:
  must fix

### Finding E3: Duplicate predicted/interpolated visual arbitration still requires a full world scan every frame
- Severity: Medium
- Confidence: proven
- Main impact: frame pacing, client CPU
- Why it matters:
  The client still runs a two-pass winner-selection scan over all `WorldEntity` instances to decide which duplicate GUID copy should render. That is a symptom that the entity presentation lifecycle is still too noisy.
- Evidence:
  - `bins/sidereal-client/src/runtime/visuals.rs:243-347`
  - `bins/sidereal-client/src/runtime/plugins.rs:303-346`
- Recommendation:
  Reduce the need for arbitration by tightening the replicated entity lifecycle.
  1. Prefer one stable displayed runtime entity per GUID class.
  2. Move duplicate suppression closer to adoption/control handoff rather than scoring the whole world every frame.
  3. Add counters for duplicate GUID groups and suppression churn.
- Expected payoff:
  Moderate.
- Risk / complexity:
  Medium.
- Priority:
  should fix

## 6. Asset / Shader / Material Findings

### Finding A1: Runtime asset dependency discovery is still a whole-world polling loop
- Severity: High
- Confidence: proven
- Main impact: client CPU, startup hitching
- Why it matters:
  `queue_missing_catalog_assets_system()` iterates fullscreen layers, runtime layers, post-process stacks, sprite shader IDs, streamed sprite shader IDs, and streamed visual asset IDs every frame, rebuilds a `HashSet`, expands dependencies, then chooses one next asset to fetch.
- Evidence:
  - `bins/sidereal-client/src/runtime/assets.rs:215-329`
  - `bins/sidereal-client/src/runtime/plugins.rs:166-170`
  - `bins/sidereal-client/src/runtime/plugins.rs:210-214`
- Recommendation:
  Replace polling discovery with a dirty dependency graph.
  1. Recompute required asset closure when relevant authored/render components change.
  2. Maintain a pending-asset queue resource.
  3. Let the fetch system consume the queue without rescanning the whole world.
- Expected payoff:
  Moderate to high, especially during world entry and dynamic content changes.
- Risk / complexity:
  Medium.
- Priority:
  should fix

### Finding A2: The client still reloads streamed shaders at world scene spawn and keeps shader/material routing more dynamic than the current runtime can cheaply support
- Severity: Medium
- Confidence: strong inference
- Main impact: startup hitching, architecture/maintainability
- Why it matters:
  A dynamic shader pipeline is directionally correct for this project, but the current runtime still pays for transitional complexity. That matters because the renderer already has too much frame-sensitive work around it.
- Evidence:
  - `bins/sidereal-client/src/runtime/scene_world.rs:55-61`
  - `bins/sidereal-client/src/runtime/mod.rs:333-340`
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
- Recommendation:
  Keep the authored render-layer direction, but simplify runtime hot paths:
  1. minimize live shader-family surface area,
  2. prewarm/bootstrap the small set of active material families,
  3. keep reloads out of latency-sensitive transitions where possible.
- Expected payoff:
  Moderate.
- Risk / complexity:
  Medium.
- Priority:
  optional improvement

## 7. Lightyear / Replication / Visibility Findings

### Finding N1: Server visibility rebuilding is expensive enough to become a rendering problem indirectly
- Severity: High
- Confidence: proven
- Main impact: replication churn, frame pacing, architecture/maintainability
- Why it matters:
  The replication server rebuilds `VisibilityScratch` from the full replicated set each tick, reconstructs multiple `HashMap`/`HashSet` indexes, computes candidate sets per client, writes player visibility disclosure/grid components, then loops every replicated entity against every live client to gain/lose visibility.
- Evidence:
  - `bins/sidereal-replication/src/replication/visibility.rs:552-850`
  - `bins/sidereal-replication/src/replication/visibility.rs:852-1125`
  - `bins/sidereal-replication/src/plugins.rs:71-82`
- Recommendation:
  Keep the visibility contract, but stop recomputing the entire world every tick.
  1. Maintain an incremental spatial index resource.
  2. Track dirty moved entities, dirty ownership/faction/public visibility changes, and dirty client observer anchors.
  3. Rebuild per-client candidate sets only when anchor/range/view-mode changes, then incrementally apply moved-entity changes.
  4. Separate telemetry generation from the hot path.
- Expected payoff:
  High on both server cadence and client smoothness under load.
- Risk / complexity:
  High.
- Priority:
  must fix

### Finding N2: Client-side transform/bootstrap/adoption fallback still indicates replication lifecycle churn
- Severity: Medium
- Confidence: proven
- Main impact: frame pacing, replication churn
- Why it matters:
  The client still needs:
  1. replicated spatial component backfills,
  2. hierarchy parent spatial repairs,
  3. invalid `ChildOf` sanitization,
  4. interpolated-without-history transform bootstrap,
  5. delayed visibility reveal,
  6. duplicate render winner suppression.
  These are pragmatic safeguards, but together they mean the display path is still compensating for a noisy replication lifecycle.
- Evidence:
  - `bins/sidereal-client/src/runtime/plugins.rs:139-157`
  - `bins/sidereal-client/src/runtime/transforms.rs:1-146`
  - `bins/sidereal-client/src/runtime/replication.rs:44-120`
  - `bins/sidereal-client/src/runtime/replication.rs:550-585`
- Recommendation:
  Treat these as transition scaffolding. Reduce the number of emergency repair systems by tightening the adoption contract and ensuring spatially renderable entities arrive with a sufficient baseline component set.
- Expected payoff:
  Moderate.
- Risk / complexity:
  Medium.
- Priority:
  should fix

### Finding N3: The current architecture does at least have the right smoothness primitive now
- Severity: Low
- Confidence: proven
- Main impact: frame pacing
- Why it matters:
  The client now enables Lightyear frame interpolation and attaches `FrameInterpolate<Transform>` when replicated entities have spatial motion state.
- Evidence:
  - `bins/sidereal-client/src/runtime/mod.rs:128-140`
  - `bins/sidereal-client/src/runtime/replication.rs:559-563`
- Recommendation:
  Keep this. Do not regress back to fixed-tick render stepping.
- Expected payoff:
  Preserves current smoothness baseline.
- Risk / complexity:
  Low.
- Priority:
  keep

## 8. Server-Side Contributors To Render Slowness

The server does not render, but it can still make rendering feel bad by delivering unstable, bursty, or excessive state. The visibility path is the clearest case. The replication schedule currently places visibility, owner manifest streaming, and tactical snapshot streaming in the same fixed-tick path after physics writeback (`bins/sidereal-replication/src/plugins.rs:71-82`). If that slice spikes, the client receives updates less evenly and has to absorb more churn.

The strongest server-side bottleneck hypothesis is:

1. visibility/index rebuild cost,
2. followed by per-client per-entity visibility decisions,
3. followed by extra snapshot/manifest traffic layered into the same tick.

That is more likely to degrade perceived smoothness than raw server simulation math right now.

## 9. Documentation / Architecture Divergence

The docs are directionally right:

1. Lua-authored render layers and rules are the right long-term shape.
2. Layer depth/parallax being render-derived only is correct.
3. Server-side visibility/delivery narrowing is correct.

What diverges is runtime cost discipline:

1. the code still uses polling/rebuild paths where incremental authored-state propagation would better match the architecture,
2. the fullscreen/post-process implementation is far more churn-heavy than the design intent suggests,
3. the visibility contract is right, but the current implementation is still "rebuild the world every tick" rather than "maintain authoritative incremental indexes."

## 10. End-to-End Render Flow Map

### 10.1 Asset/bootstrap to client-ready rendering

1. Client starts and configures Bevy, Avian, Lightyear, frame interpolation, materials, and runtime resources.
2. On entering `WorldLoading` / `AssetLoading`, the client polls gateway/bootstrap state and runtime asset fetch state.
3. `queue_missing_catalog_assets_system()` scans authored/rendered asset references and queues HTTP fetches.
4. `poll_runtime_asset_http_fetches_system()` writes payloads into the local cache/index.
5. `spawn_world_scene()` creates the in-world cameras and baseline scene entities, and reloads streamed shaders.
6. Once bootstrap state is complete, the client transitions into `InWorld`, where replicated entities are adopted and visuals attach from replicated authored data.

### 10.2 Replicated entity arrival to visible draw

1. Lightyear replicated entity appears.
2. Client adoption path inserts `WorldEntity`, `PendingInitialVisualReady`, hidden visibility state, and optional `FrameInterpolate<Transform>`.
3. Transform fallback systems seed renderable transforms until interpolation history is ready.
4. Duplicate suppression decides which runtime copy is visible.
5. Render-layer resolution assigns the entity to a world layer.
6. Streamed visual/planet/thruster/projectile systems attach child visuals and update their transforms.
7. Reveal path unhides the entity once a valid initial pose exists.
8. Gameplay camera, fullscreen layers, post-process layers, and UI cameras render the result.

### 10.3 Camera-relative/world-layer transform derivation

1. Gameplay entities hold authoritative world-space state.
2. Render-layer resolution chooses a layer definition.
3. Visual child transforms are offset from camera motion according to layer/parallax policy.
4. Camera systems resolve the follow anchor, apply zoom smoothing, and feed camera motion state back to layered visuals and overlays.

### 10.4 Fullscreen background/foreground/post-process execution

1. Replicated fullscreen layer definitions and post-process stacks are read into local caches.
2. Sync systems create/update fullscreen renderables and post-process renderables.
3. Backdrop/fullscreen cameras are forced active and window-sized each frame.
4. Last-stage material update systems write fullscreen shader uniforms before presentation.

## 11. Performance Budget Map

This section is partly inferential.

### 11.1 Client CPU

Likely hottest paths:

1. fullscreen/post-process sync churn,
2. render-layer registry rebuild and assignment resolution,
3. asset dependency scanning,
4. duplicate suppression and visual child cleanup/attach passes,
5. UI/nameplate/tactical overlay updates,
6. transform/bootstrap/adoption scaffolding.

### 11.2 Client GPU

Likely hot contributors:

1. multiple active cameras and composition passes,
2. fullscreen starfield/background/post-process passes,
3. shader-heavy planet and effect materials,
4. draw-call inflation from material-per-entity shader paths.

### 11.3 Client main-thread stalls

Likely contributors:

1. per-frame mesh/material asset creation,
2. render-world extraction churn from repeated component reinsertion,
3. shader/material reload transitions,
4. large-frame UI and overlay updates when tactical/debug features are active.

### 11.4 Server tick cost affecting visual smoothness

Likely hottest contributor:

1. `update_network_visibility()` full-world scratch rebuild and per-client visibility evaluation.

Secondary contributors:

1. owner manifest streaming,
2. tactical snapshot streaming,
3. any logging enabled in hot replication paths.

### 11.5 Network / replication delivery cost affecting render churn

Likely contributors:

1. relevance churn causing adoption/despawn/visibility churn,
2. duplicate runtime copies around control handoff,
3. delivering more entities than the camera can usefully present.

## 12. Prioritized Remediation Plan

### Top 5 highest-ROI changes

1. Fix fullscreen/post-process sync so existing renderables reuse mesh and material handles.
2. Make render-layer registry compilation and per-entity layer assignment incremental instead of frame-polled.
3. Replace runtime asset dependency discovery polling with a dirty authored-dependency queue.
4. Rework server visibility into incremental indexes and dirty updates rather than full rebuild per tick.
5. Reduce active camera/pass count and disable debug overlay rendering paths unless explicitly enabled.

### Quick wins

1. Share one unit quad mesh across fullscreen and post-process renderables.
2. Cache fullscreen material handles by `(family, authored settings fingerprint)` instead of creating them every frame.
3. Gate debug overlay camera and debug text systems behind an enabled flag, not just hidden visibility.
4. Add counters for duplicate GUID groups and fullscreen/post-process renderable rebuild counts.

### Medium-size refactors

1. Change render-layer systems to observer/change-driven updates.
2. Split client `Update` responsibilities into lower-frequency or change-driven pipelines.
3. Collapse low-value shader-backed visuals back to plain sprite batching where possible.

### Large architectural changes

1. Incremental server visibility index and dirty-set pipeline.
2. A more batch-friendly generic 2D shader/material path that avoids material-per-entity proliferation.

### Order of operations

1. Fix fullscreen/post-process churn first.
2. Add instrumentation immediately after that so later changes are measurable.
3. Then optimize client polling/rebuild systems.
4. Then optimize server visibility.
5. Only after the above, decide whether GPU-side shader/material redesign is still necessary.

### What to measure before and after

1. average frame time,
2. 95th/99th percentile frame time,
3. draw count,
4. number of fullscreen/post-process renderables,
5. material asset counts by family,
6. mesh asset count over time,
7. per-system timing for render-layer, assets, visuals, UI, visibility,
8. server visibility tick time percentiles.

## 13. Instrumentation / Profiling Gaps

Missing telemetry that should be added:

1. per-frame count of fullscreen renderable rebuilds,
2. per-frame count of post-process renderable rebuilds,
3. live `Assets<Mesh>` and `Assets<Material2d>` counts by family,
4. draw-call and visible-entity counts by camera/layer,
5. per-system timings for:
   - render-layer registry sync,
   - layer assignment resolution,
   - asset queue discovery,
   - duplicate suppression,
   - streamed visual attach/update,
   - tactical overlay update,
   - server visibility update,
6. replication cadence histogram:
   - snapshot intervals,
   - visibility gain/lose counts,
   - adoption/despawn counts,
7. frame pacing metrics:
   - average,
   - p95,
   - p99,
   - max,
8. shader/material compile/reload timing,
9. per-camera pass timing if available from WGPU/Bevy diagnostics.

Recommended tools:

1. `bevy::diagnostic` custom counters/resources for live runtime counts,
2. a proper profiler such as Tracy or Puffin for system timing,
3. optional WGPU capture on native for pass cost confirmation,
4. replication-side tracing around `update_network_visibility()`.

## 14. Runtime Catalog Appendix

### 14.1 Client runtime pieces most relevant to rendering

- `bins/sidereal-client/src/runtime/plugins.rs`
  Status: active runtime
  Responsibility: schedules replication adoption, visuals, lighting, UI, and backdrop systems.
- `bins/sidereal-client/src/runtime/backdrop.rs`
  Status: active runtime
  Responsibility: fullscreen layers, post-process renderables, fullscreen materials, backdrop camera sync.
- `bins/sidereal-client/src/runtime/visuals.rs`
  Status: active runtime
  Responsibility: streamed visuals, planet passes, thrusters, tracers, duplicate suppression.
- `bins/sidereal-client/src/runtime/render_layers.rs`
  Status: active runtime
  Responsibility: runtime render-layer registry compilation and per-entity assignment.
- `bins/sidereal-client/src/runtime/assets.rs`
  Status: active runtime
  Responsibility: runtime asset dependency scanning and HTTP fetch queueing.
- `bins/sidereal-client/src/runtime/scene_world.rs`
  Status: active runtime
  Responsibility: in-world cameras and baseline scene entities.
- `bins/sidereal-client/src/runtime/camera.rs`
  Status: active runtime
  Responsibility: gameplay camera follow, zoom, overlay camera sync.
- `bins/sidereal-client/src/runtime/transforms.rs`
  Status: transitional/migration code
  Responsibility: transform fallback/bootstrap for replicated entities.

### 14.2 Replication server pieces affecting rendering indirectly

- `bins/sidereal-replication/src/replication/visibility.rs`
  Status: active runtime
  Responsibility: authoritative candidate filtering, authorization, delivery gating, player visibility disclosure.
- `bins/sidereal-replication/src/plugins.rs`
  Status: active runtime
  Responsibility: schedules visibility, owner manifest, tactical snapshot, persistence timing relative to physics.

### 14.3 Shared gameplay/render-support modules

- `crates/sidereal-game/src/render_layers.rs`
  Status: active runtime
  Responsibility: validation contract for runtime layers/rules/stacks.
- `crates/sidereal-game/src/lib.rs`
  Status: active runtime
  Responsibility: shared gameplay plugin, hierarchy rebuild, fixed-step systems.

## 15. Bottom Line

The render-layer architecture should be kept. The current bottlenecks are not a reason to abandon it.

The first job is to remove obvious churn bugs and convert polling/rebuild paths into incremental state propagation. Until that is done, further shader polish or content-side visual work will keep landing on top of a renderer that is spending too much time rebuilding its own bookkeeping. 

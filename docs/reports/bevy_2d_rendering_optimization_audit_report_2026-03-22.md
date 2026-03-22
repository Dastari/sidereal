# Bevy 2D Rendering Optimization Audit Report

- Report date: 2026-03-22
- Prompt source: `docs/prompts/bevy_2d_rendering_optimization_audit_prompt.md`
- Scope: code-only audit of client rendering, client scheduling, prediction/interpolation, asset/shader delivery, and replication/server visibility paths that materially affect perceived or actual render performance.
- Limitations:
  - No live frame capture or GPU profile was available.
  - No packet trace, RenderDoc capture, Tracy capture, or Bevy system timings were available.
  - GPU conclusions are inference-only unless directly implied by pass/material/camera topology.

## 1. Executive Summary

This codebase does not primarily look like a normal "the GPU is too slow at drawing sprites" problem. The stronger explanation is that the game can feel slow because the client is spending a large amount of work compensating for unstable runtime state:

1. duplicated predicted/interpolated/confirmed presentation lanes,
2. repeated transform/bootstrap recovery to avoid origin flashes and stalled visuals,
3. several world-wide maintenance passes in `Update` and `PostUpdate`,
4. server visibility churn that can respawn visibility, resend spatial state, and destabilize the client presentation lane.

The render-layer direction is broadly correct and worth keeping. The current implementation cost is high because the runtime is still carrying a transitional stack: duplicate visual suppression, dynamic rule resolution, runtime asset dependency scans, multiple camera passes, and content-specific material families all remain active at once.

The highest-ROI work is not "optimize shaders first". The first wins are:

1. remove the need for client duplicate-resolution and transform repair passes,
2. reduce full-world scans and per-frame recomputation in presentation systems,
3. tighten visibility/relevance churn so the client stops doing attach/detach/bootstrap work for entities that are oscillating around relevance boundaries,
4. budget fullscreen/pass count and move expensive overlay rebuilds behind stronger gating.

## 2. What Most Likely Makes The Game Feel Slow

The strongest repository-specific explanation is frame pacing and visual instability, not raw average FPS:

1. The client has explicit repair systems for stalled interpolated transforms and missing interpolation markers in [`bins/sidereal-client/src/runtime/transforms.rs:268`](bins/sidereal-client/src/runtime/transforms.rs:268) and [`bins/sidereal-client/src/runtime/transforms.rs:309`](bins/sidereal-client/src/runtime/transforms.rs:309).
2. The client resolves duplicate logical entities every frame because predicted and interpolated copies coexist and need winner selection in [`bins/sidereal-client/src/runtime/visuals.rs:531`](bins/sidereal-client/src/runtime/visuals.rs:531).
3. Camera follow is intentionally delayed to `PostUpdate` after Lightyear interpolation and visual correction because earlier timing caused visible disagreement/jitter; that is explicitly documented in code in [`bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:18`](bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:18).
4. The server visibility path can re-grant visibility and explicitly queue spatial resends on visibility gains in [`bins/sidereal-replication/src/replication/visibility.rs:2005`](bins/sidereal-replication/src/replication/visibility.rs:2005), which is a strong sign that state arrival cadence and relevance churn are still affecting presentation smoothness.

That combination usually feels "slow" even when the GPU is not saturated.

## 3. Critical Findings

### Finding 1: Presentation smoothness is dominated by prediction/interpolation instability and duplicate-lane repair

- Severity: Critical
- Confidence: proven
- Main impact: frame pacing, client CPU, replication churn, architecture/maintainability
- Why it matters:
  - The client has multiple systems whose only job is to compensate for presentation instability: interpolation marker repair, stalled transform recovery, hidden-until-ready bootstrap, duplicate winner selection, and camera scheduling after rollback correction.
  - This is classic "feels bad even when FPS is okay" territory.
- Evidence:
  - `bins/sidereal-client/src/runtime/transforms.rs:268`
  - `bins/sidereal-client/src/runtime/transforms.rs:309`
  - `bins/sidereal-client/src/runtime/transforms.rs:358`
  - `bins/sidereal-client/src/runtime/visuals.rs:531`
  - `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:18`
  - `bins/sidereal-client/src/runtime/motion.rs:64`
  - `bins/sidereal-client/src/runtime/replication.rs:1`
- Concrete recommendation:
  - Make predicted/interpolated clone ownership deterministic enough that the client no longer needs `suppress_duplicate_predicted_interpolated_visuals_system`, `recover_stalled_interpolated_world_entity_transforms`, and most of the hidden-until-ready fallback path.
  - Treat removal of those systems as a success metric, not a nice-to-have cleanup.
- Expected payoff: largest perceived-smoothness gain in the repo
- Risk/complexity of fixing: high
- Priority: must fix

### Finding 2: The client keeps several world-wide maintenance scans alive every frame

- Severity: High
- Confidence: proven
- Main impact: client CPU, frame pacing
- Why it matters:
  - Several `Update` systems perform broad scans over `WorldEntity` or authored render config state every frame. Even when each pass is moderate alone, together they create a persistent CPU floor and amplify long-tail spikes during churn.
- Evidence:
  - Duplicate group maintenance scans the world and removed-component streams in `bins/sidereal-client/src/runtime/visuals.rs:531`
  - Render-layer assignment does full scans on generation changes and targeted scans otherwise in `bins/sidereal-client/src/runtime/render_layers.rs:193`
  - Runtime asset dependency graph rebuild scans authored layers, post-process stacks, shader IDs, and visual IDs in `bins/sidereal-client/src/runtime/assets.rs:313` and `bins/sidereal-client/src/runtime/assets.rs:840`
  - Nameplate syncing scans every canonical presentation entity with health in `bins/sidereal-client/src/runtime/ui.rs:2017`
  - Debug snapshot scans all `WorldEntity` instances when enabled in `bins/sidereal-client/src/runtime/debug_overlay.rs:148`
- Concrete recommendation:
  - Collapse broad `WorldEntity` scans into narrower lifecycle-driven registries.
  - Precompute render-attachment candidate sets on adoption/despawn instead of rediscovering them every frame.
  - Move rule assignment, asset dependency collection, and duplicate grouping toward event-driven invalidation with bounded work queues.
- Expected payoff: meaningful CPU reduction and fewer frame spikes on busy scenes
- Risk/complexity of fixing: medium-high
- Priority: must fix

### Finding 3: Current render composition uses many cameras and fullscreen passes for a 2D scene that is still CPU-unstable

- Severity: High
- Confidence: proven
- Main impact: client GPU, extraction overhead, frame pacing
- Why it matters:
  - The in-world scene boots separate cameras for backdrop, planet bodies, gameplay, debug overlay, fullscreen foreground, post-process, and a persistent UI overlay camera.
  - That is acceptable only if the presentation stack is otherwise stable. Right now it is not, so pass count is compounding an already expensive frame.
- Evidence:
  - Camera topology in `bins/sidereal-client/src/runtime/scene_world.rs:41`
  - Post-process node insertion in `bins/sidereal-client/src/runtime/post_process.rs:40`
  - Runtime fullscreen/post-process renderable syncing in `bins/sidereal-client/src/runtime/backdrop.rs:349`
  - Fullscreen quad resync in `bins/sidereal-client/src/runtime/backdrop.rs:722`
- Concrete recommendation:
  - Budget the pass stack explicitly:
    - keep backdrop,
    - keep gameplay,
    - keep UI,
    - make planet-body pass conditional,
    - make explosion distortion conditional and telemetry-backed,
    - keep debug as opt-in only.
  - Do not add more cameras/passes until the client CPU and presentation churn are under control.
- Expected payoff: lower GPU overhead and less render-world churn; modest but real smoothness gain
- Risk/complexity of fixing: medium
- Priority: should fix

### Finding 4: Planet visuals are expensive by design and currently bypass frustum culling

- Severity: High
- Confidence: proven
- Main impact: client GPU, extraction overhead
- Why it matters:
  - Planet passes spawn multiple child quads per entity and explicitly attach `NoFrustumCulling`.
  - The update path recomputes transforms/material uniforms for all planet visual passes every `PostUpdate`.
- Evidence:
  - Planet pass attachment in `bins/sidereal-client/src/runtime/visuals.rs:1855`
  - `NoFrustumCulling` on planet body/cloud/ring passes in `bins/sidereal-client/src/runtime/visuals.rs:1937`, `bins/sidereal-client/src/runtime/visuals.rs:1989`, `bins/sidereal-client/src/runtime/visuals.rs:2068`
  - Per-frame planet visual update in `bins/sidereal-client/src/runtime/visuals.rs:2174`
- Concrete recommendation:
  - Add coarse projected-screen culling for planet pass trees before attaching/updating child visuals.
  - Keep `NoFrustumCulling` only if Bevy frustum culling is incompatible with the camera-relative/parallax projection, and document that explicitly.
  - Reduce per-planet pass count when authored stacks do not need clouds/rings.
- Expected payoff: large GPU/extraction win in planet-heavy scenes
- Risk/complexity of fixing: medium
- Priority: should fix

## 4. Client Render Pipeline Findings

### Finding 5: Runtime asset/shader attach can still hitch on the main thread during relevance and hot reload events

- Severity: High
- Confidence: strong inference
- Main impact: startup hitching, frame pacing, client CPU
- Why it matters:
  - The download/persist flow is async, but decoded images/SVG tessellation, shader reload, material creation, mesh creation, and child attachment still happen in frame systems.
  - Hot-reload and late relevance gains can therefore produce visible hitching even when network fetch is off-thread.
- Evidence:
  - Runtime asset queue/poll in `bins/sidereal-client/src/runtime/assets.rs:472` and `bins/sidereal-client/src/runtime/assets.rs:603`
  - `cached_image_handle` decodes bytes into `Image` on demand in `bins/sidereal-client/src/runtime/assets.rs:393`
  - `cached_svg_handle` tessellates on demand in `bins/sidereal-client/src/runtime/assets.rs:415`
  - Streamed visual attachment creates mesh/material children in `bins/sidereal-client/src/runtime/visuals.rs:960`
  - Fullscreen material allocation/rebind happens in `bins/sidereal-client/src/runtime/backdrop.rs:456`
- Concrete recommendation:
  - Prewarm and cache decoded/tessellated assets for bootstrap-critical visuals.
  - Separate asset-byte readiness from "safe to attach this frame" using a small per-frame attach budget.
  - Add telemetry for decode time, tessellation time, shader reload time, and attachment count per frame.
- Expected payoff: lower hitch frequency during world entry and hot reload
- Risk/complexity of fixing: medium
- Priority: should fix

### Finding 6: Material family count is directionally acceptable, but transitional content-specific families still inflate render setup cost

- Severity: Medium
- Confidence: proven
- Main impact: client CPU, client GPU, architecture/maintainability
- Why it matters:
  - The architecture intends a small fixed family taxonomy, but runtime still registers and maintains several effect-specific `Material2d` types.
  - That increases material/resource management complexity and reduces batching opportunities relative to a tighter generic path.
- Evidence:
  - Material plugin registration in `bins/sidereal-client/src/runtime/app_builder.rs:24`
  - Transitional status is explicitly acknowledged in `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
- Concrete recommendation:
  - Keep the render-layer architecture.
  - Continue reducing special-case material families where Bevy type-static constraints are no longer forcing them.
  - Do not churn this first; fix prediction/churn and broad scans first.
- Expected payoff: moderate long-term batching and maintenance improvement
- Risk/complexity of fixing: medium-high
- Priority: optional improvement

### Finding 7: The single post-process node is not the main bottleneck today, but it is still an expensive fullscreen tax

- Severity: Medium
- Confidence: strong inference
- Main impact: client GPU, frame pacing
- Why it matters:
  - `ExplosionDistortionPostProcessPlugin` adds an always-present render-graph node that performs a fullscreen pass whenever active shockwaves exist.
  - This is not likely the primary problem, but it is pure fullscreen work layered on top of an already heavy composition stack.
- Evidence:
  - Render-graph insertion in `bins/sidereal-client/src/runtime/post_process.rs:40`
  - View-driven fullscreen draw in `bins/sidereal-client/src/runtime/post_process.rs:94`
- Concrete recommendation:
  - Keep it, but instrument pass activation rate and GPU time before adding more post effects.
  - Consider folding it into a broader post stack only after timing data exists.
- Expected payoff: small-to-moderate
- Risk/complexity of fixing: low-medium
- Priority: optional improvement

## 5. ECS / Schedule / Transform Findings

### Finding 8: Tactical fog mask generation is an avoidable CPU spike when active

- Severity: Medium
- Confidence: proven
- Main impact: client CPU, frame pacing
- Why it matters:
  - The tactical overlay rebuilds a `384x384` fog texture on the CPU whenever view params change enough, iterating every texel and checking revealed-cell membership.
  - Camera pan/zoom while tactical view is active can therefore create repeatable CPU spikes.
- Evidence:
  - Overlay update in `bins/sidereal-client/src/runtime/ui.rs:1296`
  - Full fog texture rebuild loop in `bins/sidereal-client/src/runtime/ui.rs:1401`
- Concrete recommendation:
  - Move fog-mask generation to chunked dirty updates or GPU-side sampling from a compact cell texture.
  - At minimum, throttle rebuild cadence and quantize pan/zoom invalidation more aggressively.
- Expected payoff: moderate for tactical mode smoothness
- Risk/complexity of fixing: medium
- Priority: should fix

### Finding 9: Nameplate maintenance is still broad and scales with entity count

- Severity: Medium
- Confidence: proven
- Main impact: client CPU
- Why it matters:
  - Nameplate synchronization scans all canonical world entities with health, sorts targets, and then a second pass projects them each frame in `PostUpdate`.
  - This is manageable at low counts but will become visible as combat density rises.
- Evidence:
  - Allocation/sync path in `bins/sidereal-client/src/runtime/ui.rs:2017`
  - Projection/update path in `bins/sidereal-client/src/runtime/ui.rs:2106`
- Concrete recommendation:
  - Add distance/importance caps and avoid tracking every health-bearing entity as a default HUD target.
  - Use a maintained "eligible for nameplate" set rather than rediscovering every frame.
- Expected payoff: moderate CPU reduction in busy scenes
- Risk/complexity of fixing: medium
- Priority: should fix

### Finding 10: Debug overlay is heavy but correctly gated

- Severity: Low
- Confidence: proven
- Main impact: client CPU, debug-only
- Why it matters:
  - When enabled, it scans all world entities, asset/material counts, and pooled effect state.
  - This is expensive, but the cost is acceptable because it is operator-gated.
- Evidence:
  - Snapshot collection in `bins/sidereal-client/src/runtime/debug_overlay.rs:148`
  - Draw path in `bins/sidereal-client/src/runtime/debug_overlay.rs:555`
- Concrete recommendation:
  - Keep the overlay, but add per-system timing directly to the overlay so it can justify its own cost.
- Expected payoff: low production impact; high debugging value
- Risk/complexity of fixing: low
- Priority: optional improvement

## 6. Asset / Shader / Material Findings

### Finding 11: The asset-delivery architecture is correct and should be preserved

- Severity: Low
- Confidence: proven
- Main impact: architecture/maintainability
- Why it matters:
  - The code follows the desired architecture: HTTP delivery for bytes, shared cache/index, runtime invalidation, no replication payload streaming.
  - This is not the wrong direction; the remaining issue is runtime attach cost and telemetry gaps.
- Evidence:
  - Contract in `docs/features/asset_delivery_contract.md`
  - Client fetch path in `bins/sidereal-client/src/runtime/assets.rs:472`
  - Replication catalog invalidation in `bins/sidereal-replication/src/replication/assets.rs:95`
- Concrete recommendation:
  - Preserve the architecture.
  - Optimize decode/attach and add better timing counters instead of redesigning delivery.
- Expected payoff: preserves correct long-term direction
- Risk/complexity of fixing: low
- Priority: preserve

## 7. Lightyear / Replication / Visibility Findings

### Finding 12: Visibility and relevance churn on the server is a direct contributor to client render instability

- Severity: High
- Confidence: proven
- Main impact: replication churn, frame pacing, client CPU
- Why it matters:
  - The server recomputes desired visibility per entity per client and applies membership diffs every fixed tick.
  - On visibility gains it queues spatial resends explicitly.
  - The client then has to adopt entities, seed transforms, choose duplicate winners, attach visuals, and possibly fetch assets.
  - That is a direct server-to-client path for "render feels unstable".
- Evidence:
  - Visibility scheduling in `bins/sidereal-replication/src/plugins.rs:104`
  - Per-client candidate/context/membership update in `bins/sidereal-replication/src/replication/visibility.rs:1650`
  - Visibility gain resend in `bins/sidereal-replication/src/replication/visibility.rs:2005`
  - Client adoption and transform bootstrap in `bins/sidereal-client/src/runtime/plugins/replication_plugins.rs:34`
- Concrete recommendation:
  - Add hysteresis and/or stickiness to visibility membership near range boundaries.
  - Separate "authorized but dormant" from "fully adopted renderable" on the client so small membership oscillations do not re-trigger the entire attach/bootstrap path.
- Expected payoff: large smoothness gain in moving/edge-of-range scenes
- Risk/complexity of fixing: high
- Priority: must fix

### Finding 13: Tactical lane cadence is bursty by design and can amplify perceived hitching

- Severity: Medium
- Confidence: strong inference
- Main impact: replication churn, client CPU, bandwidth
- Why it matters:
  - Tactical updates send deltas every `0.5s` and snapshots every `2.0s`.
  - That cadence is reasonable functionally, but it creates visible burst opportunities when combined with HUD overlay rebuilds and fog-mask regeneration.
- Evidence:
  - Tactical cadence constants in `bins/sidereal-replication/src/replication/tactical.rs:24`
  - Client tactical overlay update path in `bins/sidereal-client/src/runtime/ui.rs:1296`
- Concrete recommendation:
  - Measure contact-count and fog-delta payload sizes against client overlay update cost.
  - If bursts are visible, smooth delivery or split heavy tactical UI work across frames.
- Expected payoff: moderate in map-heavy sessions
- Risk/complexity of fixing: medium
- Priority: should fix

## 8. Server-Side Contributors To Render Slowness

The server is not just a background concern here. It is actively shaping client smoothness.

1. Visibility update work is large and central. The hot path builds runtime layer definition maps, refreshes client contexts, derives candidate sets, and applies per-entity visibility diffs in [`bins/sidereal-replication/src/replication/visibility.rs:1650`](bins/sidereal-replication/src/replication/visibility.rs:1650).
2. The server logs a five-second visibility summary, but it does not expose the per-client/per-entity spike telemetry needed to correlate those updates with client hitching in [`bins/sidereal-replication/src/replication/visibility.rs:2267`](bins/sidereal-replication/src/replication/visibility.rs:2267).
3. Asset catalog invalidation is correct architecturally, but any catalog change can propagate into client shader/image rebind activity while the game is live in [`bins/sidereal-replication/src/replication/assets.rs:95`](bins/sidereal-replication/src/replication/assets.rs:95).

## 9. Documentation / Architecture Divergence

### Divergence 1: `dr-0027` is stale about fullscreen implementation status

- Severity: Medium
- Confidence: proven
- Why it matters:
  - `dr-0027` still says fullscreen background config entities remain replicated authoring surfaces while actual rendered fullscreen quads are client-only derived cached scene entities.
  - Current code and the active visibility contract disagree.
- Evidence:
  - Stale text in `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
  - Active contract says fullscreen authored entities render directly in `docs/features/visibility_replication_contract.md`
  - Code applies renderability directly onto fullscreen authored entities in `bins/sidereal-client/src/runtime/backdrop.rs:78`
- Recommendation:
  - Update `dr-0027` with a dated note that fullscreen authored entities now render directly again, while post-process still uses client-side derived renderables.

### Architecture That Should Be Preserved

1. Lua-authored render layers and rule-based assignment are directionally correct.
2. HTTP asset delivery with a validated local cache is correct.
3. The server-authoritative visibility contract is correct.
4. Camera-relative/parallax derivation is correct in principle; the issue is runtime cost and transitional implementation complexity.

## 10. End-to-End Render Flow Map

### 10.1 Asset/bootstrap to client-ready rendering

1. Startup/bootstrap manifests arrive through gateway/auth flows.
2. Client validates cache and fetches missing runtime assets in `bins/sidereal-client/src/runtime/assets.rs:472`.
3. Shader bytes are reloaded into Bevy shader assets in `bins/sidereal-client/src/runtime/scene_world.rs:58`.
4. In-world scene boots multiple cameras and overlay entities in `bins/sidereal-client/src/runtime/scene_world.rs:41`.
5. Runtime visuals attach images/materials/child meshes on demand in `bins/sidereal-client/src/runtime/visuals.rs:960`.

### 10.2 Replicated entity arrival to visible draw

1. Server grants visibility and may resend current spatial state on gain in `bins/sidereal-replication/src/replication/visibility.rs:2005`.
2. Client ensures replicated entities have spatial/render basics in `bins/sidereal-client/src/runtime/replication.rs:106`.
3. Client adopts replicated entities and synchronizes frame interpolation markers in `bins/sidereal-client/src/runtime/plugins/replication_plugins.rs:34`.
4. Client seeds missing transforms / hides entities until ready in `bins/sidereal-client/src/runtime/transforms.rs:358`.
5. Visuals plugin suppresses duplicate copies and attaches visual children in `bins/sidereal-client/src/runtime/visuals.rs:531` and `bins/sidereal-client/src/runtime/visuals.rs:960`.
6. PostUpdate camera-relative transform systems derive final visible transforms in `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:18`.

### 10.3 Camera-relative/world-layer transform derivation

1. World entities keep authoritative positions.
2. Resolved render layer carries parallax/depth metadata from `bins/sidereal-client/src/runtime/render_layers.rs:193`.
3. Streamed child visuals apply camera-relative offsets in `bins/sidereal-client/src/runtime/visuals.rs:1783`.
4. Planet visuals apply their own camera-relative translation and layer scaling in `bins/sidereal-client/src/runtime/visuals.rs:1855`.

### 10.4 Fullscreen/post-process execution

1. Backdrop/fullscreen foreground/post-process renderables are synchronized in `bins/sidereal-client/src/runtime/backdrop.rs:349`.
2. Dedicated cameras render backdrop, foreground, and post-process layers from `bins/sidereal-client/src/runtime/scene_world.rs:63`.
3. Explosion distortion inserts a render-graph fullscreen pass in `bins/sidereal-client/src/runtime/post_process.rs:40`.

### 10.5 Prediction/reconciliation/interpolation to final presented motion

1. Local input enters fixed-step prediction in `bins/sidereal-client/src/runtime/plugins/replication_plugins.rs:118`.
2. Motion ownership avoids enabling local physics writes unless a predicted clone exists in `bins/sidereal-client/src/runtime/motion.rs:64`.
3. Lightyear interpolation and rollback visual correction run.
4. Sidereal then runs transform stall recovery and delayed camera follow in `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:18`.

## 11. Performance Budget Map

Inference labels:

1. Client CPU, proven hot:
  - duplicate visual suppression,
  - render-layer assignment maintenance,
  - runtime asset dependency scans,
  - nameplate sync/projection,
  - tactical fog texture rebuilds,
  - planet visual update loops,
  - debug overlay when enabled.
2. Client GPU, strong inference:
  - multiple active cameras,
  - fullscreen passes,
  - uncullable planet pass trees,
  - alpha-heavy fullscreen background and effect materials,
  - possible overdraw from layered planet/cloud/ring passes.
3. Client main-thread stalls, strong inference:
  - image decode,
  - SVG tessellation,
  - shader reload and material attachment,
  - large bursts of entity adoption/visual child spawning.
4. Server tick cost affecting smoothness, proven:
  - visibility cache/index/client-context refresh,
  - per-entity visibility diff application,
  - visibility-gain spatial resend logic.
5. Network/replication cost affecting render churn, strong inference:
  - relevance oscillation near delivery bounds,
  - tactical burst cadence,
  - asset catalog invalidation causing in-world reload/rebind work.

## 12. Specific Confirm / Refute Judgments

1. The game is GPU-bound in normal gameplay.
  - Refute as primary cause. Evidence is insufficient for normal-play GPU saturation, and code shape points more strongly at CPU/pacing/churn.
2. The game is CPU-bound on the client in normal gameplay.
  - Strongly supported.
3. ECS scheduling/query work bottlenecks more than draw submission.
  - Strongly supported.
4. Replication/update churn bottlenecks more than rendering itself.
  - Strongly supported for perceived slowness; not fully proven for total frame time.
5. The game feels slow mainly because of frame pacing/interpolation issues rather than raw frame time.
  - Strongly supported.
6. Shader/material diversity is defeating batching enough to matter.
  - Moderately supported, but secondary to prediction/churn and broad scans.
7. Too many fullscreen or post-process passes are active for the current payoff.
  - Supported.
8. Off-screen or non-visible entities are still paying too much render-related cost.
  - Supported, especially planet pass trees and broad UI/maintenance scans.
9. Asset/shader compilation or loading hitching is a meaningful stall source.
  - Strong inference.
10. Server-side visibility/replication behavior is causing client render instability or overload.
  - Proven.
11. The current render-layer architecture is directionally correct and should be kept.
  - Proven.
12. The current render-layer/material implementation has avoidable transitional cost.
  - Proven.

## 13. Prioritized Remediation Plan

### Top 5 highest-ROI changes

1. Eliminate duplicate predicted/interpolated presentation lanes as a steady-state client concern.
2. Add visibility hysteresis/stickiness so range-boundary churn stops forcing adopt/bootstrap/attach cycles.
3. Replace broad per-frame world scans with lifecycle-driven registries for render attachments, asset dependency demand, and HUD targets.
4. Add coarse culling and pass budgeting for planet/fullscreen/post-process paths.
5. Budget runtime asset attach work per frame and instrument decode/reload/attach timings.

### Quick wins

1. Add hard caps and distance gating for nameplates.
2. Quantize tactical fog invalidation more aggressively.
3. Record pass activation counts for explosion distortion and backdrop layers.
4. Stop logging/status-updating on every routine runtime asset event in high-churn scenarios unless debugging.

### Medium refactors

1. Maintain a dedicated registry of render-attachment candidates instead of scanning `WorldEntity`.
2. Split `visuals.rs`, `ui.rs`, and `visibility.rs` into domain modules so budgets and ownership become tractable.
3. Separate "asset is byte-ready" from "attach now" with a frame-budgeted activation queue.

### Large architectural changes

1. Rework control/prediction/relevance lifecycle so a logical entity has one presentation winner without client-side repair logic.
2. Introduce server-side visibility dampening or client-side dormant-authorized state to prevent churn-induced respawn behavior.

### What to measure before/after each major fix

1. Per-frame visible entity count by lane: confirmed, interpolated, predicted, duplicate-suppressed.
2. Per-frame attach/detach counts for streamed visuals, planet passes, fullscreen renderables.
3. Per-system timing for duplicate suppression, render-layer resolution, asset dependency sync, nameplate update, tactical fog rebuild.
4. Visibility gain/loss counts per client per second and resends on gain.
5. Frame-time percentiles, not just average FPS.

## 14. Instrumentation / Profiling Gaps

Missing telemetry that should be added first:

1. Frame pacing:
  - frame-time histogram,
  - 95th/99th percentile frame time,
  - hitch counter over 16.7ms / 33.3ms / 50ms.
2. Per-system timing:
  - Bevy timings for every system in `ClientVisualsPlugin`, replication runtime plugin, UI post-update plugin, and server visibility update.
3. Render pass timing:
  - backdrop pass,
  - planet pass camera,
  - gameplay camera,
  - foreground camera,
  - post-process node.
4. Draw/material/entity counts:
  - visible draw count,
  - material instance count by family,
  - fullscreen pass count,
  - planet pass child count,
  - streamed visual child count.
5. Asset/shader stall telemetry:
  - decode ms,
  - tessellation ms,
  - shader reload ms,
  - attach count and ms per frame,
  - hot-reload invalidation counts.
6. Replication/visibility telemetry:
  - candidate count per client,
  - visible gain/loss count per client,
  - visibility resend count,
  - entity adoption count,
  - predicted/interpolated duplicate group count,
  - tactical delta/snapshot payload size and contact count.

## 15. Runtime Catalog Appendix

### Client runtime pieces that materially affect rendering performance

1. `bins/sidereal-client/src/runtime/visuals.rs`
  - Active runtime
  - High impact
  - Transitional/migration cost present
2. `bins/sidereal-client/src/runtime/backdrop.rs`
  - Active runtime
  - High impact
  - Transitional/migration cost present
3. `bins/sidereal-client/src/runtime/transforms.rs`
  - Active runtime
  - High impact
  - Contains fallback/repair logic likely removable later
4. `bins/sidereal-client/src/runtime/render_layers.rs`
  - Active runtime
  - High impact
5. `bins/sidereal-client/src/runtime/assets.rs`
  - Active runtime
  - High impact
6. `bins/sidereal-client/src/runtime/ui.rs`
  - Active runtime
  - High impact
7. `bins/sidereal-client/src/runtime/post_process.rs`
  - Active runtime
  - Moderate impact
8. `bins/sidereal-client/src/runtime/debug_overlay.rs`
  - Debug/diagnostic
  - Heavy when enabled
9. `bins/sidereal-client/src/runtime/scene_world.rs`
  - Active runtime
  - Important for pass/camera topology

### Replication server pieces that materially affect rendering performance

1. `bins/sidereal-replication/src/replication/visibility.rs`
  - Active runtime
  - High impact
  - Monolithic and optimization-hostile
2. `bins/sidereal-replication/src/replication/tactical.rs`
  - Active runtime
  - Moderate impact
3. `bins/sidereal-replication/src/replication/assets.rs`
  - Active runtime
  - Moderate impact
4. `bins/sidereal-replication/src/replication/runtime_state.rs`
  - Active runtime
  - Moderate impact

### Gateway/bootstrap/asset-delivery pieces

1. `docs/features/asset_delivery_contract.md`
  - Active contract
  - Correct direction
2. Client startup/bootstrap/auth asset paths
  - Active runtime
  - Indirect startup hitching impact

### Shared gameplay/render-support crates and modules

1. `crates/sidereal-game/src/components/runtime_render_layer_definition.rs`
  - Active runtime schema
2. `crates/sidereal-game/src/components/runtime_world_visual_stack.rs`
  - Active runtime schema
3. `crates/sidereal-game/src/render_layers.rs`
  - Active runtime support
4. `crates/sidereal-runtime-sync/src/lib.rs`
  - Active runtime support

## 16. Final Recommendation

Do not treat this as a shader micro-optimization task first. The first performance project should be "stabilize the presentation lifecycle" across server visibility, client adoption, interpolation, and duplicate resolution. If that work succeeds, several expensive client-side repair systems should disappear entirely. That is the fastest path to making the game feel materially smoother.

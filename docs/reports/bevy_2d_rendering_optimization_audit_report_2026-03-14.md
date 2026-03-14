# Bevy 2D Rendering Optimization Audit Report

Status note 2026-03-14: fresh audit generated from prompt-guided code inspection of prompt files, source-of-truth docs, and runtime code. I did not inspect existing files under `docs/reports/` or `docs/plans/`.

## Executive Summary

Sidereal does not look primarily GPU-bound in normal gameplay. The stronger evidence points to client CPU schedule pressure, duplicate visual ownership/correction churn, tactical-map-specific CPU work, and server-side visibility/replication churn that destabilizes what the renderer is asked to present.

The biggest "game feels slow" problem is frame pacing, not just average frame time. The client currently carries several repair layers to keep predicted, interpolated, and confirmed visuals aligned, and those layers exist because the authoritative/presented motion ownership model is still not cleanly collapsed.

The biggest local client rendering hotspot is the tactical map overlay path. When active, it does broad per-frame camera/UI/marker work and rewrites a CPU-generated fog mask texture every frame. The biggest cross-stack bottleneck is the replication visibility pipeline, which does substantial per-client and per-entity work at 60 Hz and can indirectly create client churn even when the renderer itself is not saturated.

The render-layer architecture is directionally correct and worth keeping, but the current implementation still pays transitional CPU cost every `Update`. It is solving a real data-driven problem in a more expensive way than the runtime can currently afford.

## Confirm / Refute

1. GPU-bound in normal gameplay: mostly refuted. I found more evidence of client CPU and pacing pressure than raw draw-submission saturation.
2. CPU-bound on the client in normal gameplay: likely true. Large `Update` and `PostUpdate` chains, duplicate-visual suppression, transform recovery, render-layer assignment, and tactical overlay work all point this way.
3. ECS scheduling/query work bottlenecks more than actual draw submission: likely true. The hot paths are schedule-heavy and query-heavy before they become GPU-heavy.
4. Replication/update churn bottlenecks more than rendering itself: likely true in many scenes. The visibility path and clone/adoption churn look more important than pure sprite fill cost.
5. Slow feel mainly comes from frame pacing/interpolation issues rather than raw frame time: likely true.
6. Shader/material diversity is defeating batching enough to matter: partially true. It is not the top problem, but eight `Material2dPlugin`s plus multiple layer/passthrough paths increase extraction, material diversity, and batching fragmentation.
7. Too many fullscreen/post-process passes are active for current payoff: partially true. The pass count is higher than the current stability of the rest of the runtime justifies.
8. Off-screen or non-visible entities still pay too much render-related cost: true. There are multiple hot paths that keep processing entities even when the useful visible subset should be much smaller.
9. Asset/shader compilation or loading hitching is a meaningful stall source: plausible at startup and hot reload, but not proven as the primary steady-state issue from code alone.
10. Server visibility/replication behavior is causing client render instability or overload: true.
11. Current render-layer architecture is directionally correct and should be kept: true.
12. Current render-layer/material implementation has avoidable transitional cost and should be simplified: true.

## Findings

### 1. "Map mode" currently means two different things in UI and replication code

- Severity: Medium
- Type: architecture, maintainability, performance
- Priority: should fix
- Why it matters: pressing `M` enables the tactical map overlay and starts a zoom transition, but the replication-facing `ClientLocalViewMode::Map` state is keyed off a deeper camera-distance threshold. If that split is intentional, the runtime is functioning as designed. The issue is that the codebase currently uses the same "map mode" label for two different concepts: visual transition state and server visibility mode.
- Evidence:
  - `bins/sidereal-client/src/runtime/control.rs:141-154`
  - `crates/sidereal-game/src/components/tactical_map_ui_settings.rs:44-49`
  - `bins/sidereal-client/src/runtime/ui.rs:517-533`
  - `bins/sidereal-replication/src/replication/visibility.rs:1171-1217`
- Recommendation: document the distinction explicitly and rename or clarify the replication-side state so it is obvious that `M` starts a visual map transition, while server `Map` mode only activates once the zoom reaches the strategic threshold. If that distinction is no longer useful, collapse both concepts into one canonical definition.

### 2. Tactical map overlay is the clearest immediate client-side hotspot

- Severity: Critical
- Type: performance
- Priority: must fix
- Why it matters: the tactical map path does broad per-frame work when active: camera activation, HUD visibility, overlay visibility, dynamic marker bookkeeping, SVG asset cache use, cursor/title updates, and material updates. On top of that, the fog mask texture is rebuilt on the CPU every frame.
- Evidence:
  - `bins/sidereal-client/src/runtime/ui.rs:562-760`
  - `bins/sidereal-client/src/runtime/ui.rs:1197-1415`
- Specific issue: `update_tactical_fog_mask_texture` mutates a `384x384` `R8Unorm` image from CPU memory every frame and builds a temporary `HashSet` of explored cells before walking the mask buffer.
- Recommendation:
  - Gate more of the overlay work behind change detection or a lower-frequency map update cadence.
  - Stop rebuilding the fog mask every frame; move to incremental dirty-cell updates, a GPU-side mask, or an offscreen texture updated only when map pan/zoom or fog state actually changes.
  - Split overlay state transitions from steady-state rendering so the common frame only does minimal work.

### 3. Perceived slowness is dominated by visual ownership instability and frame-pacing repair layers

- Severity: Critical
- Type: architecture, performance
- Priority: must fix
- Why it matters: the client has multiple systems whose job is to repair transform and presentation instability after prediction/interpolation/adoption churn. That is strong evidence that the presented motion lane is not cleanly owned.
- Evidence:
  - `bins/sidereal-client/src/runtime/transforms.rs:123-181`
  - `bins/sidereal-client/src/runtime/transforms.rs:184-223`
  - `bins/sidereal-client/src/runtime/transforms.rs:225-283`
  - `bins/sidereal-client/src/runtime/visuals.rs:529-790`
  - `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:22-52`
- Interpretation: the renderer is paying for duplicate predicted/interpolated clones, winner selection, stale-transform recovery, and camera follow timing corrections. This is exactly the kind of code that can make a game feel laggy even when average FPS looks acceptable.
- Recommendation:
  - Collapse clone adoption and visual ownership so each visible entity has one authoritative presentation lane at a time.
  - Treat the existing fallback systems as migration scaffolding, then remove them rather than normalize them as permanent runtime layers.
  - Measure frame-time variance and rollback/correction frequency together; average FPS alone will hide this problem.

### 4. The replication visibility pipeline is a major indirect render-smoothness bottleneck

- Severity: Critical
- Type: architecture, performance
- Priority: must fix
- Why it matters: `update_network_visibility` does scratch rebuilds, client-context cache work, candidate generation, disclosure sync, and per-entity/per-client visibility application in one large 60 Hz path. Even if rendering were cheap, unstable or bursty visibility results would still make the client feel bad.
- Evidence:
  - `bins/sidereal-replication/src/plugins.rs:146-190`
  - `bins/sidereal-replication/src/replication/visibility.rs:1397-1515`
  - `bins/sidereal-replication/src/replication/visibility.rs:1616-2326`
- Specific risk points:
  - full spatial-index rebuild conditions remain broad enough to matter
  - candidate-set generation is per-client
  - final visibility application still loops entity-by-entity and client-by-client
  - telemetry already exists, which suggests this path has been hot enough to instrument
- Recommendation:
  - Split the monolith into cache build, candidate generation, disclosure sync, and membership diff stages with explicit budgets.
  - Push more work toward persistent incremental caches instead of scratch rebuilds.
  - Clarify whether the deeper zoom threshold is intentionally the point where server-side strategic delivery begins, because the current naming makes that hard to infer from code alone.

### 5. Render-layer architecture is correct in direction, but current implementation is still too expensive

- Severity: High
- Type: architecture, performance
- Priority: should fix
- Why it matters: the repo is moving toward Lua-authored render layers and generic runtime rules, which is a good direction. The implementation, however, still runs registry sync and assignment resolution in hot frame paths and performs world scans, cloning, hashing, and rule compilation work that should become more event-driven over time.
- Evidence:
  - `bins/sidereal-client/src/runtime/render_layers.rs:20-193`
  - `bins/sidereal-client/src/runtime/render_layers.rs:195-250`
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
- Recommendation:
  - Keep the architecture.
  - Replace frame-driven registry rebuilding with explicit invalidation.
  - Narrow assignment recompute scopes further and move compiled-rule generation out of normal `Update` whenever authored definitions are unchanged.

### 6. Client render prep is schedule-heavy before it ever becomes GPU-heavy

- Severity: High
- Type: performance, maintainability
- Priority: should fix
- Why it matters: the client’s active in-world loop is distributed across dense `Update`, `PostUpdate`, and `Last` chains. That increases scheduler overhead, widens the amount of state touched each frame, and makes profiling noisier.
- Evidence:
  - `bins/sidereal-client/src/runtime/plugins/replication_plugins.rs:39-99`
  - `bins/sidereal-client/src/runtime/plugins/presentation_plugins.rs:29-111`
  - `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:18-93`
  - `bins/sidereal-client/src/runtime/app_setup.rs:160-243`
- Recommendation:
  - Introduce stricter runtime budgets per phase.
  - Convert always-on maintenance systems into event-driven or state-gated systems where possible.
  - Separate "entity became relevant" work from "steady-state per-frame presentation" work.

### 7. Multi-camera and multi-pass usage is higher than current runtime stability justifies

- Severity: Medium
- Type: performance
- Priority: should fix
- Why it matters: Sidereal currently keeps dedicated cameras for backdrop, planet bodies, gameplay, debug overlay, fullscreen foreground, post-process, and UI overlay. That is not automatically wrong, but it increases extraction/submission work and multiplies the cost of every other instability in the frame.
- Evidence:
  - `bins/sidereal-client/src/runtime/scene_world.rs:68-208`
  - `bins/sidereal-client/src/runtime/scene.rs`
  - `bins/sidereal-client/src/runtime/app_builder.rs:24-36`
- Recommendation:
  - Re-evaluate which passes need their own camera now versus later.
  - Collapse passes that only exist to support transitional visual layering.
  - Keep fullscreen/background passes, but be stricter about adding new dedicated cameras until frame pacing is stable.

### 8. Fullscreen and no-frustum-culling paths are defensible in isolation, but dangerous in aggregate

- Severity: Medium
- Type: performance
- Priority: optional improvement
- Why it matters: fullscreen quads and planet visuals often intentionally bypass Bevy’s default culling. That is fine for a small number of carefully managed passes, but it compounds cost if other systems are already doing too much per frame.
- Evidence:
  - `bins/sidereal-client/src/runtime/scene_world.rs:68-208`
  - `bins/sidereal-client/src/runtime/plugins/presentation_plugins.rs:90-111`
  - custom planet/fullscreen visual handling in `bins/sidereal-client/src/runtime/visuals.rs`
- Recommendation: keep custom culling where visually necessary, but budget it explicitly and avoid adding more uncullable renderables until the duplicate-visual and tactical overlay problems are resolved.

### 9. Asset/bootstrap behavior is not the main steady-state problem, but startup and reload hitch risk is real

- Severity: Medium
- Type: performance
- Priority: optional improvement
- Why it matters: asset bootstrap catalog loading and audio catalog loading are already offloaded on the gateway, and client HTTP fetches are not obviously happening on the render thread. That is good. The risk is less "main thread file IO every frame" and more "content invalidation/adoption churn causes visible hitches."
- Evidence:
  - `bins/sidereal-gateway/src/api.rs:372-417`
  - `bins/sidereal-gateway/src/api.rs:564-679`
  - `bins/sidereal-client/src/runtime/plugins/replication_plugins.rs:47-55`
  - `bins/sidereal-client/src/runtime/plugins/presentation_plugins.rs:83-89`
- Recommendation:
  - Profile startup separately from steady-state gameplay.
  - Track shader/material reload and asset adoption counts alongside frame spikes.
  - Do not prioritize asset streaming changes ahead of the ownership, visibility, and tactical overlay problems.

### 10. Diagnostics are present, but the profiling surface is still missing the most actionable budgets

- Severity: Low
- Type: maintainability
- Priority: optional improvement
- Why it matters: the codebase already has `FrameTimeDiagnosticsPlugin`, HUD perf counters, render-layer perf counters, visibility telemetry, and debug overlay hooks. What it still lacks is a short list of authoritative budgets that connect client frame spikes to server visibility spikes, rollback bursts, and asset adoption churn.
- Evidence:
  - `bins/sidereal-client/src/runtime/app_builder.rs:36`
  - `bins/sidereal-client/src/runtime/ui.rs:648-655`
  - `bins/sidereal-client/src/runtime/render_layers.rs:21-25`
  - `bins/sidereal-replication/src/replication/visibility.rs:2173-2326`
- Recommendation: log and graph, at minimum, per-frame duplicate-visual group count, rollback/correction counts, visibility candidate counts, streamed-asset adoption counts, and tactical overlay milliseconds.

## End-to-End Render Flow Map

### 1. Asset/bootstrap to client-ready rendering

- Gateway serves `/assets/bootstrap-manifest` and `/assets/{asset_guid}` from a Lua-authored registry-derived runtime catalog (`bins/sidereal-gateway/src/api.rs:372-460`, `564-679`).
- Client runtime tracks asset dependency state, queues missing assets, polls HTTP fetches, then attaches streamed visual assets in `Update` (`bins/sidereal-client/src/runtime/plugins/replication_plugins.rs:47-55`, `bins/sidereal-client/src/runtime/plugins/presentation_plugins.rs:83-89`).
- Render-layer and shader assignment sync happen before streamed visuals attach, so presentation depends on both replication adoption and authored layer/rule state (`bins/sidereal-client/src/runtime/plugins/presentation_plugins.rs:29-57`).

### 2. Replicated entity arrival to visible draw

- Replicated entities are adopted into native client runtime structures in `Update` (`bins/sidereal-client/src/runtime/plugins/replication_plugins.rs:56-68`).
- Transform history markers and confirmed/interpolated pose sync happen next (`bins/sidereal-client/src/runtime/plugins/replication_plugins.rs:58-68`).
- Duplicate predicted/interpolated visuals are then resolved, and visual children/effects are attached (`bins/sidereal-client/src/runtime/plugins/presentation_plugins.rs:34-57`).
- Camera follow and final visual-layer transforms happen in `PostUpdate` after Lightyear interpolation and rollback visual correction (`bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:20-52`).

### 3. Camera-relative/world-layer transform derivation

- Gameplay camera state is maintained as a top-down distance-based camera (`bins/sidereal-client/src/runtime/scene_world.rs:108-132`).
- Post-interpolation camera follow updates the gameplay camera, then planet-body/UI/debug cameras are synchronized from it (`bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:36-44`).
- Streamed visual layer transforms and planet-body visuals are updated after camera motion state is refreshed, so camera-relative presentation is downstream of the corrected visual pose rather than physics alone (`bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:44-52`).

### 4. Fullscreen background/foreground/post-process execution

- The client builds dedicated material plugins for starfield, background, nebula, streamed sprites, asteroid sprites, planet visuals, runtime effects, and tactical map overlay (`bins/sidereal-client/src/runtime/app_builder.rs:24-36`).
- Dedicated cameras then render backdrop, planet body, gameplay, debug, fullscreen foreground, post-process, and UI overlay passes (`bins/sidereal-client/src/runtime/scene_world.rs:68-208`).
- Backdrop/post-process renderables are synchronized every `Update`, then final fullscreen material updates happen in `Last` (`bins/sidereal-client/src/runtime/plugins/presentation_plugins.rs:90-111`, `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:75-93`).

### 5. Prediction/reconciliation/interpolation to final presented motion

- Lightyear client plugins, Avian replication, and `FrameInterpolationPlugin<Transform>` are installed in client startup (`bins/sidereal-client/src/runtime/app_setup.rs:115-139`).
- The runtime then layers its own bootstrap sync for missing interpolation history, explicit `FrameInterpolate<Transform>` marker maintenance, and stale-transform recovery (`bins/sidereal-client/src/runtime/transforms.rs:123-283`).
- Camera follow waits until after Lightyear interpolation and rollback correction to reduce disagreement with the finally rendered pose (`bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:22-40`).
- This is effective as damage control, but it confirms that the authoritative presentation flow is still more complex than it should be.

## Performance Budget Map

The breakdown below is partly inferential from code structure; inferences are labeled.

### Client CPU

- Highest risk:
  - tactical map overlay and fog mask work
  - duplicate-visual suppression and transform repair
  - render-layer registry/assignment maintenance
  - streamed visual attach/update pipelines
- Evidence:
  - `bins/sidereal-client/src/runtime/ui.rs:562-760`
  - `bins/sidereal-client/src/runtime/ui.rs:1197-1415`
  - `bins/sidereal-client/src/runtime/visuals.rs:529-790`
  - `bins/sidereal-client/src/runtime/render_layers.rs:20-250`

### Client GPU

- Likely secondary in normal play.
- Cost sources:
  - multiple fullscreen and post-process passes
  - planet/body/effect materials
  - alpha-heavy overlay and background work
- Inference: GPU cost rises meaningfully during tactical overlay and fullscreen effects, but the codebase shows more CPU-side frame-shaping pressure than raw GPU saturation.

### Client Main-Thread Stalls

- Highest risk:
  - tactical fog mask rebuild and general tactical overlay work
  - large one-frame adoption/attachment churn
  - hot-reload/catalog invalidation bursts
- Less likely primary steady-state source:
  - gateway catalog build, because that is already offloaded from request threads
  - client HTTP fetches, because the visible fetch path is async/off-main-thread

### Server Tick Cost That Affects Visual Smoothness

- Highest risk:
  - visibility entity-cache refresh
  - spatial-index maintenance
  - candidate generation
  - per-entity membership updates
- Evidence:
  - `bins/sidereal-replication/src/plugins.rs:151-190`
  - `bins/sidereal-replication/src/replication/visibility.rs:1397-1515`
  - `bins/sidereal-replication/src/replication/visibility.rs:1616-2326`

### Network/Replication Delivery Cost That Affects Render Churn

- Highest risk:
  - visibility candidate overreach
  - unclear separation between visual map transition state and replication map-delivery state
  - clone/adoption/despawn churn that forces duplicate-visual winner selection
- Inference: current client presentation complexity suggests the client is spending render-adjacent work compensating for unstable or over-broad authoritative delivery rather than simply drawing too many sprites.

## What To Fix First

1. Clarify and document the distinction between visual tactical-map transition state and replication-side map-delivery mode so the current threshold behavior is obviously intentional.
2. Remove the tactical map fog-mask full-frame CPU rebuild and aggressively gate the rest of the overlay path.
3. Simplify duplicate predicted/interpolated/confirmed presentation so one visible entity has one clean presentation owner.
4. Split and budget the replication visibility monolith; stop treating it as one acceptable 60 Hz blob.
5. Convert render-layer registry and assignment maintenance from frame-driven work to explicit invalidation.
6. Only after the above, revisit pass count, batching, material consolidation, and lower-level Bevy render tuning.

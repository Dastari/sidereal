# Bevy 2D Rendering Optimization Audit Report

Status: Active  
Report date: 2026-03-10  
Prompt source path: `docs/prompts/bevy_2d_rendering_optimization_audit_prompt.md`  
Scope: Client-visible render smoothness across Bevy 2D rendering, camera/layer/material/shader paths, ECS scheduling, Lightyear prediction/interpolation handoff, asset/bootstrap behavior, and replication/visibility delivery that affects perceived render performance.  
Limitations: Static code audit only. No live frame captures, RenderDoc traces, Tracy/Puffin captures, packet captures, or GPU timing data were available. Any conclusion about GPU saturation or percentile frame spikes is inference unless explicitly tied to code behavior.  

Update note (2026-03-10):
- This report supersedes the earlier same-day draft.
- After re-auditing the client and replication code, the strongest bottlenecks are broad ECS polling, serialized asset delivery, per-instance material pressure, UI/per-view update cost, and replication visibility cadence. The prior claim that fullscreen quads are reallocated every frame was incorrect; fullscreen quad caching already exists in `bins/sidereal-client/src/native/backdrop.rs:643`.

## 1. Executive Summary

The game does not read like a normal-play 2D client that is primarily GPU-bound. It reads like a mixed frame-pacing and CPU scheduling problem, with a meaningful server-side cadence problem upstream of the renderer.

The most likely reasons the game feels slow are:

1. The replication server rebuilds visibility scratch state and reevaluates visibility for every replicated entity against every client every fixed tick in `bins/sidereal-replication/src/replication/visibility.rs:594`.
2. Client asset readiness is serialized twice: required bootstrap assets are downloaded one-by-one in `bins/sidereal-client/src/native/auth_net.rs:308`, and runtime optional assets are fetched one-at-a-time in `bins/sidereal-client/src/native/assets.rs:334`.
3. The client still has several broad polling systems in the hot path, especially shader assignment inference in `bins/sidereal-client/src/native/shaders.rs:640`, render-layer registry/assignment maintenance in `bins/sidereal-client/src/native/render_layers.rs:20` and `bins/sidereal-client/src/native/render_layers.rs:195`, and UI/world overlay work in `bins/sidereal-client/src/native/ui.rs:356` and `bins/sidereal-client/src/native/ui.rs:1696`.
4. The renderer is still paying for many custom `Material2d` instances for planet passes, effect passes, asteroid passes, and fullscreen/post-process bindings in `bins/sidereal-client/src/native/visuals.rs:1357`, `bins/sidereal-client/src/native/visuals.rs:1892`, `bins/sidereal-client/src/native/visuals.rs:2139`, `bins/sidereal-client/src/native/visuals.rs:2192`, and `bins/sidereal-client/src/native/backdrop.rs:502`.
5. The runtime clearly still has presentation-lifecycle instability around prediction/interpolation handoff, which hurts smoothness even though the project already made the correct decision to render after Lightyear interpolation/correction in `bins/sidereal-client/src/native/plugins.rs`.

The architecture that should be preserved:

1. Render-derived parallax and layer depth rather than mutating authoritative world positions.
2. Post-interpolation/post-correction camera follow.
3. Server-authored visibility with `Authorization -> Delivery -> Payload`.
4. HTTP asset payload delivery rather than streaming asset bytes over replication.
5. Lua-authored render layers and shader families as the long-term direction.

## 2. What Most Likely Makes The Game Feel Slow

The game can feel slow even when raw FPS is acceptable because several costs affect cadence and presentation quality more than average frame time:

1. Authoritative update cadence can become uneven when replication visibility work spikes on the server.
2. The client still performs broad ECS polling and UI/view bookkeeping every frame, even when authored render state is mostly stable.
3. Control handoff, duplicate-entity suppression, and transform recovery logic show that the main pain is not “missing interpolation,” but instability around the interpolation path.
4. Asset bootstrap and lazy streaming are serialized, so the game can feel slow to become visually complete even after input/render are technically running.
5. The active scene uses multiple cameras and several custom material families before the scene is cheap enough to comfortably afford them.

## 3. Critical Findings

### F1. Replication visibility is still a full scratch rebuild plus per-client fanout every fixed tick

- Severity: Critical
- Confidence: Proven
- Main impact: `replication churn`, `frame pacing`, `client CPU` indirectly via unstable delivery cadence
- Exact references:
  - `bins/sidereal-replication/src/replication/visibility.rs:594`
  - `bins/sidereal-replication/src/replication/visibility.rs:670`
  - `bins/sidereal-replication/src/replication/visibility.rs:913`
  - `bins/sidereal-replication/src/replication/visibility.rs:1124`
- Why it matters:
  - `update_network_visibility()` clears and rebuilds `VisibilityScratch` every tick.
  - It rebuilds candidate sets per client.
  - It rescans replicated entities for newly discovered landmarks per player.
  - It then loops replicated entities against live clients and mutates `ReplicationState` one client at a time.
  - Even if GPU load is moderate, bursty or delayed authoritative delivery makes the client look jittery, correction-heavy, or “laggy.”
- Concrete recommendation:
  - Persist scratch indices across ticks instead of rebuilding them wholesale.
  - Move landmark discovery to a lower-frequency lane or dirty spatial trigger.
  - Separate “entity metadata/index upkeep” from “per-client visibility decisions.”
  - Add per-tick timings for scratch build, candidate generation, discovery scan, and per-client apply.
- Expected payoff:
  - Largest likely improvement in overall smoothness under multi-entity or multi-client load.
  - Reduced client-side adoption/correction churn.
- Risk/complexity of fixing: High
- Disposition: Must fix

### F2. Asset delivery is serialized in both bootstrap and lazy runtime fetch paths

- Severity: Critical
- Confidence: Proven
- Main impact: `startup hitching`, `memory/bandwidth`, `frame pacing` indirectly via late visual completion
- Exact references:
  - `bins/sidereal-client/src/native/auth_net.rs:308`
  - `bins/sidereal-client/src/native/auth_net.rs:552`
  - `bins/sidereal-client/src/native/assets.rs:334`
  - `bins/sidereal-client/src/native/assets.rs:509`
- Why it matters:
  - Required bootstrap assets are checked and, if needed, fetched and written one-by-one inside a single async task.
  - Optional runtime fetches allow only one in-flight asset task because `RuntimeAssetHttpFetchState` stores a single `pending` task and the queue path early-returns while it exists.
  - This does not just delay content; it prolongs the period where the game feels incomplete, soft-stalled, or visually inconsistent.
- Concrete recommendation:
  - Allow bounded parallel bootstrap downloads for required assets.
  - Allow a bounded N-way runtime lazy fetch queue instead of one-at-a-time fetch.
  - Track per-asset priority so shaders/visual roots win over optional secondary art.
- Expected payoff:
  - Faster world-ready time.
  - Less runtime pop-in and fewer “renderer is slow” false positives caused by missing content.
- Risk/complexity of fixing: Medium
- Disposition: Must fix

## 4. Client Render Pipeline Findings

### F3. The client still has broad hot-path polling around shader assignment and render-layer state

- Severity: High
- Confidence: Proven
- Main impact: `client CPU`, `architecture/maintainability`
- Exact references:
  - `bins/sidereal-client/src/native/shaders.rs:640`
  - `bins/sidereal-client/src/native/render_layers.rs:20`
  - `bins/sidereal-client/src/native/render_layers.rs:195`
  - `bins/sidereal-client/src/native/render_layers.rs:530`
- Why it matters:
  - `sync_runtime_shader_assignments_system()` scans layer definitions and sprite shader references every `Update`.
  - Render-layer registry sync still polls for authored changes every frame.
  - The “targeted” render-layer assignment path still iterates archetypes/entities for watched component changes in `collect_watched_component_dirty_entities()`.
  - This is better than a naive full rescan, but it is still broad enough to matter in authored-heavy scenes.
- Concrete recommendation:
  - Convert shader assignment resolution to a change-driven registry keyed by shader family/domain.
  - Precompute watched-entity membership once instead of walking all relevant archetypes each frame.
  - Separate “authored render state changed” from “nothing changed, skip immediately.”
- Expected payoff:
  - Lower client CPU cost in normal gameplay.
  - Cleaner render-layer hot path as authored content grows.
- Risk/complexity of fixing: Medium
- Disposition: Should fix

### F4. In-world rendering currently pays for too many active view lanes before the scene is cheap enough

- Severity: High
- Confidence: Proven
- Main impact: `client CPU`, `client GPU`, `frame pacing`
- Exact references:
  - `bins/sidereal-client/src/native/scene.rs:13`
  - `bins/sidereal-client/src/native/scene_world.rs:38`
  - `bins/sidereal-client/src/native/scene_world.rs:65`
  - `bins/sidereal-client/src/native/scene_world.rs:90`
  - `bins/sidereal-client/src/native/scene_world.rs:116`
  - `bins/sidereal-client/src/native/scene_world.rs:131`
  - `bins/sidereal-client/src/native/scene_world.rs:145`
- Why it matters:
  - In-world uses backdrop, gameplay, fullscreen foreground, post-process, and UI overlay cameras, with a debug overlay camera available on top.
  - Extra cameras are not automatically wrong, but they multiply extraction/pass overhead.
  - This cost is harder to justify while the project still has high CPU overhead elsewhere.
- Concrete recommendation:
  - Treat camera count and fullscreen/post-process pass count as budgeted runtime metrics.
  - Verify whether fullscreen foreground and post-process need separate always-on cameras or can be collapsed.
  - Keep the debug overlay camera disabled unless the overlay is actually enabled.
- Expected payoff:
  - Lower baseline render cost and clearer per-pass budgeting.
- Risk/complexity of fixing: Medium
- Disposition: Should fix

### F5. Custom material instance pressure is still high enough to hurt batching and submission efficiency

- Severity: High
- Confidence: Strong inference
- Main impact: `client CPU`, `client GPU`
- Exact references:
  - `bins/sidereal-client/src/native/backdrop.rs:502`
  - `bins/sidereal-client/src/native/visuals.rs:1357`
  - `bins/sidereal-client/src/native/visuals.rs:1676`
  - `bins/sidereal-client/src/native/visuals.rs:1892`
  - `bins/sidereal-client/src/native/visuals.rs:2139`
  - `bins/sidereal-client/src/native/visuals.rs:2192`
- Why it matters:
  - Shared quad mesh caching already exists, which is correct.
  - The remaining pressure comes from per-pass and per-entity `Material2d` instances for planets, thrusters, tracers, sparks, asteroid variants, and fullscreen/post-process bindings.
  - This is the real batching blocker now, not mesh duplication.
- Concrete recommendation:
  - Pool/reuse material handles where the uniform payload can be shared.
  - Budget planet multi-pass counts explicitly.
  - Prefer a smaller number of effect/material buckets over entity-unique handles when the effect does not truly need unique uniforms.
- Expected payoff:
  - Fewer draw-state changes and lower CPU submission pressure.
- Risk/complexity of fixing: Medium to High
- Disposition: Should fix

### F6. Off-screen and overlay-heavy UI paths still do a lot of per-frame work

- Severity: High
- Confidence: Proven
- Main impact: `client CPU`, `frame pacing`
- Exact references:
  - `bins/sidereal-client/src/native/ui.rs:356`
  - `bins/sidereal-client/src/native/ui.rs:949`
  - `bins/sidereal-client/src/native/ui.rs:1590`
  - `bins/sidereal-client/src/native/ui.rs:1696`
- Why it matters:
  - Tactical overlay rebuilds dynamic markers and smooths all contacts every frame.
  - Runtime screen overlay material state is updated every frame.
  - Nameplate positioning rebuilds a `HashMap` of world entity data each frame and then walks roots again.
  - This is render-adjacent work that directly competes with the world render budget.
- Concrete recommendation:
  - Gate nameplate updates by enable state and visible count budget.
  - Split tactical overlay into lower-frequency data sync plus per-frame interpolation only for visible contacts.
  - Add per-frame counters for contact marker count, nameplate count, and tactical overlay time.
- Expected payoff:
  - Better frame consistency in HUD-heavy scenes and tactical-map mode.
- Risk/complexity of fixing: Medium
- Disposition: Should fix

### F7. Fullscreen and planet culling choices are mostly correct, but they need budget guardrails

- Severity: Medium
- Confidence: Proven
- Main impact: `client GPU`, `client CPU`
- Exact references:
  - `bins/sidereal-client/src/native/backdrop.rs:59`
  - `bins/sidereal-client/src/native/backdrop.rs:580`
  - `bins/sidereal-client/src/native/visuals.rs:1357`
  - `bins/sidereal-client/src/native/visuals.rs:1676`
  - `bins/sidereal-client/src/native/visuals.rs:1760`
- Why it matters:
  - Fullscreen layers and planet passes deliberately use `NoFrustumCulling` where a world-frustum test is incorrect or insufficient.
  - Planet visuals then apply manual projected-view culling, which is directionally right.
  - The missing piece is not “turn culling back on,” but “track how many fullscreen and large-body passes are active.”
- Concrete recommendation:
  - Keep the current fullscreen/parallax approach.
  - Add counters for fullscreen pass count, planet pass count, and visible planet pass count.
- Expected payoff:
  - Better profiling clarity without breaking the render-layer contract.
- Risk/complexity of fixing: Low
- Disposition: Optional improvement

## 5. ECS / Schedule / Transform Findings

### F8. The project already made the right interpolation/camera decision, which means remaining slowness is around that lane, not the absence of that lane

- Severity: High
- Confidence: Proven
- Main impact: `frame pacing`
- Exact references:
  - `bins/sidereal-client/src/native/mod.rs:129`
  - `bins/sidereal-client/src/native/transforms.rs:111`
  - `bins/sidereal-client/src/native/transforms.rs:174`
  - `bins/sidereal-client/src/native/plugins.rs`
- Why it matters:
  - Lightyear frame interpolation is enabled.
  - The client seeds interpolation markers and recovers obviously stalled interpolated transforms.
  - Camera follow is intentionally scheduled after interpolation and visual correction.
  - This strongly suggests that “game feels slow” is currently more about cadence, handoff churn, or broad schedule cost than missing interpolation fundamentals.
- Concrete recommendation:
  - Preserve this schedule order.
  - Measure correction frequency, predicted/adopted entity delay, and stalled-transform recovery count before changing the camera/interpolation model again.
- Expected payoff:
  - Avoids regressing the correct part of the render-smoothness architecture.
- Risk/complexity of fixing: Low
- Disposition: Preserve; instrument further

### F9. Duplicate predicted/interpolated visual suppression is still a transitional runtime tax

- Severity: Medium
- Confidence: Proven
- Main impact: `frame pacing`, `architecture/maintainability`
- Exact references:
  - `bins/sidereal-client/src/native/visuals.rs:356`
  - `bins/sidereal-client/src/native/replication.rs:781`
  - `bins/sidereal-client/src/native/replication.rs:847`
  - `bins/sidereal-client/src/native/resources.rs:301`
- Why it matters:
  - The client still keeps duplicate-lifecycle machinery alive and hides the loser rather than preventing the duplicate from being a render concern in the first place.
  - The path is more incremental than a naive full rescan, but it remains transitional complexity in the hot path.
- Concrete recommendation:
  - Move duplicate winner selection closer to adoption/control-handoff transitions.
  - Keep one winner-per-GUID registry and surface winner-swap metrics in debug/profiling tools.
- Expected payoff:
  - Lower presentation churn and simpler render ownership.
- Risk/complexity of fixing: Medium
- Disposition: Should fix

### F10. Client-side runtime still mixes too many concerns inside a few monolithic files, which directly blocks optimization

- Severity: Medium
- Confidence: Proven
- Main impact: `architecture/maintainability`
- Exact references:
  - `bins/sidereal-client/src/native/visuals.rs` (2687 lines)
  - `bins/sidereal-client/src/native/backdrop.rs` (1930 lines)
  - `bins/sidereal-client/src/native/ui.rs` (1847 lines)
  - `bins/sidereal-replication/src/replication/visibility.rs` (1741 lines)
- Why it matters:
  - The repo explicitly says large runtime refactors should split mixed concerns into domain modules.
  - These files now mix lifecycle, rendering, visibility, effect simulation, UI, profiling, and migration logic.
  - That makes it much harder to isolate hot paths and reason about invariants.
- Concrete recommendation:
  - Split these files by domain hot path before attempting deeper optimization passes.
  - Keep entrypoints focused on wiring, and move per-feature logic behind narrower modules.
- Expected payoff:
  - Faster profiling/iteration and lower risk of optimization regressions.
- Risk/complexity of fixing: Medium
- Disposition: Should fix

## 6. Asset / Shader / Material Findings

### F11. Shader-family assignment is still inferred from hardcoded layer IDs and first-match heuristics

- Severity: High
- Confidence: Proven
- Main impact: `architecture/maintainability`, `startup hitching`
- Exact references:
  - `bins/sidereal-client/src/native/shaders.rs:640`
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
  - `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`
- Why it matters:
  - The runtime still special-cases names like `bg_starfield`, `bg_space_background_base`, and `bg_space_background_nebula`.
  - It also falls back to “first matching sprite shader” style inference for some families.
  - That is valid transitional code, but it is not the intended steady-state generic family registry.
- Concrete recommendation:
  - Move shader-slot assignment into explicit authored family/domain metadata from the authoritative catalog or layer definition.
  - Stop inferring family slots from special layer names.
- Expected payoff:
  - Cleaner runtime shader registry and fewer unnecessary reloads/inferences.
- Risk/complexity of fixing: Medium
- Disposition: Should fix

### F12. Shader reload boundaries are coarse, but this is secondary to the scheduling/cadence issues

- Severity: Medium
- Confidence: Proven plus inference
- Main impact: `startup hitching`
- Exact references:
  - `bins/sidereal-client/src/native/scene_world.rs:38`
  - `bins/sidereal-client/src/native/shaders.rs:569`
  - `bins/sidereal-client/src/native/shaders.rs:640`
- Why it matters:
  - Streamed shaders are reinstalled on scene entry and when assignment generation changes.
  - That can cause hitches around world entry or hot reload.
  - It is not the strongest steady-state bottleneck, but it will be visible during iteration and initial load.
- Concrete recommendation:
  - Add timing and generation counters first.
  - Defer deeper shader-pipeline redesign until server cadence and client polling costs are under control.
- Expected payoff:
  - Better startup profiling and less blind shader-pipeline churn.
- Risk/complexity of fixing: Low to Medium
- Disposition: Optional improvement

## 7. Lightyear / Replication / Visibility Findings

### F13. The codebase still shows presentation churn around control handoff and relevance changes

- Severity: High
- Confidence: Strong inference
- Main impact: `frame pacing`, `replication churn`
- Exact references:
  - `bins/sidereal-client/src/native/replication.rs:520`
  - `bins/sidereal-client/src/native/replication.rs:781`
  - `bins/sidereal-client/src/native/transforms.rs:174`
  - `bins/sidereal-client/src/native/bootstrap.rs:86`
- Why it matters:
  - Deferred predicted adoption, conflicting marker cleanup, initial-visual gating, and stalled-transform recovery all exist for good reasons, but together they indicate a lifecycle that can still get noisy under churn.
  - That kind of noise often feels like render slowness even when the GPU is not the limiting factor.
- Concrete recommendation:
  - Add counters for adoption wait time, conflicting marker sanitization, stalled-transform recoveries, and winner swaps.
  - Use those metrics to decide whether the next fix belongs in Lightyear lifecycle handling or presentation ownership.
- Expected payoff:
  - Better root-cause separation between networking churn and rendering cost.
- Risk/complexity of fixing: Medium
- Disposition: Should fix

### F14. The server-side visibility contract is directionally correct and should be preserved

- Severity: Low
- Confidence: Proven
- Main impact: `architecture/maintainability`
- Exact references:
  - `docs/features/visibility_replication_contract.md`
  - `bins/sidereal-replication/src/replication/visibility.rs:1467`
  - `bins/sidereal-replication/src/replication/visibility.rs:1524`
- Why it matters:
  - The code still respects the right conceptual order: authorize first, then narrow delivery.
  - The performance problem is implementation cost, not the contract itself.
- Concrete recommendation:
  - Keep the authorization/delivery separation.
  - Optimize indices and cadence, not policy correctness.
- Expected payoff:
  - Preserves security/runtime correctness while optimizing hot paths.
- Risk/complexity of fixing: Low
- Disposition: Preserve

## 8. Server-Side Contributors To Render Slowness

The main server-side contributors are:

1. Visibility scratch rebuild and per-client fanout in `bins/sidereal-replication/src/replication/visibility.rs:594`.
2. Per-player landmark discovery scanning inside the visibility tick in `bins/sidereal-replication/src/replication/visibility.rs:913`.
3. Visibility-source construction and candidate-cell/candidate-entity rebuilding each tick in `bins/sidereal-replication/src/replication/visibility.rs:382` and `bins/sidereal-replication/src/replication/visibility.rs:418`.
4. Asset hot-reload catalog polling in `bins/sidereal-replication/src/replication/assets.rs:45` and `bins/sidereal-replication/src/replication/assets.rs:92`, which is not a steady-state render bottleneck but can add noise during live authoring/testing.

## 9. Documentation / Architecture Divergence

### D1. `DR-0027` still describes cached client-only fullscreen copies, but current code and newer docs no longer do that

- Evidence:
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md` section 5.0 item 8 still describes client-only cached fullscreen renderable copies.
  - `docs/features/visibility_replication_contract.md` 2026-03-09 update says those client-local fullscreen copies were removed.
  - Current runtime matches the newer contract: fullscreen renderables are attached to the authored fullscreen entities in `bins/sidereal-client/src/native/backdrop.rs:59` and `bins/sidereal-client/src/native/backdrop.rs:580`.
- Recommendation:
  - Update `DR-0027` with a dated 2026-03-10 note so the active runtime path is not ambiguous.

### D2. Runtime shader assignment is still more hardcoded than `DR-0029` implies

- Evidence:
  - `bins/sidereal-client/src/native/shaders.rs:640` still infers families from special layer IDs and first-match query order.
- Recommendation:
  - Document this as an explicit transitional state in `DR-0029` or close the gap in code.

## 10. End-to-End Render Flow Map

### 10.1 Asset/bootstrap to client-ready rendering

1. Client runtime boots Bevy/Avian/Lightyear and registers material families in `bins/sidereal-client/src/native/mod.rs`.
2. Gateway bootstrap manifest is requested and processed in `bins/sidereal-client/src/native/auth_net.rs:308` and `bins/sidereal-client/src/native/auth_net.rs:552`.
3. World scene spawn reloads streamed shaders and creates the in-world camera/pass stack in `bins/sidereal-client/src/native/scene_world.rs:38`.
4. Runtime optional asset discovery watches authored layer/visual references in `bins/sidereal-client/src/native/assets.rs:160`.
5. Visual systems attach streamed sprites, planet passes, effects, overlays, and fullscreen layers in `bins/sidereal-client/src/native/plugins.rs` via `visuals`, `backdrop`, and `ui`.

### 10.2 Replicated entity arrival to visible draw

1. Lightyear/Avian replication clones arrive and are adopted in `bins/sidereal-client/src/native/replication.rs:520`.
2. Spatial/visibility/bootstrap components are ensured in `bins/sidereal-client/src/native/replication.rs:48`.
3. Frame interpolation markers and fallback transform seeding are maintained in `bins/sidereal-client/src/native/transforms.rs:111` and `bins/sidereal-client/src/native/transforms.rs:174`.
4. Render-layer assignment resolves in `bins/sidereal-client/src/native/render_layers.rs:195`.
5. Visual children/materials attach in `bins/sidereal-client/src/native/visuals.rs:763` and `bins/sidereal-client/src/native/visuals.rs:1357`.

### 10.3 Camera-relative/world-layer transform derivation

1. Authoritative world positions remain in physics/world-space components.
2. Camera follow resolves after interpolation/correction.
3. Layer parallax and screen-scale derive render-local offsets in `bins/sidereal-client/src/native/visuals.rs:1822` and related helpers.

### 10.4 Fullscreen background/foreground/post-process execution

1. Fullscreen authored ECS entities are synchronized into renderable fullscreen state in `bins/sidereal-client/src/native/backdrop.rs:59`.
2. Authored post-process stacks spawn/maintain client renderable passes in `bins/sidereal-client/src/native/backdrop.rs:317`.
3. Backdrop/fullscreen transforms are normalized to the current viewport in `bins/sidereal-client/src/native/backdrop.rs:692` and `bins/sidereal-client/src/native/backdrop.rs:723`.

### 10.5 Prediction/reconciliation/interpolation to final presented motion

1. Fixed-step simulation and client prediction run at 60 Hz.
2. Lightyear frame interpolation is active.
3. Sidereal seeds/fixes transform interpolation where clone lifecycles arrive incomplete.
4. Camera follow runs after interpolation/correction, then layered visuals update from the final same-frame camera state.

## 11. Performance Budget Map

Inference labels are explicit below.

### 11.1 Client CPU

Most likely hot paths:

1. Proven: UI tactical/nameplate loops.
2. Proven: Shader assignment inference plus render-layer maintenance.
3. Proven: Duplicate lifecycle and transform bootstrap bookkeeping.
4. Strong inference: High custom-material counts increasing draw submission overhead.

### 11.2 Client GPU

Most likely hot paths:

1. Strong inference: Multiple active camera/pass lanes.
2. Strong inference: Fullscreen and large alpha-blended effects.
3. Strong inference: Planet multi-pass materials and effect materials defeating batching.

### 11.3 Client main-thread stalls

Most likely hot paths:

1. Proven: Serialized asset bootstrap and lazy fetch.
2. Proven: Coarse shader reinstall boundaries.
3. Strong inference: Effect/planet material churn during entity creation and content changes.

### 11.4 Server tick cost that affects visual smoothness

Most likely hot paths:

1. Proven: Visibility scratch rebuild.
2. Proven: Candidate-set rebuilds.
3. Proven: Landmark discovery rescans.
4. Proven: Per-entity per-client replication visibility mutation.

### 11.5 Network/replication delivery cost that affects render churn

Most likely hot paths:

1. Strong inference: Uneven visibility tick duration creates uneven delivery cadence.
2. Proven: Controlled-entity adoption can be delayed on missing replicated motion components.
3. Strong inference: Duplicate relevance/prediction handoff keeps presentation churn alive longer than ideal.

## 12. Prioritized Remediation Plan

### 12.1 Top 5 highest-ROI changes

1. Incrementalize server visibility scratch/index work and remove per-tick landmark rescans.
2. Parallelize required asset bootstrap downloads and allow bounded parallel runtime lazy fetches.
3. Replace per-frame shader assignment inference with explicit family/domain registration.
4. Reduce UI overlay/nameplate/tactical per-frame work and measure it separately.
5. Reduce custom material instance pressure for planet/effect/fullscreen families.

### 12.2 Quick wins

1. Add timings and counters before changing architecture again.
2. Disable debug overlay camera unless the overlay is enabled.
3. Add pass-count, camera-count, and material-count diagnostics to the debug overlay/BRP.
4. Move landmark discovery off the main visibility tick.

### 12.3 Medium-size refactors

1. Dedicated shader-family assignment registry.
2. Bounded async asset fetch pool with prioritization.
3. Winner-per-GUID presentation registry near replication adoption.
4. Split `visuals.rs`, `backdrop.rs`, `ui.rs`, and replication `visibility.rs` by domain.

### 12.4 Large architectural changes

Only after measurement:

1. Collapse or redesign some custom material families if batching data proves it matters more than schedule work.
2. Revisit pass/camera composition if camera/pass metrics show it is a top-3 runtime cost after the CPU/server work is reduced.

### 12.5 Dependencies / order of operations

1. Instrument first.
2. Fix server visibility cadence.
3. Fix asset serialization bottlenecks.
4. Remove client hot-path polling where possible.
5. Re-measure.
6. Only then decide how much material-family redesign is justified.

### 12.6 What to measure before and after each major fix

1. Frame time average, p95, and p99.
2. Visibility tick duration and per-stage breakdown.
3. Candidate count, visible entity count, and adoption delay count.
4. Camera count, fullscreen/post-process pass count, draw count, and active custom material count.
5. Bootstrap time to first render and time to fully ready assets.

## 13. Instrumentation / Profiling Gaps

Missing telemetry that should be added:

1. Per-system timers for:
  - `update_network_visibility`
  - `submit_asset_bootstrap_request` result processing
  - `queue_missing_catalog_assets_system`
  - `sync_runtime_shader_assignments_system`
  - `sync_runtime_render_layer_registry_system`
  - `resolve_runtime_render_layer_assignments_system`
  - `update_tactical_map_overlay_system`
  - `update_entity_nameplate_positions_system`
2. Per-frame counts for:
  - active cameras
  - fullscreen passes
  - post-process passes
  - streamed visual children
  - planet passes
  - active tracer/spark/effect entities
3. Material counters for:
  - `PlanetVisualMaterial`
  - `RuntimeEffectMaterial`
  - `AsteroidSpriteShaderMaterial`
  - fullscreen material bindings
4. Asset/shader telemetry for:
  - bootstrap queue size
  - runtime fetch queue size
  - in-flight fetch count
  - shader reload generation/time
5. Replication/visibility telemetry for:
  - per-client candidate counts
  - per-client visible counts
  - discovery scan time
  - visibility scratch rebuild time
  - adoption wait duration
  - duplicate winner swaps

## 14. Runtime Catalog Appendix

### 14.1 Client runtime plugins/systems/resources

- `ClientVisualsPlugin` in `bins/sidereal-client/src/native/plugins.rs`: active runtime
- `ClientLightingPlugin` in `bins/sidereal-client/src/native/plugins.rs`: active runtime
- `ClientUiPlugin` in `bins/sidereal-client/src/native/plugins.rs`: active runtime
- `RuntimeShaderAssignments` in `bins/sidereal-client/src/native/shaders.rs`: active runtime, transitional implementation
- `DuplicateVisualResolutionState` in `bins/sidereal-client/src/native/resources.rs`: transitional/migration code
- `DebugOverlayState`, `DebugOverlaySnapshot` in `bins/sidereal-client/src/native/resources.rs`: debug/diagnostic
- `UiOverlayCamera`, backdrop/gameplay/post-process camera stack in `bins/sidereal-client/src/native/scene.rs` and `bins/sidereal-client/src/native/scene_world.rs`: active runtime

### 14.2 Replication server plugins/systems/resources

- `ReplicationVisibilityPlugin` in `bins/sidereal-replication/src/plugins.rs`: active runtime
- `VisibilityScratch` and `VisibilityRuntimeConfig` in `bins/sidereal-replication/src/replication/visibility.rs`: active runtime
- `ClientObserverAnchorPositionMap` in `bins/sidereal-replication/src/replication/visibility.rs`: active runtime
- `AssetHotReloadState` in `bins/sidereal-replication/src/replication/assets.rs`: active runtime, tooling-heavy during authoring
- `PlayerControlDebugState` in `bins/sidereal-replication/src/replication/runtime_state.rs`: debug/diagnostic

### 14.3 Gateway/bootstrap/asset-delivery pieces

- Asset bootstrap manifest fetch path in `bins/sidereal-client/src/native/auth_net.rs`: active runtime
- Runtime asset cache/index in `bins/sidereal-client/src/native/assets.rs` and `crates/sidereal-asset-runtime`: active runtime
- Gateway asset manifest/payload routes and runtime catalog build path: active runtime
- Catalog hot reload invalidation path in replication `assets.rs`: active runtime

### 14.4 Shared gameplay/render-support crates and modules

- `sidereal-game` render-layer/world-visual/fullscreen/post-process components: active runtime
- `sidereal-runtime-sync` hierarchy/runtime-entity mapping: active runtime
- `sidereal-net` Lightyear protocol/messages: active runtime
- Hardcoded layer-name shader slot inference in client shader sync: transitional/migration code
- Duplicate visual suppression path: likely removable after lifecycle stabilization

## 15. Confirm / Refute Summary

1. The game is GPU-bound in normal gameplay: Not proven; unlikely to be the primary current bottleneck.
2. The game is CPU-bound on the client in normal gameplay: Likely true.
3. ECS scheduling/query work matters more than raw draw submission right now: Likely true.
4. Replication/update churn matters more than rendering itself: Often true under load; it is a top-tier contributor.
5. The game feels slow mainly because of frame pacing/interpolation-adjacent issues rather than raw FPS: Likely true.
6. Shader/material diversity is defeating batching enough to matter: Likely true for planet/effect/fullscreen custom material paths.
7. Too many fullscreen or post-process passes are active for the current payoff: Likely true.
8. Off-screen or non-visible entities still pay too much render-related cost: Yes, mostly via schedule/UI/material/update work rather than pure draw cost.
9. Asset/shader compilation or loading hitching is meaningful: Yes, but secondary to server cadence and client polling.
10. Server-side visibility/replication behavior is causing client render instability or overload: Likely true.
11. The render-layer architecture is directionally correct and should be kept: Yes.
12. The current render-layer/material implementation has avoidable transitional cost that should be simplified: Yes.

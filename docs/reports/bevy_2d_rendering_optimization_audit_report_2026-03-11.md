# Bevy 2D Rendering Optimization Audit Report

Date: 2026-03-11
Scope: Client-visible rendering and smoothness
Prompt source: `docs/prompts/bevy_2d_rendering_optimization_audit_prompt.md`
Limitations: Static code audit only. No live GPU captures, frame-time traces, packet captures, or server profiler output were available. Any statement about GPU saturation is inference unless directly supported by code structure.

Update note (2026-03-11):
- This re-audit supersedes the March 10 rendering report.
- Two earlier severe findings are no longer active: bootstrap asset fetches and runtime lazy asset fetches are now bounded-parallel (`4` concurrent lanes each), not single-file serialized.
- The dominant remaining problem is mixed frame pacing and CPU cadence, especially replication visibility cadence upstream of the renderer.

## 1. Executive Summary

The game still does not read like a client that is primarily limited by simple 2D draw submission. It reads like a mixed frame-pacing problem where server tick cost, replication churn, client ECS polling, duplicate-presentation maintenance, and a fairly expensive multi-camera/material path combine to make the game feel slow.

The most important current conclusions:

1. The strongest bottleneck is still upstream of the renderer: replication visibility rebuild/apply work in [`bins/sidereal-replication/src/replication/visibility.rs:667`](bins/sidereal-replication/src/replication/visibility.rs:667).
2. The client still pays a large amount of render-adjacent CPU work each frame in [`bins/sidereal-client/src/native/plugins.rs:260`](bins/sidereal-client/src/native/plugins.rs:260), [`bins/sidereal-client/src/native/render_layers.rs:20`](bins/sidereal-client/src/native/render_layers.rs:20), [`bins/sidereal-client/src/native/shaders.rs:640`](bins/sidereal-client/src/native/shaders.rs:640), and [`bins/sidereal-client/src/native/ui.rs:1696`](bins/sidereal-client/src/native/ui.rs:1696).
3. The renderer is likely not primarily GPU-bound in normal gameplay. The code suggests the main pain is unstable cadence and CPU work around the render path, not raw sprite fill cost.
4. The current render-layer direction is still correct and should be kept. The problem is the amount of transitional and content-specific Rust logic still wrapped around it.

## 2. Critical Findings

### F1: Replication visibility cadence is still the largest indirect render-smoothness problem
- Severity: Critical
- Confidence: Proven
- Type: architecture, performance
- Priority: must fix
- Why it matters:
  The renderer can only look smooth if authoritative updates arrive steadily. The replication server still clears and rebuilds `VisibilityScratch`, recomputes candidate sets, reruns landmark discovery, and then applies visibility decisions across replicated entities and clients every fixed tick. That makes the client look slow even when GPU cost is moderate.
- Exact references:
  - [`bins/sidereal-replication/src/plugins.rs:88`](bins/sidereal-replication/src/plugins.rs:88)
  - [`bins/sidereal-replication/src/replication/visibility.rs:667`](bins/sidereal-replication/src/replication/visibility.rs:667)
  - [`bins/sidereal-replication/src/replication/visibility.rs:815`](bins/sidereal-replication/src/replication/visibility.rs:815)
  - [`bins/sidereal-replication/src/replication/visibility.rs:913`](bins/sidereal-replication/src/replication/visibility.rs:913)
  - [`bins/sidereal-replication/src/replication/visibility.rs:1180`](bins/sidereal-replication/src/replication/visibility.rs:1180)
- Concrete recommendation:
  Make visibility incremental. Persist indices across ticks, narrow discovery cadence, and stop applying visibility as a broad full-runtime pass every fixed tick.

## 3. High Findings

### F2: The client still has a very large always-on render-adjacent schedule
- Severity: High
- Confidence: Proven
- Type: performance, maintainability
- Priority: should fix
- Why it matters:
  In-world `Update` and `PostUpdate` still run a long chain of systems for replicated adoption, asset dependency maintenance, render-layer registry sync, layer assignment, duplicate suppression, visual attachment, fullscreen sync, lighting, HUD, tactical overlays, camera sync, and visual transform updates. Even if each piece is individually reasonable, the aggregate cost competes directly with smooth presentation.
- Exact references:
  - [`bins/sidereal-client/src/native/plugins.rs:109`](bins/sidereal-client/src/native/plugins.rs:109)
  - [`bins/sidereal-client/src/native/plugins.rs:139`](bins/sidereal-client/src/native/plugins.rs:139)
  - [`bins/sidereal-client/src/native/plugins.rs:260`](bins/sidereal-client/src/native/plugins.rs:260)
  - [`bins/sidereal-client/src/native/plugins.rs:359`](bins/sidereal-client/src/native/plugins.rs:359)
  - [`bins/sidereal-client/src/native/plugins.rs:430`](bins/sidereal-client/src/native/plugins.rs:430)
- Concrete recommendation:
  Budget the hot path explicitly. Move more of this work behind dirty checks, state changes, or lower-frequency lanes. The client schedule is now big enough that optimization should start with schedule shape, not micro-tuning individual systems.

### F3: Duplicate predicted/interpolated presentation handling is still a frame-pacing tax
- Severity: High
- Confidence: Proven
- Type: performance, maintainability
- Priority: should fix
- Why it matters:
  The client still keeps duplicate winner-selection state and a suppression pass alive in the hot path. That is a direct signal that the presentation lifecycle is still compensating for replicated adoption/handoff complexity instead of receiving one clearly renderable winner.
- Exact references:
  - [`bins/sidereal-client/src/native/visuals.rs:374`](bins/sidereal-client/src/native/visuals.rs:374)
  - [`bins/sidereal-client/src/native/visuals.rs:591`](bins/sidereal-client/src/native/visuals.rs:591)
  - [`bins/sidereal-client/src/native/replication.rs:471`](bins/sidereal-client/src/native/replication.rs:471)
  - [`bins/sidereal-client/src/native/replication.rs:797`](bins/sidereal-client/src/native/replication.rs:797)
  - [`bins/sidereal-client/src/native/transforms.rs:174`](bins/sidereal-client/src/native/transforms.rs:174)
- Concrete recommendation:
  Resolve winner selection closer to adoption/control-handoff and make duplicate suppression the rare fallback, not a standing visual maintenance system.

### F4: Camera/pass count is still heavy for the current optimization state
- Severity: High
- Confidence: Proven
- Type: performance
- Priority: should fix
- Why it matters:
  The in-world scene still uses distinct backdrop, planet, gameplay, debug overlay, fullscreen foreground, post-process, and UI overlay camera lanes. This can be justified in a mature pipeline, but it is expensive baseline render/extraction overhead while other CPU-heavy systems are still broad and unstable.
- Exact references:
  - [`bins/sidereal-client/src/native/scene.rs:16`](bins/sidereal-client/src/native/scene.rs:16)
  - [`bins/sidereal-client/src/native/scene_world.rs:38`](bins/sidereal-client/src/native/scene_world.rs:38)
  - [`bins/sidereal-client/src/native/scene_world.rs:61`](bins/sidereal-client/src/native/scene_world.rs:61)
  - [`bins/sidereal-client/src/native/scene_world.rs:86`](bins/sidereal-client/src/native/scene_world.rs:86)
  - [`bins/sidereal-client/src/native/scene_world.rs:101`](bins/sidereal-client/src/native/scene_world.rs:101)
  - [`bins/sidereal-client/src/native/scene_world.rs:127`](bins/sidereal-client/src/native/scene_world.rs:127)
  - [`bins/sidereal-client/src/native/scene_world.rs:142`](bins/sidereal-client/src/native/scene_world.rs:142)
  - [`bins/sidereal-client/src/native/scene_world.rs:156`](bins/sidereal-client/src/native/scene_world.rs:156)
- Concrete recommendation:
  Count passes and cameras as a budgeted resource. Re-evaluate whether fullscreen foreground and post-process must remain separate always-on camera lanes right now. The debug overlay camera should not stay active by default.

### F5: Material diversity still looks high enough to matter
- Severity: High
- Confidence: Strong inference
- Type: performance
- Priority: should fix
- Why it matters:
  Shared quad mesh caching exists, so the old “reallocate fullscreen mesh every frame” concern is gone. The remaining render-path cost is material diversity: fullscreen materials, planet multi-pass materials, asteroid materials, thruster/effect materials, projectile/tracer/spark effect materials, and per-path custom `Material2d` usage. That reduces batching and increases render-world churn.
- Exact references:
  - [`bins/sidereal-client/src/native/backdrop.rs:624`](bins/sidereal-client/src/native/backdrop.rs:624)
  - [`bins/sidereal-client/src/native/visuals.rs:781`](bins/sidereal-client/src/native/visuals.rs:781)
  - [`bins/sidereal-client/src/native/visuals.rs:1317`](bins/sidereal-client/src/native/visuals.rs:1317)
  - [`bins/sidereal-client/src/native/visuals.rs:1851`](bins/sidereal-client/src/native/visuals.rs:1851)
  - [`bins/sidereal-client/src/native/visuals.rs:2188`](bins/sidereal-client/src/native/visuals.rs:2188)
  - [`bins/sidereal-client/src/native/debug_overlay.rs:125`](bins/sidereal-client/src/native/debug_overlay.rs:125)
- Concrete recommendation:
  Reduce entity-unique material instances where shared buckets are sufficient. Planet and effect paths need an explicit material-instance budget.

## 4. Medium Findings

### F6: Render-layer and shader assignment maintenance are still broad polling systems
- Severity: Medium
- Confidence: Proven
- Type: performance, maintainability
- Priority: should fix
- Why it matters:
  The render-layer registry and assignment code is more incremental than before, but it still runs active authored-state checks every frame. Shader assignment inference also still scans runtime render layers and shader-bearing entities every frame.
- Exact references:
  - [`bins/sidereal-client/src/native/render_layers.rs:20`](bins/sidereal-client/src/native/render_layers.rs:20)
  - [`bins/sidereal-client/src/native/render_layers.rs:195`](bins/sidereal-client/src/native/render_layers.rs:195)
  - [`bins/sidereal-client/src/native/shaders.rs:640`](bins/sidereal-client/src/native/shaders.rs:640)
- Concrete recommendation:
  Move further toward change-driven authored render-state maintenance. The current code is acceptable migration logic, but not a final cheap hot path.

### F7: UI and tactical overlay work still consume too much per-frame budget
- Severity: Medium
- Confidence: Proven
- Type: performance
- Priority: should fix
- Why it matters:
  Tactical overlays, nameplate synchronization, owned-entity panel updates, and runtime screen overlay pass updates all stay active in the in-world `Update` path. This is render-adjacent CPU work that can easily dominate a 2D frame even when the GPU is fine.
- Exact references:
  - [`bins/sidereal-client/src/native/plugins.rs:359`](bins/sidereal-client/src/native/plugins.rs:359)
  - [`bins/sidereal-client/src/native/ui.rs:1696`](bins/sidereal-client/src/native/ui.rs:1696)
  - [`bins/sidereal-client/src/native/ui.rs:1811`](bins/sidereal-client/src/native/ui.rs:1811)
- Concrete recommendation:
  Lower tactical/nameplate update frequency, separate data refresh from presentation interpolation, and add counters for visible overlay elements.

### F8: Runtime shader reload on streamed asset arrival can still hitch
- Severity: Medium
- Confidence: Proven
- Type: performance
- Priority: should fix
- Why it matters:
  When a streamed WGSL asset arrives, the client can immediately reinstall runtime shader assets. That is directionally correct for hot-reload/content iteration, but it is also a hitch vector in normal runtime if shader fetches happen during active play.
- Exact references:
  - [`bins/sidereal-client/src/native/assets.rs:549`](bins/sidereal-client/src/native/assets.rs:549)
  - [`bins/sidereal-client/src/native/shaders.rs:538`](bins/sidereal-client/src/native/shaders.rs:538)
  - [`bins/sidereal-client/src/native/shaders.rs:560`](bins/sidereal-client/src/native/shaders.rs:560)
- Concrete recommendation:
  Separate dev hot-reload behavior from normal runtime behavior, or at least batch shader reloads so one asset arrival does not immediately force a visible pipeline churn.

### F9: Off-screen and non-visible world entities still participate in too much maintenance work
- Severity: Medium
- Confidence: Strong inference
- Type: performance
- Priority: should fix
- Why it matters:
  Even where final draw visibility is controlled, several maintenance systems still query broad sets of world entities or child visuals. That means the renderer is paying CPU-side upkeep for entities that are not materially contributing to the frame.
- Exact references:
  - [`bins/sidereal-client/src/native/visuals.rs:1245`](bins/sidereal-client/src/native/visuals.rs:1245)
  - [`bins/sidereal-client/src/native/visuals.rs:1636`](bins/sidereal-client/src/native/visuals.rs:1636)
  - [`bins/sidereal-client/src/native/visuals.rs:1910`](bins/sidereal-client/src/native/visuals.rs:1910)
  - [`bins/sidereal-client/src/native/visuals.rs:2472`](bins/sidereal-client/src/native/visuals.rs:2472)
- Concrete recommendation:
  Narrow maintenance queries by visibility state, camera relevance, or active-pass membership where possible.

## 5. Confirm / Refute

### Specific statements

1. The game is GPU-bound in normal gameplay.
   - Refute, based on code structure. No direct evidence. The stronger evidence points to server cadence plus client CPU-side maintenance.
2. The game is CPU-bound on the client in normal gameplay.
   - Likely yes, but mixed with server-induced cadence instability.
3. The game is bottlenecked by ECS scheduling/query work more than actual draw submission.
   - Likely yes.
4. The game is bottlenecked by replication/update churn more than rendering itself.
   - Yes. Strongest current evidence.
5. The game feels slow mainly because of frame pacing/interpolation issues rather than raw frame time.
   - Yes, with the caveat that the root is not missing interpolation fundamentals; it is cadence and handoff churn around that lane.
6. Shader/material diversity is defeating batching enough to matter.
   - Likely yes, though probably secondary to visibility cadence and broad CPU work.
7. Too many fullscreen or post-process passes are active for the current visual payoff.
   - Likely yes.
8. Off-screen or non-visible entities are still paying too much render-related cost.
   - Likely yes.
9. Asset/shader compilation or loading hitching is a meaningful stall source.
   - Yes.
10. Server-side visibility/replication behavior is causing client render instability or overload.
   - Yes.
11. The current render-layer architecture is directionally correct and should be kept.
   - Yes.
12. The current render-layer/material implementation has avoidable transitional cost that should be simplified.
   - Yes.

## 6. End-to-End Render Flow Map

### 6.1 Asset/bootstrap to client-ready rendering

1. Client authenticates and requests `Enter World`.
2. Gateway returns replication transport info plus bootstrap asset manifest endpoints.
3. Client bootstrap fetches required assets through authenticated HTTP, validates checksums, writes cache/index state, then transitions to world loading.
4. World scene spawn installs cameras and reloads runtime shader handles from cached/authored shader assignments.
5. Runtime asset dependency and lazy fetch systems continue filling optional assets while replicated entities begin rendering.

### 6.2 Replicated entity arrival to visible draw

1. Lightyear delivers replicated entities/clones.
2. Client adoption logic marks entities as `WorldEntity`, inserts runtime visual asset ids and prediction/interpolation markers, and hides visuals until initial transform readiness.
3. Render-layer registry/assignment systems derive each entity's resolved world layer.
4. Visual attachment systems spawn sprite/mesh/material children for streamed visuals, planets, projectiles, and effects.
5. Post-interpolation camera and transform sync systems place the visuals for the current frame before transform propagation.

### 6.3 Camera-relative/world-layer transform derivation

1. Gameplay camera follow happens in `PostUpdate` after Lightyear interpolation and visual correction.
2. `CameraMotionState` captures the camera-relative world offset/parallax position.
3. Streamed sprite and planet visual systems derive per-layer parallax offsets and z bias from resolved render-layer definitions.
4. Fullscreen layers remain non-spatial overlays and use dedicated render layers/cameras.

### 6.4 Fullscreen background/foreground/post-process execution

1. World scene spawns dedicated backdrop, fullscreen foreground, and post-process cameras.
2. Backdrop systems resolve fullscreen layer selection and post-process renderables from authored runtime data plus remaining legacy compatibility paths.
3. Fullscreen entities are material-bound and rendered through dedicated render layers and cameras.

### 6.5 Prediction/reconciliation/interpolation to final presented motion

1. Lightyear/Avian own main predicted and interpolated motion lanes.
2. Client adds/maintains `FrameInterpolate<Transform>` markers for eligible world entities.
3. Fallback transform seeding/recovery exists for late or stalled interpolated visuals.
4. Camera follow intentionally runs after interpolation/correction so the camera samples the frame that will actually be presented.

## 7. Performance Budget Map

These are inferential budget groupings from code structure, not measured timings.

### 7.1 Client CPU

- Very likely high:
  - replicated adoption and duplicate suppression
  - render-layer registry/assignment maintenance
  - fullscreen/world visual maintenance
  - tactical/nameplate/HUD update work
  - camera/pass synchronization

### 7.2 Client GPU

- Likely moderate but non-trivial:
  - multiple active cameras/passes
  - fullscreen background/foreground/post-process draws
  - planet multi-pass materials
  - alpha/effect materials and overlay passes

### 7.3 Client main-thread stalls

- Likely meaningful:
  - shader reload on streamed WGSL arrival
  - cache/index writes on asset fetch completion
  - broad ECS polling with `block_on(poll_once(...))` completion checks

### 7.4 Server tick cost that affects visual smoothness

- Very likely dominant:
  - visibility scratch rebuild
  - candidate/discovery generation
  - entity/client visibility apply loop

### 7.5 Network/replication delivery cost that affects render churn

- Very likely meaningful:
  - visibility-driven gain/loss churn
  - duplicated predicted/interpolated presentation maintenance
  - unstable or bursty authoritative cadence leading to correction-heavy presentation

## 8. What To Fix First

1. Rework replication visibility/update cadence first. This is the most likely largest smoothness gain.
2. Then reduce client hot-path schedule breadth around adoption, duplicate suppression, render-layer polling, and overlay maintenance.
3. Then collapse unnecessary always-on camera/pass cost and reduce material-instance diversity.

## 9. Architecture Constraints That Should Be Kept

1. Keep post-interpolation/post-correction camera follow.
2. Keep render-derived parallax and layer depth; do not feed visual transforms back into simulation.
3. Keep HTTP asset payload delivery separate from replication transport.
4. Keep Lua-authored render layers and shader-family direction; simplify the Rust glue around it instead of abandoning it.

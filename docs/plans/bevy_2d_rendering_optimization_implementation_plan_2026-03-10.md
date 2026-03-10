# Bevy 2D Rendering Optimization Implementation Plan

Status: Proposed implementation plan  
Date: 2026-03-10  
Owners: client rendering + replication + asset delivery + diagnostics

Primary input:
- `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-10.md`

Update note (2026-03-10):
- This plan was revised after the refreshed March 10 audit superseded the earlier same-day draft.
- The previous plan over-prioritized fullscreen/post-process churn based on an incorrect earlier finding.
- Shared fullscreen quad caching already exists in `bins/sidereal-client/src/native/backdrop.rs:643`.
- The highest-priority work is now:
  1. instrumentation,
  2. replication visibility cadence,
  3. serialized asset delivery,
  4. shader assignment and render-layer polling cleanup,
  5. UI/tactical/nameplate frame cost,
  6. material-instance pressure only after re-measurement.

Guardrails:
- Preserve `DR-0027` and the Lua-authored render-layer/runtime shader direction.
- Preserve Lightyear transform interpolation and the current post-correction camera follow ordering.
- Do not introduce native-only architecture that blocks later WASM parity recovery.
- Native impact: primary optimization target.
- WASM impact: keep change-driven invalidation, render-layer resolution, and asset dependency tracking in shared client code where possible; no platform-specific render architecture fork is planned in this pass.

## 1. Objective

Reduce perceived slowness and actual frame-time cost by fixing the hot paths that the refreshed March 10 audit identified as highest ROI:

1. replication visibility cadence pressure,
2. serialized asset bootstrap and runtime lazy fetch,
3. frame-polled shader assignment and render-layer maintenance,
4. tactical/nameplate/overlay frame cost,
5. duplicate presentation arbitration,
6. material-instance pressure in shader-backed visual paths.

Success criteria:

1. visibility tick duration, candidate counts, and per-client visible counts are measurable and reduced under the same load,
2. required asset bootstrap and runtime lazy fetch no longer run strictly one-at-a-time,
3. unchanged authored shader/layer state causes zero unnecessary shader-assignment rebuild work per frame,
4. unchanged authored render-layer state causes zero unnecessary registry recompiles per frame,
5. tactical overlay and nameplate frame cost are measurable and reduced,
6. duplicate visual suppression no longer depends on steady-state whole-world arbitration,
7. only after the above, material-instance and pass-count reductions are measured and targeted.

## 2. Priority Order

This is the required implementation order unless measurement proves otherwise:

1. Phase 0: Instrumentation and baseline
2. Phase 1: Replication visibility cadence and scratch-cost reduction
3. Phase 2: Parallelize and prioritize asset delivery
4. Phase 3: Remove frame-polled shader assignment and render-layer maintenance
5. Phase 4: Reduce UI/tactical/nameplate frame cost
6. Phase 5: Reduce per-frame presentation arbitration and debug overhead
7. Re-measure
8. Phase 6: Material-instance/pass-count reduction only if still justified

Do not start with fullscreen/post-process churn work as the lead optimization phase. The refreshed audit does not support that as the top current bottleneck.

## 3. Phase Details

### Phase 0: Instrumentation and Baseline

Goal:  
Create the counters and timers needed to prove each later phase helped.

Work:

1. Add lightweight timing resources or diagnostics around:
   - `update_network_visibility`
   - asset bootstrap result processing in `auth_net`
   - `queue_missing_catalog_assets_system`
   - `sync_runtime_shader_assignments_system`
   - `sync_runtime_render_layer_registry_system`
   - `resolve_runtime_render_layer_assignments_system`
   - `update_tactical_map_overlay_system`
   - `update_entity_nameplate_positions_system`
2. Add counters for:
   - visibility tick duration,
   - candidate counts per client,
   - visible counts per client,
   - landmark discovery scan duration,
   - bootstrap queue size / bootstrap total time,
   - runtime lazy fetch queue size / in-flight fetch count,
   - shader reload generation and reload duration,
   - render-layer registry rebuild count,
   - render-layer assignment update count,
   - duplicate GUID group count,
   - duplicate winner swap count,
   - active cameras,
   - fullscreen pass count,
   - post-process pass count,
   - streamed visual child count,
   - planet pass count,
   - material counts for `PlanetVisualMaterial`, `RuntimeEffectMaterial`, `AsteroidSpriteShaderMaterial`, and fullscreen bindings.
3. Expose these metrics through a debug resource and, if cheap enough, the existing debug overlay or BRP path.

Files expected:

1. `bins/sidereal-client/src/native/resources.rs`
2. `bins/sidereal-client/src/native/plugins.rs`
3. `bins/sidereal-client/src/native/shaders.rs`
4. `bins/sidereal-client/src/native/render_layers.rs`
5. `bins/sidereal-client/src/native/assets.rs`
6. `bins/sidereal-client/src/native/ui.rs`
7. `bins/sidereal-client/src/native/visuals.rs`
8. `bins/sidereal-client/src/native/backdrop.rs`
9. `bins/sidereal-replication/src/replication/visibility.rs`

Acceptance criteria:

1. Before/after metrics exist for every later phase.
2. Instrumentation overhead is bounded and can be gated if needed.
3. The next agent can answer “what is expensive right now?” without another blind audit.

### Phase 1: Optimize Replication Visibility Cadence

Goal:  
Reduce server-side visibility work that destabilizes client render smoothness.

Why this is first:

1. The refreshed audit treats this as the strongest current end-to-end bottleneck.
2. Even a perfectly optimized client render path will still feel bad if authoritative delivery cadence is bursty.

Work:

1. Persist scratch indices across ticks where safe instead of clearing and rebuilding all maps and sets every tick.
2. Move static-landmark discovery onto a lower-frequency or change-driven path separate from per-tick delivery updates.
3. Cache resolved world-layer lookup inputs used by discovered-landmark delivery adjustments.
4. Split the current monolithic `update_network_visibility()` cost into measurable sub-stages:
   - scratch/index upkeep,
   - candidate generation,
   - landmark discovery,
   - per-client apply.
5. Use Phase 0 metrics to identify which stage dominates before attempting deeper structural changes.

Files expected:

1. `bins/sidereal-replication/src/replication/visibility.rs`
2. `bins/sidereal-replication/src/plugins.rs`
3. Possibly shared visibility-related resources/components if cache generations are needed

Tests:

1. Visibility authorization/delivery semantics remain unchanged.
2. Discovered landmark behavior remains correct.
3. Owner/public/faction/global render-config bypass rules remain correct.

Docs:

1. Update `docs/features/visibility_replication_contract.md` only if behavior changes.
2. If only caching and scheduling change, keep the contract unchanged and note no policy change in code comments/tests.

Acceptance criteria:

1. Visibility tick duration drops under comparable entity/client load.
2. Candidate counts and delivery results remain correct.
3. No regression in authorization or delivery narrowing semantics.

### Phase 2: Parallelize and Prioritize Asset Delivery

Goal:  
Reduce time-to-visually-ready and runtime pop-in by removing strictly serialized asset delivery.

Why this is second:

1. The refreshed audit treats serialized bootstrap and lazy fetch as a critical bottleneck.
2. This directly affects perceived slowness even when rendering itself is nominal.

Work:

1. Replace single-task bootstrap downloading with bounded parallel downloads for required assets.
2. Replace the single in-flight runtime lazy fetch model with a bounded queue of concurrent fetches.
3. Add priority rules so shader assets, root visual assets, and immediate render dependencies beat lower-value optional assets.
4. Keep dependency correctness:
   - dependency closure remains authoritative,
   - dependents do not bind before prerequisites are ready.

Files expected:

1. `bins/sidereal-client/src/native/auth_net.rs`
2. `bins/sidereal-client/src/native/assets.rs`
3. `bins/sidereal-client/src/native/resources.rs`
4. `bins/sidereal-client/src/native/plugins.rs`

Tests:

1. Required asset bootstrap still fails closed when required assets are unavailable or corrupt.
2. Runtime lazy fetch still respects dependency closure.
3. Parallel fetch does not duplicate the same asset request unnecessarily.
4. Priority handling remains deterministic.

Docs:

1. Update `docs/features/asset_delivery_contract.md` only if the contract changes.
2. If concurrency changes are implementation-only, keep the contract intact and document no contract change in code comments/tests.

Acceptance criteria:

1. Required bootstrap assets no longer run strictly one-at-a-time.
2. Runtime lazy fetch no longer allows only one in-flight asset at a time.
3. Time to visually complete representative scenes drops measurably.

### Phase 3: Remove Frame-Polled Shader Assignment and Render-Layer Maintenance

Goal:  
Convert shader assignment and render-layer upkeep from broad polling into change-driven invalidation.

Why this is third:

1. This is the largest remaining clearly client-side hot-path polling problem after visibility cadence and asset serialization.
2. It directly aligns the implementation more closely with `DR-0027` and `DR-0029`.

Work:

1. Replace hardcoded “first matching” or special-name shader assignment inference with a more explicit family/domain registry path.
2. Rebuild `RuntimeRenderLayerRegistry` only when relevant authored components change or are removed.
3. Track which world entities need layer reassignment because labels, overrides, or relevant match components changed.
4. Reduce or eliminate archetype-wide watched-component polling where a narrower dirty path is possible.

Files expected:

1. `bins/sidereal-client/src/native/shaders.rs`
2. `bins/sidereal-client/src/native/render_layers.rs`
3. `bins/sidereal-client/src/native/resources.rs`
4. `bins/sidereal-client/src/native/plugins.rs`

Tests:

1. Registry rebuild only occurs on relevant authored-state changes.
2. Entity assignment updates only occur when rule inputs change.
3. Default `main_world` fallback remains unchanged.
4. Shader family assignment remains correct for current authored layers.

Docs:

1. Update `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md` if the transitional hardcoded shader-slot behavior is materially reduced.
2. Update `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md` if authored family/domain inputs change.

Acceptance criteria:

1. Stable frame with no authored-state changes performs zero unnecessary registry rebuilds.
2. Stable frame with no relevant entity-state changes performs zero unnecessary assignment rewrites.
3. Shader assignment no longer depends primarily on special layer IDs or first-match query order where explicit data can replace it.

### Phase 4: Reduce UI / Tactical / Nameplate Frame Cost

Goal:  
Trim overlay and HUD work that currently competes directly with the world render budget.

Why this is fourth:

1. The refreshed audit identified this as a stronger current issue than fullscreen churn.
2. The tactical map and nameplate paths are always visible to players and can strongly affect “feel.”

Work:

1. Reduce tactical overlay work so lower-frequency data sync is separated from per-frame interpolation/presentation.
2. Budget and measure dynamic tactical marker count.
3. Reduce `update_entity_nameplate_positions_system()` cost:
   - avoid rebuilding more world data than necessary,
   - keep strong gating on enabled state,
   - budget visible nameplate count if necessary.
4. Ensure runtime screen overlay material updates are only doing the minimum required per frame.

Files expected:

1. `bins/sidereal-client/src/native/ui.rs`
2. `bins/sidereal-client/src/native/resources.rs`
3. `bins/sidereal-client/src/native/plugins.rs`

Tests:

1. Tactical overlay remains visually correct.
2. Nameplates remain correct for visibility/viewport bounds.
3. Overlay-disabled or nameplate-disabled frames do not do unnecessary work.

Acceptance criteria:

1. Tactical overlay frame cost is measurable and reduced.
2. Nameplate frame cost is measurable and reduced.
3. No regression in tactical overlay or nameplate correctness.

### Phase 5: Reduce Per-Frame Presentation Arbitration and Debug Overhead

Goal:  
Remove steady-state duplicate arbitration cost and trim debug-only runtime overhead.

Work:

1. Introduce or strengthen a winner-per-GUID resource updated on adoption, despawn, control handoff, and relevant marker changes.
2. Make duplicate visual suppression consume that narrower state instead of doing broad steady-state arbitration work.
3. Gate debug overlay camera activation and debug overlay systems behind `DebugOverlayState.enabled`.
4. Keep UI overlay render-layer propagation spawn-time or dirty-only.

Files expected:

1. `bins/sidereal-client/src/native/visuals.rs`
2. `bins/sidereal-client/src/native/debug_overlay.rs`
3. `bins/sidereal-client/src/native/ui.rs`
4. `bins/sidereal-client/src/native/scene_world.rs`
5. `bins/sidereal-client/src/native/plugins.rs`
6. `bins/sidereal-client/src/native/resources.rs`

Tests:

1. Duplicate winner remains stable across handoff cases.
2. Overlay-disabled frames do not run debug overlay collection/update work unnecessarily.
3. UI overlay descendants still receive the right render layer when spawned or reparented.

Acceptance criteria:

1. No steady-state whole-world duplicate GUID arbitration.
2. Debug overlay off means debug overlay camera and debug overlay systems are not active.

### Phase 6: Reduce Material-Instance and Pass Pressure

Goal:  
Lower draw submission and bind-group pressure, but only after earlier phases prove this is still a top bottleneck.

Why this is deliberately last:

1. The refreshed audit treats material-instance pressure as important, but not clearly ahead of visibility cadence, asset serialization, and client polling.
2. This phase should be driven by measurement after Phases 0 through 5, not by assumption.

Work:

1. Audit which material instances genuinely need unique per-entity data and which can share handles or uniform buckets.
2. Budget planet multi-pass visuals explicitly and avoid accidental expansion of always-on pass count.
3. Reduce custom material diversity where payoff is clear and ABI risk is low.
4. Revisit fullscreen/post-process binding reuse only if metrics show it is still a top cost after the earlier phases.

Files expected:

1. `bins/sidereal-client/src/native/visuals.rs`
2. `bins/sidereal-client/src/native/backdrop.rs`
3. `bins/sidereal-client/src/native/resources.rs`

Tests:

1. Material sharing does not leak parameters between unrelated visuals.
2. Planet/effect visuals still render correctly after sharing/bucketing changes.
3. Hot reload and scene reload remain correct.

Acceptance criteria:

1. Material-instance counts drop in representative authored scenes.
2. Draw-call pressure or render submission cost drops measurably where this phase is targeted.

## 4. Validation Matrix

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

## 5. Recommended Path Forward

This section is intentionally explicit for the next agent.

### Immediate next focus

The next agent should start with Phase 0 instrumentation and then move directly to Phase 1 replication visibility cadence work.

The next agent should not start by refactoring fullscreen/post-process renderables unless Phase 0 metrics unexpectedly show that path is still a top bottleneck.

### Required implementation order

1. Phase 0 instrumentation
2. Phase 1 visibility cadence
3. Re-measure and record results
4. Phase 2 asset delivery parallelism/prioritization
5. Re-measure and record results
6. Phase 3 shader assignment/render-layer invalidation
7. Phase 4 UI/tactical/nameplate cost
8. Phase 5 duplicate arbitration/debug overhead
9. Re-measure and only then decide whether Phase 6 is warranted

### Explicit deprioritized work

Do not treat these as lead tasks right now:

1. fullscreen quad caching,
2. “per-frame fullscreen mesh churn” fixes based on the superseded report,
3. broad material-family redesign before Phase 0 through Phase 5 have been measured.

### Decision rule for Phase 6

Only start Phase 6 if, after the earlier phases:

1. camera/pass/material counters still show material-instance pressure is a top runtime cost, or
2. frame-time improvements plateau while material counts and draw pressure remain high.

## 6. Definition of Done

This optimization pass is complete when:

1. the Phase 0 metrics exist and are easy to compare,
2. visibility tick cost is reduced without violating the visibility contract,
3. required bootstrap and runtime lazy fetch are no longer strictly serialized,
4. shader assignment and render-layer work are change-driven enough that steady-state polling cost is materially reduced,
5. tactical/nameplate/overlay frame cost is materially reduced,
6. duplicate visual arbitration is no longer a steady-state whole-world concern,
7. the native client feels materially smoother under the same authored load,
8. no native-only shortcuts make later WASM parity recovery harder.

## 7. Notes For The Next Agent

These are explicit handoff notes based on the refreshed audit.

1. The earlier same-day report was wrong about fullscreen quad allocation being the top current issue. Do not anchor your work on that conclusion.
2. Shared fullscreen quad caching already exists. If you touch fullscreen/post-process paths, do it because metrics prove they still matter, not because the old report said they were the main bug.
3. Your first job is measurement. If you cannot show timings/counters for visibility, asset delivery, render-layer/shader polling, and overlay work, you are still operating blind.
4. Your second job is server cadence. The client can only look smooth if the replication server delivers world state smoothly enough.
5. Your third job is asset readiness. Serialized bootstrap/lazy fetch makes the whole game feel slow even if rendering itself is fine.
6. After that, remove the client polling and overlay waste before you attempt bigger rendering architecture changes.
7. Treat material-instance reduction as a measured follow-up, not as the starting assumption.
8. When you update this plan or related docs, add a dated note rather than silently replacing context.

# Bevy 2D Rendering Optimization Completion Plan

Status: Active completion plan  
Date: 2026-03-12  
Owners: client rendering + replication + asset delivery + UI/HUD + diagnostics  
Primary inputs:
- `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-12.md`
- `docs/plans/bevy_2d_rendering_optimization_implementation_plan_2026-03-10.md`
- `docs/plans/rendering_optimization_pass_plan.md`

Supersedes as the current execution plan:
- `docs/plans/bevy_2d_rendering_optimization_implementation_plan_2026-03-10.md`
- `docs/plans/rendering_optimization_pass_plan.md`

Update note (2026-03-12):
- Use this document as the active execution order for the rendering optimization pass.
- The March 12 audit confirms that instrumentation, visibility caching, bounded bootstrap/runtime fetch concurrency, and parts of duplicate/render-layer cleanup have already landed.
- The remaining highest-value work is now visibility apply-loop cost, runtime asset completion hitches, steady-state client polling, tactical/nameplate HUD cost, and only then material/pass cleanup based on re-measurement.
- Preserve the current interpolation and camera-follow ordering, the Lua-authored render-layer/runtime-shader model, and the server-authoritative architecture while executing this plan.

Update note (2026-03-12, later):
- Phase 0 implementation has started.
- Client instrumentation now includes runtime asset completion-side counters for pending fetch/persist work plus poll/persist/save timing, and HUD counters for tactical marker churn plus nameplate/health-bar update work.
- These counters are surfaced through the existing F3 debug overlay so the next implementation steps can use measured native baselines instead of source-only inference.

Update note (2026-03-12, later 2):
- Phase 1 implementation has started in `bins/sidereal-replication/src/replication/visibility.rs`.
- The visibility apply loop now evaluates authorization/candidate-bypass/delivery in a single helper path and precomputes discovered-landmark layer-derived delivery/extent adjustments once per entity before the per-client loop.
- This change preserves the existing `Authorization -> Delivery -> Payload` contract while removing duplicated policy and range-evaluation work from the hottest membership-application path.

Update note (2026-03-12, later 3):
- Phase 1 entity preclassification has been extended.
- The apply path now prepares per-entity visibility policy buckets before entering the per-client branch: owner-only player anchors, global/config-visible entities, and conditional entities with cached owner/faction/public/landmark/layer facts.
- This removes more repeated root-owner/root-faction/public/landmark setup and controlled-owner branching from the inner client loop while preserving existing visibility semantics and tests.
- Future Phase 1 work should extend these prepared buckets and client lookup caches rather than collapsing back to a single generic per-client branch that re-derives root/public/faction/landmark state inside the hot loop.

Update note (2026-03-13):
- Phase 0 instrumentation slice is implemented enough to support baseline capture: asset completion, tactical HUD, and nameplate counters are now exposed through the client-side debug overlay.
- Phase 1 is in active implementation, not just planned. The visibility apply path now has a unified visibility evaluator, per-entity policy preparation, and owner/map client lookup buckets.
- The immediate next work remains inside Phase 1: split the remaining conditional entities into narrower public/faction/discovered/range buckets, then capture fresh `apply_ms` measurements before moving to Phase 2.

Update note (2026-03-13, later):
- Phase 1 conditional apply-path splitting is now implemented in `bins/sidereal-replication/src/replication/visibility.rs`.
- Prepared entity apply policy now routes conditional entities through narrower public-visible, faction-visible, discovered-landmark, and ordinary range-checked paths instead of a single generic per-client branch.
- The hot apply loop still preserves explicit `Authorization -> Delivery -> Payload` ordering, continues to reuse prepared entity facts plus owner/map client lookup buckets, and now avoids repeated faction/discovery/range branching for policy classes that do not need it.
- Visibility inline tests were tightened to lock the new prepared subpaths to current public/faction/discovered/range semantics.
- The next Phase 1 work is measurement, not more structural refactoring: capture fresh `apply_ms`/`discovery_and_candidate_ms`/candidate and gain-loss baselines and decide whether Phase 1 is complete enough to move to Phase 2.

Update note (2026-03-13, later 2):
- The native client debug overlay text panel has been split into two columns and switched to the same font handle used by the in-world dev console while preserving the existing 15px text size.
- This is a Phase 0 instrumentation usability follow-up so the expanded asset/HUD/render counters remain readable on the default native window size during baseline capture.
- WASM impact: no behavior divergence intended. The change stays inside shared Bevy UI layout/text code and compiled successfully for the `wasm32-unknown-unknown` client target with `bevy/webgpu`.

Update note (2026-03-13, later 3):
- Phase 1 is treated as qualitatively complete enough to move forward even though fresh baseline capture was deferred: representative native validation on this branch indicated a significant visible improvement in both server and client performance after the visibility apply-path changes.
- This is not a quantitative closeout. The Phase 0 and Phase 1 measurement items remain open and should still be captured later so the optimization pass retains before/after evidence.
- Phase 2 is now in progress in `bins/sidereal-client/src/runtime/assets.rs`.
- Runtime optional-asset completion no longer uses `bevy::tasks::block_on(...)` in the steady-state frame loop; fetch, persist, and index-save completions are now drained through non-blocking async completion channels.
- Cache-index saves now wait until the current fetch/persist wave drains, which avoids starting a new save for every completed runtime asset.
- Runtime asset fetch selection now uses explicit deterministic priority buckets: shader/material-critical assets first, root visual assets second, immediate render dependencies third, and lower-value optional art later, while preserving dependency-before-parent correctness.
- Native impact: this should further reduce frame hitching when runtime assets complete and make scene-critical visuals become ready in a more intentional order.
- WASM impact: no intended divergence. The implementation stays inside shared client asset/runtime code and must continue compiling for `wasm32-unknown-unknown` with `bevy/webgpu`.

Update note (2026-03-13, later 4):
- Phase 2 test coverage has been extended.
- `bins/sidereal-client/src/runtime/assets.rs` now has targeted coverage that the runtime queue path does not enqueue duplicate fetches for the same in-flight asset.
- `bins/sidereal-client/src/runtime/auth_net.rs` now has targeted coverage that bootstrap failure leaves the client in a fail-closed state (`failed = true`, bootstrap incomplete, user-visible failure status) instead of silently proceeding.
- This keeps the current asset-delivery optimization aligned with the plan requirement to preserve bootstrap failure behavior and avoid duplicate concurrent fetches while Phase 2 continues.

Update note (2026-03-13, later 5):
- Phase 2 items 1 and 2 are now materially complete across both the runtime asset path and the bootstrap/auth asset path.
- `bins/sidereal-client/src/runtime/auth_net.rs` no longer uses `bevy::tasks::block_on(future::poll_once(...))` in frame-driven gateway/bootstrap polling; those paths now mirror the runtime asset queue model by receiving async completion results through non-blocking channels.
- This closes the remaining frame-thread wait that was still present after the earlier `runtime/assets.rs` completion-path refactor.
- `bins/sidereal-client/src/runtime/assets.rs` now has tighter dependency-ordering coverage for multi-hop dependency chains: the runtime fetch selector prefers the deepest unresolved dependency before its ancestors and advances to the next parent only after that dependency is already in flight.
- Native impact: bootstrap/catalog polling should no longer stall the frame loop waiting on task polling, and deeper streamed-asset trees now have explicit regression coverage for dependency-before-parent fetch ordering.
- WASM impact: no intended divergence. The change stays inside shared client auth/bootstrap/runtime asset code and compiled successfully for the `wasm32-unknown-unknown` target with `bevy/webgpu`.

## 1. Purpose

This plan is written for a fresh agent with no prior project context.

Its job is to finish the remaining work recommended by the March 12 audit in the correct order, without restarting already-landed work and without regressing the project contracts in `AGENTS.md`.

This is not a speculative render rewrite plan. It is an execution plan for completing the current optimization pass with measurable before/after evidence.

## 2. Read Before Editing

Read these documents in this order before making code changes:

1. `AGENTS.md`
2. `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-12.md`
3. `docs/features/visibility_replication_contract.md`
4. `docs/features/asset_delivery_contract.md`
5. `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
6. `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`
7. `docs/reports/native_runtime_system_ownership_audit_2026-03-09.md`
8. `README.md`

If any intended code change conflicts with those docs, update the relevant docs in the same change or stop and resolve the ambiguity first.

## 3. Project Context for a Fresh Agent

The relevant architectural facts are:

1. Sidereal is server-authoritative. Clients never become authoritative for gameplay transforms or state.
2. Native client smoothness is the current delivery priority, but new client/runtime logic should remain shared where possible so later WASM parity recovery is not made harder.
3. The current evidence does not support a GPU-first diagnosis. The March 12 audit still reads as client CPU plus frame pacing plus remaining replication cadence pressure.
4. The render stack is intentionally data-driven. Lua-authored render-layer definitions, shader families, and runtime bindings remain the content contract.
5. Lightyear interpolation plus the current post-correction camera-follow ordering should be preserved. Do not move camera follow earlier and do not add a second transform interpolation pipeline.
6. Visibility optimization must preserve owner/public/faction/global visibility correctness and follow `docs/features/visibility_replication_contract.md`.
7. Asset delivery optimization must preserve dependency correctness, required-asset failure behavior, and the gateway/cache contract in `docs/features/asset_delivery_contract.md`.
8. The optimization rule for this pass is simple: change-driven invalidation is good; steady-state per-frame polling and rebuild churn are bad.

## 4. Current State Snapshot

As of the March 12 audit, the following work is already landed or materially reduced:

1. Visibility no longer rebuilds the entire scratch state in the old March 10 shape. Persistent caches, spatial indices, diff-based membership state, and split landmark discovery are already present.
2. Bootstrap asset fetches and runtime lazy asset fetches are no longer strictly serialized. Both now use bounded concurrency.
3. Telemetry is significantly better. The client and replication paths expose enough counters/timers to support evidence-driven follow-up work.
4. Render-layer assignment and duplicate-visual suppression are more incremental than they were on March 10.
5. Debug overlay gating and some selective material reuse work have already landed.

The following work is still open enough to require code changes:

1. Visibility apply-loop cost remains the top server-side smoothness risk.
2. Runtime asset completion still risks frame-thread hitches because completion work still uses `block_on(...)` in the polling path.
3. Shader assignment, render-layer upkeep, and asset dependency refresh still contain always-on client polling.
4. Tactical map and nameplate UI still do expensive per-frame world/UI reconciliation.
5. Duplicate suppression is improved, but still a transitional runtime tax.
6. Material-instance pressure and the current camera/pass baseline still need a measured follow-up after the earlier phases are complete.

## 5. Non-Negotiable Guardrails

Every phase in this plan must preserve these rules:

1. Do not replace Sidereal visibility with generic engine interest management.
2. Do not weaken owner/public/faction/global visibility rules.
3. Do not introduce native-only client architecture that blocks later WASM recovery.
4. Do not abandon Lua-authored shader/layer definitions in favor of hardcoded Rust-only rendering behavior.
5. Do not remove or bypass Lightyear interpolation safeguards to chase a false simplicity win.
6. Do not treat fullscreen/post-process churn as the top bottleneck unless new metrics prove it again.
7. Do not start material-family redesign or camera collapse work before the earlier CPU-side hot paths are re-measured.
8. Do not silently overwrite plan context. Append dated status notes when updating this document later.

## 6. Required Execution Order

Execute the remaining work in this order unless fresh measurement clearly disproves the current audit:

1. Phase 0: Baseline validation and instrumentation gap closure
2. Phase 1: Finish visibility cadence optimization by reducing apply-loop cost
3. Phase 2: Finish asset delivery optimization by removing completion hitches and enforcing priority
4. Phase 3: Finish steady-state client polling removal in shader, render-layer, and asset dependency paths
5. Phase 4: Reduce tactical map, nameplate, and HUD frame cost
6. Phase 5: Finish duplicate-presentation arbitration cleanup and debug overhead cleanup
7. Re-measure the runtime under the same representative load
8. Phase 6: Reduce material-instance pressure if the new measurements still justify it
9. Phase 7: Rationalize camera/pass baseline only if it still shows up as a meaningful limiter after the earlier work
10. Final validation, docs reconciliation, and a dated completion/status note

### 6.1 Current Execution Status

- [x] Phase 0 instrumentation slice: runtime asset completion counters added and surfaced in the client debug overlay.
- [x] Phase 0 instrumentation slice: tactical marker, nameplate, and health-bar workload counters added and surfaced in the client debug overlay.
- [ ] Phase 0 remaining: capture and record a native baseline using the new counters.
- [x] Phase 1 slice: unified visibility evaluation helper implemented in the replication visibility apply path.
- [x] Phase 1 slice: per-entity visibility policy preparation implemented for owner-only anchors, global/config entities, and conditional entities.
- [x] Phase 1 slice: owner and owner-in-map client lookup buckets implemented to short-circuit stable fast paths before generic per-client evaluation.
- [x] Phase 1 next: split conditional entities into narrower public-visible, faction-visible, discovered-landmark, and ordinary range-checked paths.
- [ ] Phase 1 next: capture fresh `apply_ms`/visibility-stage measurements and compare against the pre-optimization baseline.
- [x] Phase 1 exit gate: qualitative native validation indicates visibility apply cost is reduced enough to move to Phase 2, even though the quantitative measurement follow-up is still deferred.
- [x] Phase 2: asset completion hitch removal and priority enforcement is now in progress in `bins/sidereal-client/src/runtime/assets.rs`.
- [ ] Phase 3: steady-state client polling removal has not started yet.
- [ ] Phase 4: tactical/nameplate/HUD cost reduction beyond instrumentation has not started yet.
- [ ] Phase 5: duplicate-presentation/debug cleanup follow-up has not started yet.

## 7. Phase 0: Baseline Validation and Instrumentation Gap Closure

Status (2026-03-13):
- In progress.
- The instrumentation/code changes are partially complete.
- The remaining required step is to capture and record a baseline from a representative native run.

Goal:
Make sure the current counters and timers are real, easy to compare, and sufficient to guide the rest of the pass.

Why this phase still exists:

1. Instrumentation is partially complete, not fully closed.
2. The March 12 audit was still a static code audit, not a live profiling pass.
3. A fresh agent should not optimize blind or assume every expected metric is already exposed in a useful way.

Primary files:

1. `bins/sidereal-client/src/runtime/resources.rs`
2. `bins/sidereal-client/src/runtime/debug_overlay.rs`
3. `bins/sidereal-client/src/runtime/plugins.rs`
4. `bins/sidereal-client/src/runtime/assets.rs`
5. `bins/sidereal-client/src/runtime/ui.rs`
6. `bins/sidereal-replication/src/replication/visibility.rs`

Work:

1. Verify that the existing metrics can answer all of the following questions without extra code inspection:
   - How much time is spent in visibility total, candidate/discovery, and apply stages?
   - How many candidates and visible entities are processed per client?
   - How many bootstrap and runtime asset downloads are queued, in flight, and completed?
   - How much frame-thread time is spent finishing asset tasks, writing cache files, and saving the cache index?
   - How often do shader assignment and render-layer registry rebuilds happen in a stable frame?
   - How often does runtime asset dependency refresh perform real work in a stable frame?
   - How many tactical markers, nameplates, and health-bar updates are processed per frame?
   - How many duplicate winner swaps occur during representative play?
   - How many active cameras, fullscreen passes, post-process passes, and relevant material instances exist?
2. Add any missing low-overhead counters or timers needed for later decisions, especially:
   - asset completion/save/index-save timing,
   - tactical marker count and update timing,
   - nameplate update timing and visible-count budget,
   - health-bar update count,
   - material counts that will be used in Phase 6.
3. Ensure all instrumentation remains gated or low-overhead enough to keep measurement trustworthy.
4. Capture a native baseline using the current branch before touching later phases.
5. Record the baseline in a dated status note in this plan or in the follow-up PR description so later phases can compare against it.

Acceptance criteria:

1. A fresh agent can answer what is expensive right now without performing another blind source audit.
2. Every later phase in this plan has a corresponding metric or counter that can prove it helped.
3. Instrumentation overhead is bounded and can be disabled or sampled if needed.

## 8. Phase 1: Finish Visibility Cadence Optimization

Status (2026-03-13):
- Implemented enough to move on, with measurement follow-up still pending.
- Three implementation slices are already landed:
  1. unified authorization/candidate-bypass/delivery evaluation,
  2. per-entity policy preparation before the client loop,
  3. client lookup buckets for owner-only and owner-in-map fast paths.
- A fourth slice is also landed: conditional entities are split into public-visible, faction-visible, discovered-landmark, and ordinary range-checked prepared apply paths.
- Representative native validation after these changes indicated a significant server/client improvement, so Phase 2 can start even though the quantitative baseline capture was deferred.

Goal:
Reduce the remaining cost inside the final visibility membership/apply lane without changing visibility semantics.

Why this is first:

1. The March 12 audit still identifies this as the top remaining server-side smoothness risk.
2. Better client rendering will not feel smooth if authoritative replication cadence is still bursty.
3. The high-value architectural work is already landed; the remaining work is to make the apply lane cheaper, not to redesign the system again.

Primary files:

1. `bins/sidereal-replication/src/replication/visibility.rs`
2. `bins/sidereal-replication/src/plugins.rs`
3. Tests that cover visibility policy and discovery behavior

Work:

1. Profile the current visibility stages using the existing sub-stage timings.
2. Confirm whether `apply_ms` is the dominant remaining stage under representative entity/client load.
3. Separate cheap policy classes earlier in the pipeline so the apply loop does less branching:
   - owner-forced entities,
   - config-forced/global entities,
   - ordinary range-checked entities,
   - discovered landmark entities.
4. Cache per-entity policy-class outputs that are reused across clients so the apply loop does not recompute the same visibility-policy facts repeatedly.
5. Reduce avoidable repeated authorization and delivery checks inside the inner membership application path.
6. Preserve the current persistent caches, spatial index, diff-based membership behavior, and split landmark discovery cadence.
7. Add or tighten tests covering:
   - owner visibility,
   - public visibility,
   - faction visibility,
   - global/config bypass cases,
   - discovered landmark delivery,
   - no regression in final membership diffs.

Do not do in this phase:

1. Do not replace Sidereal visibility with Lightyear interest management.
2. Do not collapse distinct policy semantics to win a micro-benchmark.
3. Do not re-merge landmark discovery into the hot per-tick path.

Immediate next tasks (do these in order):

- [x] Split `PreparedEntityApplyPolicy::Conditional` into narrower subpaths so public-visible, faction-visible, discovered-landmark, and ordinary range-checked entities do not all pay the same generic client-evaluation shape.
- [x] Keep authorization ordering explicit: `Authorization -> Delivery -> Payload` must still be obvious in the code after the split.
- [x] Reuse the existing prepared entity facts and client lookup buckets; do not reintroduce root/public/faction/landmark derivation inside the inner client loop.
- [x] Add or tighten tests for each new conditional subpath so the fast paths are locked to current owner/public/faction/discovered semantics.
- [ ] Capture native or representative runtime measurements for `apply_ms`, `discovery_and_candidate_ms`, `candidates_per_client`, `visible_gains`, and `visible_losses`.
- [x] Add a dated status note to this plan with a clear Phase 1 go decision for moving to Phase 2. Quantitative measurement remains deferred and is still tracked separately above.

Acceptance criteria:

1. Visibility `apply_ms` or total visibility tick cost drops under comparable load.
2. Candidate counts and delivery outputs remain correct.
3. No visibility-contract regression is introduced.

## 9. Phase 2: Finish Asset Delivery Optimization

Status (2026-03-13):
- In progress.
- The first active slice is landed in `bins/sidereal-client/src/runtime/assets.rs`.
- Steady-state runtime asset completion no longer relies on `bevy::tasks::block_on(...)` inside the frame loop.
- Cache-index save starts are deferred until the current fetch/persist wave drains.
- Runtime fetch selection now has explicit deterministic priority buckets for critical shader/material assets, root visuals, immediate render dependencies, and lower-value optional art.

Goal:
Keep the new parallel fetch behavior, then remove main-thread completion hitches and make priority rules explicit.

Why this is second:

1. The network fetch side is already fixed enough to close the old March 10 finding.
2. The new remaining problem is hitching when asset work completes, writes to cache, or saves the index on the frame thread.
3. This still directly affects perceived slowness and late visual readiness.

Primary files:

1. `bins/sidereal-client/src/runtime/auth_net.rs`
2. `bins/sidereal-client/src/runtime/assets.rs`
3. `bins/sidereal-client/src/runtime/resources.rs`
4. `bins/sidereal-client/src/runtime/plugins.rs`

Work:

1. Remove `bevy::tasks::block_on(...)` from frame-driven asset completion paths where it currently waits for completion-side work.
2. Move cache file writes and cache-index save work off the frame thread or onto a deferred/batched completion path.
3. Batch or debounce cache-index saves so one completed asset does not imply one immediate index save.
4. Make asset priority rules explicit and measurable:
   - shader and material-critical assets first,
   - root visual assets second,
   - immediate render dependencies third,
   - lower-value optional art later.
5. Preserve dependency-before-parent correctness so dependent visuals do not bind before prerequisites are ready.
6. Preserve fail-closed behavior for required bootstrap assets.
7. Add or tighten tests for:
   - required bootstrap failure behavior,
   - runtime dependency closure correctness,
   - no duplicate fetch of the same asset under concurrency,
   - deterministic priority ordering where required.

Do not do in this phase:

1. Do not remove the bounded concurrency limits without measurement.
2. Do not allow asset priority shortcuts to violate dependency ordering.
3. Do not move asset payload delivery onto the replication transport.

Acceptance criteria:

1. Bootstrap and runtime fetch remain concurrent.
2. Asset completion no longer blocks the frame thread in the current hot path.
3. Cache-index save behavior is measurably less hitch-prone.
4. Scene visual readiness improves or at minimum stops hitching when assets complete.

## 10. Phase 3: Finish Steady-State Client Polling Removal

Goal:
Convert remaining broad polling in shader assignment, render-layer upkeep, and asset dependency refresh into narrower change-driven or generation-driven work.

Why this is third:

1. The March 12 audit now treats this as the strongest likely client CPU cost.
2. This phase attacks always-on bookkeeping that happens before draw submission is even the question.
3. It keeps the project aligned with DR-0027 and the runtime shader-family model rather than drifting into ad hoc behavior.

Primary files:

1. `bins/sidereal-client/src/runtime/shaders.rs`
2. `bins/sidereal-client/src/runtime/render_layers.rs`
3. `bins/sidereal-client/src/runtime/assets.rs`
4. `bins/sidereal-client/src/runtime/resources.rs`
5. `bins/sidereal-client/src/runtime/plugins.rs`

Work:

1. Replace per-frame shader-assignment scans with a cached authored-state product keyed by relevant catalog generations and removal/change cursors.
2. Rebuild `RuntimeRenderLayerRegistry` only when relevant authored layer definitions, matching rules, or post-process stacks actually change.
3. Track which runtime entities need layer reassignment because the relevant inputs changed, instead of re-walking broad entity sets every frame.
4. Convert runtime asset dependency refresh into a dirty-queue, change-driven, or generation-driven path so stable scenes do not perform repeated whole-world dependency decision work.
5. Preserve current fallback semantics such as `main_world` unless the docs are updated in the same change.
6. If this phase materially reduces transitional hardcoded shader-slot logic, update the runtime shader-family decision doc in the same change.
7. Add or tighten tests for:
   - registry rebuild only on relevant authored-state changes,
   - assignment updates only on relevant entity/input changes,
   - asset dependency refresh idles in steady state,
   - current authored shader family/domain behavior remains correct.

Do not do in this phase:

1. Do not replace the data-driven model with hardcoded Rust mappings as a shortcut.
2. Do not introduce stale-cache behavior by under-invalidation.
3. Do not accept broad per-frame polling because it "early-outs quickly"; the point is to avoid the work in stable frames.

Acceptance criteria:

1. Stable frames with no authored or relevant entity-state changes perform zero unnecessary registry rebuilds.
2. Stable frames perform zero unnecessary shader-assignment rewrites.
3. Stable frames perform zero unnecessary asset-dependency refresh work.
4. Existing authored behavior remains correct.

## 11. Phase 4: Reduce Tactical Map, Nameplate, and HUD Frame Cost

Goal:
Remove the most obvious per-frame world/UI reconciliation cost from always-visible HUD systems.

Why this is fourth:

1. The March 12 audit still calls this a high-severity client CPU and feel problem.
2. Players pay this cost in normal play even when world rendering itself is otherwise acceptable.

Primary files:

1. `bins/sidereal-client/src/runtime/ui.rs`
2. `bins/sidereal-client/src/runtime/resources.rs`
3. `bins/sidereal-client/src/runtime/plugins.rs`

Work:

1. Split tactical map behavior into:
   - lower-frequency structural sync,
   - per-frame interpolation/presentation only for already-known markers.
2. Stop rebuilding larger-than-necessary marker/nameplate maps every frame when the underlying set did not change.
3. Gate tactical and nameplate work hard on feature enable state and visible-count budgets.
4. Add explicit budgets and counters for:
   - active tactical markers,
   - active nameplates,
   - health-bar updates,
   - viewport transform work.
5. Replace nested health-bar target lookup paths with a direct target cache where possible.
6. Keep tactical/nameplate correctness for visibility, viewport bounds, and winner selection behavior.
7. Add or tighten tests for:
   - overlay-disabled frames doing no unnecessary work,
   - nameplates remaining correct near viewport bounds,
   - tactical markers remaining visually correct after structural caching changes.

Do not do in this phase:

1. Do not hide the cost by lowering update frequency so far that the HUD becomes visibly wrong.
2. Do not couple UI correctness to debug-only state.
3. Do not reintroduce world scans elsewhere to compensate for removed UI scans.

Acceptance criteria:

1. Tactical overlay update cost is measurable and reduced.
2. Nameplate update cost is measurable and reduced.
3. Health-bar update cost is measurable and reduced.
4. HUD correctness is preserved.

## 12. Phase 5: Finish Duplicate-Presentation Arbitration and Debug Cleanup

Goal:
Move duplicate winner/suppression work further out of the steady-state frame path and ensure debug-only systems stay truly inactive when disabled.

Why this is fifth:

1. The March 12 audit shows this area is improved, but not fully retired.
2. This phase should build on the already-landed dirty-guid direction instead of starting over.

Primary files:

1. `bins/sidereal-client/src/runtime/visuals.rs`
2. `bins/sidereal-client/src/runtime/replication.rs`
3. `bins/sidereal-client/src/runtime/debug_overlay.rs`
4. `bins/sidereal-client/src/runtime/ui.rs`
5. `bins/sidereal-client/src/runtime/scene_world.rs`
6. `bins/sidereal-client/src/runtime/plugins.rs`
7. `bins/sidereal-client/src/runtime/resources.rs`

Work:

1. Verify the existing winner-per-GUID data flow and extend it so winner changes are driven by lifecycle events:
   - spawn/adoption,
   - despawn,
   - control handoff,
   - relevant marker changes.
2. Remove any remaining steady-state broad arbitration work that is only there to rediscover already-known winners.
3. Ensure debug overlay systems and debug overlay camera activation are fully gated behind enabled state.
4. Keep UI overlay render-layer propagation spawn-time or dirty-only.
5. Add or tighten tests for:
   - winner stability during handoff cases,
   - duplicate suppression correctness when predicted/interpolated ownership changes,
   - debug overlay off meaning the corresponding collection/update path is inactive.

Acceptance criteria:

1. No steady-state whole-world duplicate arbitration remains.
2. Duplicate winner swaps are measurable and only happen on real lifecycle changes.
3. Debug overlay off means no meaningful debug-overlay runtime work is happening.

## 13. Re-Measure Before Starting the Late Phases

After Phases 0 through 5, stop and re-measure under the same representative native load used earlier.

Produce a dated status note that answers:

1. Did visibility `apply_ms` materially improve?
2. Did asset completion hitches materially improve?
3. Did render-layer/shader/asset-dependency steady-state work materially drop?
4. Did tactical/nameplate/HUD frame cost materially drop?
5. Are material-instance counts or pass/camera counts now one of the top remaining constraints?

Do not start the late phases until these answers exist.

## 14. Phase 6: Reduce Material-Instance Pressure

Goal:
Reduce batching loss, draw-state churn, and related client CPU/GPU pressure only if the re-measurement still shows this as a top remaining cost.

Why this is late:

1. The March 12 audit still flags material-instance pressure as real.
2. The same audit also says earlier CPU-side work is still more important right now.
3. This phase should therefore be evidence-driven, not assumed.

Primary files:

1. `bins/sidereal-client/src/runtime/visuals.rs`
2. `bins/sidereal-client/src/runtime/backdrop.rs`
3. `bins/sidereal-client/src/runtime/resources.rs`
4. Any tests covering planet/effect/fullscreen visual correctness

Work:

1. Audit which material instances really need unique per-entity parameters and which can share handles or bucketed uniforms.
2. Treat planet pass count and planet material count as explicit budgets.
3. Reduce avoidable unique material allocation in:
   - planet multi-pass visuals,
   - pooled effect visuals,
   - fullscreen/post-process attachment paths.
4. Prefer small reusable pools or buckets where parameter sharing is safe.
5. Preserve visual correctness, hot reload behavior, and live authored updates.
6. Add or tighten tests for:
   - no parameter leakage between unrelated visuals,
   - correct planet/effect rendering after sharing changes,
   - hot reload and scene reload correctness.

Decision rule:

Start this phase only if the re-measurement shows that material counts, draw pressure, or submission cost are still among the top remaining bottlenecks.

Acceptance criteria:

1. Material-instance counts drop in representative scenes.
2. Render submission or draw pressure drops measurably where targeted.
3. No authored/runtime visual regression is introduced.

## 15. Phase 7: Rationalize Camera and Pass Baseline

Goal:
Reduce baseline pass/extraction/composition overhead only if it still matters after the earlier phases are complete.

Why this is last:

1. The March 12 audit calls the current camera/pass layout heavy, but not yet conclusively wrong.
2. Collapsing passes too early is a high-regression move and can break scene composition ordering.

Primary files:

1. `bins/sidereal-client/src/runtime/scene_world.rs`
2. `bins/sidereal-client/src/runtime/camera.rs`
3. `bins/sidereal-client/src/runtime/plugins.rs`
4. `bins/sidereal-client/src/runtime/mod.rs`
5. `bins/sidereal-client/src/runtime/debug_overlay.rs`

Work:

1. Use the active-camera and pass counters from earlier phases to identify whether the current baseline is still a meaningful cost.
2. Disable or collapse only those always-on passes that the re-measurement proves are low-value relative to their cost.
3. Keep composition semantics explicit when collapsing any camera/pass path.
4. Preserve debug overlay optionality and keep it fully disabled when not in use.
5. Validate that camera follow, fullscreen ordering, post-process ordering, and UI overlay correctness remain intact.

Decision rule:

Start this phase only if the re-measurement shows that pass/camera overhead remains a meaningful limiter after Phases 0 through 6.

Acceptance criteria:

1. Active pass/camera count is reduced only where justified by metrics.
2. Frame cost or pacing improves measurably where the reduction is targeted.
3. No composition-order regression is introduced.

## 16. Phase Output Requirements

Every implementation phase in this plan must include:

1. Code updates
2. Tests for the touched behavior
3. Doc updates if behavior or contracts changed
4. A short dated status note added to this plan summarizing:
   - what was changed,
   - what metrics improved or did not improve,
   - what remains next

## 17. Validation Requirements

Before marking any phase complete, run the relevant targeted tests and then run the repo quality gates required by `AGENTS.md`:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

If client code changed, also run:

```bash
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

If a required local toolchain is missing, note that explicitly in the phase status or PR description.

## 18. First Working Session Checklist for a Fresh Agent

On the first session using this plan, do exactly this:

1. Read the documents listed in Section 2.
2. Open the files named in Phase 0 and verify which counters/timers already exist.
3. Build the touched targets so the baseline branch is in a known-good state.
4. Capture a baseline with the current metrics.
5. Start Phase 1 only after the baseline exists.
6. After each phase, record what changed and what improved before moving on.

## 19. Definition of Done

This optimization pass is complete when:

1. The baseline and after-phase metrics are recorded and easy to compare.
2. Visibility apply-loop cost is reduced without violating the visibility contract.
3. Asset completion no longer introduces the current frame-thread hitch path.
4. Shader assignment, render-layer upkeep, and asset dependency refresh are change-driven enough that stable frames stay quiet.
5. Tactical map, nameplate, and HUD frame cost are materially reduced.
6. Duplicate-presentation arbitration is no longer a steady-state whole-world concern.
7. Material-instance pressure and camera/pass baseline have been either reduced or explicitly ruled out by measurement after the earlier phases.
8. The native client feels materially smoother under comparable load.
9. No change in this pass makes later WASM parity recovery harder.

## 20. Explicit Non-Goals

Do not spend time on these unless later measurement proves they have become top priorities:

1. Reopening the old fullscreen-first optimization theory from the superseded March 9 plan
2. Replacing Sidereal-specific visibility with a generic upstream interest-management model
3. Introducing a second interpolation ownership path
4. Broad render architecture rewrites that are not justified by the measured bottlenecks from this pass

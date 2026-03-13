# Bevy 2D Rendering Optimization Audit Report

Status: Active  
Report date: 2026-03-13  
Prompt source path: `docs/prompts/bevy_2d_rendering_optimization_audit_prompt.md`  
Supersedes: `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-12.md`  
Scope: Current client-visible rendering smoothness, with emphasis on regressions introduced after the March 12 audit baseline.  
Limitations: Static code audit only. No fresh live frame captures, RenderDoc traces, Tracy/Puffin captures, packet captures, or GPU timings were available for this pass. Statements about runtime weight remain source-based unless explicitly tied to concrete code shape.

Update note (2026-03-13):
- This pass was run specifically to compare the current worktree against the March 12 rendering audit and completion plan.
- The strongest newly introduced regressions are not in visibility or asset bootstrap. They are in the client presentation path: a new fullscreen distortion post-process lane, heavier destruction-effect rendering, and a broadened thruster-plume hot-path query.
- One previously identified HUD issue has improved: nameplate projection now runs in `PostUpdate` after transform propagation instead of in `Update`.

## 1. Executive Summary

The March 12 core conclusions still broadly hold: the game does not read like a normal-play client that is primarily GPU-bound all the time, and the biggest historical wins in visibility and asset completion are still intact.

The new performance drop is most plausibly coming from two client-side regressions:

1. A new explosion-distortion fullscreen post-process path now adds an additional full-screen pass on the gameplay camera whenever explosions are active.
2. Thruster plume systems regressed from component-filtered queries to broader world scans plus per-entity string matching in a hot `Update` path.

There is also a third, scene-conditional regression:

3. Destruction explosions are now much larger and longer-lived than weapon-impact explosions, and the explosion shader itself is more expensive. In combat-heavy scenes this increases alpha overdraw, keeps more explosion entities active for longer, and keeps the new distortion pass alive for longer.

The March 12 server-side visibility and asset-fetch findings do not appear to have reopened in the current diff. The current slowdown reads as a client presentation regression, not a reversion of the earlier visibility/asset work.

## 2. What Changed Since 2026-03-12

### Improvements

1. Nameplate projection moved out of `Update` and into `PostUpdate` after transform propagation in `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:56-72`, and was removed from `Update` in `bins/sidereal-client/src/runtime/plugins/ui_plugins/in_world.rs:31-49`.
2. That is directionally correct and partially closes the March 12 HUD scheduling concern.

### Regressions

1. A new fullscreen explosion-distortion post-process plugin is now always added for non-headless clients in `bins/sidereal-client/src/runtime/app_setup.rs:198-203`.
2. That plugin installs a new render-graph node and full-screen pass in `bins/sidereal-client/src/runtime/post_process.rs:39-74` and `bins/sidereal-client/src/runtime/post_process.rs:112-160`.
3. Thruster plume attachment/update no longer filter on the gameplay `Engine` component. They now query broadly and identify engines by `EntityLabels` string scans in `bins/sidereal-client/src/runtime/visuals.rs:2222-2285` and `bins/sidereal-client/src/runtime/visuals.rs:2288-2365`.
4. Destruction explosions now reuse the explosion pool with much larger scale and much longer lifetime in `bins/sidereal-client/src/runtime/visuals.rs:110-113` and `bins/sidereal-client/src/runtime/visuals.rs:262-323`.
5. The explosion fragment shader became materially heavier in `data/shaders/runtime_effect.wgsl:140-188`.
6. `ThrusterPlumeShaderSettings` visibility was widened from owner-only to public in `crates/sidereal-game/src/components/thruster_plume_shader_settings.rs:43-48`, increasing replication surface for a visual tuning component.

## 3. Current Findings

### F1. A new fullscreen distortion pass reopens a class of cost the March 12 plan explicitly deferred

- Severity: High
- Confidence: Proven
- Main impact: `client GPU`, `client CPU`, `frame pacing`, `combat-scene spikes`
- Exact references:
  - `docs/plans/bevy_2d_rendering_optimization_completion_plan_2026-03-12.md:149-165`
  - `bins/sidereal-client/src/runtime/app_setup.rs:198-203`
  - `bins/sidereal-client/src/runtime/post_process.rs:39-74`
  - `bins/sidereal-client/src/runtime/post_process.rs:112-160`
  - `bins/sidereal-client/src/runtime/scene_world.rs:177-203`
  - `data/shaders/explosion_distortion_post_process.wgsl:37-54`
- Why it matters:
  - The March 12 plan explicitly said not to move back into fullscreen/post-process churn before earlier CPU hot paths were re-measured.
  - The client already keeps separate fullscreen foreground and post-process camera lanes alive.
  - The new distortion path adds another post-tonemapping full-screen pass on the gameplay camera whenever any encoded shockwave is active.
  - The render node creates a new bind group each active frame and issues a full-screen draw over the gameplay target.
  - This is exactly the kind of “looks expensive in fights, feels like the renderer got slower” regression that can show up even if the game is still not globally GPU-bound in quiet scenes.
- Current disposition:
  - This is the clearest new render-path regression relative to March 12.
  - It does not invalidate the March 12 overall CPU/pacing thesis, but it absolutely makes the pass baseline heavier during combat.
- Recommended next action:
  - Gate this effect behind a runtime toggle immediately for comparison.
  - Capture native before/after timings with the distortion pass disabled.
  - If the effect stays, move it behind an explicit budget and only enable it when measured combat cost is acceptable.

### F2. Thruster plume hot-path filtering regressed from component-based ECS selection to broad world scans plus string matching

- Severity: High
- Confidence: Proven
- Main impact: `client CPU`, `steady-state frame cost`, `ECS scheduling/query work`
- Exact references:
  - `bins/sidereal-client/src/runtime/visuals.rs:2222-2285`
  - `bins/sidereal-client/src/runtime/visuals.rs:2288-2365`
  - `bins/sidereal-client/src/runtime/visuals.rs:325-330`
- Why it matters:
  - The previous query shape filtered thruster work to actual `Engine` entities.
  - The current code now iterates `WorldEntity` plus children/pass state, then runs `eq_ignore_ascii_case("engine")` over `EntityLabels` for every candidate.
  - That is a direct regression against the March 12 recommendation to reduce always-on client polling and broad hot-path bookkeeping.
  - This cost lands every frame in normal gameplay, not only during destruction-heavy scenes.
- Current disposition:
  - This is the strongest likely steady-state client CPU regression in the current diff.
  - It is also unnecessary if the actual gameplay `Engine` component is still available, which it is in `crates/sidereal-game/src/components/engine.rs`.
- Recommended next action:
  - Restore component-filtered queries for thruster systems.
  - If label-based fallback is needed for content migration, perform it once during adoption/attachment and cache the result instead of rescanning strings every frame.

### F3. Destruction explosions now keep more expensive effect work alive much longer, and the shader cost also increased

- Severity: Medium-High
- Confidence: Proven
- Main impact: `client GPU`, `alpha overdraw`, `effect update cost`, `combat-scene spikes`
- Exact references:
  - `bins/sidereal-client/src/runtime/visuals.rs:110-113`
  - `bins/sidereal-client/src/runtime/visuals.rs:262-323`
  - `bins/sidereal-client/src/runtime/plugins/presentation_plugins.rs:58-88`
  - `bins/sidereal-client/src/runtime/visuals.rs:3162-3192`
  - `data/shaders/runtime_effect.wgsl:140-188`
- Why it matters:
  - Weapon-impact explosions used a short `0.18s` TTL; destruction explosions now use `0.65s`, much larger base scale, and much larger growth.
  - The explosion pool is still updated every frame, so increasing lifetime and coverage directly increases how often those entities stay active and visible.
  - The shader itself now does more math per fragment: extra trigonometry, extra radial distortion work, more layered falloffs, and higher-energy composition.
  - Larger soft alpha quads also raise overdraw risk, especially when several destruction effects overlap near the camera.
  - Because the fullscreen distortion pass is keyed off active explosion state, this change also keeps that pass active for longer.
- Current disposition:
  - This is likely not the top steady-state cause by itself, but it is a very plausible explanation for “performance suddenly tanks when ships start exploding.”
- Recommended next action:
  - Treat destruction-effect frequency, concurrent active explosions, and distortion-active frames as explicit combat budgets.
  - Measure combat scenes with destruction FX disabled separately from the distortion pass so the two regressions can be isolated.

### F4. Public replication of thruster plume settings increases delivery volume for a visual tuning component

- Severity: Medium
- Confidence: Proven
- Main impact: `replication churn`, `client component churn`, `cross-client visual overhead`
- Exact references:
  - `crates/sidereal-game/src/components/thruster_plume_shader_settings.rs:43-48`
- Why it matters:
  - March 12 explicitly warned that over-delivery can make rendering appear slow by increasing client churn even when actual draw cost is moderate.
  - Widening a persistable replicated visual-settings component from owner-only to public increases component delivery to every observing client.
  - This is probably not the main source of the reported drop on its own, but it pushes in the wrong direction at the same time the thruster systems became more expensive locally.
- Current disposition:
  - This is a secondary regression unless remote thruster settings are now changing frequently in live play.
- Recommended next action:
  - Confirm whether public replication is strictly required for current visuals.
  - If only the final plume presentation needs to be public, prefer replicating a cheaper render intent or stable baked setting instead of a richer tuning payload.

## 4. Specific Statements: Confirmed or Refuted

1. `The game is GPU-bound in normal gameplay.`
   - Still not supported as the default reading.
   - In quiet scenes, the stronger evidence still points to client CPU and pacing.
   - In destruction-heavy scenes, the new post-process plus heavier explosion shader likely add meaningful GPU spikes.

2. `The game is CPU-bound on the client in normal gameplay.`
   - More likely true than it was on March 12 because of the thruster-plume query regression.

3. `The game is bottlenecked by ECS scheduling/query work more than actual draw submission.`
   - Still likely true in normal gameplay.
   - The new thruster-path regression reinforces this.

4. `The game is bottlenecked by replication/update churn more than rendering itself.`
   - The old visibility/asset problems do not appear to have reopened.
   - The current regression reads more client-presentation-heavy than the March 10-12 bottlenecks.

5. `The game feels slow mainly because of frame pacing/interpolation issues rather than raw frame time.`
   - Still partly true overall.
   - The new fullscreen distortion path adds a more conventional raw render-cost spike during combat on top of the existing pacing sensitivity.

6. `Shader/material diversity is defeating batching enough to matter.`
   - Still likely true.
   - The current diff does not fix that, and destruction FX makes effect-material pressure more visible during combat.

7. `Too many fullscreen or post-process passes are active for the current visual payoff.`
   - More true than in the March 12 baseline, because a new gameplay-camera fullscreen pass has been added before the planned re-measurement gates.

8. `Off-screen or non-visible entities are still paying too much render-related cost.`
   - Still likely true in general, but not the main newly introduced regression in this diff.

9. `Asset/shader compilation or loading hitching is a meaningful source of stalls.`
   - No new evidence in this diff that the March 12 asset-completion improvements regressed.

10. `Server-side visibility/replication behavior is causing client render instability or overload.`
   - No new evidence that the March 12 visibility improvements regressed in the current worktree.

11. `The current render-layer architecture is directionally correct and should be kept.`
   - Yes.
   - The new regressions are layered on top of it, not proof against it.

12. `The current render-layer/material implementation has avoidable transitional cost that should be simplified.`
   - Yes.
   - The new distortion pass increases that transitional cost.

## 5. End-to-End Render Flow Map

### 5.1 Asset/bootstrap to client-ready rendering

1. The March 12 bounded-concurrency bootstrap and runtime asset work remains in place.
2. Client scene startup now additionally installs `ExplosionDistortionPostProcessPlugin` for non-headless clients before the visual and lighting plugins.
3. Once the gameplay camera exists, the plugin ensures it carries `ExplosionDistortionSettings`, which are then extracted into the render world.

### 5.2 Replicated entity arrival to visible draw

1. Replicated world entities still flow through adoption, render-layer assignment, and visual attachment as before.
2. Destruction events now also send `ServerEntityDestructionMessage` to relevant clients, which activates pooled explosion visuals on receipt.
3. Those same explosion visuals now feed both the world-space explosion draw path and the new gameplay-camera distortion pass.

### 5.3 Camera-relative/world-layer transform derivation

1. No major regression found here.
2. Nameplate projection is improved because it now runs after transform propagation in `PostUpdate`.

### 5.4 Fullscreen background/foreground/post-process execution

1. Existing fullscreen foreground and post-process camera lanes remain active.
2. The new distortion node adds a separate post-tonemapping full-screen pass on the gameplay camera itself when active shockwaves exist.
3. This means combat can now pay both the existing post-process lane cost and the new gameplay-camera post-process cost.

### 5.5 Prediction/reconciliation/interpolation to final presented motion

1. No new evidence was found that the interpolation/camera ordering regressed.
2. The primary new issue is extra client presentation work after the motion state is already correct enough to render.

## 6. Performance Budget Map

This section is partly inferential and explicitly labeled as such.

### 6.1 Client CPU

- Proven:
  - Thruster plume attach/update now do broader ECS scans and string matching every frame.
  - Explosion distortion settings are rebuilt every frame in `PostUpdate`, including viewport projection work for active explosions.
  - The new distortion render node creates a bind group each active frame.
- Inference:
  - Steady-state client CPU likely regressed most from the plume query change.

### 6.2 Client GPU

- Proven:
  - A new full-screen post-process pass runs when explosions are active.
  - Explosion fragment work is heavier and destruction quads are much larger/longer-lived.
- Inference:
  - GPU spikes are likely higher specifically during destruction-heavy combat, but this still does not prove that the game is globally GPU-bound.

### 6.3 Client Main-Thread Stalls

- No reopened regression found in the March 12 asset/bootstrap completion work.
- The current diff reads more as added per-frame work than blocking IO/stall reintroduction.

### 6.4 Server Tick Cost That Affects Visual Smoothness

- No new visibility-cadence regression was found in the current changes.
- The added destruction message fanout is much smaller than the March 10 visibility problem and uses `InputChannel` (`SequencedUnreliable`), so it is not the leading suspect.

### 6.5 Network/Replication Delivery Cost That Affects Render Churn

- Proven:
  - `ThrusterPlumeShaderSettings` now replicate publicly.
  - Destruction messages now deliver extra effect events.
- Inference:
  - This is secondary to the local client regressions unless visual-setting churn is frequent in live play.

## 7. Priority Order

1. Revert or re-gate the new explosion distortion pass first. It is the clearest new render-path regression and the easiest thing to isolate in measurement.
2. Restore component-filtered thruster plume queries second. This is the clearest steady-state client CPU regression.
3. Re-measure combat with destruction FX active versus disabled to determine how much of the drop is coming from the larger/heavier explosion path.
4. Revisit the public replication scope of `ThrusterPlumeShaderSettings` only after the first three items are measured.

## 8. Bottom Line

Compared with the March 12 baseline, the biggest regressions are client-side and recent:

1. Added fullscreen explosion distortion.
2. Broadened thruster-plume hot-path queries.
3. Heavier, larger, longer-lived destruction explosions.

The March 12 visibility and asset-delivery improvements still appear intact. The slowdown currently looks like a presentation-path regression layered on top of an already CPU-sensitive client, not a reopening of the older infrastructure bottlenecks.

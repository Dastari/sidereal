# Multiplayer Prediction / Interpolation Reliability Plan

Status: Active implementation plan  
Date: 2026-03-15  
Owners: client runtime + replication + prediction/interpolation + diagnostics  
Primary inputs:
- `docs/reports/lightyear_multiplayer_prediction_interpolation_audit_report_2026-03-15.md`
- `docs/plans/lightyear_runtime_alignment_implementation_plan.md`
- `docs/plans/bevy_2d_rendering_optimization_completion_plan_2026-03-12.md`
- `docs/features/prediction_runtime_tuning_and_validation.md`
- `docs/features/lightyear_upstream_issue_snapshot.md`

Update note (2026-03-15):
- This plan exists because the current branch has two overlapping problems:
  1. an older stationary-entity bootstrap failure where some confirmed replicated entities appear at `0,0` until movement,
  2. newer client-side control/presentation regressions introduced while reducing duplicate ownership churn.
- The implementation priority in this plan is reliability first, then cleanup, then re-optimization.
- Performance still matters, but the performance rule for this effort is specific:
  - prefer change-driven invalidation, targeted caches, and bounded dirty-set work,
  - avoid reintroducing broad `every frame -> every entity` scans unless no narrower contract is possible.

Update note (2026-03-15, later):
- Initial implementation work has started.
- The local player control-resolution path in `bins/sidereal-client/src/runtime/replication.rs` now prefers the predicted player-anchor clone instead of depending on the canonical runtime registry entry for first-join control binding.
- The replication visibility path in `bins/sidereal-replication/src/replication/visibility.rs` now forces a targeted resend of current spatial motion components when a client newly gains visibility to a spatial entity. This is intended to close the observed “stationary entity stays at origin until later movement delta” gap without introducing a broad client-side repair scan.
- These are not treated as final closeout. The next required validation step is the Phase 0 / Phase 1 late-join stationary repro: remote ship stationary, asteroids stationary, no movement until after observation.

## 1. Purpose

Stabilize Sidereal's multiplayer runtime so that:

1. late join with stationary replicated entities is correct,
2. local control attach is correct on first join,
3. predicted / interpolated / confirmed presentation ownership has one explicit contract,
4. future optimization work does not keep re-breaking multiplayer correctness,
5. steady-state runtime cost stays change-driven rather than depending on whole-world polling.

This is not a pure Lightyear migration plan and not a pure rendering plan. It is a reliability plan for the hybrid runtime Sidereal actually has today.

## 2. Read Before Editing

Read these in this order before making code changes:

1. `AGENTS.md`
2. `docs/reports/lightyear_multiplayer_prediction_interpolation_audit_report_2026-03-15.md`
3. `docs/plans/lightyear_runtime_alignment_implementation_plan.md`
4. `docs/features/prediction_runtime_tuning_and_validation.md`
5. `docs/features/lightyear_upstream_issue_snapshot.md`
6. `docs/reports/native_runtime_system_ownership_audit_2026-03-09.md`
7. `docs/features/visibility_replication_contract.md`
8. `README.md`

If the intended code change alters runtime behavior or ownership rules, update the relevant docs in the same change. Do not leave the contract implicit in code only.

## 3. Problem Statement

The current audit shows that the runtime is failing at two different layers:

1. **Initial replicated state correctness**
   - some stationary confirmed entities have a correct authoritative server `Position`, but the client root remains at `Position = [0,0]` until a later movement delta arrives.
   - This affects remote ships and static/stationary world objects such as asteroids.

2. **Client lane resolution correctness**
   - the local ship can already exist as `Predicted` and still fail to bind correctly to HUD/control/presentation logic.
   - newer duplicate/presentation cleanup made more systems depend on a stronger winner-selection contract than the runtime lifecycle could actually guarantee.

The branch also has a process gap:

- reliability-sensitive cleanup work was allowed to proceed without a repeatable multiplayer validation gate.

## 4. Non-Negotiable Guardrails

### 4.1 Authority and Architecture

1. Keep server-authoritative simulation and transport-authenticated input routing.
2. Do not replace Sidereal's control-transfer model with a pretend “stock Lightyear example” contract.
3. Keep shared gameplay/prediction/runtime logic shared between native and WASM where possible.
4. Preserve current visibility / redaction semantics.

### 4.2 Performance Rule

The project should not “solve” reliability by falling back to constant broad scans.

Required rule:

1. Prefer event-driven or dirty-driven updates.
2. Prefer GUID-indexed caches and targeted sets over whole-query steady-state arbitration.
3. Prefer one canonical cached mapping per concern over repeated ad hoc world scans in multiple systems.
4. Only accept `every frame -> every entity` work where:
   - the entity set is intentionally tiny and bounded, or
   - there is no correct narrower signal available.

Examples of acceptable patterns:

1. dirty GUID set for duplicate presentation recompute,
2. cached player-anchor / controlled GUID resolution,
3. targeted adoption retries for entities missing bootstrap state,
4. targeted visibility/client bucket maintenance on membership changes.

Examples of patterns to avoid:

1. every frame full-world GUID winner arbitration,
2. every frame whole-world control target rediscovery,
3. every frame HUD scans that rebuild the same target set from scratch,
4. every frame root-entity spatial repair across all replicated entities.

### 4.3 Reliability Rule

No phase in this plan is complete until the required multiplayer regression checklist passes. Performance gains do not override correctness failures.

## 5. Desired End State

The runtime should converge to these stable rules:

1. A stationary replicated entity late-joining into relevance appears immediately at the correct position and rotation.
2. The local player's first controlled entity binds once and consistently:
   - input,
   - motion authority,
   - camera,
   - HUD,
   - debug overlay.
3. Presentation ownership is explicit:
   - control/HUD owner,
   - visual presentation owner,
   - debug classification owner,
   - bootstrap fallback owner.
4. Duplicate predicted/interpolated/confirmed clone resolution is change-driven and contract-driven, not inferred differently by multiple systems.
5. A fresh optimization pass can build on this runtime without re-opening multiplayer bootstrap bugs.

## 6. Required Execution Order

Execute phases in this order:

1. Phase 0: Freeze and measure the repro
2. Phase 1: Fix stationary replicated-state bootstrap
3. Phase 2: Define the lane-ownership contract
4. Phase 3: Implement control/HUD/presentation resolution against that contract
5. Phase 4: Replace broad arbitration with change-driven caches
6. Phase 5: Add multiplayer regression coverage and replayable validation
7. Phase 6: Re-measure and only then resume further optimization work

Do not skip to Phase 4 or Phase 6 before Phase 1 and Phase 2 are complete.

## 7. Phase 0: Freeze and Measure the Repro

Goal:

Create a stable baseline for the exact failures now occurring.

Primary files:

1. `bins/sidereal-client/src/runtime/debug_overlay.rs`
2. `bins/sidereal-client/src/runtime/replication.rs`
3. `bins/sidereal-client/src/runtime/transforms.rs`
4. `bins/sidereal-client/src/runtime/visuals.rs`
5. `bins/sidereal-client/src/runtime/ui.rs`
6. `docs/reports/lightyear_multiplayer_prediction_interpolation_audit_report_2026-03-15.md`

Tasks:

1. Record one reproducible native validation script for:
   - client 1 joins with owned ship,
   - client 2 already in world and stationary,
   - visible asteroids/landmarks already stationary,
   - no ship movement until after observation.
2. Capture baseline evidence for:
   - local controlled ship lane,
   - remote ship lane,
   - stationary asteroid position on server,
   - stationary asteroid position on client.
3. Make sure the current debug overlay exposes:
   - predicted / confirmed / interpolated counts,
   - control lane,
   - duplicate winner swaps,
   - anomalies.
4. Save one dated status note after baseline capture so later phases can compare against it.

Acceptance criteria:

1. The team can re-run the same repro after each phase.
2. The exact “stationary at origin until movement” case is captured, not just described.

## 8. Phase 1: Fix Stationary Replicated-State Bootstrap

Goal:

Make late-join stationary replicated entities appear correctly before any movement delta occurs.

Primary suspected areas:

1. `bins/sidereal-replication/src/replication/visibility.rs`
2. `bins/sidereal-client/src/runtime/replication.rs`
3. `bins/sidereal-client/src/runtime/transforms.rs`
4. `crates/sidereal-net/src/lightyear_protocol/registration.rs`
5. any Lightyear/Avian bootstrap or gain-visibility resend path touched by those files

Key question to resolve:

Why can a server entity have a correct authoritative `Position` while the client root for the same GUID remains at `Position = [0,0]` and `Transform.translation = [0,0,0]` until movement?

Tasks:

1. Trace the full late-join path for a stationary confirmed entity:
   - visibility gain on replication server,
   - full-state component send,
   - client spawn/adoption,
   - initial transform/bootstrap sync.
2. Verify whether stationary confirmed entities receive:
   - initial `Position`,
   - initial `Rotation`,
   - any required `Confirmed<T>` state,
   - any Lightyear/Avian bootstrap required for transform sync.
3. Determine whether the missing/corrupt initial state is caused by:
   - server visibility resend behavior,
   - client adoption timing,
   - Lightyear/Avian initial component hydration,
   - later client-side overwrite/reset.
4. Fix the issue with the narrowest correct change.
5. Add targeted tests for:
   - stationary late-join ship bootstrap,
   - stationary late-join asteroid/bootstrap,
   - no dependency on later movement to become visible in the right place.

Implementation constraints:

1. Do not add a broad per-frame spatial repair scan over all replicated roots.
2. If a temporary repair path is needed, it must be bounded:
   - only newly adopted entities,
   - only entities missing bootstrap completion,
   - removed once bootstrap succeeds.

Acceptance criteria:

1. Stationary remote ships appear at the correct position immediately on late join.
2. Stationary asteroids/landmarks appear at the correct position immediately on late join.
3. No movement is required to “unstick” initial placement.

## 9. Phase 2: Define the Lane-Ownership Contract

Goal:

Make the runtime contract explicit before more cleanup continues.

Contract to document:

1. Which lane owns local control intent?
2. Which lane owns simulation writing?
3. Which lane owns HUD/control state?
4. Which lane owns visual presentation?
5. Which lane owns debug classification?
6. What fallback is allowed before interpolated history is ready?
7. How does predicted -> observer transition work?
8. How does observer -> predicted transition work?
9. How does the persisted player anchor participate in all of the above?

Primary files/docs:

1. `docs/features/prediction_runtime_tuning_and_validation.md`
2. new or updated feature-scoped contract doc under `docs/features/`
3. `bins/sidereal-client/src/runtime/replication.rs`
4. `bins/sidereal-client/src/runtime/transforms.rs`
5. `bins/sidereal-client/src/runtime/visuals.rs`
6. `bins/sidereal-client/src/runtime/debug_overlay.rs`
7. `bins/sidereal-client/src/runtime/camera.rs`
8. `bins/sidereal-client/src/runtime/ui.rs`

Tasks:

1. Write the contract doc first.
2. Explicitly separate:
   - canonical runtime entity ID mapping,
   - local player authoritative control resolution,
   - canonical visual presentation owner,
   - debug overlay lane reporting.
3. Document which of those may intentionally resolve to different clones at different lifecycle stages.
4. Reference upstream Lightyear constraints where relevant:
   - `#1034` PredictionSwitching
   - `#1380` required-component interpolation issue
   - PR `#1421` confirmed-history initialization on `Interpolated` add

Acceptance criteria:

1. A developer can answer “which clone should this system read?” without reverse-engineering several files.
2. Future cleanup work can be judged against a written contract instead of intuition.

## 10. Phase 3: Implement Control / HUD / Presentation Resolution Against The Contract

Goal:

Make all local client systems resolve against the same lifecycle rules.

Primary files:

1. `bins/sidereal-client/src/runtime/replication.rs`
2. `bins/sidereal-client/src/runtime/transforms.rs`
3. `bins/sidereal-client/src/runtime/visuals.rs`
4. `bins/sidereal-client/src/runtime/debug_overlay.rs`
5. `bins/sidereal-client/src/runtime/camera.rs`
6. `bins/sidereal-client/src/runtime/ui.rs`
7. `bins/sidereal-client/src/runtime/motion.rs`
8. `bins/sidereal-client/src/runtime/input.rs`

Tasks:

1. Land the local player control-resolution fix cleanly:
   - local player anchor lookup must not depend on the wrong runtime registry assumption.
2. Split runtime concerns so they do not all depend on one over-broad “canonical” marker:
   - control binding may need one resolution path,
   - visual winner may need another,
   - debug overlay may report both.
3. Tighten interpolated readiness rules so placeholder state does not become visible too early.
4. Ensure predicted local ship, camera follow, HUD, and debug all agree immediately on join.
5. Ensure remote observer ships do not stay on the confirmed lane longer than intended once interpolation is genuinely ready.

Implementation constraints:

1. Do not put back every-frame whole-world GUID arbitration in multiple systems.
2. If one central resolver is added, it must be:
   - dirty-driven where possible,
   - cached by GUID,
   - explicit about lifecycle stage.

Acceptance criteria:

1. The local ship is predicted and fully bound on first join.
2. Clicking the owned ship menu is not required to make HUD/control state “catch up.”
3. The debug overlay no longer reports false local/remote predicted anomalies in the steady state.

## 11. Phase 4: Replace Broad Arbitration With Change-Driven Caches

Goal:

Keep the reliability fixes without sliding back into expensive steady-state scans.

Primary candidates:

1. duplicate visual resolution in `bins/sidereal-client/src/runtime/visuals.rs`
2. control-target and player-anchor resolution in `bins/sidereal-client/src/runtime/replication.rs`
3. HUD/nameplate target selection in `bins/sidereal-client/src/runtime/ui.rs`
4. debug entity classification in `bins/sidereal-client/src/runtime/debug_overlay.rs`

Tasks:

1. Introduce explicit per-GUID or per-runtime-ID caches where concern-specific resolution is needed.
2. Drive cache invalidation from:
   - component add/remove,
   - ownership marker changes,
   - transform/bootstrap readiness changes,
   - control-target changes.
3. Remove broad re-discovery scans once the cache path is correct and covered.
4. For nameplates and similar UI paths, reuse canonical target sets instead of recomputing from world queries in multiple systems.

Acceptance criteria:

1. Steady-state frames do not perform repeated whole-world clone arbitration when nothing relevant changed.
2. Control/presentation correctness remains intact under join, swap, and remote movement.
3. Performance gains from earlier cleanup are preserved or improved.

## 12. Phase 5: Add Multiplayer Regression Coverage

Goal:

Make this class of regression hard to reintroduce.

Required coverage types:

1. targeted Rust tests where practical,
2. repeatable multiplayer repro checklist,
3. BRP snapshot validation for key scenarios.

Tasks:

1. Add targeted tests for:
   - local player control resolution,
   - predicted/interpolated readiness rules,
   - duplicate winner resolution when readiness changes,
   - stationary bootstrap once the root cause is fixed.
2. Add a documented validation checklist under `docs/features/` or this plan for:
   - client joins with own ship controlled,
   - remote ship stationary on join,
   - stationary asteroids/landmarks visible on join,
   - observer movement after stationary bootstrap,
   - control swap handoff if relevant.
3. Where practical, add tooling or scripts that make BRP comparison less manual.

Acceptance criteria:

1. Future runtime cleanup must pass a concrete multiplayer checklist instead of a vague “it seemed smoother” validation.
2. At least the most recent regressions in the audit are locked by tests or deterministic repro steps.

## 13. Phase 6: Re-Measure Before Resuming Further Optimization

Goal:

Only resume deeper cleanup or optimization work after the runtime is correct again.

Tasks:

1. Re-run the Phase 0 baseline after Phases 1 through 5.
2. Compare:
   - local control attach correctness,
   - remote ship lane correctness,
   - stationary bootstrap correctness,
   - duplicate winner churn,
   - relevant HUD/nameplate counters,
   - any newly added cache invalidation counters.
3. Update:
   - this plan with a dated status note,
   - the multiplayer audit report if conclusions materially change,
   - any feature contract docs touched by the implementation.

Acceptance criteria:

1. Reliability is restored.
2. The branch still avoids broad steady-state scans.
3. The next optimization pass has an honest, updated baseline.

## 14. Mandatory Multiplayer Validation Checklist

No phase touching prediction/interpolation/presentation/control is complete unless this checklist passes:

1. Client joins and its owned ship is immediately recognized as the controlled predicted entity.
2. Input, camera, HUD, and debug overlay all agree about the controlled ship on first join.
3. A second client already in world and stationary appears immediately in the correct place.
4. Stationary asteroids/landmarks already in relevance appear immediately in the correct place.
5. After remote movement begins, the observer lane becomes smooth without requiring a second attach or menu interaction.
6. No root entity remains stuck at origin waiting for a later movement delta.
7. No “remote guid ... resolved as predicted” anomaly remains for the local player's own controlled entity in steady state.

## 15. Immediate Task List

The immediate next implementation tasks from this plan are:

1. Capture and record the exact late-join stationary bootstrap repro from Phase 0.
2. Trace the full replication server -> client bootstrap path for one affected asteroid GUID.
3. Fix the stationary bootstrap root cause without adding a whole-world repair scan.
4. Commit the local player control-resolution fix only after it is validated against the corrected bootstrap path.
5. Write the lane-ownership contract doc before continuing broader presentation cleanup.

### 2026-03-15 Status Note

Initial implementation work has started.

- Client control resolution no longer waits on canonical runtime-registry ownership before accepting the authoritative `ControlledEntityGuid` from the local player anchor. If the anchor says "control ship GUID X" before X has a registry entry, the client now keeps X as the provisional control target and lets GUID-based predicted-clone resolution bind control/HUD correctly.
- A matching control-tag fallback was added so `ControlledEntity`/`SimulationMotionWriter` binding can resolve directly from the raw GUID during bootstrap instead of silently behaving like free-roam.
- Targeted client tests now cover both provisional-control cases.

Remaining Phase 1 priority is unchanged:

- The stationary remote/bootstrap bug is still not closed. Current evidence shows at least one observer/interpolated entity can still exist client-side with default spatial state at login, so the next implementation step remains tracing why the initial authoritative pose is absent or not being applied on the client.

### 2026-03-15 Implementation Note

The control/bootstrap path has been realigned to reduce Lightyear divergence:

- The client no longer adds its own `Connected/RemoteId/LocalId` repair hook after `Linked`. Instead, it relies on Lightyear's raw-connection lifecycle and only adds an explicit `InputTimelineConfig` with Sidereal's chosen no-input-delay policy when the Lightyear `Client` component appears.
- Server control/auth/bootstrap no longer queue deferred `PendingControlledByBindings` and no longer cycle `lose_visibility()/gain_visibility()` to force sender-local respawns during handoff.
- Control ownership is now applied by one authoritative reconciliation pass before `ReplicationBufferSystems::BeforeBuffer`. Auth/bootstrap/control update only the authoritative player->controlled-entity mapping; Lightyear-facing `ControlledBy`, `Replicate`, `PredictionTarget`, and `InterpolationTarget` are derived from that state in one place.
- Targeted regression tests now cover both the owner-predicted controlled-ship case and stale-owner cleanup after a binding disappears.

This closes the specific architecture gaps that were keeping transport/bootstrap/control in a partially self-repaired state. It does not, by itself, prove that the stationary-at-origin observer bootstrap bug is fixed in live play; that still requires live multi-client verification against the checklist above.

### 2026-03-15 Observer Anchor Follow-Up

Live verification after the control/bootstrap redesign exposed a separate server-side visibility contract bug:

- the local controlled ship now boots correctly,
- but remote tactical visibility was still being evaluated from the persisted player-anchor transform,
- which can legitimately remain at `0,0` while the currently controlled ship has moved.

The server observer-anchor path now needs to resolve through `PlayerControlledEntityMap` first and only fall back to the player anchor for free-roam or incomplete bootstrap. This matches the design-doc rule that the player/observer anchor follows the controlled entity during active ship control.

### 2026-03-15 Visibility Registration Follow-Up

Later live BRP dumps showed a second server bootstrap-ordering issue:

- authenticated clients were entering `ClientVisibilityRegistry` during auth,
- but controlled-ship `PredictionTarget` / `InterpolationTarget` reconciliation was still happening later in the frame,
- so the first clone the owner saw for their ship could be the observer/interpolated lane instead of the owner-predicted lane.

The registration path now needs to be delayed until after control-role reconciliation in the normal `Update` flow so the first visibility gain for a controlled ship uses the correct Lightyear role assignment.

### 2026-03-15 Observer Interpolation Target Follow-Up

The next late-join BRP comparison exposed a remaining asymmetry in server role delivery for controlled ships:

- one client could see a remote controlled ship only as a confirmed-style clone,
- another client could see a remote controlled ship only as an interpolated clone at `0,0`,
- the server world state itself still had the correct non-zero authoritative positions for both ships.

The control reconciler was still deriving controlled-ship observer interpolation from `RemoteId -> NetworkTarget::AllExceptSingle(...)`, while owner `Replicate` and owner `PredictionTarget` were already using sender-entity manual targets. That meant bootstrap timing could temporarily leave `InterpolationTarget` absent for a controlled ship even though the owner binding already existed, producing a one-sided lane split at visibility gain.

The server now needs to derive controlled-ship observer interpolation from the authenticated sender entities directly:

- owner lane stays on manual sender-entity targeting,
- observer interpolation target is the current set of authenticated client entities except the owner,
- interpolation no longer depends on `RemoteId` readiness at the exact handoff/bootstrap moment.

This should remove the remaining confirmed-vs-interpolated asymmetry at spawn. It does not yet prove the interpolated `0,0` bootstrap itself is solved; that still needs live verification after rebuild/restart.

### 2026-03-15 Canonical Pose Bootstrap Follow-Up

After the observer interpolation target change, live repros still showed both clients receiving the remote ship as an interpolated clone pinned at `0,0`, while the replication server still held the correct authoritative non-zero pose for the same GUID.

That points to a narrower client bootstrap problem:

- the interpolated clone can appear before it has usable interpolation history,
- current pose on that clone can still be the default origin,
- but the canonical confirmed clone for the same `EntityGuid` may already exist locally with the correct authoritative pose.

The client transform/bootstrap path now needs to use that canonical confirmed clone as the fallback authority for initial interpolated rendering:

- `sync_interpolated_world_entity_transforms_without_history` should seed from the canonical confirmed clone before falling back to the interpolated clone's own current pose,
- `reveal_world_entities_when_initial_transform_ready` should allow initial reveal from that same canonical confirmed pose instead of waiting for history or showing origin.

This keeps the fix narrow and avoids reintroducing a broad per-frame whole-world scan: the lookup stays GUID-scoped through `RuntimeEntityHierarchy`.

### 2026-03-16 Lightyear Visibility Rearm Follow-Up

The latest control-path logs narrowed one of the remaining failures further:

- client control requests were leaving the client,
- server control handoff was resolving and ACKing them,
- client ACK handling was updating the authoritative controlled GUID,
- but the owner `Predicted` clone still was not appearing after a ship handoff.

At that point the request/ACK path was no longer the problem. The remaining gap was post-ACK lane transition.

The relevant upstream constraint is in Lightyear's visibility state handling: per-sender visibility entries retain predicted/interpolated spawn flags and `gain_visibility()` is the path that reapplies them on re-entry. That means changing `PredictionTarget` / `InterpolationTarget` on an already-visible entity is not enough by itself for dynamic handoff; the entity must be re-armed for the affected visible clients so Lightyear emits a fresh spawn with the new lane flags.

The server-side control reconciler now needs to treat role changes as a first-class visibility event:

- when `ControlledBy`, `Replicate`, `PredictionTarget`, or `InterpolationTarget` changes on a player anchor or controlled ship,
- gather the currently visible clients for that entity from `VisibilityMembershipCache`,
- for those currently visible clients only, call `lose_visibility()` then `gain_visibility()` on the entity's `ReplicationState`.

This keeps the rearm narrow and deterministic:

- no broad world rescan,
- no global visibility churn,
- only entities whose owner/observer lane assignment actually changed are re-spawned to the already-visible clients that need the new flags.

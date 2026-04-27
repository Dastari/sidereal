# Native Focus-Loss Prediction Recovery Plan

Status note 2026-04-23: Active implementation plan. This plan addresses the current native-client issue where alt-tab/focus loss can leave client prediction/input far enough behind or out of phase that rollback/reconciliation cannot recover cleanly.

Status note 2026-04-24: Implementation started with the narrow recovery-state/input-quiescence slice. The direct predicted-state realignment phase is constrained to the active local predicted root only, requires fresh confirmed state, and must not use a broad prediction-manager reset as a substitute for scoped recovery.

Status note 2026-04-24 follow-up: First implementation slice is now in place. `NativePredictionRecoveryState` / `NativePredictionRecoveryTuning` are initialized in the client runtime; focus transitions force a neutral realtime input boundary; refocus after `SIDEREAL_CLIENT_FOCUS_RECOVERY_MIN_UNFOCUSED_S` enters a short active-input suppression window; debug overlay/logs expose recovery phase, suppression state, last unfocused duration, transition count, and forced-neutral send count. Direct predicted Avian realignment remains intentionally unimplemented pending native repro logs with these diagnostics.

Status note 2026-04-26 follow-up: Two native clients running side by side showed that the zero unfocused prediction window caused artificial test-only stalls and snapbacks because only one OS window can be focused at a time. The default was changed so unfocused clients preserve the focused prediction budget while still sending neutral input boundaries; `SIDEREAL_CLIENT_UNFOCUSED_MAX_PREDICTED_TICKS=0` remains available for strict focus-stall diagnostics.

## 1. Problem

Sidereal already has focus-aware tuning:

1. unfocused client input is neutralized in `bins/sidereal-client/src/runtime/input.rs`,
2. stale realtime input expires on the replication server,
3. native input delay is non-zero,
4. unfocused max predicted ticks defaults to zero,
5. debug overlay tracks window focus and controlled tick gap.

That reduces stale held input and excessive prediction lead, but it does not fully solve the failure mode where the native client is backgrounded long enough that:

1. Bevy/OS scheduling throttles or pauses client updates,
2. Lightyear client input/prediction timeline falls behind or loses useful local/confirmed alignment,
3. the client returns with a gap larger than the configured rollback budget,
4. local prediction cannot reconcile gracefully.

The target fix is not to keep raising `SIDEREAL_CLIENT_MAX_ROLLBACK_TICKS`. Long focus stalls should produce a deliberate prediction recovery/resync path, not hundreds of ticks of rollback.

## 2. Target Standard

On native focus loss/regain:

1. The server must not keep simulating stale held input.
2. The client must not keep applying active local input while unfocused.
3. On focus regain after a meaningful stall, the client must recover prediction from the latest confirmed authoritative state instead of expecting rollback to bridge the entire gap.
4. Recovery must preserve server authority and single-writer motion ownership.
5. Recovery must not mutate observer/interpolated clones as if they were local predicted writers.
6. Recovery must not cause a control-target switch or erase the authoritative persisted controlled entity.
7. WASM/shared runtime impact must be explicit; native-specific behavior must stay behind platform/runtime boundaries when needed.

## 3. Current Code Touchpoints

Primary client files:

1. `bins/sidereal-client/src/runtime/transport.rs`
   - configures `InputTimelineConfig`,
   - adapts timeline tuning when `Window.focused` changes.
2. `bins/sidereal-client/src/runtime/input.rs`
   - neutralizes input when unfocused,
   - sends realtime input heartbeats,
   - inserts local `ActionState<PlayerInput>` / `InputMarker<PlayerInput>`.
3. `bins/sidereal-client/src/runtime/replication.rs`
   - configures `PredictionManager`,
   - tracks controlled bootstrap state,
   - has access to predicted/confirmed components.
4. `bins/sidereal-client/src/runtime/motion.rs`
   - enforces local motion writer ownership,
   - gates controlled prediction against bootstrap state.
5. `bins/sidereal-client/src/runtime/resources.rs`
   - owns input timeline tuning,
   - owns prediction correction tuning,
   - debug resources already track focus/tick gap.
6. `bins/sidereal-client/src/runtime/debug_overlay.rs`
   - exposes `Window Focus`, `Ctrl TickGap`, and related diagnostics.

Server files:

1. `bins/sidereal-replication/src/replication/input.rs`
   - expires stale realtime intent,
   - applies latest input to authoritative `ActionQueue`.
2. `bins/sidereal-replication/src/replication/health.rs`
   - exposes stale input and fixed-tick diagnostics.

Related docs:

1. `docs/features/prediction_runtime_tuning_and_validation.md`
2. `docs/plans/prediction_handoff_avian_visibility_foundation_refactor_plan_2026-03-22.md`
3. `docs/reports/client_server_network_audit_report_2026-04-23.md`

## 4. Design Direction

Introduce an explicit native focus recovery state, separate from controlled-entity handoff state.

Example shape:

```rust
pub enum NativePredictionRecoveryPhase {
    Focused,
    Unfocused {
        started_at_s: f64,
    },
    Recovering {
        regain_at_s: f64,
        suppress_input_until_s: f64,
        reason: PredictionRecoveryReason,
    },
}

pub enum PredictionRecoveryReason {
    FocusStall,
    RollbackGapExceeded,
    ConfirmedTickGapExceeded,
    ConfirmedStateMissing,
}
```

The exact type names can change. The important contract is:

1. focus state is explicit,
2. recovery is time-bounded,
3. input suppression is intentional,
4. predicted state is realigned from confirmed authoritative state before active input resumes.

## 5. Phase Plan

### Phase 0: Instrument The Failure

Goal: make the alt-tab failure measurable before changing recovery behavior.

Status 2026-04-24: Partially implemented. Client recovery-phase diagnostics and focus transition logs are present. Manual native repro logs for 5/15/30 second alt-tab cases are still required before enabling direct predicted-state realignment.

Tasks:

1. Add/verify client diagnostics for:
   - focused/unfocused transitions,
   - unfocused duration,
   - fixed ticks skipped or accumulated after focus regain if available,
   - controlled tick gap at focus loss and regain,
   - rollback abort count or rollback-gap warnings if available from Lightyear diagnostics,
   - whether active input is suppressed.
2. Add log lines on focus loss and focus regain with:
   - controlled entity id,
   - control generation,
   - current bootstrap phase,
   - focused/unfocused max predicted ticks,
   - rollback budget.
3. Document a manual repro:
   - native client enters world,
   - apply thrust/turn,
   - alt-tab for 5, 15, and 30 seconds,
   - refocus and immediately release/apply input,
   - record `Ctrl TickGap`, rollback logs, and local/confirmed positions.

Exit criteria:

1. We can distinguish "server kept stale input" from "client timeline cannot recover". In progress: recovery/input diagnostics exist; native repro evidence still needed.
2. We can reproduce and measure the failure in logs without relying only on feel. Pending native repro run.

### Phase 1: Input Quiescence On Focus Loss

Goal: make focus loss a clean input boundary.

Status 2026-04-24: Implemented in the client first slice. Focus transitions set a pending forced-neutral send; the input writer treats that as an active-input suppression boundary and clears the pending flag only after sending the neutral realtime packet.

Tasks:

1. On focus loss, force an immediate neutral realtime input send if connected and in-world.
2. Reset local last-sent input state enough that focus regain sends a fresh neutral/active transition rather than waiting for heartbeat state.
3. Suppress local `ActionState<PlayerInput>` active actions while unfocused.
4. Confirm server stale-input expiration still clears intent if the neutral packet is not delivered.

Exit criteria:

1. Server authoritative entity stops receiving held thrust/fire within the configured timeout after focus loss. Pending native/server log validation.
2. No active local input is applied while the window is unfocused. Implemented client-side; pending live validation.

### Phase 2: Focus-Regain Recovery Window

Goal: avoid immediately resuming prediction from stale local timeline state.

Status 2026-04-24: Implemented for input suppression only. Refocus after `SIDEREAL_CLIENT_FOCUS_RECOVERY_MIN_UNFOCUSED_S` enters `Recovering(FocusStall)` until `SIDEREAL_CLIENT_FOCUS_RECOVERY_SUPPRESS_INPUT_S` elapses. The first post-focus input packet is forced neutral even for shorter focus blips.

Tasks:

1. On focus regain after `SIDEREAL_CLIENT_FOCUS_RECOVERY_MIN_UNFOCUSED_S` seconds, enter a short recovery window.
2. During recovery:
   - send neutral input immediately,
   - suppress active keyboard input,
   - keep control target and selected target unchanged,
   - keep receiving replication.
3. Defaults:
   - `SIDEREAL_CLIENT_FOCUS_RECOVERY_MIN_UNFOCUSED_S=0.5`
   - `SIDEREAL_CLIENT_FOCUS_RECOVERY_SUPPRESS_INPUT_S=0.15`
4. Add native CLI equivalents if these become long-lived tuning controls.

Exit criteria:

1. Refocus does not immediately inject active input into a stale local prediction timeline. Implemented client-side; pending native repro confirmation.
2. The user-visible delay is short and bounded. Implemented with default `0.15s` suppression; pending feel validation.

### Phase 3: Predicted State Realignment From Confirmed State

Goal: make long focus stalls recover by resyncing prediction rather than rolling back across the whole stall.

Status 2026-04-24: Not implemented. Threshold tuning is parsed and logged, but no Avian motion components are copied yet. Next step is to run native repro with the new diagnostics and only implement this if the measured tick gap/rollback behavior still requires explicit realignment.

Tasks:

1. Identify the active controlled predicted root from `ControlBootstrapState`.
   - Require `ControlBootstrapPhase::ActivePredicted`.
   - Require the same entity to still carry the local predicted/control writer lane (`Predicted`, `ControlledEntity`, and `SimulationMotionWriter`) before any local motion write.
   - Do not resolve the write target by GUID alone.
2. Read latest available confirmed authoritative Avian state for that predicted root:
   - `Confirmed<Position>`,
   - `Confirmed<Rotation>`,
   - `Confirmed<LinearVelocity>`,
   - `Confirmed<AngularVelocity>`.
   - Require the confirmed baseline to be newer than the recovery start or otherwise prove it is current enough for the local timeline.
3. During recovery, if the controlled tick gap exceeds a threshold or unfocused duration exceeds a threshold:
   - copy confirmed Avian motion into the predicted root's local Avian components,
   - clear or neutralize local pending `ActionState<PlayerInput>`,
   - reset local visual interpolation baseline for the controlled entity if needed,
   - leave authoritative server state untouched.
4. If fresh confirmed state is missing, keep active input suppressed briefly and record `ConfirmedStateMissing` rather than snapping to a stale/default baseline.
5. Make this explicitly scoped to the local controlled predicted root only.
6. Do not apply this to interpolated observer clones or confirmed-only fallback entities.
7. Run the resync outside rollback and before the local predicted input/action application for that frame so stale `ActionState` cannot immediately reapply pre-focus intent.

Candidate thresholds:

1. `SIDEREAL_CLIENT_FOCUS_RECOVERY_RESYNC_AFTER_S=1.0`
2. `SIDEREAL_CLIENT_FOCUS_RECOVERY_MAX_TICK_GAP=60`

Exit criteria:

1. After a long alt-tab, the local controlled entity resumes from latest confirmed state without a rollback abort loop.
2. No observer clone or non-controlled entity receives local simulation writes.
3. Missing/stale confirmed state is visible in diagnostics and does not produce a local authoritative-looking snap to default state.

### Phase 4: Timeline Reconfiguration And Optional Prediction Manager Reset

Goal: ensure Lightyear timeline settings do not preserve stale assumptions across focus transitions.

Status 2026-04-24: Existing timeline focus reconfiguration remains unchanged. No prediction-manager reset has been added.

Tasks:

1. Keep existing behavior that sets unfocused `maximum_predicted_ticks=0`.
2. On focus regain recovery completion, reinsert the focused `InputTimelineConfig`.
3. Investigate whether the active `PredictionManager` exposes a safe reset/clear-history operation.
4. If a reset exists and is appropriate, use it only during explicit focus recovery, not as a general correction path.
5. If no safe reset exists, document that recovery is done by controlled predicted Avian state realignment only.
6. Do not use a broad/client-global reset to paper over one controlled-entity focus recovery unless Lightyear exposes semantics that preserve unrelated prediction/handoff state.

Exit criteria:

1. Focus regain does not leave the client stuck in unfocused timeline config.
2. No broad prediction-history clearing happens outside explicit recovery.

### Phase 5: Tests And Repro Harness

Goal: lock the behavior so future audit/rewrite loops do not move the target.

Status 2026-04-24: Targeted compile/lint checks pass for the client slice. Dedicated recovery state-transition tests and native repro harness are still pending.

Tests to add:

1. Unit tests for focus recovery state transitions.
2. Unit tests for neutral-input forcing on focus loss/regain.
3. Unit tests for recovery threshold decisions.
4. Integration/manual validation for:
   - 5 second alt-tab while idle,
   - 5 second alt-tab while thrusting,
   - 15 second alt-tab while rotating/thrusting,
   - refocus then immediate opposite input,
   - refocus during `PendingPredicted` handoff,
   - two-client observer view while owner alt-tabs.

Acceptance criteria:

1. No rollback-abort spam after focus regain.
2. Controlled entity resumes from plausible confirmed state within one recovery window.
3. Server does not continue stale held input beyond the stale-input timeout.
4. `Ctrl TickGap` returns to nominal bounds after recovery.
5. Duplicate visual suppression and transform recovery do not spike persistently after focus regain.

## 6. Non-Goals

1. Do not raise rollback budget as the primary fix.
2. Do not change server authority flow.
3. Do not make clients authoritatively correct server motion.
4. Do not solve dynamic control handoff in this plan except where focus recovery intersects an existing pending/active control phase.
5. Do not add native-only gameplay logic that would block later WASM parity recovery.

## 7. Open Questions

1. Does Lightyear expose a safe API to reset input/prediction history for one predicted entity or one client timeline?
2. Can we read confirmed tick/timeline gap directly from Lightyear state, or do we keep using existing `Ctrl TickGap` diagnostics?
3. Should focus recovery trigger only on native, or should browser visibility/focus events eventually map into the same shared recovery state?
4. Should recovery state be surfaced in the debug overlay as `Focused`, `Unfocused`, `Recovering`, and `Recovered`?

## 8. Recommended First Implementation Slice

Start with a narrow, low-risk slice:

1. Add `NativePredictionRecoveryState` resource and diagnostics.
2. Force immediate neutral input on focus loss and focus regain.
3. Add a short focus-regain input suppression window.
4. Add debug overlay/log visibility for the recovery phase.

Status 2026-04-24: Complete for the code path and diagnostics. Validation completed:

1. `cargo fmt --all -- --check`
2. `cargo check -p sidereal-client`
3. `cargo clippy -p sidereal-client --all-targets -- -D warnings`
4. Targeted tests:
   - `cargo test -p sidereal-client snapshot_skips_explicitly_hidden_root_candidates`
   - `cargo test -p sidereal-client debug_overlay_sent_input_stays_in_primary_header_block`

Known validation caveat 2026-04-24: `cargo test -p sidereal-client` still has three pre-existing/non-slice failures in transform/visual tests:

1. `runtime::transforms::tests::reveal_keeps_dynamic_interpolated_entity_hidden_without_authoritative_pose`
2. `runtime::visuals::tests::observer_ballistic_projectile_uses_authoritative_spawn_pose_before_first_history_sample`
3. `runtime::visuals::tests::duplicate_visual_recomputes_when_interpolated_pose_changes_from_invalid_to_valid`

Only after native focus-loss repro logs are captured with the new recovery diagnostics should we add direct predicted-state realignment from `Confirmed<T>`.

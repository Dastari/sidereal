# Control Handoff Input and Prediction Stabilization Plan

Status: Active implementation plan  
Last updated: 2026-04-28  
Owners: client runtime + replication server + shared simulation  
Scope: dynamic controlled-entity handoff, free-roam/player-anchor control, Sidereal realtime input bridge, Lightyear predicted/interpolated lane activation, local prediction ownership  
Primary references:

1. `AGENTS.md`
2. `docs/sidereal_design_document.md`
3. `docs/features/prediction_runtime_tuning_and_validation.md`
4. `docs/features/lightyear_upstream_issue_snapshot.md`
5. `docs/features/visibility_replication_contract.md`
6. `docs/plans/prediction_parity_layer_and_input_auth_plan_2026-04-26.md`
7. `docs/plans/native_focus_loss_prediction_recovery_plan_2026-04-23.md`
8. `docs/plans/lightyear_control_handoff_and_visibility_refactor_plan_2026-03-22.md`

## 0. Implementation Status

2026-04-27 investigation note:

1. This is a plan only. It records the current code state after the recent prediction, input-history, free-roam, and session-generation fixes.
2. Current behavior still breaks when changing control between a simulated controllable root and free-roam/player-anchor control. Reported symptoms are:
   - local input stops driving prediction correctly,
   - the server reports input drops,
   - stale generation or target-mismatch behavior can appear around handoff,
   - the client can enter a weird state that does not recover without restarting or reconnecting.
3. The main current contradiction is that the client bootstrap/motion systems now correctly refuse confirmed/interpolated fallback control, but the client input sender still resolves fallback entities and can attach local input authority to them.
4. Native impact: this plan targets the current native-stabilization priority.
5. WASM impact: all gameplay/input/prediction logic touched by this plan must stay shared and target-compatible. Platform differences belong only at the transport/input-device boundary.

2026-04-28 implementation note:

1. Implemented: client realtime input authority now comes only from `ControlBootstrapState::ActivePredicted`; `send_lightyear_input_messages` no longer resolves confirmed/interpolated GUID fallbacks for input ownership.
2. Implemented: `enforce_single_input_marker_owner` removes `InputMarker<PlayerInput>`, `ActionState<PlayerInput>`, and `SimulationMotionWriter` from all non-active or pending-control entities instead of retaining a fallback target.
3. Implemented: the fixed-step predicted input bridge rejects stale/non-active markers before writing `ActionQueue`, so fallback markers cannot drive gameplay if they appear through a regression.
4. Implemented: control ACK/reject lease changes reset client input send state, pending input ACK ticks, and pending neutral recovery sends.
5. Implemented: replication accepted rebinds clear short-lived realtime input state through the same explicit per-player cleanup helper used by auth/disconnect paths, and neutralize both the previous target and the newly activated target before fresh input arrives.
6. Implemented: replication server startup now names the Lightyear protocol-only input plugin explicitly and continues to avoid `lightyear::input::native::InputPlugin` server receive/update runtime.
7. Native impact: handoff should now become inert while waiting for the exact predicted control lease instead of driving a confirmed/interpolated fallback and accumulating stale input drops.
8. WASM impact: shared client/runtime and replication-server behavior only; no target-specific branch was introduced.
9. Remaining work: validate repeated entity/free-roam/entity handoff and two-client visibility/movement in native runtime; if predicted clones still do not appear reliably after ACK, continue with the stable owner-predicted pool phase.

## 1. Purpose

Give a fresh agent enough detail to implement a real fix instead of another partial shim.

The desired result is:

1. switching from entity control to free roam is deterministic,
2. switching from free roam back to an entity is deterministic,
3. switching between two owned controllable entities is deterministic,
4. no local client system writes prediction input to a confirmed/interpolated fallback,
5. no stale input snapshot from a previous target can keep poisoning server-side input routing,
6. Lightyear lane changes are either made reliable or avoided by keeping a stable owner-predicted pool,
7. one and only one local entity owns `InputMarker<PlayerInput>` and `SimulationMotionWriter` at any time.

This plan intentionally treats "ship" as an example of a controlled entity. Generic systems should use entity/control-root terminology unless behavior is truly ship-only.

## 2. Current Code Map

Read these files first before editing:

1. `bins/sidereal-client/src/runtime/resources.rs`
   - `ClientInputAckTracker`, lines around 59-61.
   - `ClientControlRequestState`, lines around 91-98.
   - `DeferredPredictedAdoptionState`, lines around 874-891.
   - `ControlBootstrapPhase` / `ControlBootstrapState`, lines around 893-924.
   - `ClientInputSendState`, lines around 1072-1089.
2. `bins/sidereal-client/src/runtime/replication.rs`
   - `sync_controlled_entity_tags_system`, lines around 1788-1919.
   - This system is currently the strongest client-side control bootstrap guard.
3. `bins/sidereal-client/src/runtime/input.rs`
   - `send_lightyear_input_messages`, lines around 248-450.
   - `enforce_single_input_marker_owner`, lines around 511-557.
4. `bins/sidereal-client/src/runtime/motion.rs`
   - `apply_predicted_input_to_action_queue`.
   - `seed_controlled_predicted_motion_from_confirmed`.
   - `enforce_motion_ownership_for_world_entities`.
5. `bins/sidereal-client/src/runtime/plugins/replication_plugins.rs`
   - schedule ordering for view-state sync, control requests/results, input send, and prediction systems.
6. `bins/sidereal-client/src/runtime/app_setup.rs`
   - additional motion-ownership scheduling.
7. `bins/sidereal-client/src/runtime/control.rs`
   - `receive_lightyear_control_results`.
8. `bins/sidereal-replication/src/replication/control.rs`
   - `receive_client_control_requests`, lines around 154-501.
   - `reconcile_control_replication_roles`, lines around 666-888.
9. `bins/sidereal-replication/src/replication/input.rs`
   - `ClientInputTickTracker::clear_player`, lines around 29-33.
   - `LatestRealtimeInputsByPlayer`, lines around 75-78.
   - `RealtimeInputActivityByPlayer`, lines around 80-83.
   - stale-generation and target-mismatch clearing in the fixed-step input drain, lines around 570-635.
10. `bins/sidereal-replication/src/replication/auth.rs`
   - session-ready generation bootstrap and disconnect/session cleanup behavior.

## 3. Current Failure Mode

A typical broken handoff looks like this:

1. Client is actively predicting target A.
2. User changes control to free roam or target B.
3. Client sends a control request.
4. Server accepts, increments or confirms the control generation, updates `PlayerControlledEntityMap`, and sends an ACK.
5. Client `LocalPlayerViewState` updates to the new target and generation.
6. The target's predicted clone is not ready yet, or Lightyear does not produce it reliably after the dynamic role mutation.
7. `sync_controlled_entity_tags_system` correctly leaves the client in `PendingPredicted` and refuses to bind local control to a confirmed/interpolated fallback.
8. `send_lightyear_input_messages` does not consume `ControlBootstrapState`. It resolves the target through `resolve_entity_by_guid_prefer_predicted`, which may return an interpolated or confirmed fallback if no predicted clone exists.
9. `send_lightyear_input_messages` then inserts:
   - `SimulationMotionWriter`,
   - `InputMarker<PlayerInput>`,
   - `ActionState<PlayerInput>`,
   onto that fallback entity.
10. `enforce_single_input_marker_owner` uses the same fallback resolver, so it can keep that input marker alive.
11. Fixed-step prediction systems can now see a locally writable entity that the bootstrap/motion ownership systems were trying to reject.
12. Server-side input can still arrive with a new target/generation while the client local prediction lane is not actually active.
13. If old input snapshots, old tick streams, or durable intent state remain around, the server can report `stale_control_generation` or `controlled_target_mismatch`, clear queues, and then repeat confusing behavior across later ticks.

The key bug is not a single dropped packet. It is a state-machine split across several systems that do not use the same activation source.

## 4. Why The Recent Fixes Are Not Enough

Recent status notes in `docs/features/prediction_runtime_tuning_and_validation.md` say that free-roam/player-anchor control now follows the same predicted-readiness rule as entity control. That is true for the bootstrap and motion-ownership path, but it is not true for the input sender.

As of this investigation:

1. `ControlBootstrapState` has only:
   - `Idle`,
   - `PendingPredicted`,
   - `ActivePredicted`.
2. `sync_controlled_entity_tags_system` promotes to `ActivePredicted` only when it finds a `Predicted` clone.
3. `motion.rs` mostly respects `ControlBootstrapState` and refuses to promote confirmed/interpolated fallback entities.
4. `input.rs` still independently resolves target entities and can choose a fallback.
5. Server-side stale-input cleanup mostly happens when the fixed-step drain observes stale state. It is not yet a first-class part of the accepted control-rebind transaction.

That means the project has the right invariant documented, but not enforced at every writer.

## 5. Target Invariants

These invariants are the acceptance contract for the implementation.

1. Server authority remains one-way: client input -> shard sim -> replication/distribution -> persistence.
2. A client never authoritatively writes server state or replicated confirmed state.
3. The active control lease is identified by `(player_entity_id, controlled_entity_id, control_generation)`.
4. The client sends active input only for the currently active control lease.
5. The client applies local prediction input only to the currently active local prediction entity.
6. The client must not attach `InputMarker<PlayerInput>`, `ActionState<PlayerInput>`, or `SimulationMotionWriter` to a confirmed/interpolated fallback.
7. `LocalPlayerViewState.controlled_entity_id` is not enough to authorize local prediction. It is only the authoritative target ID. Local prediction activation requires `ControlBootstrapState`.
8. During `PendingPredicted`, local active input is suppressed. The client may send a bounded neutral boundary if needed, but must not apply active local prediction input.
9. Free-roam/player-anchor control must have an explicit activation lane:
   - preferred: an owner-predicted player anchor exists and uses the same `ActivePredicted` path,
   - acceptable fallback: a named `ActiveLocalAnchor` phase that is limited to the authenticated local player anchor and has its own reconciliation contract.
10. Exactly one local entity may have `InputMarker<PlayerInput>` at a time.
11. Exactly one local entity may have `SimulationMotionWriter` for the authenticated player at a time.
12. Server input validation remains strict. Do not "fix" this by accepting mismatched target IDs or stale control generations.
13. On accepted rebind, old input intent is neutralized and stale short-lived server input snapshots for that player are cleared.
14. Fixed-step simulation and prediction math continue using fixed-step time only.
15. Do not reintroduce legacy mirror motion components such as `PositionM`, `VelocityMps`, or `HeadingRad`.

## 6. Desired Handoff Sequence

This is the target lifecycle for every control change.

### 6.1 Active Target A

1. `ControlBootstrapState.phase = ActivePredicted { target_entity_id: A, generation: G, entity: predicted_A }`.
2. `predicted_A` has the only local `InputMarker<PlayerInput>`.
3. `predicted_A` has the only local `SimulationMotionWriter`.
4. `ClientRealtimeInputMessage` uses:
   - `player_entity_id = authenticated player`,
   - `controlled_entity_id = A`,
   - `control_generation = G`.
5. Server accepts only if the authenticated session, target, and generation match `PlayerControlledEntityMap` and `ClientControlLeaseGenerations`.

### 6.2 User Requests Target B Or Free Roam

1. Client sends a control request.
2. Client immediately creates an input boundary:
   - clear local active input markers/writers from target A,
   - reset last-sent input state enough that the next valid target does not inherit A's last actions,
   - optionally send one neutral packet for A if the connection and lease are still valid.
3. While the control request is pending, active input is suppressed.

### 6.3 Server Accepts Rebind

1. Server updates persisted `ControlledEntityGuid`.
2. Server updates `PlayerControlledEntityMap`.
3. Server increments `ClientControlLeaseGenerations` when the effective controlled entity changed.
4. Server neutralizes old target A.
5. Server also neutralizes target B at activation start if B could have stale durable intent from a previous control lease.
6. Server clears short-lived input state for the player:
   - `LatestRealtimeInputsByPlayer`,
   - `RealtimeInputActivityByPlayer`,
   - `ClientInputTickTracker`,
   - per-player rate-limit window if needed.
7. Server sends `ServerControlAckMessage` with target B and generation `G + 1`.

### 6.4 Client Receives ACK

1. Client updates `LocalPlayerViewState.controlled_entity_id`.
2. Client updates `LocalPlayerViewState.controlled_entity_generation`.
3. Client resets handoff-local input state:
   - `ClientInputSendState.last_sent_target_entity_id`,
   - `ClientInputSendState.last_sent_actions`,
   - `ClientInputSendState.last_sent_at_s`,
   - `ClientInputAckTracker.pending_ticks`,
   - any recovery neutral boundary that should not leak across the new generation.
4. Client transitions `ControlBootstrapState` to `PendingPredicted { target_entity_id: B, generation: G + 1 }`.
5. Client does not attach local input authority until a valid activation entity exists.

### 6.5 Target B Becomes Locally Predictable

1. If B has an owner-predicted clone, client transitions to `ActivePredicted`.
2. Bootstrap seeds motion from confirmed state exactly once for that generation.
3. Input sender uses the active bootstrap entity directly.
4. The first active input packet uses the new target and generation.
5. Prediction, server input, and confirmed correction are now on the same control lease.

## 7. Root Causes To Fix

### RC1: The input sender bypasses the control bootstrap state

`send_lightyear_input_messages` currently derives target ownership from `LocalPlayerViewState` and `resolve_entity_by_guid_prefer_predicted`. That resolver may return a non-predicted fallback.

Fix:

1. Add `ControlBootstrapState` to `send_lightyear_input_messages`.
2. Replace fallback resolution with a helper that returns an input-authorized local entity only when the bootstrap phase is active.
3. Do not insert `InputMarker<PlayerInput>`, `ActionState<PlayerInput>`, or `SimulationMotionWriter` unless the helper returns an active entity.
4. Treat `PendingPredicted` as input-suppressed.

### RC2: The marker cleanup system uses the same fallback resolver

`enforce_single_input_marker_owner` currently uses `resolve_entity_by_guid_prefer_predicted` and may keep markers on fallback entities.

Fix:

1. Add `ControlBootstrapState` to `enforce_single_input_marker_owner`.
2. Keep markers only on the entity named by:
   - `ControlBootstrapPhase::ActivePredicted { entity, .. }`, or
   - the explicit local-anchor activation phase if implemented.
3. Remove input markers and action state from every other entity.
4. Also remove `SimulationMotionWriter` from entities that are no longer active where ownership systems do not already do this in the same frame.

### RC3: Handoff does not reset all client-side input state

`receive_lightyear_control_results` updates view state and request state, but there is no single control-generation boundary that clears all local input sender and ack state.

Fix:

1. Introduce a small helper such as `reset_client_input_for_control_generation(...)`.
2. Call it on accepted ACK when target or generation changes.
3. Call it on rejected ACK if the authoritative target/generation differs from local state.
4. Clear pending ack ticks because ticks from the previous lease are no longer meaningful for the new stream.
5. Reset last-sent target/actions so the first valid packet after activation is sent immediately.

### RC4: Server input state is cleared reactively, not as part of rebind

`drain_realtime_player_inputs_to_action_queue` removes stale-generation and mismatched-target snapshots when it sees them, but an accepted control rebind should itself be the hard boundary.

Fix:

1. Move short-lived player input cleanup into a reusable helper in `replication/input.rs`.
2. Call that helper from `receive_client_control_requests` after an accepted rebind and before ACK flush.
3. Keep the drain-time cleanup as a safety net.
4. Add a metric/log field so we can see that rebind cleanup happened deliberately, not only as drop fallout.

### RC5: Durable intent can survive target swaps

The server already neutralizes the previous target on rebind. That protects against old target A continuing to thrust after leaving it. Target B can also carry stale durable intent from a previous control lease if it was not neutralized during an earlier failure path.

Fix:

1. On accepted rebind, neutralize the old target.
2. Also neutralize the newly resolved target before fresh input arrives when `rebind_required` is true.
3. Make the helper idempotent and safe for player-anchor/free-roam entities that do not have every ship-specific component.
4. Use generic names in any new helper, for example `queue_neutralize_control_intent`, not ship-specific terminology.

### RC6: Dynamic Lightyear role mutation may be the wrong primitive

`reconcile_control_replication_roles` currently changes `PredictionTarget` / `InterpolationTarget` based on the currently controlled entity and then rearms visible clients. If Lightyear does not reliably support changing the predicted target among already visible entities, Sidereal must stop depending on that dynamic spawn/despawn behavior for every handoff.

2026-04-28 update:

Latest native logs showed the accepted free-roam -> ship handoff reaching the server correctly: generation-2 ship input was accepted and the authoritative ship moved. The client failure happened when the already-visible interpolated ship was promoted into prediction and the old client-side mixed-marker sanitizer removed one role locally after Lightyear had already attached both `Predicted` and `Interpolated` sidecar state. The fix implemented for this phase is to make role rearm a staged server-side visibility transition: send a loss first, suppress re-gain for one membership pass, then let normal visibility respawn the entity with its new role. The client-side sanitizer was removed so mixed-role entities are no longer treated as a valid recoverable state.

2026-04-28 cleanup/test update:

1. Removed the remaining active client-side handoff repair systems that fabricated missing `Confirmed<T>` motion sidecars for interpolated entities, inserted a missing Lightyear `PredictionManager`, continuously synced interpolated transforms without Lightyear history, or repaired predicted/interpolated visual transforms in PostUpdate.
2. Kept only the hidden-until-ready presentation gate and pure diagnostics; invalid mixed-role topology should now fail visibly in logs/tests instead of being locally patched.
3. Tightened the existing two-headless-client motion e2e so, when the database fixture provides two mobile controlled entities and both clients become ready, changing remote presentation transforms are a hard assertion rather than an optional diagnostic.
4. Unit tests remain useful only for small invariants. The acceptance gate for this bug class must be an e2e/runtime test using real `sidereal-replication` and `sidereal-client` binaries, real Lightyear transport, and log/diagnostic assertions for control generation, predicted role spawn, input acceptance, and remote movement.

Fix direction:

1. First implement RC1-RC5. They are required regardless of Lightyear strategy.
2. Add diagnostics that prove whether a predicted clone appears within a bounded time after ACK.
3. If dynamic role mutation still fails, switch to a stable owner-predicted control pool:
   - for the authenticated connected player, keep currently relevant owned controllable roots owner-predicted,
   - keep the player anchor owner-predicted while the player is connected,
   - continue using Sidereal's control-generation state to decide which one receives local input authority,
   - do not expose non-owned or non-visible entities,
   - keep observer interpolation for other clients.
4. In that model, Lightyear no longer has to create a new predicted clone at every control swap. The predicted lane already exists; Sidereal only moves the input writer.

### RC7: Schedule ordering can allow one frame of mismatched state

Control ACK handling runs in `Update`. Input send runs in `FixedPreUpdate`. Bootstrap/tag sync also runs in `Update`. If ACK updates the view state before bootstrap activation has caught up, fixed input can see the new target/generation while the bootstrap phase is still pending or stale.

Fix:

1. Make `send_lightyear_input_messages` depend on `ControlBootstrapState`, not only `LocalPlayerViewState`.
2. Ensure `sync_controlled_entity_tags_system` runs after `receive_lightyear_control_results` or explicitly document why a one-frame pending state is acceptable.
3. Add tests or schedule assertions that the first fixed tick after ACK cannot attach input markers to fallback entities.

## 8. Proposed Implementation Phases

### Phase 1: Add diagnostics before changing behavior

Goal: make the failure visible in one short local repro.

Changes:

1. Add client logs behind `SIDEREAL_DEBUG_CONTROL_LOGS=1` or the existing debug-env mechanism for:
   - ACK received target/generation,
   - bootstrap phase transition,
   - active input authorized target/entity/generation,
   - active input suppressed reason,
   - fallback entity found but denied.
2. Add counters to the debug overlay if low risk:
   - control phase,
   - current target/generation,
   - active input entity,
   - pending predicted wait time,
   - denied fallback input count.
3. Add server logs behind `SIDEREAL_DEBUG_INPUT_DROP_LOGS=1` for:
   - accepted input target/generation,
   - stale generation,
   - target mismatch,
   - rebind cleanup count,
   - neutralized old/new target.

Expected result:

1. A handoff log should read as a single lease transition.
2. During pending, logs should say "input suppressed: pending predicted", not silently attach input to a fallback.

### Phase 2: Make `ControlBootstrapState` the only client input authority source

Goal: remove the split-brain client state.

Concrete changes:

1. In `bins/sidereal-client/src/runtime/input.rs`, add `control_bootstrap: Res<ControlBootstrapState>` to `send_lightyear_input_messages`.
2. Create a helper in `input.rs` or a small shared client runtime module:

   ```rust
   struct ActiveControlInputTarget {
       entity: Entity,
       target_entity_id: String,
       generation: u64,
   }
   ```

   The helper should return `Some(ActiveControlInputTarget)` only for active phases.
3. For `ControlBootstrapPhase::ActivePredicted`, use the exact `entity`, `target_entity_id`, and `generation` from the phase.
4. For `ControlBootstrapPhase::PendingPredicted`, return `None` and record a suppression reason.
5. For `ControlBootstrapPhase::Idle`, return `None`.
6. If an `ActiveLocalAnchor` phase is introduced, it must validate:
   - target matches authenticated player entity ID by canonical GUID,
   - entity is the local player anchor,
   - no non-player controlled entity can enter this branch.
7. Build `ClientRealtimeInputMessage.control_generation` from the active target helper, not directly from stale view state.
8. Keep `LocalPlayerViewState` as the authoritative desired target, but not as local prediction authorization.
9. If there is no active input target:
   - remove stale local input markers/writers,
   - do not apply active input locally,
   - do not send active input,
   - optionally send a bounded neutral boundary if an already active previous lease needs it.

Important:

Do not call `resolve_entity_by_guid_prefer_predicted` from input ownership code after this phase except for diagnostics. It is fine for rendering, selection, and debug display. It is not fine as an input authority oracle.

### Phase 3: Reset client input streams at control-generation boundaries

Goal: a previous lease cannot contaminate the next lease.

Concrete changes:

1. In `bins/sidereal-client/src/runtime/control.rs`, when ACK changes target or generation:
   - clear `ClientInputAckTracker.pending_ticks`,
   - reset `ClientInputSendState.last_sent_target_entity_id`,
   - reset `ClientInputSendState.last_sent_actions`,
   - set `ClientInputSendState.last_sent_at_s = f64::NEG_INFINITY`,
   - clear or re-evaluate `NativePredictionRecoveryState.pending_neutral_send` so it does not accidentally send the wrong generation.
2. Do the same when a reject corrects the client to a different authoritative target/generation.
3. Add a `ClientControlInputEpochState` if direct reset calls become scattered:

   ```rust
   #[derive(Debug, Resource, Default)]
   pub(crate) struct ClientControlInputEpochState {
       pub active_target_entity_id: Option<String>,
       pub active_generation: u64,
       pub reset_count: u64,
   }
   ```

4. Use canonical player IDs when comparing free-roam/player-anchor target IDs.

Expected result:

1. First packet after activation is always emitted for the new target/generation.
2. Input ack tracking does not include ticks from an obsolete target/generation stream.

### Phase 4: Make server rebind cleanup explicit

Goal: accepted control changes are clean server input boundaries.

Concrete changes:

1. In `bins/sidereal-replication/src/replication/input.rs`, add a helper like:

   ```rust
   pub(crate) fn clear_realtime_input_for_control_rebind(
       player_entity_id: PlayerEntityId,
       latest: &mut LatestRealtimeInputsByPlayer,
       activity: &mut RealtimeInputActivityByPlayer,
       tick_tracker: &mut ClientInputTickTracker,
       rate_limits: &mut InputRateLimitState,
   ) { ... }
   ```

2. Call it from `receive_client_control_requests` on accepted rebind.
3. If the borrow graph makes this hard, split control request acceptance into:
   - a pure validation/resolution system that emits a `ControlRebindAccepted` event,
   - a cleanup system that consumes that event and owns input-state resources,
   - an ACK flush system after cleanup.
4. Keep session/disconnect cleanup in `auth.rs`, but route it through the same helper if possible.
5. Keep fixed-step drain cleanup as defensive backup.

Expected result:

1. `controlled_target_mismatch` and `stale_control_generation` counters may increment for genuinely invalid packets, but not continuously after a valid handoff.
2. A stale latest snapshot from target A cannot keep affecting target B.

### Phase 5: Neutralize old and new control intent on accepted rebind

Goal: server-side durable intent starts neutral for every new lease.

Concrete changes:

1. Keep `queue_neutralize_control_intent(&mut commands, currently_bound_entity)`.
2. Add neutralization for `resolved_target_entity` when `rebind_required` is true.
3. Ensure this is safe for:
   - Avian physics-backed controlled roots,
   - non-physics player anchors,
   - entities without `FlightComputer`,
   - entities without `AfterburnerState`,
   - future generic controlled entities.
4. If the current helper is too ship-specific internally, refactor it to be generic while preserving current behavior.

Expected result:

1. No target starts a new lease with held thrust, turn, brake, afterburner, or queued actions from an older lease.
2. Free-roam and non-ship targets are not forced through ship-specific assumptions.

### Phase 6: Decide Lightyear role strategy from evidence

Goal: stop depending on unsupported or unreliable dynamic behavior if the evidence says it is the core blocker.

After Phases 1-5, run the repro. If `PendingPredicted` still persists after ACK because the predicted clone does not appear, implement the stable owner-predicted pool.

Stable owner-predicted pool requirements:

1. Scope is currently relevant, owned, controllable roots for the authenticated connected player, plus the player anchor.
2. Do not make every persisted owned entity in the galaxy owner-predicted. Visibility and interest management still bound what the client receives.
3. Server role reconciliation should derive:
   - owner prediction target for relevant owned controllable roots,
   - owner prediction target for the player anchor,
   - observer interpolation for non-owner clients,
   - active control mapping from `PlayerControlledEntityMap`.
4. Client input authority still moves only through `ControlBootstrapState`.
5. Non-active owner-predicted entities must not receive `InputMarker<PlayerInput>` or `SimulationMotionWriter`.
6. Add logs showing whether a handoff reused an existing predicted clone or waited for a newly spawned one.

If Lightyear requires `ControlledBy` for prediction target semantics:

1. Inspect the forked Lightyear code before changing Sidereal semantics.
2. Prefer a minimal, well-documented Sidereal-side role model.
3. Do not re-enable Lightyear's native server input receiver as the authoritative input path.
4. Keep Sidereal's authenticated `ClientRealtimeInputMessage` path as the server input source.

Expected result:

1. Control swaps do not rely on Lightyear creating a new predicted clone at the exact handoff moment.
2. Ship -> free roam -> ship transitions become a local writer move among already valid prediction lanes.

### Phase 7: Tests

Add tests before broad refactors where possible. Use `tests/*.rs` by default. If private ECS system access requires inline Rust tests, update `scripts/inline_rust_test_allowlist.txt` in the same change.

Client tests:

1. `send_lightyear_input_messages` does not insert `InputMarker<PlayerInput>` while `ControlBootstrapPhase::PendingPredicted`.
2. `send_lightyear_input_messages` does not insert `SimulationMotionWriter` while `PendingPredicted`.
3. `send_lightyear_input_messages` sends active input only when `ControlBootstrapPhase::ActivePredicted` matches the target and generation.
4. `enforce_single_input_marker_owner` removes markers from fallback entities when bootstrap is pending.
5. `enforce_single_input_marker_owner` keeps only the exact active bootstrap entity.
6. ACK target/generation changes reset `ClientInputSendState` and `ClientInputAckTracker`.
7. Free-roam/player-anchor activation uses either a real predicted anchor or the explicit local-anchor phase. It must not use a confirmed/interpolated fallback.

Server tests:

1. Accepted control rebind clears `LatestRealtimeInputsByPlayer` for the player.
2. Accepted control rebind clears `RealtimeInputActivityByPlayer` for the player.
3. Accepted control rebind clears `ClientInputTickTracker` streams for the player.
4. Accepted control rebind neutralizes the previous target.
5. Accepted control rebind neutralizes the new target before fresh input.
6. Stale input from the previous target cannot cause repeated `controlled_target_mismatch` after a valid rebind.
7. Input with mismatched player ID, target ID, or generation is still rejected.

Integration or harness tests:

1. Enter world -> control entity A -> move.
2. Entity A -> free roam -> move.
3. Free roam -> entity A -> move.
4. Entity A -> entity B -> move.
5. Repeat the sequence at least 20 times in one session.
6. Assert:
   - no more than one local input marker,
   - no more than one local simulation writer,
   - server input drop counters do not keep increasing after settle,
   - predicted active target changes to the expected target/generation,
   - server authoritative motion changes for the selected target when input is active.

## 9. Manual Repro And Validation Procedure

Use two native clients where possible because single-client happy paths can hide observer and focus side effects.

Suggested debug environment:

```bash
SIDEREAL_DEBUG_CONTROL_LOGS=1 \
SIDEREAL_DEBUG_INPUT_DROP_LOGS=1 \
SIDEREAL_DEBUG_MOTION_REPLICATION=1 \
SIDEREAL_DEBUG_PREDICTION_BOOTSTRAP=1
```

Scenario:

1. Start gateway, replication, and a native client.
2. Enter world.
3. Control a simulated entity and hold forward input for several seconds.
4. Release input.
5. Switch to free roam/player-anchor.
6. Wait until the client reports active prediction for the new lease.
7. Move in free roam.
8. Switch back to the original entity.
9. Move again.
10. Switch to a second owned controllable entity if available.
11. Repeat steps 5-10.
12. Watch logs for:
    - `PendingPredicted` lasting beyond the configured warning threshold,
    - fallback input denied,
    - active input authorized target/generation,
    - server accepted input target/generation,
    - stale generation after settle,
    - controlled target mismatch after settle.

Success criteria for manual validation:

1. During pending handoff, active input is intentionally suppressed.
2. Once active, local prediction moves immediately and server authoritative motion follows.
3. Repeated handoffs recover without reconnecting.
4. Server input drop counters are quiet after each handoff settles.
5. Observer clients still see authoritative motion through interpolation.

## 10. Acceptance Criteria

The work is complete only when all of these hold:

1. `send_lightyear_input_messages` no longer uses confirmed/interpolated fallback resolution for input authority.
2. `enforce_single_input_marker_owner` no longer uses confirmed/interpolated fallback resolution for marker retention.
3. `ControlBootstrapState` is the single client-side source of local input authorization.
4. Control ACK/reject target-generation changes reset client input stream state.
5. Server accepted rebinds clear short-lived realtime input state for the player.
6. Server accepted rebinds neutralize old and new target intent safely.
7. Free-roam/player-anchor control has a documented activation path and does not silently fallback-write confirmed/interpolated state.
8. If Lightyear dynamic role switching remains unreliable, the stable owner-predicted pool is implemented or this plan is updated with a better proven strategy.
9. Tests cover pending, active, reject/correction, entity control, and free-roam control.
10. Native client validation covers entity -> free roam -> entity handoff without requiring restart or reconnect.
11. No security rule is weakened. Input remains authenticated, generation-scoped, and target-validated.

## 11. Quality Gates

Run targeted tests for every touched crate. Before marking the implementation complete, run the repo gates from `AGENTS.md`:

```bash
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings
CARGO_INCREMENTAL=0 cargo check --workspace
```

Because this plan touches client behavior, also run:

```bash
CARGO_INCREMENTAL=0 cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
CARGO_INCREMENTAL=0 cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

If a local target or cross-linker is not installed, record that explicitly and make sure CI still covers it.

## 12. Things Not To Do

1. Do not make the server accept stale control generations to hide handoff bugs.
2. Do not make the server accept mismatched controlled target IDs to hide handoff bugs.
3. Do not attach local input markers to confirmed/interpolated entities.
4. Do not add a second client-side input system for free roam that bypasses the control-generation contract.
5. Do not add per-player SQL side tables for runtime control state.
6. Do not use Bevy `Entity` IDs in network or persistence boundaries.
7. Do not reintroduce legacy mirror motion components for runtime replication or simulation.
8. Do not use frame-time deltas for authoritative or predicted force/integration math.
9. Do not gate native-vs-WASM behavior with cargo features. Use `cfg(target_arch = "wasm32")` only at platform boundaries.
10. Do not solve this by adding broad transform snapback or emergency correction writers. Correction ownership must remain clear.

## 13. Open Questions For The Implementing Agent

Answer these with code evidence during implementation:

1. Does a predicted player-anchor clone reliably appear after server ACK when switching to free roam?
2. Does a predicted controlled-root clone reliably appear after server ACK when switching from free roam back to an entity?
3. Does Lightyear's fork require `ControlledBy` for owner prediction, or is `PredictionTarget` enough for the stable owner-predicted pool?
4. Does stable owner prediction for all currently relevant owned controllable roots create unacceptable bandwidth or rollback cost in current test scenes?
5. Is an explicit `ActiveLocalAnchor` phase still needed after stable owner-predicted player anchors are implemented?
6. Are input drops after handoff mostly stale generation, target mismatch, future tick, rate limit, or empty-after-filter?

Do not leave these as guesses. Add logs or tests that make the answer visible.

## 14. Definition Of Done

A fresh native session can repeatedly execute:

```text
entity A control -> free roam -> entity A control -> entity B control -> free roam -> entity B control
```

without:

1. losing local input prediction,
2. attaching input to fallback entities,
3. accumulating server input drops after the handoff settles,
4. leaving old targets with held controls,
5. requiring reconnect to recover.

When that is true, update `docs/features/prediction_runtime_tuning_and_validation.md` with a dated 2026-04-27 or later status note describing native impact and WASM impact.

## 15. 2026-04-28 Implementation Note

The active implementation tightened two ownership boundaries from this plan:

1. `LocalPlayerViewState` is not synchronized from replicated `ControlledEntityGuid` anymore. Session-ready and control ACK/reject messages are the only runtime writers for the acknowledged local control lease; replicated player components may lag and must not overwrite pending or acknowledged local intent.
2. Client adoption ignores `EntityGuid` entities without a Lightyear lane marker. A rendered runtime world entity must arrive through `Replicated`, `Predicted`, or `Interpolated`; unclassified GUID entities are not a fallback visual lane.
3. Server role rearm now applies to every entity under the affected visibility root, not just the root whose `PredictionTarget` / `InterpolationTarget` changed. This keeps mounted children/modules from continuing to stream updates after the parent lane has been despawned and reclassified.

These are contract fixes, not shims: they remove stale writers and broaden the existing Lightyear role-transition boundary to the whole replicated visibility tree.

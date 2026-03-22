# Prediction Runtime Tuning and Validation

Status: Active post-migration tuning tracker  
Scope: Lightyear-native prediction/interpolation behavior verification and production default tuning

## 1. Purpose

Track remaining non-structural work after Lightyear-native migration completion:
- prediction/interpolation behavior tuning under load,
- rollback/correction default validation,
- deferred adoption telemetry baselining.

## 2. Current Baseline

- Lightyear-native replication/prediction/interpolation is active.
- Legacy world-delta runtime paths are removed.
- Legacy mirror-motion components are removed from runtime simulation/replication flow.
- Fixed-step simulation remains authoritative at 60 Hz.

2026-03-11 update:
- Shared core `SIM_TICK_HZ` now matches the active 60 Hz client and replication runtime.

## 2.1 Current Native Runtime Status (2026-03-08)

- Native client now reaches in-world state and can render replicated ships after world entry.
- In-world controls are not yet functioning reliably for the controlled entity.
- Motion/correction behavior still shows intermittent jumping/snapping and needs focused native debugging before feel tuning can be considered complete.
- Native runtime stabilization is the immediate priority; resumed browser/WASM parity validation should wait until these native control and motion issues are under control.

## 2.2 Runtime Safeguards (2026-02-26)

- Client realtime input sending is change-driven with heartbeat:
  - send immediately when action set changes,
  - send immediately when routed controlled entity changes,
  - otherwise send heartbeat at 10 Hz (`0.1s`) to preserve liveness.
  - when the primary window is unfocused, client sends neutral intent snapshots (prevents stuck held-key intent across focus changes).
- Transport channel QoS separation:
  - realtime input uses `InputChannel` (`SequencedUnreliable`, latest-wins),
  - control/session uses `ControlChannel` (reliable),
  - asset stream/request/ack uses `AssetChannel` (reliable, isolated from input path).
- Server ingress keeps latest-intent semantics:
  - validates tick ordering and rate limits per authenticated player,
  - stores latest input snapshot by player/tick,
  - expires realtime input snapshots after `REPLICATION_REALTIME_INPUT_TIMEOUT_SECONDS` (default `0.35s`) so authoritative motion cannot stay latched if the client loses focus, background-throttles, or misses the neutral heartbeat,
  - drains into `ActionQueue` by replace/overwrite (`queue.clear()` then push latest actions), never backlog append.
- Remote/non-controlled visual smoothing path:
  - replicated Avian motion components (`Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`) are registered with Lightyear interpolation functions in protocol registration.
  - non-controlled root entities are now receive-only without `Predicted`/`Interpolated` markers, so replication applies motion directly to Avian components and transform sync follows authoritative values.
  - client render transform sync applies frame-rate-independent visual smoothing for non-controlled, non-proxy entities and snaps on large corrections, while controlled/proxy entities continue snap-sync behavior.
  - this avoids `Predicted -> Interpolated` marker-transition gaps for remote roots while preserving local controlled prediction/reconciliation behavior.
- Visibility/anchor consistency guard:
  - replication observer-anchor position updates and visibility lookups canonicalize `player_entity_id` (UUID wire form) before spatial-delivery evaluation to avoid asymmetric visibility when mixed ID formats are present.
  - server visibility and observer-anchor sampling in `FixedUpdate` now read authoritative Avian `Position` first (with transform fallback) instead of relying solely on `GlobalTransform`, preventing stale/zero spatial samples during low-FPS transform-propagation lag.
  - replication delivery scope range is runtime-tunable via `SIDEREAL_VISIBILITY_DELIVERY_RANGE_M` (default `300` meters), enabling controlled validation of range-culling behavior without code changes.
- Auth/visibility handshake stability guard:
  - client auth resend retries are gated by `ServerSessionReady` for the selected player (not by first replicated-world observation), preventing repeated auth rebinding loops while visibility is still warming up.
  - replication auth handling is idempotent for repeated same-client/same-player auth messages and no longer replays global `lose_visibility` reset for that case.
- Remote-root anchor consistency fallback:
  - replication server mirrors controlled ship motion (`Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`) onto the corresponding player anchor in fixed tick.
  - client fallback that aligns remote controlled ship-root motion to remote player-anchor motion is disabled by default; enable only for diagnostics with `SIDEREAL_CLIENT_REMOTE_ROOT_ANCHOR_FALLBACK=1`.
- Client single-writer hardening for remote entities:
  - client fixed-tick writer path excludes generic global mutation systems (`validate_action_capabilities` and global `recompute_total_mass`); mass/inertia sync is controlled-entity-scoped.
  - client-side idle-stabilization and angular clamp post-physics systems are scoped to `ControlledEntity` only, preventing local mutation of replicated remote interpolated `LinearVelocity`/`AngularVelocity` when `FlightComputer` is present on remote roots.
  - flight writer systems are gated by runtime `FlightControlAuthority` marker:
    - server assigns marker to authoritative `FlightComputer` roots by default,
    - client assigns marker only to locally controlled root and removes it from receive-only roots,
    - replicated entities retain `FlightComputer` data (no destructive client stripping), preserving component parity for effects/inspection while enforcing single-writer motion ownership.
- WASM impact:
  - no target-specific branching introduced,
  - interpolation registration and scheduling behavior are shared between native and WASM builds.

2026-03-22 update:

- Dynamic predicted/interpolated handoff is still a live Lightyear edge in Sidereal's current fork/runtime shape.
- Native client input timeline no longer defaults to zero input delay.
  - Sidereal now defaults `SIDEREAL_CLIENT_INPUT_DELAY_TICKS` to `2` for native timeline setup, with `--input-delay-ticks` as the equivalent CLI override.
  - Reason: the project is currently reproducing multi-second confirmed-vs-predicted drift and aborted rollbacks under `fixed_input_delay(0)`, and Lightyear upstream already treats zero-delay localhost timing as fragile.
- Client runtime now seeds missing `Confirmed<T>` mirrors for Avian motion components (`Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`) when an already-existing replicated entity is in the `Interpolated` lane but still only has raw motion components.
  - This is a bounded transition-bootstrap guard for control handoff / visibility role churn, not a new steady-state presentation contract.
  - Reason: upstream Lightyear still carries an explicit interpolation TODO for "if `Interpolated` is added on an existing entity" and Sidereal exercises that path during control transfer.
  - Goal: let the observer lane become immediately presentable and interpolation-ready without waiting for a later delta to populate `Confirmed<T>`.
- Conflicting `Predicted` + `Interpolated` marker cleanup on the client is now transition-driven rather than an every-frame scan.
  - Sidereal still sanitizes invalid mixed-lane entities locally because dynamic handoff can reuse a local entity and leave both markers present.
  - The sanitizer now runs when either marker is added, and the winning lane depends on whether the entity is the active local control target or an observer-only entity.
- Native impact:
  - reduces the chance that the local prediction timeline outruns confirmed state badly enough to exceed the rollback budget after focus churn or localhost sync jitter.
  - reduces long observer-transition stalls where the authoritative/server lane is advancing but the local observer presentation is still waiting for confirmed bootstrap.
- WASM impact:
  - shared runtime behavior only; no target-specific branching added.

## 2.4 Input Liveness Guard (2026-03-09)

- Sidereal now treats realtime input snapshots as short-lived intent, not durable movement state.
- The replication server clears authoritative input for a player when no fresh realtime snapshot has arrived within `REPLICATION_REALTIME_INPUT_TIMEOUT_SECONDS` (default `0.35s`).
- This guard is intentionally longer than the client heartbeat interval (`0.1s`) so ordinary jitter does not zero live controls, but short enough to stop stale held-key motion when a native client is alt-tabbed or OS-throttled before it can deliver an unfocused neutral snapshot.
- Native impact:
  - losing window focus should no longer leave the server simulating old movement/fire intent indefinitely if the client stops running its fixed input send path in the background.
- WASM impact:
  - no WASM-specific branching; the same authoritative stale-input expiry applies to browser clients.

## 2.3 Dynamic Handoff Lightyear Exception (2026-03-09)

- Lightyear applies `Predicted` / `Interpolated` classification from the spawn action delivered to a receiver.
- Sidereal's dynamic control handoff can promote an already-visible entity into the owner-predicted lane after initial replication.
- For that Sidereal-specific case, the replication server intentionally forces a sender-local respawn transition on handoff by cycling visibility for the affected receiver after updating `PredictionTarget` / `InterpolationTarget`.
- For owner-specific control lanes, Sidereal prefers `manual(vec![client_sender_entity])` targets once the concrete `ClientOf` sender entity is known.
  - This is narrower than the generic peer-id `NetworkTarget` form used in many Lightyear examples.
  - The reason is Sidereal's runtime can retarget ownership after connect/auth/hydration, and sender-entity targeting avoids depending on a second remote-id-to-sender resolution step during those handoff transitions.
- The persisted player-anchor replication sync is intentionally idempotent.
  - Sidereal keeps reevaluating anchor-vs-ship control mode every fixed tick, but it must not blindly reinsert Lightyear target components each frame.
  - Replacing `PredictionTarget` / `InterpolationTarget` unnecessarily can fight the hook-driven per-sender replication state that Lightyear maintains.
- This is an intentional exception to the simpler "predict on first spawn and never retarget" model used by many Lightyear examples.

## 3. Remaining Work

1. Validate prediction/interpolation behavior under gameplay load:
   - confirmed/predicted/interpolated entity behavior remains stable under connect/disconnect churn.
   - controlled entity input path actually produces authoritative in-world motion.
   - intermittent correction/jump behavior is removed or reduced to intentional correction cases only.
2. Validate and lock correction/rollback defaults:
   - `SIDEREAL_CLIENT_MAX_ROLLBACK_TICKS`
   - `SIDEREAL_CLIENT_INSTANT_CORRECTION`
3. Run controlled multi-client load sessions and capture deferred-adoption telemetry:
   - `avg_wait_s`
   - `max_wait_s`
4. Lock recommended defaults for defer/adoption diagnostics:
   - `SIDEREAL_CLIENT_DEFER_WARN_AFTER_S`
   - `SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S`
   - `SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S`
   - `SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S`

2026-03-11 update:

1. Native client prediction/runtime tuning values now have command-line equivalents in addition to env vars; use `sidereal-client --help` for the current native option surface.
2. The old env-driven debug startup toggles were removed from the native client startup path. This playbook should only reference active prediction/runtime tuning inputs, not debug-only launch flags.

## 4. Runtime Tuning Playbook

1. Start with defaults:
   - `SIDEREAL_CLIENT_DEFER_WARN_AFTER_S=1.0`
   - `SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S=1.0`
   - `SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S=4.0`
   - `SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S=30.0`
2. Run at least 2 concurrent clients with repeated reconnect + immediate input bursts.
3. Watch logs for:
   - `predicted adoption delay summary` (`samples`, `avg_wait_s`, `max_wait_s`)
   - controlled-adoption delay/stall warnings
   - correction/rollback configuration logs from prediction-manager setup
4. Tune thresholds:
   - raise warn thresholds if harmless startup delays spam warnings,
   - lower dialog threshold if real control gaps are being hidden.

## 5. Acceptance Criteria

- Controlled entity appears consistently within acceptable join latency under expected load.
- Prediction anomaly warnings are rare or absent during nominal operation.
- Locked defaults are documented in this file and reflected in runtime env documentation.

## 6. References

- `docs/sidereal_implementation_checklist.md`
- `docs/sidereal_design_document.md`
- `bins/sidereal-client/src/native.rs`
- `bins/sidereal-replication/src/replication/runtime_state.rs`

2026-03-22 update:

1. Shared authoritative flight now uses fixed-step time for thrust application.
   - `crates/sidereal-game/src/flight.rs` now reads `Res<Time<Fixed>>` in `apply_engine_thrust`.
   - This aligns the client prediction path and replication authoritative path with the repo rule that simulation math must be fixed-step only.
2. Client control binding now uses an explicit bootstrap state rather than clone preference alone.
   - `bins/sidereal-client/src/runtime/resources.rs` now defines `ControlBootstrapState` / `ControlBootstrapPhase`.
   - `bins/sidereal-client/src/runtime/replication.rs` now keeps non-anchor ship control in `PendingPredicted` until a real `Predicted` root exists, instead of falling back to confirmed/interpolated ship control.
3. Motion ownership now consumes that bootstrap state as an input contract.
   - `bins/sidereal-client/src/runtime/motion.rs` prefers the explicit active predicted bootstrap state instead of rediscovering control solely from clone scoring.
4. Debug overlay diagnostics now expose the control bootstrap phase directly.
   - This makes the two-client repros easier to classify as `Pending`, `Anchor`, or `Predicted` control states instead of relying only on `Control Lane`.
5. Replication role rearm was narrowed.
   - `bins/sidereal-replication/src/replication/control.rs` now rearms visible clients only when the replication topology itself changes (`Replicate`, `PredictionTarget`, `InterpolationTarget`), not on every control-bookkeeping mutation.

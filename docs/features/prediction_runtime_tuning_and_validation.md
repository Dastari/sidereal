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
- Fixed-step simulation remains authoritative at 30 Hz.

## 2.1 Runtime Safeguards (2026-02-26)

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

## 3. Remaining Work

1. Validate prediction/interpolation behavior under gameplay load:
   - confirmed/predicted/interpolated entity behavior remains stable under connect/disconnect churn.
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

## 4. Runtime Tuning Playbook

1. Start with defaults:
   - `SIDEREAL_CLIENT_DEFER_WARN_AFTER_S=1.0`
   - `SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S=1.0`
   - `SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S=4.0`
   - `SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S=30.0`
2. Run at least 2 concurrent clients with repeated reconnect + immediate input bursts.
3. Watch logs for:
   - `predicted adoption delay summary` (`samples`, `avg_wait_s`, `max_wait_s`)
   - `prediction runtime summary` (`replicated`, `predicted`, `interpolated`, `controlled`)
   - anomaly warnings (`no controlled entity`, `zero Predicted markers`)
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

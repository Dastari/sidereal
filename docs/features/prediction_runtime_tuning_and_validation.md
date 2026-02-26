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
- Server ingress keeps latest-intent semantics:
  - validates tick ordering and rate limits per authenticated player,
  - stores latest input snapshot by player/tick,
  - drains into `ActionQueue` by replace/overwrite (`queue.clear()` then push latest actions), never backlog append.
- Remote/non-controlled visual smoothing path:
  - interpolated entities are excluded from direct transform sync writes,
  - `refresh_interpolated_visual_targets_system` captures snapshot-to-snapshot targets on replicated motion changes,
  - `apply_interpolated_visual_smoothing_system` applies frame interpolation (`lerp`/`slerp`) between those targets.
- WASM impact:
  - no target-specific branching introduced,
  - behavior and scheduling are shared between native and WASM builds.

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

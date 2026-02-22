# Sidereal v3 Implementation Checklist

Status: Active implementation tracker  
Primary spec: `docs/sidereal_design_document.md`

## 1. Current Architecture Baseline

- [x] Single authoritative simulation runtime in `sidereal-replication`.
- [x] Lightyear-native replication/prediction/interpolation active for client/server runtime.
- [x] Legacy world-delta runtime paths removed from active gameplay flow.
- [x] Legacy gameplay mirror motion components removed from runtime pathways.
- [x] Graph-record persistence/hydration path active (`GraphEntityRecord` / `GraphComponentRecord`).

## 2. Vertical Slice Gameplay Readiness

- [ ] Login/register/logout loop remains stable across repeated sessions.
- [ ] Player consistently enters world with controlled ship and active HUD.
- [ ] Flight feel validated in live play:
  - [ ] forward thrust responsiveness,
  - [ ] braking behavior,
  - [ ] turning smoothness,
  - [ ] no residual drift after settle.
- [ ] Camera follow/look-ahead behavior validated under correction events.
- [ ] Background/fullscreen layer rendering remains stable across fresh sessions.

## 3. Prediction and Rollback Tuning (Remaining Migration Work)

- [ ] Complete prediction/interpolation behavior tuning under gameplay load.
- [ ] Validate and lock production defaults for:
  - [ ] `SIDEREAL_CLIENT_MAX_ROLLBACK_TICKS`
  - [ ] `SIDEREAL_CLIENT_INSTANT_CORRECTION`
- [ ] Run controlled multi-client load sessions and capture adoption telemetry:
  - [ ] `avg_wait_s`
  - [ ] `max_wait_s`
- [ ] Lock recommended defaults for:
  - [ ] `SIDEREAL_CLIENT_DEFER_WARN_AFTER_S`
  - [ ] `SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S`
  - [ ] `SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S`
  - [ ] `SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S`

## 4. Gameplay ECS and Data Model Expansion

- [ ] Continue capability-driven component expansion in `crates/sidereal-game`.
- [ ] Keep generated component schema as source-of-truth for persistable gameplay data.
- [ ] Add/update tests for new component persistence/hydration roundtrips.
- [ ] Ensure hierarchy/mount relationship persistence remains deterministic.
- [ ] Avoid service-local gameplay duplication; keep gameplay logic in shared crates.

## 5. Visibility and Permissions

- [ ] Complete migration away from residual property-key compatibility paths.
- [ ] Keep visibility/range logic generic over entities.
- [ ] Validate ownership/faction/public visibility behavior under live multi-client scenarios.
- [ ] Verify unauthorized fields are never serialized.

## 6. WASM and Transport Direction

- [ ] Keep native and WASM builds green on each client/runtime change.
- [ ] Maintain WebRTC-first direction for WASM transport boundary implementation.
- [ ] Keep gameplay/prediction logic shared between native and WASM builds.
- [ ] Restrict platform differences to transport/bootstrap boundary code only.

## 7. Docs and Maintenance

- [ ] Keep `docs/sidereal_design_document.md` aligned with implemented runtime behavior.
- [ ] Keep `docs/migrate_to_lightyear_prediction.md` as migration/tuning history and remaining tuning tracker.
- [ ] Store future feature proposals under `docs/features/`.
- [ ] Keep old audits historical only under `docs/archive/`.

## 8. Quality Gates

Run for significant changes:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

When client behavior/protocol/prediction changes:

```bash
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

## 9. Definition of Done (Per Feature Change)

- [ ] Implementation complete with correct crate/service boundaries.
- [ ] Unit tests updated in touched crates.
- [ ] Integration tests updated when cross-service flow changes.
- [ ] Docs updated in same change for enforceable behavior changes.
- [ ] Required quality gates pass.

# Sidereal v3 Implementation Checklist

Status: Active implementation tracker  
Primary spec: `docs/sidereal_design_document.md`
Update note (2026-03-09):
- Documentation taxonomy normalized:
  - plans live under `docs/plans/`,
  - decisions live under `docs/decisions/`,
  - reports live under `docs/reports/`.

## 1. Current Architecture Baseline

- [x] Single authoritative simulation runtime in `sidereal-replication`.
- [x] Lightyear-native replication/prediction/interpolation active for client/server runtime.
- [x] Legacy world-delta runtime paths removed from active gameplay flow.
- [x] Legacy gameplay mirror motion components removed from runtime pathways.
- [x] Graph-record persistence/hydration path active (`GraphEntityRecord` / `GraphComponentRecord`).

## 2. Vertical Slice Gameplay Readiness

Status note (2026-03-08):
- Native client can now authenticate, enter world, and render replicated ships in-world.
- Current blocking issues are native control/input failure in-world and intermittent motion/correction jumping.
- Native control/prediction/camera stabilization is the active priority; remaining WASM parity follow-through is deferred until this baseline is stable.

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
- [ ] Enforce scope separation:
  - [ ] Keep world truth, authorization scope, and delivery scope as distinct server-side stages.
  - [ ] Ensure client stream subscriptions never widen authorization.
- [ ] Enforce and verify default scanner visibility floor behavior:
  - [ ] Treat scanner range as server-enforced fog-of-war in replication delivery logic.
  - [ ] Player observer uses default `300m` scanner floor when no scanner modules extend range.
  - [ ] Aggregate authorization/scanner coverage over all owned entities and attachment chains, not only current control focus.
  - [ ] Non-public entities outside scanner range are not delivered to that client.
  - [ ] Ownership/public/faction visibility exceptions are preserved as explicit policy rules.
- [ ] Implement and verify sensitive-data redaction:
  - [ ] Non-owned contacts receive physical/render-safe fields by default.
  - [ ] Cargo manifests/private subsystem internals/transfer details remain omitted unless explicitly authorized.
  - [ ] Entities leaving authorization are removed through authoritative removal flow.
- [ ] Implement scan-intel grant pipeline:
  - [ ] Store temporary grants with observer-target scope, source, and expiry.
  - [ ] Apply grant field scopes (`physical_public`, `combat_profile`, `cargo_summary`, `cargo_manifest`, `systems_detail`) when building outbound payload masks.
  - [ ] Revoke/expire grants to immediate redaction reversion.
- [ ] Implement camera-centered replication delivery culling (network bind culling):
  - [ ] Use `ClientViewUpdateMessage.camera_position_m` as delivery culling anchor in replication visibility/delivery flow.
  - [ ] Apply camera culling in top-down XY space only (`x`, `y`), excluding `z` from culling decisions.
  - [ ] Add configurable edge buffer radius outside camera viewport bounds to prevent high-speed boundary snap-in/pop-in.
  - [ ] Prevent replication delivery for entities outside camera delivery volume when they cannot be rendered client-side.
  - [ ] Keep camera culling as an additional narrowing filter after authorization/visibility policy (never a bypass).
  - [ ] Add integration coverage demonstrating scanner visibility + camera delivery culling interaction.
- [ ] Add stream-tiered delivery behavior:
  - [ ] Keep focus/local stream, strategic/minimap stream, and intel/scan-result stream permission-filtered by shared redaction policy.
  - [ ] Validate that strategic/intel streams do not leak unauthorized internals.
- [ ] Add spatial query scaling work:
  - [ ] Use spatial indexing for visibility candidate selection instead of full-world per-client scans.
  - [ ] Track visibility query metrics (`candidates_per_frame`, `included_per_frame`, `query_time_per_client`).

## 6. WASM and Transport Direction

Status note (2026-03-08):
- Browser transport direction and shared-runtime architecture remain the intended target.
- End-to-end WASM parity validation is temporarily back-burnered while native in-world control and motion stability are unresolved.
- New work should avoid introducing native-only architecture that would make later WASM parity recovery harder.

- [ ] Resume full native/WASM parity validation after native control/prediction stability is restored.
- [x] Maintain WebTransport-first direction for WASM transport boundary implementation.
- [ ] Keep gameplay/prediction logic shared between native and WASM builds.
- [ ] Restrict platform differences to transport/bootstrap boundary code only.
- [ ] Complete WASM Lightyear transport enablement (WebTransport-first with explicit fallback path):
  - [x] Enable Lightyear/browser transport features needed for accepted WASM transport contract and explicit fallback behavior.
  - [ ] Keep current native UDP/raw-connection transport path for non-WASM targets without changing authority/prediction flow.
  - [x] Add WASM transport bootstrap module(s) that connect browser clients via Lightyear transport adapters; avoid gameplay-system forks.
- [ ] Remove temporary WASM scaffold-only runtime and wire shared client runtime plugin stack for both targets:
  - [x] Move WASM target dependencies (`sidereal-game`, `sidereal-net`, `sidereal-runtime-sync`, and Lightyear client transport deps) into target-compatible client config.
  - [x] Ensure protocol registration, prediction/interpolation, and input tick flow compile and run for `wasm32` without `cfg` branching in gameplay systems.
- [ ] Validate and document WASM runtime parity after transport hookup:
  - [x] Confirm browser build boots with `bevy/webgpu` and compiles the shared auth/bootstrap/asset/runtime path.
  - [ ] Run browser runtime validation against a live gateway + replication stack and confirm authoritative entity delivery end to end.
  - [ ] Confirm predicted local input ownership is preserved (replicated control state does not overwrite local pending intent).
  - [ ] Confirm docs (`docs/sidereal_design_document.md`) describe final WASM transport wiring and any explicit fallback behavior.

## 7. Docs and Maintenance

- [ ] Keep `docs/sidereal_design_document.md` aligned with implemented runtime behavior.
- [ ] Keep `docs/features/prediction_runtime_tuning_and_validation.md` updated as the prediction tuning tracker.
- [ ] Store future feature contracts/references under `docs/features/`.
- [ ] Store future implementation and migration plans under `docs/plans/`.
- [ ] Store future audit outputs under `docs/reports/`.
- [ ] Store future decision detail docs under `docs/decisions/`.

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

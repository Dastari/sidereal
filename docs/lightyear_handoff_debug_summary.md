# Lightyear Handoff Debug Summary

## Context
- Goal: implement Phase 1 Lightyear integration (native rollback + correction for controlled entity), while preserving remote behavior and future control handoff support.
- Current concern: the system may no longer align with intended Lightyear flow because of many local patches.
- Status: unresolved. Some specific issues improved, but core client-to-client state disagreement and handoff bugs persist.

## Primary Symptoms Observed
- Debug overlay (AABB/circle/FPS text) flickers heavily.
- Significant FPS degradation compared to previous stable baseline.
- Client 2 often does not see Client 1 ship movement/rotation updates correctly.
- Controlled -> free roam transition causes local ship to appear to stop instantly on Client 1.
- Re-selecting the ship causes jump/snap to server-agreed position.
- Earlier desync patterns included rotation and velocity divergence; rotation improved after mass/inertia parity fixes, velocity/state divergence still observed.

## Key Runtime Signals Seen In Logs
- Prediction summaries repeatedly showed:
  - `predicted=1`, `interpolated=4`, `controlled=1`
  - `rollback_active=false`, `rollback_entries_total=0`
  - manager present and configured.
- Interpolation pipeline diagnostics eventually reported:
  - `interpolated=4 with_confirmed_history=4 missing_history=0`
- Despite that, client world views still diverged and remote movement remained incorrect in several tests.

## Root-Cause Hypotheses We Investigated
- Missing `ConfirmedHistory` when adding `Interpolated` after `Confirmed<C>` existed.
- Multiple writers mutating remote entity state (anchor fallback + interpolation + ownership systems).
- Marker churn (`Predicted`/`Interpolated`/handoff marker) causing repeated structural edits.
- Visual/rendering of wrong duplicate entity (anchor/control-holder vs intended remote root).
- Control transition path not preserving a valid interpolation continuation for remote display.

## Implemented Fixes / Changes

### 1) Controlled release braking behavior
- Server control handoff now sets:
  - `flight_computer.throttle = 0.0`
  - `yaw_input = 0.0`
  - `brake_active = true`
- Purpose: stop uncontrolled ships from excessive coasting after release.

### 2) Prevent anchor fallback from overriding interpolated entities
- In `sync_remote_controlled_ship_roots_from_player_anchors`, added `Without<Interpolated>`.
- Later changed fallback default to OFF unless explicitly enabled:
  - env: `SIDEREAL_CLIENT_REMOTE_ROOT_ANCHOR_FALLBACK=1` to force-enable.
- Purpose: avoid second-writer conflicts with Lightyear interpolation.

### 3) Predicted->Interpolated smooth handoff path
- Added transition component and smoother:
  - `PredictedToInterpolatedTransition`
  - brief exponential smoothing toward confirmed state before adding `Interpolated`.
- Purpose: reduce abrupt visual discontinuity when control changes.

### 4) Fixed repeated interpolation-history reseeding churn
- Found local code repeatedly reinserting interpolated+history path every tick for entities already interpolated.
- Added guards to avoid re-seeding when marker already present.
- Purpose: reduce archetype churn/perf issues and prevent interpolation pipeline resets.

### 5) Reduced marker rewrite churn for controlled target
- Motion ownership now only rewrites predicted/interpolated markers when state is actually incorrect.
- Purpose: avoid constant marker thrash.

### 6) Visual duplicate suppression hardening
- Suppression logic updated so non-render authority entities are deprioritized/hidden:
  - entities with `ControlledEntityGuid`
  - entities with `PlayerTag`
- Visual child cleanup and streamed visual attach paths were updated to avoid attaching visuals to those entities.
- Purpose: prevent rendering stale/anchor duplicates instead of authoritative displayed root.

### 7) Camera/free-roam conflict fix
- Camera lock system previously forced camera to player anchor even in detached/free-roam mode.
- Updated to respect detached mode and resolve camera anchor correctly.
- Purpose: stop free-roam camera from being overridden every frame.

### 8) Angular inertia/mass parity fixes
- Addressed mass/inertia mismatch path that caused simulation divergence (especially rotation drift).
- Result: rotation sync improved significantly in testing.

### 9) Upstream Lightyear patch for interpolation history handoff gap
- Forked Lightyear and implemented:
  - history init when `Interpolated` is added after `Confirmed<C>` exists.
  - tests for both insertion orders.
- Fork commit used:
  - `Dastari/lightyear` @ `c96ae904`
- `sidereal_v3` now points to this git rev in workspace `Cargo.toml`.

### 10) Removed local history seeding hack in client
- Client helper now inserts only `Interpolated` marker and relies on Lightyear hooks for `ConfirmedHistory`.
- Purpose: converge toward Lightyear-native flow and remove workaround.

## Current State (After Above)
- Build checks pass (native/wasm/windows target checks for `sidereal-client`).
- The targeted history-gap fix is present upstream (fork) and wired in project.
- However, the core issue is still reported:
  - overlay flicker,
  - client disagreement,
  - poor/incorrect remote movement visibility,
  - control release/handoff still unreliable in real gameplay.

## Why We May Still Be Broken
- The client replication/prediction stack has accumulated multiple compensating systems over time.
- Even if each fix is locally reasonable, combined behavior may violate Lightyear’s expected ownership/marker lifecycle.
- There may still be hidden multi-writer paths or duplicate-entity selection mistakes.
- Our pipeline may be functionally correct in isolated metrics (`missing_history=0`) but wrong in entity selection/visibility/application order.

## What A Fresh Agent Should Validate First
1. Confirm single-writer invariants for `Position/Rotation/LinearVelocity/AngularVelocity` per entity per mode.
2. Trace one GUID across server/client1/client2 to verify:
   - which entity is rendered,
   - which entity owns prediction/interpolation markers,
   - which system writes state each tick.
3. Audit all places that add/remove:
   - `Predicted`
   - `Interpolated`
   - `ControlledEntity`
   - transition marker component
4. Verify no fallback/anchor system writes motion into entities managed by interpolation.
5. Verify replication visibility/delivery for the affected ship GUID to Client 2 is continuous and not range/authorization-flapping.
6. Re-check render suppression/winner selection by `EntityGuid` so the displayed entity is the one receiving motion updates.

## Lightyear Fork/Branch Details
- Local Lightyear repo: `/home/toby/dev/lightyear`
- Branch: `fix/interpolated-handoff-confirmed-history`
- Commit: `c96ae904`
- Patch scope:
  - `lightyear_interpolation/src/interpolation_history.rs`
  - `lightyear_interpolation/src/plugin.rs`
- PR intentionally not opened yet (pending validation in game project).

## Sidereal Wiring Details
- Workspace now points to Lightyear fork commit:
  - `/home/toby/dev/sidereal_v3/Cargo.toml`
- Lockfile updated accordingly:
  - `/home/toby/dev/sidereal_v3/Cargo.lock`
- Local history seeding workaround removed from:
  - `/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/replication.rs`

## Conclusion
- We implemented many targeted fixes and an upstream Lightyear patch for a real interpolation-hand-off gap.
- The game still exhibits major runtime desync/handoff issues.
- A fresh, systematic audit is justified and likely necessary to remove layered compensations and restore a clean Lightyear-conformant flow.

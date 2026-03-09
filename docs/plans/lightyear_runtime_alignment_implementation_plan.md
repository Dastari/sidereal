# Lightyear Runtime Alignment Implementation Plan

Status: Active  
Date: 2026-03-08  
Source: `docs/lightyear_audit_report_2026-03-08.md`

## 0. Current Implementation Status

Implemented in this refactor branch:

- Phase 1 is in place:
  - client runtime now adds `FrameInterpolationPlugin<Transform>`,
  - replicated world entities with replicated `Position` + `Rotation` now receive `FrameInterpolate<Transform>`.
- Phase 2 is partially in place:
  - removed the extra player-anchor render-copy smoothing layer from the camera schedule,
  - added GUID-targeted lifecycle audit logging via `SIDEREAL_CLIENT_LIFECYCLE_AUDIT_GUID`.
- Phase 3 is partially in place:
  - `BallisticWeapon` now carries explicit `projectile_speed_mps`,
  - ballistic weapons with projectile speed > 0 now spawn true projectile entities in shared fixed-step gameplay,
  - client and replication runtimes add `PreSpawned` for those projectile entities in their fixed-step lanes,
  - replication server assigns shooter prediction target + observer interpolation targets for spawned projectiles,
  - client tracers now skip projectile-backed weapons,
  - client attaches a simple projectile visual for projectile entities.

Still pending from the full recommendation set:

- deeper duplicate winner-selection cleanup beyond current diagnostics/smoothing removal,
- broader handoff retest and validation loop,
- workspace-wide quality gates and cross-target checks.

## 1. Purpose

Turn the prioritized recommendations from the Lightyear audit into an execution plan that can be implemented incrementally without losing Sidereal's core architectural constraints:

- dynamic predicted-entity swapping,
- persisted player-anchor plus free-roam camera model,
- strict server-side visibility and redaction,
- authenticated authoritative input path.

## 2. Goals

1. Make client motion and rendering visibly smoother under localhost and real network conditions.
2. Reduce local compensation layers that fight the intended Lightyear lifecycle.
3. Add a proper ballistic projectile path for weapons that are intended to behave like real inertial space projectiles.
4. Keep native and WASM client runtime paths aligned.

## 3. Phase Order

### Phase 1: Frame interpolation

Implement real Lightyear frame interpolation in the client runtime.

Target outcomes:

- render-time smoothing between fixed ticks,
- less visible fixed-step jitter at high framerates,
- no authority changes.

Work:

- add `FrameInterpolationPlugin` to the client runtime,
- attach `FrameInterpolate<Transform>` to replicated world entities that should visually smooth,
- verify schedule ordering remains compatible with LightyearAvian `PositionButInterpolateTransform`.

### Phase 2: Instrumentation and lifecycle cleanup

Target outcomes:

- clearer understanding of which entity is rendered for a GUID,
- reduced predicted/interpolated duplicate ambiguity,
- lower risk handoff debugging.

Work:

- add diagnostics around duplicate GUID render winners,
- audit and reduce duplicate-suppression churn where safe,
- keep camera/anchor smoothing from layering over incorrect entity selection.

### Phase 3: Proper ballistic projectile path

Target outcomes:

- weapons that are meant to be physical projectiles become real projectile entities,
- projectile initial velocity correctly inherits shooter inertial velocity,
- local shooter gets immediate predicted/prespawned projectile feedback,
- observers get interpolated projectile state.

Work:

- add projectile gameplay components,
- add authoritative projectile spawn/update/despawn systems,
- move relevant weapons off hitscan/tracer reconstruction,
- use Lightyear prediction/interpolation/prespawn flow for projectile entities.

### Phase 4: Validation

Work:

- targeted tests in touched crates,
- client native compile check,
- WASM compile check,
- broader follow-up checks if the environment supports them.

## 4. Non-Goals

- replacing Sidereal's authoritative input path with upstream Lightyear server-native input,
- replacing Sidereal's strict visibility/redaction contract with Lightyear rooms/relevance,
- removing the persisted player-anchor/free-roam model,
- rewriting all weapons to projectiles in one pass regardless of design intent.

## 5. Success Criteria

1. Client motion looks materially smoother at high framerate on localhost.
2. Dynamic control swap still works under predicted/interpolated handoff.
3. Projectile weapons intended to behave ballistically no longer rely on reconstructed hitscan tracers.
4. No authority regressions are introduced in input, visibility, or persistence boundaries.

# Debug Handoff: Control/Camera/Replication Drift (2026-02-25)

## Current Problem Statement

Two concurrent clients are still seeing divergent world state for remote entities (position/rotation mismatch), despite multiple control/camera fixes.

Observed now:
- Camera jitter while controlling ship was fixed.
- Control switching (ship <-> free roam/player) is much more stable than at session start.
- But multi-client state remains inconsistent:
  - Client A and Client B do not agree on remote ship transforms.
  - Remote movement can still appear wrong/stale on one client.
  - Visibility/culling behavior appears questionable from player perspective.

## High-Level Symptom History

Initial issues reported:
- Selecting owned ship changed UI state but camera stayed in free camera.
- WASD moved player camera instead of selected ship.
- After relog, black/starfield-only states and repeated auth bind loops.
- Duplicate ship entities observed in world/DB/server.
- Control flickering between player and ship without input.
- Stale control request sequences dropped on server.

After iterative fixes:
- Camera follow/control flicker mostly resolved.
- Ship control request/ack path improved.
- Reconnect robustness improved.
- Debug overlay corrected for one bad ghost pairing path.

Still open:
- Two clients out-of-sync on transforms for the same entities.

---

## Key Changes Made This Session

### 1) Control Protocol / Intent-Ack Flow

- Replaced ad-hoc control path with explicit request/ack/reject:
  - `ClientControlRequestMessage`
  - `ServerControlAckMessage`
  - `ServerControlRejectMessage`
- Added/registered these in Lightyear protocol and message registration.
- Client now keeps pending control request state and clears on matching ack/reject.
- Added control request resend timer on client.
- Added server sequence tracking reset on fresh auth bind.

Files touched (relevant):
- `crates/sidereal-net/src/lightyear_protocol/messages.rs`
- `crates/sidereal-net/src/lightyear_protocol/registration.rs`
- `bins/sidereal-client/src/native.rs`
- `bins/sidereal-replication/src/replication/view.rs`
- `bins/sidereal-replication/src/replication/auth.rs`

### 2) Removed Focus-Based Flow

- Removed reliance on `focused_entity_id` semantics per requested design direction.
- Shifted behavior toward `ControlledEntityGuid` authoritative flow.

### 3) Camera Follow/Locking Corrections

- `resolve_camera_anchor_entity` was incorrectly hardcoded to player runtime id.
- Changed to prefer current `controlled_entity_id`, fallback to player.
- End-of-frame camera lock now follows controlled ship as intended.

File:
- `bins/sidereal-client/src/native.rs`

### 4) Reconnect/Logout Robustness

- Logout now triggers Lightyear `Disconnect` instead of hard-despawn only.
- Reconnect path can re-`Connect` existing raw client if disconnected.
- Server auth bind now force-resets per-client visibility state to force fresh baseline on reconnect.

Files:
- `bins/sidereal-client/src/native.rs`
- `bins/sidereal-replication/src/replication/auth.rs`

### 5) Duplicate Ship Spawn Hardening

- Added guard in bootstrap processing to reuse existing runtime ship for same `(entity_id, owner)` instead of spawning duplicate.

File:
- `bins/sidereal-replication/src/replication/simulation_entities.rs`

### 6) Debug Overlay Corrections

- Removed a broken cross-entity GUID pairing path that produced drifting false “server ghost” AABB.
- Overlay now uses controlled entity confirmed snapshot path.
- Then changed controlled highlighting to use authoritative `LocalPlayerViewState.controlled_entity_id` first.

File:
- `bins/sidereal-client/src/native.rs`

### 7) Attempted Client Non-Controlled Mutation Reduction

- For non-controlled root entities on client adoption path:
  - removed local `ActionQueue` + `FlightComputer` to reduce accidental local authoring.
- Also modified shared flight systems:
  - `process_flight_actions` now hull-only (`Without<MountedOn>`)
  - `apply_engine_thrust` control map now hull-only input source

Files:
- `bins/sidereal-client/src/native.rs`
- `crates/sidereal-game/src/flight.rs`

---

## What Is Likely Still Wrong (Current Suspicions)

## A) Client is still writing motion it should not write

Even after removing some components from non-controlled roots, there may still be a path where client-side systems mutate entities that should be receive-only/interpolated.

Likely places to audit first:
- Client systems running in fixed update that mutate physics/transform and are not strictly scoped to local controlled entity.
- Any sync path that writes `Transform`/`Position` for entities not local-controlled.

## B) Conflicting ownership of motion state (single-writer violation)

There may still be more than one motion writer active for a given runtime mode:
- replication/interpolation path
- local gameplay/physics path
- camera/helper sync path

Need strict assertion of single-writer per controlled/non-controlled category.

## C) Visibility interpretation mismatch (policy vs expectation)

User expectation: yellow circle == hard cull boundary.
Actual server policy can include owner/public/faction authorization before range-based delivery narrowing.

Possible confusion:
- Seeing remote entities outside yellow circle may be valid if policy grants visibility.
- But this does not explain cross-client transform disagreement; that still indicates motion ownership bug.

## D) Possible stale runtime mappings after reconnect or multi-client binds

Even with improvements, runtime entity ID mapping or control binding may still transiently mismatch across clients.

---

## Fast Repro (Current)

1. Reset DB.
2. Create two accounts.
3. Log both clients in.
4. Observe remote entity positions/rotations between clients (same scene, same timestamp).
5. They diverge (per latest screenshot).

---

## Recommended Next Debug Plan (Fresh Session)

1. Add authoritative-motion write audit on client:
- Log every system that writes `Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`, `Transform`.
- Include entity runtime id and whether entity is local-controlled.
- Hard assert: non-controlled roots are never written by local gameplay systems.

2. Add per-entity ownership tags at runtime:
- Explicit marker for `LocalPredictedControlled` vs `RemoteInterpolated`.
- Gate all motion-authoring systems with these markers.

3. Temporarily disable client-side shared flight systems entirely for non-headless path and verify if cross-client divergence disappears.
- If divergence disappears, root cause is confirmed as local-authoring leak.

4. Add server visibility decision trace (temporary):
- For one tracked entity and two clients, print branch:
  - owner/public/faction/scanner/range decision.
- Confirms whether culling behavior is policy-expected.

5. Validate remote consistency with deterministic capture:
- Snapshot transform for a target entity every N ticks on both clients.
- Compare numerically, not visually.

---

## Files with Highest Debug Value Next

- `bins/sidereal-client/src/native.rs`
  - adoption/tagging of replicated entities
  - fixed update pipeline and motion writers
  - controlled tag assignment
- `crates/sidereal-game/src/flight.rs`
  - force application and action processing scope
- `bins/sidereal-replication/src/replication/visibility.rs`
  - policy/authorization + delivery scope logic
- `bins/sidereal-replication/src/replication/view.rs`
  - control request routing and entity rebinding

---

## Notes

- This session included extensive iterative fixes; many early symptoms are improved.
- Remaining issue appears architectural/runtime-consistency rather than simple UI/control message bug.
- Priority should now be strict single-writer enforcement for motion state on client.

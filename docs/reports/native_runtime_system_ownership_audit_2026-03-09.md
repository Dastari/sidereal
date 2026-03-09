# Native Runtime System Ownership Audit

Status: Active  
Date: 2026-03-09

Update note (2026-03-09):
- Dynamic control handoff can currently reuse the same Lightyear local entity when switching an already-visible ship from observer interpolation into owner prediction.
- Lightyear will insert the new `Predicted` marker from the respawned spawn action, but it does not automatically clear the stale `Interpolated` marker from that reused local entity.
- Sidereal now sanitizes that dual-marker state on the client: control targets keep `Predicted`, former control targets keep `Interpolated`.
- Future audits should treat `Predicted + Interpolated` on the same runtime entity as a concrete runtime bug, not an acceptable mixed mode.

Update note (2026-03-09):
- On login/bootstrap, the native client must not auto-request control of the player anchor just because no authoritative control target has replicated yet.
- Sidereal persists the last authoritative control target on the player entity, and the client must wait for that replicated state before issuing a handover request unless the player explicitly asked to switch control.
- Future audits should treat any startup path that falls back to `session.player_entity_id` as a speculative control override bug, not as a harmless bootstrap convenience.

Update note (2026-03-09):
- Asteroid field members currently use `CollisionProfile::Aabb` / `CollisionOutlineM` and do not need continuously rotating kinematic physics bodies to preserve gameplay collisions.
- Treating them as rotating Avian kinematic bodies created avoidable fixed-step physics, replication, and persistence churn for 100+ always-dirty world entities.
- Sidereal now treats these field asteroids as static physics obstacles with randomized initial heading only until a separate visual-only spin lane exists.

Update note (2026-03-09):
- The replication server `Update` loop previously defaulted to a `100 Hz` scheduler cap while authoritative simulation still ran in `FixedUpdate` at `60 Hz`.
- That cap was meant as an idle CPU guard, but under real load it can bunch message drain / send / auth work behind scheduler sleep and make replication feel bursty even when simulation time is nominal.
- Sidereal now runs replication `Update` uncapped by default and only reintroduces a cap when `REPLICATION_UPDATE_CAP_HZ` is explicitly set for profiling or operational reasons.

Update note (2026-03-09):
- Fullscreen background rendering had drifted from the documented model: the client was creating client-local fullscreen renderable copies from authored fullscreen entities instead of letting the authored entities become the render targets.
- That copy/source split made fullscreen passes harder to reason about and was a likely contributor to zoom-threshold black flashes when the copied runtime layer state diverged from the hidden authored source.
- Sidereal now renders fullscreen layers directly from the authored replicated entities again, and fullscreen-phase authored entities bypass the normal spatial visual bootstrap gate.
Scope: `bins/sidereal-client` and `bins/sidereal-replication`

## 1. Purpose

This audit complements the Lightyear feature audit. The earlier audit focused on whether Sidereal was using the right Lightyear features and where it diverged from recommended patterns.

This document focuses on a different failure mode:

- systems that mutate motion/render state in the wrong schedule,
- systems that overwrite state they do not own,
- systems that sample a transform before another runtime path finishes updating it,
- Sidereal-specific exceptions that are intentional and must stay documented.

This audit exists because several native-runtime bugs were not "Lightyear is missing a feature" problems. They were ownership/order bugs inside Sidereal's own systems.

## 2. Audit Method

The audit reviewed:

- client plugin scheduling in [plugins.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/plugins.rs)
- client app wiring in [mod.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/mod.rs)
- client transform/motion/render systems in:
  - [replication.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/replication.rs)
  - [motion.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/motion.rs)
  - [transforms.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/transforms.rs)
  - [camera.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/camera.rs)
  - [visuals.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/visuals.rs)
- server plugin scheduling in [plugins.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/plugins.rs) and [main.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/main.rs)
- server control/replication/visibility systems in:
  - [control.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/control.rs)
  - [simulation_entities.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/simulation_entities.rs)
  - [visibility.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/visibility.rs)
  - [auth.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/auth.rs)
  - [combat.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/combat.rs)

## 3. Client Ownership Map

### 3.1 Fixed-step motion writers

These are the only systems that should affect authoritative local controlled motion on the client:

- [motion.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/motion.rs): `apply_predicted_input_to_action_queue`
- shared gameplay in [mod.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/mod.rs):
  - `process_character_movement_actions`
  - `process_flight_actions`
  - `apply_engine_thrust`
  - `update_ballistic_projectiles`
- [motion.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/motion.rs): `enforce_controlled_planar_motion`
- shared gameplay in [mod.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/mod.rs):
  - `stabilize_idle_motion`
  - `clamp_angular_velocity`

Important invariant:

- for a controlled predicted entity, only one fixed-tick pipeline may write `Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`

### 3.2 Update/PostUpdate transform writers

These systems write `Transform` or follow transforms but are not authoritative motion writers:

- [transforms.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/transforms.rs)
  - confirmed-only transform sync
  - interpolated no-history bootstrap
- [camera.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/camera.rs)
  - gameplay camera transform
  - UI/debug overlay camera transforms
- [visuals.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/visuals.rs)
  - streamed visual child transforms
  - planet visual pass transforms
  - thruster plume transforms
  - projectile/tracer/impact VFX transforms
- [ui.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/ui.rs)
  - screen-space overlay transforms only
- [backdrop.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/backdrop.rs)
  - fullscreen/background camera-space transforms only

### 3.3 Control/runtime tag writers

These systems do not own motion directly but decide which entity is allowed to own it:

- [replication.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/replication.rs): `sync_controlled_entity_tags_system`
- [motion.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/motion.rs): `enforce_motion_ownership_for_world_entities`

These are high-risk systems because a wrong decision here produces "feels jerky" symptoms even when the physics/prediction code itself is correct.

## 4. Server Ownership Map

### 4.1 Fixed-step simulation/motion writers

- shared gameplay systems configured in [main.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/main.rs)
- [simulation_entities.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/simulation_entities.rs): `enforce_planar_motion`

### 4.2 Replication target/control writers

- [control.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/control.rs)
  - clears previous controlled binding
  - applies owner prediction/interpolation mode to player anchor
  - receives client control requests
- [simulation_entities.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/simulation_entities.rs)
  - `apply_pending_controlled_by_bindings`
  - inserts `ControlledBy`, `Replicate`, `PredictionTarget`, `InterpolationTarget`
- [auth.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/auth.rs)
  - owner-only player anchor replication on authenticated bind
- [combat.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/combat.rs)
  - projectile prediction/interpolation targeting

### 4.3 Visibility/sender-state writers

- [visibility.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/visibility.rs)
  - mutates `ReplicationState` per client each fixed tick
- [control.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/control.rs)
  - sender-local respawn cycle on control handoff
- [simulation_entities.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/simulation_entities.rs)
  - sender-local respawn after controlled binding applied

These are intentional writers. They are not themselves a bug, but they are a critical part of Sidereal's dynamic handoff exception.

## 5. Findings

### 5.1 Fixed: client camera sampled the ship before Lightyear's final visual transform finished

Severity: high

Problem:

- Lightyear `FrameInterpolationPlugin::<Transform>` runs in `PostUpdate`, before `TransformSystems::Propagate`
- Lightyear prediction correction can then still mutate the predicted `Transform` in `RollbackSystems::VisualCorrection`
- Sidereal gameplay camera follow originally ran in `Update`, and later still only waited for frame interpolation
- Result: camera could lock to a pre-correction predicted ship transform, while the rendered ship used the final post-interpolation/post-correction transform later in the same frame

Symptom:

- the ship appears to move/jitter/rubber-band on screen even against a plain solid background
- this can happen even when prediction itself is active and rollback is not firing

Fix:

- moved camera follow/sync systems to `PostUpdate`
- ordered them after `FrameInterpolationSystems::Interpolate`
- ordered them after `RollbackSystems::VisualCorrection`
- kept them before `TransformSystems::Propagate`

Relevant code:

- [plugins.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/plugins.rs)

Why this was missed by the earlier Lightyear audit:

- this is a Sidereal schedule/ownership problem, not a "missing Lightyear feature" problem

### 5.2 Fixed earlier: player-anchor replication mode was being rewritten every fixed tick on server

Severity: high

Problem:

- [control.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/control.rs): `sync_player_anchor_replication_mode`
- previously reinserted `Replicate` / `PredictionTarget` / `InterpolationTarget` continuously
- that can fight Lightyear's hook-driven sender-state lifecycle

Fix:

- made the system idempotent
- switched owner-specific targeting to sender-entity `manual(vec![client_entity])`

Why this is a Sidereal exception:

- dynamic handoff already knows the exact sender entity
- using sender-entity targeting avoids a second remote-id resolution dependency during handoff

### 5.3 Fixed earlier: dynamic handoff required sender-local respawn to re-enter Lightyear spawn classification

Severity: high

Problem:

- Lightyear applies `Predicted` / `Interpolated` classification from the receiver's spawn action
- Sidereal can change the owner prediction lane after the entity is already visible

Fix:

- sender-local respawn transition by cycling `ReplicationState` visibility `Lost -> Gained` for the affected client after handoff

Relevant code:

- [control.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/control.rs)
- [simulation_entities.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/simulation_entities.rs)

### 5.4 Confirmed risk: streamed world-visual child transforms are camera-relative writers

Severity: medium

Problem:

- [visuals.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/visuals.rs): `update_streamed_visual_layer_transforms_system`
- rewrites `StreamedVisualChild` transforms every frame from `camera_motion.world_position_xy` and render-layer parallax

Current status:

- this is required for true parallax world layers
- for entities on the default `main_world` layer (`parallax_factor = 1.0`), the offset is zero and this is harmless
- for any controlled/root gameplay entity that ends up on a non-`1.0` parallax world layer, the visible child can drift relative to the parent transform

Action:

- treat this as a hard rule: controlled ships and other physics roots must remain on world layers with `parallax_factor = 1.0`
- if future content intentionally breaks that rule, the visual-child transform code must special-case controlled/local predicted roots

### 5.5 Confirmed complexity hotspot: duplicate visual suppression still arbitrates clone winners in the render layer

Severity: medium

Problem:

- [visuals.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/visuals.rs): `suppress_duplicate_predicted_interpolated_visuals_system`
- this still decides which clone is visible per GUID

### 5.6 2026-03-09 update: interpolated observer reveal and transform recovery are intentional Sidereal safeguards

Status update: 2026-03-09

Problem:

- fresh observer entities could stay hidden until the remote source moved again because initial reveal waited for interpolation history or confirmed wrappers, even when a valid current pose was already present
- observer `Transform` could also diverge badly from interpolated `Position`/`Rotation`, producing "freeze then catch up" visuals for remote ships

Fix:

- [transforms.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/transforms.rs):
  - `reveal_world_entities_when_initial_transform_ready` now allows interpolated entities to reveal from their current sampled pose, not only from history/confirmed state
  - `sync_frame_interpolation_markers_for_world_entities` keeps `FrameInterpolate<Transform>` aligned with late-arriving spatial clones
  - `recover_stalled_interpolated_world_entity_transforms` is a narrow late-frame safeguard that only re-seeds/snap-recovers observer transforms when the Lightyear visual lane is obviously stale or uninitialized

Why this is a Sidereal-specific exception:

- stock Lightyear examples do not combine dynamic predicted-entity handoff, persistent free-roam player anchors, and strict visibility-driven clone churn in the same way
- Sidereal therefore needs a small amount of defensive client-side recovery logic around observer presentation
- this recovery should remain narrow and should not become a general replacement for Lightyear's normal interpolation path

### 5.7 2026-03-09 update: debug overlay root selection must not depend on render culling

Status update: 2026-03-09

Problem:

- collision AABB debug overlay winner selection was using `ViewVisibility`
- that ties debug rendering to ordinary view-layer/culling churn instead of stable gameplay-root identity
- the result can look like AABBs are flickering even when the underlying collision data is stable

Fix:

- [debug_overlay.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/debug_overlay.rs): `draw_debug_overlay_system`
- root winner selection now respects explicit `Visibility::Hidden` but does not use `ViewVisibility` as an eligibility gate

Why this is correct:

- the debug overlay is a diagnostic projection of gameplay state, not a gameplay renderable
- it should track stable logical winners per GUID, not camera culling state

### 5.6 Fixed: debug overlay was drawing stable duplicate runtime roots instead of one logical winner per GUID

Severity: medium

Problem:

- [debug_overlay.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/debug_overlay.rs): `draw_debug_overlay_system`
- the overlay queried all `WorldEntity` roots and drew collision AABBs/outlines from every matching copy
- in Sidereal, a logical entity can legitimately have:
  - a confirmed replicated root
  - a predicted clone for the owner
  - an interpolated clone for observers
- BRP on client 1 showed this concretely for the local corvette: both a confirmed root and a predicted root were present at the same time

Symptom:

- debug collision AABBs appear to flash or double-draw even when the underlying collision-bearing world entities are stable
- this is especially misleading because the render layer already suppresses duplicate visuals, but the debug overlay previously did not

Fix:

- the debug overlay now resolves one root winner per gameplay GUID before drawing
- hidden or non-view-visible roots are skipped
- the local predicted ship still gets its explicit confirmed ghost from `Confirmed<T>` on the predicted clone, instead of drawing the plain confirmed duplicate as a second live box

Important distinction:

- this was not caused by the anonymous render-only BRP entities
- the client currently keeps a pool of 96 hidden tracer-bolt render entities, which show up in BRP as anonymous `Aabb`/`Transform`/`Visibility` nodes because their gameplay-only tracer marker is not exposed there
- those entities are stable and are not the source of collision AABB debug flashing

Why it matters:

- if clone lifecycle drifts again, visuals can look wrong before the underlying simulation state is obviously wrong
- render-layer arbitration is a late symptom suppressor, not a substitute for clean predicted/interpolated lifecycle

### 5.6 Client adoption path remains confirmed-entity centric

Severity: medium

Problem:

- [replication.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/replication.rs): `adopt_native_lightyear_replicated_entities`
- canonical runtime mapping is pinned to the confirmed `Replicated` entity
- predicted/interpolated clone resolution is done later by GUID queries

Why it is acceptable:

- this is currently intentional and matches Sidereal's world-entity/adoption model

Why it remains risky:

- any downstream system that assumes the confirmed root is the only visually relevant root can reintroduce clone desync bugs

### 5.7 Server transform sync systems are not the current jitter source

Severity: low

Observed systems:

- [simulation_entities.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/simulation_entities.rs): `sync_controlled_entity_transforms`
- [simulation_entities.rs](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/simulation_entities.rs): `sync_world_entity_transforms_from_world_space`

Assessment:

- these are server-side transform mirrors for world/debug/hydration consistency
- they are not writing client-render transforms directly
- they are not the main cause of the currently observed client jitter

## 6. Rules Going Forward

### 6.1 Client rules

- Camera follow of predicted/interpolated gameplay entities must sample the same post-interpolation transform that will be rendered that frame.
- No client system in `Update` should assume it is seeing the final transform for a predicted entity if Lightyear frame interpolation runs later in `PostUpdate`.
- Controlled/local-predicted world visuals must not inherit non-`1.0` parallax camera offsets.
- Render suppression systems may hide duplicates, but they must not become the mechanism that defines simulation ownership.

### 6.2 Server rules

- Dynamic handoff may use sender-entity `manual(...)` replication targets when the concrete `ClientOf` sender is already known.
- Systems that continuously reevaluate owner control state must be idempotent and must not blindly reinsert Lightyear target components every tick.
- Sender-local respawn during handoff is an allowed Sidereal exception because ownership can change after initial visibility.

## 7. Remaining Audit Targets

These areas still deserve follow-up if jitter remains after the camera schedule fix:

- whether any controlled ship visuals are assigned to a non-`main_world` render layer
- whether any UI/nameplate/screen overlay path is visually anchored to the wrong clone
- whether any predicted-entity render transform is still being stepped at fixed-tick cadence despite `FrameInterpolate<Transform>`
- whether any server visibility churn still causes avoidable duplicate spawn/despawn behavior under rapid handoff

## 8. Relationship To The Lightyear Audit

Use this document together with:

- [lightyear_audit_report_2026-03-08.md](/home/toby/dev/sidereal_v3/docs/reports/lightyear_audit_report_2026-03-08.md)

The Lightyear audit answers:

- are we using the right Lightyear features and patterns?

This audit answers:

- are Sidereal's own client/server systems respecting single-writer ownership and schedule alignment once those features exist?

# Prediction, Handoff, Avian, and Visibility Foundation Refactor Plan

- Date: 2026-03-22
- Status: Proposed active redesign plan
- Scope: Native client prediction/interpolation/control lifecycle, Lightyear fork integration, Avian2D bootstrap correctness, and retention of Sidereal's server-authoritative visibility model

## 1. Problem Statement

Sidereal's current multiplayer runtime is not failing because "rendering is a bit slow." It is failing because the control, prediction, confirmed, interpolated, and Avian motion lanes are allowed to enter invalid transitional states and then rely on local repair systems to stay usable.

The clearest current symptoms are:

1. local inputs can appear delayed or frozen after focus churn / alt+tab,
2. rollback aborts (`Trying to do a rollback of N ticks. The max is 100! Aborting`) can leave the locally controlled predicted ship visually stalled while the authoritative/server ghost continues moving,
3. predicted/interpolated control handoff is not robust on existing entities,
4. observer clients can fail to see authoritative movement after ownership/control changes,
5. Avian2D-backed entities can appear at `0,0` during bootstrap or late relevance gain,
6. the client still runs broad repair systems every frame to hide duplicate-lane and bootstrap instability.

This plan treats those as one foundation problem:

`resolved control target` is currently allowed to diverge from `healthy predicted/confirmed/interpolated lane state`.

That contract is not acceptable for a server-authoritative MMO foundation.

## 2. What Sidereal Must Retain

This refactor must preserve the following project requirements. These are not optional:

1. server-authoritative simulation and disclosure,
2. dynamic control handoff between existing entities,
3. Avian2D as the authoritative runtime physics layer for dynamic simulated entities,
4. separate visibility concerns:
   - authorization: what a client is allowed to know,
   - delivery narrowing: what we currently choose to send/render for performance,
5. spatial partitioning / relevance culling as MMO-scale optimization,
6. player-anchor semantics and free-roam support,
7. owner-only, observer, tactical, and manifest lanes from the visibility contract,
8. shared gameplay/prediction code that remains compatible with later WASM parity work.

The canonical visibility ordering from [visibility_replication_contract.md](../features/visibility_replication_contract.md) remains mandatory:

1. Authorization
2. Delivery
3. Payload

Delivery optimization must never widen authorization.

## 3. Current Evidence and Code Fault Lines

### 3.1 Client control currently binds around lane availability instead of around lane health

In [`bins/sidereal-client/src/runtime/replication.rs`](../../bins/sidereal-client/src/runtime/replication.rs), [`sync_controlled_entity_tags_system`](../../bins/sidereal-client/src/runtime/replication.rs) chooses a control target by scanning matching GUID clones and preferring:

1. `Predicted`
2. `Interpolated`
3. confirmed-only fallback

The current code already contains a defensive warning:

```rust
bevy::log::warn!(
    "controlled runtime target {} has no Predicted clone yet; refusing to bind local control to confirmed/interpolated fallback",
    missing_id
);
```

Reference:

- [`bins/sidereal-client/src/runtime/replication.rs:1376`](../../bins/sidereal-client/src/runtime/replication.rs)

That warning is correct, but it is still operating too late in the lifecycle. The system is attempting to discover whether a valid control lane exists by scanning clone state after the fact.

### 3.2 Client motion ownership still performs broad reconciliation because runtime lanes are not deterministic enough

[`enforce_motion_ownership_for_world_entities`](../../bins/sidereal-client/src/runtime/motion.rs) still reconciles motion ownership by scanning root world entities, inspecting `Predicted`/`Interpolated`, and dynamically stripping or inserting motion-related state.

Reference:

- [`bins/sidereal-client/src/runtime/motion.rs:67`](../../bins/sidereal-client/src/runtime/motion.rs)

This is a containment system, not the ideal architecture.

### 3.3 Client presentation still carries duplicate suppression and transform recovery as ongoing runtime behavior

The rendering audit correctly identified these systems as architectural debt rather than "nice stabilizers":

- duplicate winner arbitration:
  - [`bins/sidereal-client/src/runtime/visuals.rs:531`](../../bins/sidereal-client/src/runtime/visuals.rs)
- interpolated transform recovery:
  - [`bins/sidereal-client/src/runtime/transforms.rs:309`](../../bins/sidereal-client/src/runtime/transforms.rs)
- predicted transform recovery:
  - [`bins/sidereal-client/src/runtime/transforms.rs:373`](../../bins/sidereal-client/src/runtime/transforms.rs)

The target architecture is not "keep these forever but tune them." The target architecture is to make these unnecessary outside rare diagnostics.

### 3.4 Server control handoff currently reclassifies replication roles by mutating targets and rearming visibility

[`reconcile_control_replication_roles`](../../bins/sidereal-replication/src/replication/control.rs) mutates:

1. `ControlledBy`
2. `PredictionTarget`
3. `InterpolationTarget`
4. `Replicate`

and then explicitly rearms visible clients with:

```rust
replication_state.lose_visibility(*client_entity);
replication_state.gain_visibility(*client_entity);
```

References:

- [`bins/sidereal-replication/src/replication/control.rs:519`](../../bins/sidereal-replication/src/replication/control.rs)
- [`bins/sidereal-replication/src/replication/control.rs:534`](../../bins/sidereal-replication/src/replication/control.rs)

This is the strongest current integration stress point. It is exactly where Sidereal is pushing Lightyear beyond the simple static-ownership pattern used by the demo.

### 3.5 Fixed-step authoritative flight still violates the repo's own fixed-time rule

The Rust audit correctly flagged that [`apply_engine_thrust`](../../crates/sidereal-game/src/flight.rs) still reads `Res<Time>` instead of `Res<Time<Fixed>>`.

Reference:

- [`crates/sidereal-game/src/flight.rs:162`](../../crates/sidereal-game/src/flight.rs)

This is not the whole prediction failure, but it is part of the same foundation problem: authoritative and predicted motion paths must be mechanically fixed-step correct before higher-level reconciliation can be trusted.

## 4. What Lightyear and Bevy Are Already Telling Us

### 4.1 Lightyear's `spaceships` demo is using a simpler, more reliable ownership contract

The working demo in `/home/toby/dev/lightyear/demos/spaceships` does a few things Sidereal currently does not:

1. prediction ownership is mostly static from spawn,
2. client-side control binding happens on `On<Add, (Player, Predicted)>`,
3. the demo uses a non-zero fixed input delay,
4. gameplay movement avoids `Interpolated` entities rather than supporting broad transitional states.

Relevant references in the fork:

- `demos/spaceships/src/main.rs`
- `demos/spaceships/src/server.rs`
- `demos/spaceships/src/client.rs`
- `demos/spaceships/src/shared.rs`

The lesson is not "Sidereal should become static-ownership only." The lesson is:

`Predicted` must become a bootstrap-complete lifecycle state, not a marker we hope will show up after role churn.

### 4.2 Lightyear itself still documents the exact transition gap Sidereal is hitting

The fork still contains the interpolation TODO:

> if `Interpolated` is added on an existing entity, we need to swap all its existing interpolated components to `Confirmed<C>`

Reference:

- `/home/toby/dev/lightyear/lightyear_interpolation/src/plugin.rs`

That is directly relevant. Sidereal relies on role changes for existing replicated entities, not only on spawn-time role assignment.

### 4.3 Bevy's fixed-step guidance matches the architecture Sidereal needs

Bevy's fixed-step guidance is explicit:

1. simulation should run in fixed schedules,
2. visual transforms should interpolate from physical state,
3. cameras should follow interpolated visuals rather than feeding back into simulation state.

Primary references:

- Bevy fixed timestep example: <https://bevy.org/examples/movement/physics-in-fixed-timestep/>
- Bevy `Time<Fixed>` docs: <https://docs.rs/bevy/latest/bevy/time/struct.Fixed.html>

This aligns with the repo's own rule:

- simulation/prediction math must use fixed-step resources only,
- visual correction must not become a hidden simulation writer.

### 4.4 MMO topology direction: server-authoritative interest management is still correct

Sidereal's topology direction remains sound:

1. server authoritative simulation,
2. per-client relevance / visibility filtering,
3. owner-specific and observer-specific lanes,
4. spatial partitioning for candidate generation,
5. dynamic control over existing entities.

What must change is not the MMO topology. What must change is the lifecycle contract between:

1. server role assignment,
2. replication delivery,
3. client bootstrap of predicted/interpolated/confirmed/Avian state.

This is an inference from the repo's own design and current runtime behavior, validated against Lightyear and Bevy guidance.

## 5. Root Cause Summary

Sidereal currently relies on three assumptions that are not safe:

1. mutating `PredictionTarget` / `InterpolationTarget` on the server is "close enough" to creating a bootstrap-ready owner or observer lane on the client,
2. client control can be resolved from GUID and clone preference alone,
3. Avian spatial/bootstrap state can be repaired locally after relevance/handoff without undermining prediction correctness.

Those assumptions produce the observed failures:

1. `alt+tab` / focus churn lets the owner timeline outrun confirmed state until rollback exceeds budget,
2. predicted-to-interpolated and interpolated-to-predicted handoff can leave entities half-bootstrapped,
3. observer entities can miss valid motion/bootstrap history and remain visually stale or origin-snapped,
4. the client compensates with duplicate suppression, hidden-until-ready logic, and transform recovery scans.

## 6. Target Architecture

## 6.1 Hard design rule

Sidereal should never treat:

`controlled GUID resolved`

as equivalent to:

`predicted control lane is valid and ready`.

The target lifecycle is:

1. server grants a control lease,
2. server applies owner/observer replication role changes,
3. client receives the lane and waits for bootstrap completeness,
4. only then does the client bind local simulation writers and camera/HUD control.

## 6.2 Explicit control lifecycle

Introduce explicit lifecycle resources/components in Sidereal instead of inferring control solely from clone presence.

Example target shape on the client:

```rust
#[derive(Resource, Debug, Clone)]
pub struct ControlBootstrapState {
    pub desired_guid: Option<Uuid>,
    pub generation: u64,
    pub active: ClientControlState,
}

#[derive(Debug, Clone)]
pub enum ClientControlState {
    Idle,
    PendingBootstrap {
        guid: Uuid,
        generation: u64,
        since_tick: u32,
    },
    ActivePredicted {
        guid: Uuid,
        entity: Entity,
        generation: u64,
    },
}
```

Example target shape on the server:

```rust
#[derive(Component, Debug, Clone)]
pub struct ActiveControlLease {
    pub client_entity: Entity,
    pub controlled_guid: Uuid,
    pub generation: u64,
}
```

The important property is not the exact type names. It is that control lease, replication role mutation, and local writer binding become separate stages with explicit success conditions.

## 6.3 Bootstrap completeness contract

For a dynamic root entity to become locally controlled, all of the following must be true:

1. `Predicted` exists,
2. the entity has authoritative spatial components required for Avian simulation:
   - `Position`
   - `Rotation`
   - `LinearVelocity`
   - `AngularVelocity`
3. corresponding confirmed baseline exists for reconciliation:
   - `Confirmed<T>` for motion components,
   - `ConfirmedTick`,
   - any required history/buffer components used by the fork,
4. simulation-writer ownership is unique,
5. camera and presentation bind to the same canonical predicted root.

For an observer entity to become presentable, all of the following must be true:

1. `Interpolated` exists,
2. confirmed components and interpolation history exist,
3. Avian/world spatial components are initialized from authoritative state,
4. `Transform`/frame interpolation lane is seeded before the entity is shown.

## 6.4 Avian2D contract

Avian2D must remain the simulation authority for dynamic simulated roots, but it must not be asked to bootstrap from incomplete replication state.

Normative direction:

1. dynamic simulated roots always bootstrap from replicated motion state before visual visibility is enabled,
2. remote observer roots do not appear with a live `Dynamic` body at origin while waiting for authoritative position,
3. role-transitioned entities must receive the same Avian bootstrap guarantees as spawn-time entities,
4. visual transform recovery systems remain temporary diagnostics, not the steady-state contract.

## 6.5 Visibility contract retention

Sidereal must keep:

1. authorization as the source of truth,
2. delivery narrowing as optimization only,
3. payload redaction as a later stage,
4. player-anchor observer identity,
5. spatial partitioning / grid-based candidate generation.

What changes is how role churn interacts with visibility:

1. visibility should stop being used as a blunt re-bootstrap hammer for every role change,
2. owner/observer role changes need a deterministic per-entity bootstrap path,
3. visibility rearm should become the exception, not the primary control-handoff mechanism.

## 7. Refactor Strategy

## Phase 0: Freeze Symptoms and Establish Validation Gates

Before major refactor work, lock the current repros into explicit validation scenarios:

1. two local clients, two ships, same server, fresh DB,
2. late join after one client moves far from origin,
3. alt+tab / focus churn on the controlling client,
4. control swap:
   - player anchor -> ship,
   - ship -> player anchor,
   - ship A -> ship B,
5. reconnect with persisted controlled ship after server restart.

Success criteria for every phase:

1. no rollback abort spam,
2. no remote dynamic root visible at `0,0` without authoritative reason,
3. observer clients see authoritative movement after owner input,
4. local predicted control does not bind without a valid predicted lane,
5. duplicate suppression and transform recovery move toward zero steady-state activity.

## Phase 1: Patch the Lightyear Fork for Existing-Entity Lane Transition Correctness

Primary target: `/home/toby/dev/lightyear`

This phase fixes the generic problem in the networking library layer instead of adding more Sidereal-side shims.

### 7.1.1 Required fork work

1. complete existing-entity `Interpolated` bootstrap:
   - ensure `Confirmed<T>` mirrors exist when `Interpolated` is added late,
   - ensure `ConfirmedHistory<T>` exists and is seeded consistently,
2. audit existing-entity `Predicted` bootstrap:
   - verify current state/history/tick bootstrap when `Predicted` is added after spawn,
   - add deterministic tests for role-switching on existing entities,
3. verify Lightyear Avian integration during role changes:
   - `Position`/`Rotation` sync,
   - child collider updates,
   - transform sync order,
4. confirm whether owner/observer lane switching requires a first-class transition helper in the fork.

### 7.1.2 Strong candidate fix already identified

The fork branch `fix/interpolated-handoff-confirmed-history` already contains a generic improvement for interpolation history initialization when `Interpolated` is added after `Confirmed<C>`.

That should be kept and expanded rather than replaced with more Sidereal-local fallback logic.

### 7.1.3 Deliverables

1. fork-level tests that cover:
   - owner predicted root on spawn,
   - existing predicted root becoming interpolated,
   - existing interpolated root becoming predicted,
   - observer movement after owner control change,
2. Lightyear-side bootstrap helper(s) for existing-entity role changes,
3. no Sidereal-specific visibility or player-anchor logic in the fork patch.

## Phase 2: Redesign Sidereal Control Around Leases and Bootstrap States

After the fork can support existing-entity lane transitions, redesign Sidereal's control layer around explicit control bootstrap states.

### 7.2.1 Server changes

In [`bins/sidereal-replication/src/replication/control.rs`](../../bins/sidereal-replication/src/replication/control.rs):

1. keep authenticated session binding checks,
2. keep player-anchor semantics,
3. replace implicit "desired owner by entity" behavior with explicit lease generation,
4. separate:
   - control lease assignment,
   - replication target mutation,
   - client-visible control ack readiness.

Example direction:

```rust
pub enum ControlLeaseStage {
    Requested,
    ReplicationRolesApplied,
    ClientBootstrapReady,
    Active,
}
```

The server should not report control fully active until the replication role transition has become consistent enough for the client to bind local writers.

### 7.2.2 Client changes

In [`bins/sidereal-client/src/runtime/replication.rs`](../../bins/sidereal-client/src/runtime/replication.rs):

1. stop resolving control from clone preference alone,
2. replace ad hoc target scans with a control-bootstrap state machine,
3. bind `ControlledEntity` and `SimulationMotionWriter` only after bootstrap completeness,
4. keep player-anchor free-roam as a distinct control mode, not as evidence that ship control can safely fall back to confirmed/interpolated state.

## Phase 3: Make Avian Bootstrap Explicit and Deterministic

Primary targets:

- [`bins/sidereal-client/src/runtime/motion.rs`](../../bins/sidereal-client/src/runtime/motion.rs)
- [`bins/sidereal-client/src/runtime/transforms.rs`](../../bins/sidereal-client/src/runtime/transforms.rs)
- Lightyear Avian integration in `/home/toby/dev/lightyear/lightyear_avian`

### 7.3.1 Required outcomes

1. no dynamic remote root appears at origin while waiting for first authoritative pose,
2. remote observer roots do not gain active motion ownership accidentally,
3. predicted roots and observer roots bootstrap Avian state through separate, explicit paths,
4. world-position/static-lane entities remain distinct from Avian dynamic roots.

### 7.3.2 Practical implementation direction

1. create a narrow "bootstrap pending" visual gate for dynamic entities,
2. seed motion state before enabling visible transform / render attachment,
3. stop using broad transform recovery as the normal bootstrap path,
4. keep recovery systems only as temporary assertions/failsafes until they can be removed.

## Phase 4: Reduce Role-Churn Dependence on Visibility Rearm

Primary targets:

- [`bins/sidereal-replication/src/replication/control.rs`](../../bins/sidereal-replication/src/replication/control.rs)
- [`bins/sidereal-replication/src/replication/visibility.rs`](../../bins/sidereal-replication/src/replication/visibility.rs)

### 7.4.1 Required outcomes

1. control handoff no longer depends on `lose_visibility` / `gain_visibility` as the default repair path,
2. visibility remains authoritative and scalable,
3. spatial partitioning stays the candidate-generation optimization layer,
4. static landmark discovery and dynamic visibility continue to obey the documented contract.

### 7.4.2 Related audit work to fold in

The Rust audit's Finding D1 should be included in this refactor:

1. static landmark discovery should reuse the maintained visibility spatial index instead of rebuilding parallel indexing work.

That work belongs in the same visibility/control cleanup window because it reduces hot-path complexity in the same runtime area.

## Phase 5: Remove or Shrink the Current Repair Layer

Only after Phases 1 through 4 are stable should Sidereal remove the current repair stack.

Priority candidates for deletion or major reduction:

1. duplicate visual winner arbitration:
   - [`bins/sidereal-client/src/runtime/visuals.rs:531`](../../bins/sidereal-client/src/runtime/visuals.rs)
2. broad motion ownership reconciliation:
   - [`bins/sidereal-client/src/runtime/motion.rs:67`](../../bins/sidereal-client/src/runtime/motion.rs)
3. interpolated transform recovery:
   - [`bins/sidereal-client/src/runtime/transforms.rs:309`](../../bins/sidereal-client/src/runtime/transforms.rs)
4. predicted transform recovery:
   - [`bins/sidereal-client/src/runtime/transforms.rs:373`](../../bins/sidereal-client/src/runtime/transforms.rs)

Success here is not merely "the game still works." Success is that these systems stop carrying core correctness.

## Phase 6: Fold in Adjacent Foundation Fixes From the Audits

The following audit findings should be treated as part of this refactor, not as unrelated cleanup:

1. fix authoritative flight to use `Res<Time<Fixed>>`:
   - [`crates/sidereal-game/src/flight.rs:162`](../../crates/sidereal-game/src/flight.rs)
2. generalize mass bootstrap away from `ShipTag`-specific behavior:
   - [`crates/sidereal-game/src/mass.rs:232`](../../crates/sidereal-game/src/mass.rs)
3. split oversized hot-path modules where ownership boundaries are currently blurred:
   - `visibility.rs`
   - `visuals.rs`
4. reduce broad world-wide maintenance scans identified by the rendering audit once the lifecycle becomes deterministic.

## 8. Example Control-Bootstrap Flow

This is an illustrative target flow, not final code:

```rust
// Server fixed tick
fn advance_control_handoff(...) {
    // 1. validate request against authenticated player binding
    // 2. assign lease generation
    // 3. mutate owner/observer replication targets
    // 4. wait for role mutation confirmation / bootstrap-ready signal
    // 5. then mark lease active
}

// Client update
fn resolve_control_bootstrap(...) {
    match control_state.active {
        ClientControlState::PendingBootstrap { guid, generation, .. } => {
            if let Some(entity) = find_predicted_root_for_guid(guid) {
                if predicted_lane_bootstrap_complete(entity) {
                    commands.entity(entity).insert((ControlledEntity { .. }, SimulationMotionWriter));
                    control_state.active = ClientControlState::ActivePredicted {
                        guid,
                        entity,
                        generation,
                    };
                }
            }
        }
        _ => {}
    }
}
```

The critical behavior is:

1. no simulation writer before bootstrap completion,
2. no confirmed/interpolated fallback for non-anchor local ship control,
3. observer presentation can stay visible independently of owner control state,
4. local camera/UI binds to the same canonical runtime root as local motion ownership.

## 9. Validation Matrix

Each phase should be validated against:

1. fresh DB, client 1 logs in, moves away from origin, client 2 joins,
2. server restart without DB reset, both clients reconnect to persisted state,
3. alt+tab/focus churn while applying thrust and rotation,
4. player anchor free-roam to ship control and back,
5. ship-to-ship handoff,
6. observer visibility enter/leave/re-enter across spatial boundaries,
7. local host with two clients and one dedicated server,
8. native build first, then WASM compile validation for shared client/runtime changes.

Metrics to track:

1. rollback abort count,
2. controlled tick gap,
3. count of duplicate visual groups,
4. count of transform recovery interventions,
5. count of role-change visibility rearms,
6. count of dynamic roots shown before motion bootstrap completes.

## 10. Non-Goals

This plan does not propose:

1. replacing Lightyear wholesale right now,
2. dropping Avian2D,
3. removing Sidereal's authorization/delivery visibility contract,
4. flattening the game into static always-predicted ownership,
5. adding more permanent client-side shims as the primary fix.

Sidereal should be able to swap between predicted entities. The refactor goal is to make that true by design instead of by repair logic.

## 11. Initial Implementation Order

Recommended order:

1. patch the Lightyear fork for existing-entity lane transition correctness,
2. fix fixed-step authoritative motion correctness in shared gameplay,
3. redesign Sidereal control lease/bootstrap state machine,
4. make Avian bootstrap deterministic for dynamic roots,
5. reduce visibility rearm dependence during role changes,
6. delete or drastically shrink the current duplicate/repair systems,
7. then optimize remaining render- and visibility-side hot paths.

## 12. 2026-03-22 Status Note

Recent local stabilization work added:

1. trigger-based conflicting marker cleanup,
2. missing `Confirmed<T>` seeding for interpolated motion components,
3. non-zero native input delay tuning,
4. `Ctrl TickGap` overlay telemetry,
5. predicted/interpolated transform recovery safeguards.

Those changes were useful for diagnosis and short-term containment, but they do not change the redesign direction in this plan. The target remains to move generic lane-transition correctness into the Lightyear fork and to reduce Sidereal's steady-state dependence on repair systems.

2026-03-22 implementation update:

1. Phase 1 started:
   - `crates/sidereal-game/src/flight.rs` now uses `Res<Time<Fixed>>` for authoritative thrust.
   - `crates/sidereal-game/src/mass.rs` generalized the mass bootstrap entrypoint from ship-specific roots to generic dynamic rigid-body roots.
2. Phase 2 started:
   - `bins/sidereal-client/src/runtime/resources.rs` now defines explicit `ControlBootstrapState` / `ControlBootstrapPhase`.
   - `bins/sidereal-client/src/runtime/replication.rs` now keeps ship control in a pending bootstrap state until a real `Predicted` root exists.
   - targeted tests now cover "pending without predicted clone" and "bind only when predicted clone exists".
3. Phase 4 started:
   - `bins/sidereal-replication/src/replication/control.rs` now narrows visibility rearm to actual replication-topology changes.
4. Phase 5 has not started in earnest yet:
   - duplicate visual suppression and transform recovery are still present and should remain until the fork-level existing-entity lane transition work is complete.

# Lightyear Control Handoff, Prediction, and Visibility Refactor Plan

Status: Active redesign / implementation plan  
Date: 2026-03-22  
Owners: client runtime + replication server + Lightyear fork + Avian2D integration

Update note (2026-04-24):
- The March 22 audit reports that originally informed this plan were removed during documentation cleanup because fresh 2026-04-23 reports supersede them.
- This plan remains active for control/prediction/visibility refactor direction; use the 2026-04-23 reports for current audit findings.

## 1. Purpose

Rebuild Sidereal's prediction/control/observer lifecycle onto a reliable multiplayer foundation that:

1. keeps server authority,
2. preserves Sidereal's visibility and disclosure contract,
3. supports Avian2D correctly,
4. supports dynamic control transfer between existing entities,
5. removes the need for broad client-side repair systems as the normal runtime contract.

This plan is explicitly not "add more shims until the demo feels okay".

The current behavior proves the underlying lifecycle is wrong:

1. two local dedicated clients can enter states where no valid `Predicted` control root exists even though a control GUID resolves,
2. owner prediction can exist while confirmed state diverges beyond rollback budget and Lightyear aborts reconciliation,
3. observers can fail to receive or present the same authoritative motion stream,
4. Avian2D bootstrap can still show spatial entities at `0,0`,
5. client presentation currently depends on duplicate suppression, transform recovery, and fallback arbitration.

Those are foundation problems, not polish problems.

## 2. Primary Inputs

Local project inputs:

1. `AGENTS.md`
2. `docs/features/visibility_replication_contract.md`
3. `docs/features/prediction_runtime_tuning_and_validation.md`
4. `docs/features/lightyear_upstream_issue_snapshot.md`
5. `docs/features/lightyear_integration_analysis.md`
6. `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-04-23.md`
7. `docs/reports/rust_codebase_audit_report_2026-04-23.md`
8. `docs/plans/multiplayer_prediction_interpolation_reliability_plan_2026-03-15.md`
9. `docs/reports/client_server_network_audit_report_2026-04-23.md`

External references used for this plan:

1. Lightyear book: Advanced systems  
   <https://cbournhonesque.github.io/lightyear/book/tutorial/advanced_systems.html>
2. Lightyear book: Visual interpolation  
   <https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/visual_interpolation.html>
3. Lightyear book: System order  
   <https://cbournhonesque.github.io/lightyear/book/concepts/bevy_integration/system_order.html>
4. Lightyear book: Interest management  
   <https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/interest_management.html>
5. Bevy `Time<Fixed>` docs  
   <https://docs.rs/bevy/latest/bevy/time/struct.Fixed.html>

## 3. Current Problems To Fix

The redesign is primarily scoped around these proven failures:

### 3.1 Input issues when alt+tabbing

Observed behavior:

1. owner client sends input,
2. server authoritative ship / confirmed ghost moves,
3. owner predicted ship stalls or diverges,
4. Lightyear aborts rollback because the gap exceeds the rollback budget.

Evidence:

1. `bins/sidereal-client/src/runtime/transport.rs`
2. `bins/sidereal-client/src/runtime/resources.rs`
3. `bins/sidereal-client/src/runtime/debug_overlay.rs`
4. `bins/sidereal-client/src/runtime/replication.rs`

Specific repo symptoms:

1. `Ctrl TickGap` exceeds rollback budget,
2. `lightyear_prediction::rollback` logs `Trying to do a rollback of 116-156 ticks. The max is 100! Aborting`,
3. focus churn correlates with divergence.

### 3.2 Prediction / interpolation handoff

Observed behavior:

1. control GUID resolves before a healthy predicted root exists,
2. existing entities can transition between owner-predicted and observer-interpolated roles,
3. role changes currently rely on runtime mutation of `PredictionTarget` / `InterpolationTarget` plus visibility rearm,
4. client logic then repairs mixed or incomplete lane state after the fact.

Evidence:

1. `bins/sidereal-replication/src/replication/control.rs`
2. `bins/sidereal-client/src/runtime/replication.rs`
3. `bins/sidereal-client/src/runtime/visuals.rs`
4. `bins/sidereal-client/src/runtime/motion.rs`
5. `docs/features/lightyear_upstream_issue_snapshot.md`
6. upstream Lightyear issue `#1034` `PredictionSwitching`

### 3.3 Avian2D bootstrap and `0,0` spatial state

Observed behavior:

1. remote / newly relevant / transitioned spatial entities can appear at `0,0`,
2. stationary entities can remain incorrect until later movement,
3. dynamic bodies or their visible transforms can start from uninitialized spatial state.

Evidence:

1. `bins/sidereal-replication/src/replication/visibility.rs`
2. `bins/sidereal-client/src/runtime/replication.rs`
3. `bins/sidereal-client/src/runtime/transforms.rs`
4. `docs/plans/multiplayer_prediction_interpolation_reliability_plan_2026-03-15.md`
5. upstream Lightyear issue `#1380`

### 3.4 Additional problems that must be fixed during this refactor

Pulled from the March 22 audit findings and retained as refactor concerns; use the 2026-04-23 audit reports for current evidence:

1. Fixed-step gameplay still reads generic `Time` in authoritative flight paths and must be migrated to `Time<Fixed>`.
   Source: superseded March 22 Rust audit, Finding A1; current follow-up is tracked through `docs/reports/rust_codebase_audit_report_2026-04-23.md`.
2. Client presentation currently depends on duplicate-lane repair and broad scans.
   Source: superseded March 22 rendering audit, Findings 1 and 2; current follow-up is tracked through `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-04-23.md`.
3. Visibility churn and spatial resend/rearm behavior are directly destabilizing presentation.
   Source: superseded March 22 rendering audit, Findings 1 and 12; current follow-up is tracked through `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-04-23.md`.
4. Static landmark discovery currently duplicates spatial indexing work and should converge on one maintained visibility index.
   Source: superseded March 22 Rust audit, Finding D1; current follow-up is tracked through `docs/reports/rust_codebase_audit_report_2026-04-23.md`.

## 4. Sidereal Requirements That Must Be Retained

This plan must preserve the following non-negotiable Sidereal needs.

### 4.1 Server authority

Keep:

1. `client input -> shard sim -> replication/distribution -> persistence`
2. authenticated session binding for inputs
3. UUID/entity-id boundary rules

### 4.2 Dynamic control transfer

Sidereal must still support:

1. player anchor control,
2. ship control,
3. ship-to-anchor and anchor-to-ship switching,
4. future ship-to-ship switching between already-existing entities.

The redesign must not fall back to "prediction only works if the entity was predicted from spawn forever".

### 4.3 Avian2D

Keep:

1. authoritative Avian motion components as the canonical runtime motion state,
2. single-writer ownership for predicted controlled motion,
3. mass / inertia parity between gameplay and Avian,
4. deterministic bootstrap of dynamic bodies and transforms.

### 4.4 Visibility and disclosure

Keep the full contract in `docs/features/visibility_replication_contract.md`:

1. `Authorization -> Delivery -> Payload`
2. tactical lane and owner lane separation,
3. delivery culling as optimization only, not authorization widening,
4. player anchor as observer identity,
5. discovered landmark semantics,
6. spatial partitioning as the performance narrowing layer.

This refactor must not collapse "what the client may know" and "what the client should be sent right now" into one informal heuristic.

### 4.5 MMO-scale topology direction

Sidereal needs a topology that scales toward MMO behavior:

1. dedicated authoritative server / shard,
2. server-owned interest management,
3. bounded per-client local bubble,
4. lower-rate tactical/intel lanes,
5. owner-only side channels for private state,
6. no per-frame global client repair scans that grow with visible entity count.

Inference from MMO topology requirements:

1. prediction should stay limited to the locally controlled entity set,
2. observers should consume server-approved snapshots / deltas,
3. visibility membership and payload disclosure must be server-side,
4. client-side "discover all candidate entities and decide later" is not viable at MMO scale.

## 5. What `lightyear/demos/spaceships` Gets Right

The `spaceships` demo works because it uses Lightyear in a simpler, library-aligned way than Sidereal currently does.

Relevant references:

1. `/home/toby/dev/lightyear/demos/spaceships/src/server.rs:127`
2. `/home/toby/dev/lightyear/demos/spaceships/src/client.rs:67`
3. `/home/toby/dev/lightyear/demos/spaceships/src/shared.rs:128`
4. `/home/toby/dev/lightyear/demos/spaceships/src/main.rs:65`

Important lessons:

1. Prediction ownership is stable from spawn.
2. Input binding happens when `Predicted` actually exists.
3. Physics/movement systems cleanly avoid `Interpolated` entities.
4. Input delay is not zero.
5. The demo does not depend on duplicate visual suppression or transform recovery as a normal path.

Sidereal cannot copy the demo directly because it needs dynamic control transfer and visibility partitioning, but the redesign should make dynamic handoff behave like spawn-time prediction semantics:

1. activate owner lane first,
2. wait for valid predicted bootstrap,
3. then bind control,
4. never bind control to an entity whose lane is not ready.

## 6. Lightyear / Bevy Best-Practice Direction To Follow

### 6.1 Lightyear

From the Lightyear book:

1. prediction is for the entity the player controls,
2. interpolation is for entities shown slightly in the past,
3. fixed-step gameplay systems should run in the Lightyear/Bevy fixed schedule in the correct system set order,
4. input delay is a real tuning knob, not something to force to zero by default,
5. interest management should narrow replication on top of the chosen replication targets.

Implication for Sidereal:

1. local control should bind only to `Predicted`,
2. observer presentation should bind only to `Interpolated` or direct confirmed receive-only paths by explicit contract,
3. existing-entity role transitions must produce a fully bootstrapped target lane before gameplay/UI/control systems switch to it,
4. the Lightyear fork should own the generic transition-correctness logic where possible.

### 6.2 Bevy

From Bevy `Time<Fixed>` docs:

1. fixed-step simulation belongs in `FixedUpdate`,
2. systems in that schedule must use the fixed clock correctly,
3. simulation must not depend on render framerate.

Implication for Sidereal:

1. `crates/sidereal-game` authoritative systems must use fixed time resources consistently,
2. alt-tab/background/render pacing issues must not change authoritative gameplay math,
3. visual smoothing belongs in render/presentation lanes, not in authoritative physics state.

## 7. Redesign Goals

The refactor is complete only when these are true:

1. No controlled ship is ever considered active unless a bootstrap-ready predicted root exists.
2. Predicted, confirmed, and observer lanes stay on one coherent authoritative motion stream.
3. Existing-entity control handoff works without leaving mixed or half-bootstrapped lane state.
4. Observer clients see the same authoritative ship motion the owner's confirmed stream sees.
5. Avian2D spatial bootstrap never shows a newly visible / transitioned dynamic entity at `0,0`.
6. The runtime can remove:
   - `suppress_duplicate_predicted_interpolated_visuals_system`,
   - most transform recovery fallbacks,
   - broad winner arbitration as a steady-state requirement.
7. The design remains compatible with server-side authorization/delivery/payload visibility rules.

## 8. Non-Goals

This plan is not:

1. a rewrite away from Lightyear,
2. a removal of server-side visibility control,
3. a switch to client-authoritative movement,
4. a shortcut to "just raise rollback budget",
5. a plan to make host-client mode the reference correctness path.

## 9. Proposed Architecture

### 9.1 High-level model

Split the runtime into three explicit concerns:

1. `Control Lease`
   - which logical entity the server says this player currently controls
2. `Lane Bootstrap`
   - whether owner predicted and observer interpolated lanes are fully initialized for that logical entity on each client
3. `Presentation Ownership`
   - which local entity is allowed to drive:
     - input,
     - physics writes,
     - camera,
     - HUD,
     - rendered root

None of these should be inferred ad hoc from a GUID plus a pile of cleanup systems.

### 9.2 Server-side control lease state

Introduce explicit server-side state, conceptually:

```rust
struct ActiveControlLease {
    player_entity_id: PlayerEntityId,
    controlled_guid: uuid::Uuid,
    generation: u64,
}

struct PendingControlLease {
    player_entity_id: PlayerEntityId,
    requested_guid: uuid::Uuid,
    generation: u64,
}
```

Normative behavior:

1. Server validates requested control target ownership.
2. Server increments lease generation on accepted switch.
3. Server updates replication targets for the controlled entity and anchor.
4. Server re-arms visibility only for the affected owner/observer audience.
5. Server emits an explicit control-lease result message carrying generation.
6. Lease is not considered "healthy" until replication targets for that generation are active.

### 9.3 Client-side control bootstrap state

Introduce explicit client state, conceptually:

```rust
enum ControlBootstrapState {
    None,
    Pending {
        guid: uuid::Uuid,
        generation: u64,
        since_s: f64,
    },
    Active {
        guid: uuid::Uuid,
        generation: u64,
        predicted_entity: Entity,
    },
}
```

Normative behavior:

1. On login or control switch, client enters `Pending`.
2. Client waits for a predicted entity matching:
   - control GUID,
   - current generation,
   - required Avian motion state,
   - confirmed tick / confirmed component bootstrap.
3. Only when those are present does the client insert:
   - `ControlledEntity`
   - `SimulationMotionWriter`
   - input routing
   - camera lock
   - HUD ownership
4. If the predicted lane disappears or becomes invalid, client drops back to `Pending`, not to confirmed fallback control.

### 9.4 Lane bootstrap contract

For an entity to be bootstrap-ready in a lane:

#### Predicted owner lane

Required:

1. `Predicted`
2. Avian motion components present:
   - `Position`
   - `Rotation`
   - `LinearVelocity`
   - `AngularVelocity` when applicable
3. valid `Confirmed<T>` mirrors for rollback/correction comparisons
4. valid `ConfirmedTick`
5. any required `PredictionHistory` state for registered predicted components

#### Interpolated observer lane

Required:

1. `Interpolated`
2. required current spatial components present
3. valid `Confirmed<T>` mirrors
4. valid `ConfirmedTick`
5. `ConfirmedHistory<T>` created and seeded correctly for interpolated components
6. frame interpolation state seeded only after spatial bootstrap is valid

### 9.5 Avian2D ownership contract

The refactor must establish one writer per lane:

1. Predicted owner lane:
   - local fixed-step writes motion
   - server corrections update confirmed stream
2. Observer interpolated lane:
   - no local authoritative physics writes
   - local interpolation may drive visual representation only
3. Confirmed lane:
   - canonical replicated state storage
   - not a visible presentation winner for controlled ship motion

Normative rule:

1. No dynamic rigidbody should be active locally until the lane's spatial bootstrap is valid.
2. No "helpful" local fallback should create a dynamic body at `0,0`.

### 9.6 Visibility architecture to retain

Keep the current server contract:

1. authorization scope,
2. delivery scope,
3. payload scope,

in that order.

Refactor target:

1. visibility should choose which clients get which logical entities and lanes,
2. lane bootstrap should choose when those lanes are usable locally,
3. presentation should not re-decide visibility or authorization.

### 9.7 Spatial partitioning

Retain:

1. server-side spatial preselection,
2. local bubble delivery culling,
3. tactical low-rate lane,
4. landmark exceptions,
5. owner-only manifest lanes.

Refactor target:

1. spatial partitioning remains on the server,
2. bootstrap resends and lease rearms become explicit generation-driven events, not ad hoc churn.

## 10. Fork vs Sidereal Ownership Split

### 10.1 Patch in the Lightyear fork

These are generic enough to belong in `/home/toby/dev/lightyear`:

1. existing-entity interpolation bootstrap correctness,
2. existing-entity prediction bootstrap correctness,
3. correct `ConfirmedHistory` initialization when `Interpolated` is added after `Confirmed<C>`,
4. correct `ConfirmedTick` / history / component propagation on lane transitions,
5. generic prediction/interpolation switching support or a narrower internal equivalent.

Current fork branch already contains one upstreamable fix:

1. `/home/toby/dev/lightyear` branch `fix/interpolated-handoff-confirmed-history`
2. commit `c96ae904`
3. file `lightyear_interpolation/src/interpolation_history.rs`

Required next fork work:

1. investigate whether `Predicted` added on an existing entity also needs symmetrical bootstrap hooks,
2. add generic tests for existing-entity owner/observer role changes under dedicated client/server mode,
3. verify Avian2D integration during those transitions.

### 10.2 Keep in Sidereal

These remain Sidereal-specific:

1. player anchor semantics,
2. ship/anchor/ship control policy,
3. visibility/disclosure logic,
4. tactical and owner manifest lanes,
5. server auth and ownership validation,
6. MMO-side spatial partitioning policy.

## 11. Example Code Direction

### 11.1 Server-side control lease application

Illustrative sketch:

```rust
fn apply_control_lease_transition(
    lease: &PendingControlLease,
    commands: &mut Commands,
    membership_cache: &VisibilityMembershipCache,
    replication_state: &mut ReplicationState,
    controlled_entity: Entity,
    owner_client: Entity,
    observer_clients: &[Entity],
) {
    commands.entity(controlled_entity).insert((
        PredictionTarget::manual(vec![owner_client]),
        InterpolationTarget::manual(observer_clients.to_vec()),
        ControlLeaseGeneration(lease.generation),
    ));

    // Generation-scoped rearm, only for affected viewers.
    for client in std::iter::once(&owner_client).chain(observer_clients.iter()) {
        if replication_state.is_visible(*client) {
            replication_state.lose_visibility(*client);
            replication_state.gain_visibility(*client);
        }
    }
}
```

### 11.2 Client-side control binding

Illustrative sketch:

```rust
fn activate_control_when_predicted_ready(
    mut commands: Commands,
    mut bootstrap: ResMut<ControlBootstrapState>,
    predicted_roots: Query<(
        Entity,
        &EntityGuid,
        Has<Predicted>,
        Option<&Position>,
        Option<&Rotation>,
        Option<&LinearVelocity>,
        Option<&ConfirmedTick>,
    )>,
) {
    let ControlBootstrapState::Pending { guid, generation, .. } = *bootstrap else {
        return;
    };

    let Some((entity, ..)) = predicted_roots.iter().find(|(_, entity_guid, is_predicted, pos, rot, vel, tick)| {
        *is_predicted
            && entity_guid.0 == guid
            && pos.is_some()
            && rot.is_some()
            && vel.is_some()
            && tick.is_some()
    }) else {
        return;
    };

    commands.entity(entity).insert((ControlledEntity { /* ... */ }, SimulationMotionWriter));
    *bootstrap = ControlBootstrapState::Active { guid, generation, predicted_entity: entity };
}
```

The key point is not the exact code. The point is that control activation must be conditioned on lane readiness, not just target GUID knowledge.

## 12. Concrete Phase Plan

### Phase 0: Freeze, instrument, and stop adding new repair systems

Goals:

1. freeze the current baseline,
2. stop further local repair churn,
3. make the current failures measurable.

Tasks:

1. Keep `Ctrl TickGap` and rollback budget telemetry active.
2. Add targeted owner/observer motion replication logs for controlled ships.
3. Capture one reproducible dedicated-native script:
   - fresh DB,
   - client 1 login,
   - client 2 login,
   - owner movement,
   - alt-tab churn,
   - reconnect / second entry.
4. Mark which local repair systems are temporary and scheduled for deletion.

Exit criteria:

1. same repro is runnable after every phase,
2. no new presentation shim is added without being explicitly temporary in this plan.

### Phase 1: Fork-level lane transition correctness

Goals:

1. patch generic Lightyear transition gaps in the fork,
2. remove the need for app-side partial bootstrap fixes where the bug is generic.

Tasks in `/home/toby/dev/lightyear`:

1. upstream / retain `c96ae904` confirmed-history-on-interpolated-add fix.
2. Audit `Predicted`-added-on-existing-entity behavior:
   - `ConfirmedTick`
   - `PredictionHistory`
   - required replicated components
3. Add dedicated-server tests for:
   - existing entity becomes predicted for owner,
   - existing predicted entity becomes interpolated for observer,
   - Avian2D motion components remain valid through the transition.
4. Verify role transition on existing entity does not require app-side duplicate marker repair.

Exit criteria:

1. fork has dedicated tests covering existing-entity lane transitions,
2. generic bootstrap gaps are solved in the fork where possible.

### Phase 2: Replace GUID-first control binding with bootstrap-gated control activation

Goals:

1. control becomes a state machine,
2. no more confirmed/interpolated fallback for ship control.

Target files:

1. `bins/sidereal-client/src/runtime/replication.rs`
2. `bins/sidereal-client/src/runtime/motion.rs`
3. `bins/sidereal-client/src/runtime/control.rs`
4. `bins/sidereal-client/src/runtime/resources.rs`

Tasks:

1. Add explicit client `ControlBootstrapState`.
2. Separate:
   - desired control GUID,
   - authoritative control lease,
   - active predicted control entity.
3. Bind `ControlledEntity` / `SimulationMotionWriter` only when predicted bootstrap is valid.
4. Remove confirmed/interpolated fallback for non-anchor ship control.
5. Anchor free-roam remains an explicit separate lane/state.

Exit criteria:

1. no ship is ever `Controlled` without a valid predicted root,
2. first login and reconnect no longer produce "controlled guid resolved without a Predicted root".

### Phase 3: Server-side generation-scoped control lease and visibility rearm

Goals:

1. make server-side retargeting deterministic,
2. stop ad hoc role churn.

Target files:

1. `bins/sidereal-replication/src/replication/control.rs`
2. `bins/sidereal-replication/src/replication/visibility.rs`
3. `bins/sidereal-replication/src/replication/runtime_state.rs`

Tasks:

1. Add generation-scoped control lease state.
2. Apply `PredictionTarget` / `InterpolationTarget` only on lease changes.
3. Re-arm only the affected audience and only for the current generation.
4. Include generation in server control ack/result messages.
5. Ensure owner and observer motion delivery both target the same authoritative state stream.

Exit criteria:

1. control switches no longer rely on repeated target mutation every tick,
2. rearm churn becomes explicit, bounded, and testable.

### Phase 4: Avian2D bootstrap correctness

Goals:

1. no dynamic body at `0,0`,
2. no transform bootstrap from invalid motion state.

Target files:

1. `bins/sidereal-client/src/runtime/replication.rs`
2. `bins/sidereal-client/src/runtime/transforms.rs`
3. `bins/sidereal-client/src/runtime/motion.rs`
4. corresponding Lightyear fork Avian integration code if needed

Tasks:

1. Define exact bootstrap-ready rules for predicted and interpolated Avian roots.
2. Prevent dynamic rigidbody activation before valid spatial bootstrap.
3. Remove broad spatial repair once bootstrap rules are correct.
4. Add stationary late-join and transitioned-entity tests.

Exit criteria:

1. remote or transitioned entities never appear at `0,0`,
2. no Avian dynamic body activates from default-origin bootstrap.

### Phase 5: Delete client presentation repair layers

Goals:

1. make the runtime rely on clean lifecycle, not repair systems.

Target files:

1. `bins/sidereal-client/src/runtime/visuals.rs`
2. `bins/sidereal-client/src/runtime/transforms.rs`
3. `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs`

Tasks:

1. remove `suppress_duplicate_predicted_interpolated_visuals_system` once duplicate groups are no longer real state,
2. remove or narrow transform recovery systems to diagnostics-only fallbacks,
3. collapse broad winner arbitration into lifecycle-driven registries,
4. re-measure frame pacing.

Exit criteria:

1. these systems are deleted or reduced to bounded exceptional paths:
   - duplicate winner suppression,
   - most stalled transform repair,
   - hidden-until-ready hacks for normal steady state.

### Phase 6: Audit-driven adjacent fixes that must land inside this refactor

Tasks:

1. Change authoritative flight systems to use fixed time:
   - `crates/sidereal-game/src/flight.rs`
2. Reuse maintained visibility spatial index for landmark discovery instead of rebuilding parallel maps:
   - `bins/sidereal-replication/src/replication/visibility.rs`
3. Start splitting monolithic hot-path modules by domain ownership:
   - `visibility.rs`
   - `visuals.rs`
4. Replace broad per-frame scans with lifecycle registries where this refactor already touches those systems.

## 13. Success Metrics

This refactor is successful only if all of these pass:

1. Two dedicated local clients can log in from a fresh DB and both see correct initial positions.
2. After server restart and reconnect, both clients still receive valid predicted control roots.
3. Owner movement:
   - moves predicted root,
   - moves confirmed ghost,
   - is visible to observer.
4. `Ctrl TickGap` remains comfortably below rollback budget during normal local operation and under alt-tab churn.
5. No rollback abort spam occurs in nominal native local testing.
6. No client-side duplicate GUID winner arbitration is needed for normal ship presentation.
7. No spatial root appears at `0,0` waiting for later movement.

## 14. Validation Matrix

Required validation scenarios:

1. fresh DB, first login, one client
2. fresh DB, two clients, both stationary
3. client 1 moves, client 2 joins later
4. server restart, reconnect
5. ship -> free roam -> ship
6. ship A -> ship B control switch
7. alt-tab owner client during live thrust / rotation
8. observer client alt-tab during owner movement
9. stationary late-join asteroid / landmark / ship

## 15. Deliverables

This plan should produce:

1. Lightyear fork patches and tests in `/home/toby/dev/lightyear`
2. Sidereal server control lease refactor
3. Sidereal client control-bootstrap state machine
4. Avian2D bootstrap contract enforcement
5. removal of major repair systems
6. doc updates for:
   - `docs/features/prediction_runtime_tuning_and_validation.md`
   - `docs/features/visibility_replication_contract.md`
   - `AGENTS.md` if contributor rules change

## 16. Immediate Next Step

Start with Phase 1, not more local client repair:

1. upstream or retain the Lightyear fork confirmed-history transition fix,
2. add a dedicated fork-level test for existing-entity owner/observer switching with Avian2D,
3. only after that replace Sidereal's GUID-first control binding with bootstrap-gated activation.

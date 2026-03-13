# Lightyear Integration Analysis

Status: Investigation / evaluation  
Scope: Audit of current Lightyear usage vs available features; evaluation of full Lightyear interpolation adoption  
Lightyear version: 0.26.4

## 1. Executive Summary

Sidereal uses Lightyear as its networking backbone but bypasses two of its most valuable client-side systems: **interpolation** and **rollback-driven prediction reconciliation**. Instead, custom implementations handle both. This was pragmatic during early development but introduces maintenance burden, visual quality gaps (especially during control swap), and divergence from upstream improvements.

This document evaluates what Lightyear features are used, what is bypassed, and whether full adoption of Lightyear's interpolation and prediction correction is feasible given Sidereal's unique requirements (frequent control swap between predicted entities).

---

## 2. Current Lightyear Usage Inventory

### 2.1 Features Actively Used

| Feature | How Used |
|---|---|
| **ServerPlugins / ClientPlugins** | Core plugin infrastructure, tick synchronization |
| **Component replication** | `Replicate`, `Replicated`, `ReplicationGroup` — all gameplay components |
| **Replication visibility** | `gain_visibility` / `lose_visibility` / `is_visible` — spatial interest management |
| **Custom channels** | `ControlChannel` (reliable), `InputChannel` (sequenced unreliable), `AssetChannel` (reliable) |
| **Typed messages** | 13 bidirectional messages for auth, control, input, assets, session |
| **NativeInputPlugin** | `ActionState<PlayerInput>` / `InputMarker<PlayerInput>` for input tagging |
| **Predicted marker** | Applied to locally controlled entity only |
| **Confirmed\<T\> components** | Read by custom reconciliation for Position, Rotation, LinearVelocity |
| **PredictionManager** | Rollback policy and correction policy configured |
| **ControlledBy** | Server-side authority binding with `Lifetime::Persistent` |
| **LightyearAvianPlugin** | Avian2D integration (`AvianReplicationMode::Position`) |
| **Per-entity ReplicationGroup** | Enforced to avoid update starvation in shared group |
| **Transport** | `UdpIo` / `ServerUdpIo` for raw UDP |

### 2.2 Features Registered but Bypassed

| Feature | Status |
|---|---|
| **Interpolation lerp functions** | Registered for Position, Rotation, LinearVelocity, AngularVelocity via `add_interpolation_with()` — but `Interpolated` marker is **stripped from all entities** on the client |
| **Lightyear rollback/resimulation** | PredictionManager exists and is configured, but actual correction is done by a custom `reconcile_controlled_prediction_with_confirmed` system that reads `Confirmed<T>` and applies snap/smooth manually |

### 2.3 Features Not Used At All

| Feature | Description |
|---|---|
| **`Interpolated` marker lifecycle** | All entities have Predicted/Interpolated stripped; only controlled entity gets `Predicted` re-applied |
| **`ConfirmedHistory<C>`** | Server state history buffer for interpolation — never populated because `Interpolated` is never kept |
| **`VisualCorrection<D>`** | Lightyear's built-in rollback visual smoothing (error decay over time) |
| **`DeterministicPredicted`** | For entities that should be rolled back but not trigger rollbacks |
| **`ReplicationTarget<Predicted/Interpolated>`** | Server-side per-peer prediction/interpolation targeting |
| **`PreSpawned` / pre-prediction** | Client-side entity pre-spawning before server confirmation |
| **Authority transfer** | `RequestAuthority` / `GiveAuthority` / `AuthorityBroker` |
| **Frame interpolation** | `lightyear_frame_interpolation` for visual smoothing between FixedMain ticks |
| **Built-in control transfer** | Lightyear has no first-class control swap, but has building blocks that could simplify the custom protocol |

---

## 3. Current Custom Implementations (What Lightyear Would Replace)

### 3.1 Custom Prediction Reconciliation

**File**: `bins/sidereal-client/src/runtime/motion.rs` — `reconcile_controlled_prediction_with_confirmed`

The custom system reads `Confirmed<Position>`, `Confirmed<Rotation>`, `Confirmed<LinearVelocity>` and applies threshold-based correction:

| Component | Snap Threshold | Smooth Threshold | Smooth Factor |
|---|---|---|---|
| Position | >= 64m | >= 2m | lerp 0.25 |
| Rotation | >= 0.8 rad | >= 0.08 rad | slerp 0.25 |
| LinearVelocity | >= 2.0 mps or pos snap | always | lerp 0.35 |

Additional stabilization:
- `stabilize_controlled_idle_motion`: zeros residual drift when controls neutral
- `clamp_controlled_angular_velocity`: caps at 2.0 rad/s
- `enforce_controlled_planar_motion`: NaN/infinite sanitization

**Why this exists**: Lightyear's built-in rollback was reportedly causing issues with the control swap flow. When the controlled entity changes, Lightyear's rollback history for the old entity becomes stale or empty, and the new entity has no prediction history yet. The custom reconciliation avoids relying on rollback entirely by doing continuous correction against confirmed state.

### 3.2 Custom Visual Smoothing for Remote Entities

**File**: `bins/sidereal-client/src/runtime/transforms.rs` — `sync_world_entity_transforms_from_physics`

All non-controlled entities use exponential visual smoothing:

```
alpha = 1.0 - exp(-20.0 * delta_secs)
transform.x += (target.x - current.x) * alpha
```

With a snap threshold of 64m. Controlled entities and nearby collision proxies get instant snap.

**Why this exists**: Since `Interpolated` markers are stripped from all entities, Lightyear's interpolation pipeline never runs. This custom smoothing fills the gap but has significant limitations (see section 5).

### 3.3 Custom Control Swap Protocol

**Files**: `bins/sidereal-replication/src/replication/control.rs`, `bins/sidereal-client/src/runtime/replication.rs`

Entirely custom message-based protocol:
1. Client sends `ClientControlRequestMessage` with sequence number
2. Server validates ownership, resolves GUID, updates `ControlledEntityGuid` on player entity
3. Server sends `ServerControlAckMessage` or `ServerControlRejectMessage`
4. Server neutralizes previous entity (`ActionQueue` cleared, `FlightComputer` zeroed)
5. Server rebinds `ControlledBy` and teleports player anchor
6. Client `converge_local_prediction_markers_system` ensures only controlled entity has `Predicted`
7. Client `enforce_motion_ownership_for_world_entities` strips `RigidBody`/`ActionQueue`/`FlightControlAuthority` from non-controlled entities

---

## 4. The Core Problem: Control Swap and Interpolation History

### 4.1 The Scenario

Sidereal allows players to frequently swap which entity they control (e.g., switching between ships). When a swap occurs:

1. **Entity A** (previously controlled/predicted) must transition to a **remote/interpolated** state
2. **Entity B** (previously remote) must transition to a **predicted** state

### 4.2 Why Lightyear Interpolation Was Abandoned

When Entity A loses `Predicted` and needs to become `Interpolated`:

- Lightyear's interpolation requires `ConfirmedHistory<C>` — a buffer of at least two consecutive server state snapshots to interpolate between.
- Entity A was predicted, so its `ConfirmedHistory` was never being populated (prediction uses `PredictionHistory` instead, which tracks predicted — not confirmed — state).
- **Result**: Entity A has no interpolation history. It would either freeze at its last position or snap to the next server update with no smoothing.

When Entity B gains `Predicted`:

- It needs `PredictionHistory` seeded with current state to enable rollback comparison.
- The entity's confirmed state might be multiple ticks behind the current client tick.
- **Result**: First few frames after swap could trigger unnecessary rollbacks or visual pops.

### 4.3 Current Workaround

The project sidesteps this entirely:
- **No entity ever gets `Interpolated`** — the marker is stripped from everything.
- **Non-controlled entities use custom exponential smoothing** — directly on replication updates, no history buffer needed.
- **Custom reconciliation** — avoids relying on Lightyear's rollback history lifecycle.

This works but has costs (section 5).

---

## 5. Pros and Cons of Current Approach vs Full Lightyear

### 5.1 Current Custom Approach

**Pros:**
- Control swap is smooth — no interpolation history gaps because custom smoothing doesn't need history
- Simple mental model — one entity predicted, everything else receives direct replication updates with exponential smoothing
- No dependency on Lightyear's internal interpolation/prediction lifecycle management
- Predictable behavior — no surprising interactions between Lightyear's rollback and custom code
- Works today — the system is functional and tuned

**Cons:**
- **No true interpolation** — exponential smoothing is reactive (chases the latest update), not predictive. Between server updates (33ms at 30Hz), remote entities appear to decelerate then jump, creating micro-stuttering visible at high framerates or during close observation.
- **No rollback correctness** — custom reconciliation applies continuous correction but doesn't re-simulate from the confirmed tick. If prediction diverges significantly (e.g., collision), the correction path is smooth lerp, not physics-accurate resimulation. This means the corrected state may not be physically consistent.
- **No visual correction smoothing** — Lightyear's `VisualCorrection<D>` provides frame-accurate error decay that doesn't affect simulation state. The custom system modifies actual physics state directly during correction, which can feed back into the next tick's simulation.
- **Duplicate visual entities** — the project has `suppress_duplicate_predicted_interpolated_visuals_system` and `SuppressedPredictedDuplicateVisual` to handle cases where Lightyear creates predicted/interpolated copies that the custom code doesn't expect. This is a sign of impedance mismatch.
- **Nearby collision proxy complexity** — since non-controlled entities have no `RigidBody`, the client must manually create kinematic proxies within a radius for collision. With Lightyear interpolation, interpolated entities would naturally have physics state.
- **No frame interpolation** — without `lightyear_frame_interpolation`, physics runs at 30Hz but rendering is 60Hz+. There is no visual smoothing between fixed ticks, so movement appears to update at 30fps even though rendering is faster.
- **Maintenance burden** — custom reconciliation, smoothing, motion ownership enforcement, and duplicate suppression are ~500 lines of bespoke networking code that must stay in sync with Lightyear version upgrades.
- **No `is_in_rollback()` gating** — because the project uses custom reconciliation rather than Lightyear rollback, systems cannot use Lightyear's `is_in_rollback()` run condition to skip expensive work during resimulation.

### 5.2 Full Lightyear Interpolation + Prediction

**Pros:**
- **True interpolation** — `ConfirmedHistory<C>` stores multiple server snapshots and interpolates between them based on `InterpolationTimeline`. Movement appears smooth at any framerate, with consistent visual quality.
- **Physics-accurate rollback** — when server correction arrives, Lightyear snaps to confirmed state at the confirmed tick and re-runs FixedUpdate from that tick to current. The result is physically consistent, not just visually smoothed.
- **Visual correction** — `VisualCorrection<D>` decays rollback visual error over time without modifying simulation state. The simulation is always correct, but the visual smoothly catches up.
- **Frame interpolation** — `lightyear_frame_interpolation` provides render-rate-independent smoothing between FixedUpdate ticks, eliminating the 30fps visual stutter.
- **Upstream maintenance** — interpolation, rollback, correction are all maintained by Lightyear. Version upgrades bring improvements without custom code changes.
- **Consistent entity lifecycle** — Lightyear manages Confirmed/Predicted/Interpolated entity copies with proper component propagation. No duplicate suppression needed.
- **`DeterministicPredicted`** — entities that should participate in rollback but not trigger it (e.g., nearby ships) can be marked appropriately.
- **Collision simplification** — interpolated entities maintain physics state through the Lightyear/Avian integration, potentially reducing or eliminating the nearby-collision-proxy system.

**Cons:**
- **Control swap interpolation history gap** — the fundamental problem. When an entity transitions from Predicted to Interpolated, its `ConfirmedHistory` buffer is empty. Lightyear does not natively handle this transition.
- **Complexity of mode switching** — predicted entities run ahead of server time; interpolated entities run behind. Switching between modes requires bridging this time gap (~100-200ms depending on RTT + interpolation delay).
- **Potential for rollback storms** — if prediction diverges frequently (physics non-determinism, floating point drift), frequent rollbacks could cause visual stuttering worse than the current smooth correction.
- **Black-box behavior** — debugging Lightyear's internal rollback/interpolation state is harder than debugging the current custom code.
- **Risk** — significant refactor with potential for regression in an area (networking) that is currently working.

---

## 6. Feasibility Assessment: Full Lightyear Integration

### 6.1 Solving the Control Swap Problem

The core blocker — missing interpolation history on mode switch — has several potential solutions:

#### Option A: Pre-seed ConfirmedHistory on Transition

When an entity transitions from Predicted to Interpolated:
1. Read the entity's current `Confirmed<T>` values (which Lightyear maintains for predicted entities)
2. Construct a synthetic `ConfirmedHistory<C>` with the confirmed state at the current confirmed tick
3. Insert `Interpolated` marker

The first interpolation frame would have only one history entry (no interpolation possible), so the entity would hold at its confirmed position for one server update interval (~33ms), then begin interpolating normally once a second snapshot arrives.

**Feasibility**: Medium. Requires understanding Lightyear's internal `ConfirmedHistory` initialization and whether it can be externally seeded. The 33ms hold is acceptable for a control swap.

#### Option B: Hybrid Smooth-then-Interpolate

On transition from Predicted to Interpolated:
1. Remove `Predicted`, but don't add `Interpolated` immediately
2. Apply exponential smoothing (current system) for a brief window (~100ms)
3. Once `ConfirmedHistory` has accumulated 2+ entries from replication, add `Interpolated`

**Feasibility**: High. This is a pragmatic blend — the custom smoothing covers the gap, then Lightyear takes over. Requires a small transition state machine per entity.

#### Option C: Always Maintain ConfirmedHistory for Predicted Entities

Modify protocol registration so that predicted components also populate `ConfirmedHistory`:
- Lightyear updates `Confirmed<T>` on predicted entities when server state arrives
- A custom system could mirror each `Confirmed<T>` update into a `ConfirmedHistory<T>` buffer
- When transitioning to Interpolated, history is already warm

**Feasibility**: Medium-high. Adds memory overhead (maintaining two history buffers per predicted component) but is the cleanest solution. May require Lightyear API changes or careful use of internal types.

#### Option D: Keep Custom Smoothing for Non-Controlled, Use Lightyear Prediction Only

Accept that Lightyear interpolation won't work for the control-swap case and keep the current custom smoothing for remote entities, but adopt Lightyear's full rollback/resimulation + visual correction for the controlled entity.

**Feasibility**: High. This is the most incremental approach. It captures the biggest wins (physics-accurate rollback, visual correction) without touching the remote entity rendering path.

### 6.2 Recommended Path: Option D First, Then Option B

**Phase 1 — Adopt Lightyear rollback + visual correction for controlled entity (Option D)**

This captures the highest-value improvements with lowest risk:
- Replace custom `reconcile_controlled_prediction_with_confirmed` with Lightyear's native rollback
- Enable `VisualCorrection` for Position, Rotation, LinearVelocity on the controlled entity
- Keep `CorrectionPolicy::default()` (smooth) or tune error decay rate
- Keep custom exponential smoothing for all non-controlled entities (unchanged)
- Remove custom `stabilize_controlled_idle_motion` and `clamp_controlled_angular_velocity` if rollback handles these cases
- Add `is_in_rollback()` gating to expensive systems that shouldn't run during resimulation

Expected benefits: physically correct prediction, smoother controlled-entity visuals, reduced custom code.

**Phase 2 — Adopt Lightyear interpolation for remote entities (Option B)**

Once Phase 1 is stable:
- Stop stripping `Interpolated` markers from remote root entities
- Implement transition state machine: when entity loses `Predicted`, enter 100ms smoothing window, then apply `Interpolated`
- When entity gains `Predicted`, remove `Interpolated` and `ConfirmedHistory`, seed `PredictionHistory`
- Enable `lightyear_frame_interpolation` for render-rate smoothing
- Evaluate whether nearby-collision-proxy system can be simplified (interpolated entities have physics state)

**Phase 3 — Evaluate advanced features**

- `DeterministicPredicted` for nearby entities that should be rolled back with the controlled entity but not trigger rollbacks themselves
- `ReplicationTarget<Predicted>` for server-directed per-client prediction targeting
- Frame interpolation for all entities
- Authority transfer for cooperative control scenarios

---

## 7. Detailed Technical Notes

### 7.1 Lightyear 0.26.4 Architecture

Lightyear 0.26.4 is split into subcrates:

| Subcrate | Purpose |
|---|---|
| `lightyear_replication` | Entity/component state replication |
| `lightyear_prediction` | Client-side prediction + rollback |
| `lightyear_interpolation` | Smoothing remote entity updates via ConfirmedHistory |
| `lightyear_frame_interpolation` | Visual smoothing between FixedMain ticks (render-rate) |
| `lightyear_sync` | Timeline synchronization between peers |
| `lightyear_inputs_native` | Native input networking |
| `lightyear_avian2d` | Avian2D physics integration |

### 7.2 Entity Copies on Client

For a replicated entity, Lightyear can maintain up to three copies on the client:

1. **Confirmed** (`Confirmed` marker) — directly applies server replication updates. Represents authoritative server state.
2. **Predicted** (`Predicted` marker) — runs ahead of confirmed. Subject to rollback when server corrections arrive.
3. **Interpolated** (`Interpolated` marker) — runs behind confirmed. Smoothly interpolates between consecutive server updates.

Currently, Sidereal only uses the Confirmed copy (for reading `Confirmed<T>`) and manually manages `Predicted` on one entity. `Interpolated` copies are never created.

### 7.3 Rollback Flow

1. Client runs simulation ahead (client at tick T+N, server confirms tick T)
2. Server sends confirmed state for tick T → `Confirmed` entity updated
3. `PredictionManager` compares confirmed tick-T state against `PredictionHistory[T]`
4. If mismatch detected (per `RollbackPolicy`):
   - Snap entity state to confirmed tick-T state
   - Re-run FixedUpdate from T through T+N
   - `VisualCorrection` stores error = (old predicted) - (new corrected)
   - Error decays over subsequent frames
5. Net result: simulation is always physically correct, visual catches up smoothly

### 7.4 Key Configuration Types

| Type | Purpose |
|---|---|
| `RollbackPolicy` | Controls when rollbacks trigger (`Always`, `Check`, `Disabled` per state/input) |
| `CorrectionPolicy` | How fast visual correction error decays (instant or smooth) |
| `VisualCorrection<D>` | Stores current visual error being smoothed out |
| `DisableRollback` | Completely exclude entity from rollback participation |
| `DeterministicPredicted` | Entity participates in rollback but cannot trigger one |
| `ConfirmedHistory<C>` | Buffer of server states for interpolation (start → end) |
| `InterpolationDelay` | Client's interpolation lag (for server-side lag compensation) |

### 7.5 Current Component Registration

Avian physics components (manual registration):

| Component | Prediction | Interpolation |
|---|---|---|
| `Position` | yes | yes (lerp) |
| `Rotation` | yes | yes (slerp) |
| `LinearVelocity` | yes | yes (lerp) |
| `AngularVelocity` | yes | yes (lerp) |
| `LinearDamping` | yes | no |
| `AngularDamping` | yes | no |

Gameplay components with `predict = true` (via `#[sidereal_component]`):

- `TotalMassKg`, `SizeM`, `MaxVelocityMps`, `FlightTuning`, `FlightComputer`

All other replicated components (~45+) are receive-only (no prediction, no interpolation).

### 7.6 Server-Side Replication Configuration

- **Tick rate**: 60 Hz fixed timestep
- **Send mode**: `SendUpdatesMode::SinceLastAck` — delta since last acknowledged state
- **Replication groups**: Per-entity (`ReplicationGroup::new_from_entity()`)
- **Hierarchy replication**: Disabled (`HierarchyRebuildEnabled(false)`) — hierarchy tracked via `MountedOn` (UUID-based)
- **Visibility**: Spatial interest management with configurable range (`SIDEREAL_VISIBILITY_DELIVERY_RANGE_M`, default 300m)

---

## 8. Known Workarounds and Technical Debt

These items exist because of the partial Lightyear integration and would be simplified or eliminated by full adoption:

1. **Duplicate visual suppression** — `SuppressedPredictedDuplicateVisual` component and associated system exist because Lightyear sometimes creates predicted/interpolated entity copies that the custom code doesn't expect. Full adoption would make entity copy lifecycle predictable.

2. **Nearby collision proxies** — Non-controlled entities have `RigidBody` stripped and replaced with kinematic proxies within 200m. With Lightyear interpolation, interpolated entities would maintain their own physics state.

3. **Transport channel manual setup** — Both server and client re-check and re-add channel receivers every frame, suggesting channels are not automatically stable after connection.

4. **Deferred predicted adoption** — When the controlled entity first replicates, physics components may arrive over multiple frames. The client defers `Predicted` adoption until Position, Rotation, and LinearVelocity are all present, with configurable timeouts and user-facing error dialogs.

5. **Remote root anchor sync fallback** — Optional system (`SIDEREAL_CLIENT_REMOTE_ROOT_ANCHOR_FALLBACK`) that copies player anchor position to remote ship entities because remote ship root entities may not reliably receive independent position updates.

6. **Stale transport recreation** — Client despawns and recreates the transport entity on reconnect rather than reusing it, to avoid stale state across logout/login cycles.

7. **Visibility cleanup on disconnect** — Stale visibility bits are intentionally left dangling for disconnected clients to avoid a burst of outbound traffic from per-entity visibility revocations.

---

## 9. Risk Assessment

### 9.1 Phase 1 (Rollback for Controlled Entity) — Low-Medium Risk

- **Scope**: Replace ~130 lines of custom reconciliation with Lightyear-native rollback
- **Risk factors**: Avian2D physics non-determinism could cause frequent rollbacks. Flight model force application must be identical during resimulation. `apply_predicted_input_to_action_queue` already runs during rollback (it's in FixedUpdate), which is correct.
- **Mitigation**: Start with `RollbackPolicy { state: RollbackMode::Always, .. }` and tune. Use `SIDEREAL_CLIENT_INSTANT_CORRECTION=true` initially, then switch to smooth correction.
- **Rollback**: Can always revert to custom reconciliation if rollback storms are unacceptable.

### 9.2 Phase 2 (Interpolation for Remote Entities) — Medium Risk

- **Scope**: Stop stripping `Interpolated` markers, implement transition state machine
- **Risk factors**: Control swap timing, interpolation delay calibration, interaction with nearby collision proxies, potential for visual pops during transition window.
- **Mitigation**: Option B (smooth-then-interpolate) provides a safety net. The 100ms smoothing window covers the interpolation history gap.
- **Rollback**: Can keep custom smoothing as fallback per entity type.

### 9.3 Phase 3 (Advanced Features) — Low Risk

- **Scope**: Incremental feature adoption after core systems are stable
- **No structural risk**: These are additive features.

---

## 10. Recommendation

**Adopt Lightyear prediction rollback (Phase 1) as the next networking milestone.** This delivers the highest value (physically correct prediction, visual correction smoothing) with lowest risk and does not require solving the control-swap interpolation problem.

**Defer Lightyear interpolation (Phase 2) until after Phase 1 is validated under load.** The custom exponential smoothing is adequate for remote entities and the control-swap transition problem requires careful engineering.

**Do not attempt to adopt all Lightyear features simultaneously.** The current system works. Incremental adoption with validation at each step is the correct approach.

---

## 11. Implementation Checklist

### Phase 1: Lightyear-Native Prediction Rollback

- [ ] Verify `apply_predicted_input_to_action_queue` runs correctly during Lightyear rollback (it's in FixedUpdate, so it should)
- [ ] Verify all systems that read/write prediction-relevant components are in FixedUpdate and properly ordered relative to `PhysicsSystems::StepSimulation`
- [ ] Remove custom `reconcile_controlled_prediction_with_confirmed`
- [ ] Enable Lightyear's native rollback for the controlled entity (ensure `Predicted` marker triggers rollback)
- [ ] Configure `RollbackPolicy` (start with `state: Always`, then tune to `Check` after validation)
- [ ] Enable `VisualCorrection` for Position, Rotation, LinearVelocity
- [ ] Configure `CorrectionPolicy` (start with instant, tune to smooth)
- [ ] Gate expensive non-prediction systems with `is_in_rollback()` to avoid unnecessary work during resimulation
- [ ] Evaluate whether `stabilize_controlled_idle_motion` and `clamp_controlled_angular_velocity` are still needed or if rollback handles the underlying issues
- [ ] Test control swap under rollback: verify old entity cleanly loses `Predicted`, new entity cleanly gains it
- [ ] Load test with multiple clients, measure rollback frequency and visual quality

### Phase 2: Lightyear Interpolation for Remote Entities

- [ ] Stop stripping `Interpolated` from non-controlled root entities
- [ ] Implement transition state machine (Predicted → smoothing window → Interpolated)
- [ ] Implement reverse transition (Interpolated → Predicted) with PredictionHistory seeding
- [ ] Enable `lightyear_frame_interpolation` for render-rate visual smoothing
- [ ] Evaluate nearby-collision-proxy simplification (interpolated entities have physics state)
- [ ] Calibrate `InterpolationDelay` for acceptable visual latency
- [ ] Test control swap cycle: A→B→A with different entities, verify no visual artifacts
- [ ] Stress test with 10+ visible entities transitioning between modes

### Phase 3: Advanced Features

- [ ] Evaluate `DeterministicPredicted` for nearby entities
- [ ] Evaluate `ReplicationTarget<Predicted>` for server-directed prediction
- [ ] Evaluate authority transfer for cooperative control scenarios

---

## 12. Lightyear Visibility and Rooms vs Current Spatial Partitioning

### 12.1 Lightyear's Visibility Architecture

Lightyear 0.26.4 provides two visibility modes that can be used independently or combined:

**Immediate visibility** — direct per-entity, per-client control via `ReplicationState::gain_visibility(sender)` / `lose_visibility(sender)`. This is what Sidereal currently uses.

**Room-based visibility** — entities and clients are grouped into `Room` entities. When a client and entity share at least one room, the entity is visible. Reference counting handles multi-room membership: an entity in rooms A and B with a client in both has count=2. Removing the entity from room A (count drops to 1) does NOT trigger visibility loss. Only when count reaches 0 does `lose_visibility` fire.

Both modes ultimately operate through the same underlying `ReplicationState` component.

### 12.2 How Lightyear Rooms Work (0.26.4)

The old `RoomId` / `RoomManager` resource-based API from earlier versions has been replaced. Rooms are now ECS entities:

```rust
// Create a room
let room = commands.spawn(Room::default()).id();

// Add client and entity to room via triggers
commands.trigger(RoomEvent { room, target: RoomTarget::AddSender(client_entity) });
commands.trigger(RoomEvent { room, target: RoomTarget::AddEntity(entity) });

// Remove client from room
commands.trigger(RoomEvent { room, target: RoomTarget::RemoveSender(client_entity) });
```

Key properties:
- **`Room` component** holds two `EntityHashSet`s: `clients` and `entities`
- **Events are batched** per-frame in a `RoomEvents` resource and applied atomically before the replication buffer pass
- **Reference counting** via `shared_counts: EntityHashMap<EntityHashMap<u8>>` — tracks how many rooms each (client, entity) pair shares
- **Concurrent moves** are handled: entity and client both leave room R1 and join room R2 in the same frame → visibility unchanged
- **`NetworkVisibility` component** must be on the entity to opt into interest management. Without it, the entity replicates to all clients unconditionally

### 12.3 Critical Behavior: Visibility Loss = Despawn

When an entity loses visibility for a client, Lightyear sends a **despawn action** on the reliable channel. The entity is fully despawned on that client, not just paused. Re-gaining visibility triggers a full re-spawn with all component data.

This is identical to Sidereal's current behavior (since `gain_visibility`/`lose_visibility` is already the underlying mechanism), but it's important context when evaluating rooms as a coarse filter: putting a player into a "solar system room" and then removing them means every entity in that system despawns on that client and must be fully re-replicated when they return.

### 12.4 What Lightyear Rooms Could Do for Sidereal

#### Natural Mapping: Solar Systems as Rooms

Each solar system entity already exists as an ECS entity with `Position`, `SolarSystemRadius`, and `SolarSystemVisuals`. Making each solar system a `Room` is a natural extension:

| Concept | Lightyear Mapping |
|---|---|
| Solar system | `Room` entity (same entity that has `SolarSystemRadius` etc.) |
| Player enters system | `RoomEvent::AddSender(client)` |
| Player leaves system | `RoomEvent::RemoveSender(client)` |
| Entity spawns in system | `RoomEvent::AddEntity(entity)` |
| Entity moves between systems | `RemoveEntity` from old, `AddEntity` to new |
| Deep space (between systems) | Client is in no system room (or a "deep space" catchall room) |

#### Coarse Spatial Preselection Without Custom Code

The galaxy structure doc proposes a "solar-system-aware preselection" optimization:

> Before running per-entity visibility checks, compute the player's distance to each solar system center. If `distance_to_system_center - system_radius > scanner_range`, no entity in that system can be visible.

Rooms would provide this automatically. If the player is only in rooms for nearby systems, entities in distant systems are never even considered for replication. No custom preselection code needed.

#### Multi-Room Reference Counting Handles Edge Cases

A player near a system boundary could be in two system rooms simultaneously. Reference counting ensures entities in the overlapping region are visible from both rooms without double-spawning. When the player leaves one room, entities only in that room lose visibility, while shared entities remain.

An entity that transits between systems (e.g., a ship traveling from system A to system B) would be temporarily in both rooms during transit. Reference counting handles this correctly.

#### Always-Visible Entities

Solar system entities themselves (which have `PublicVisibility`) could be in a "galaxy overview" room that all clients are always in. This replaces the current ownership/public bypass logic for these specific entities.

### 12.5 What Lightyear Rooms Cannot Do

#### No Distance-Based Visibility Within Rooms

Rooms are binary: an entity is in the room or not. A player in the "Arcturus System" room sees ALL entities in that room, regardless of distance from the player. This is not sufficient for Sidereal's gameplay, where:

- Scanner range determines what you can detect within a system
- Delivery scope limits replication to entities near the observer
- Entities within a system may be thousands of meters apart

Rooms cannot replace the fine-grained distance checks. They can only serve as a **coarse prefilter**.

#### No Authorization Policy

Rooms have no concept of owner/public/faction/scanner authorization. A player in a room sees everything in that room. Sidereal's multi-layered authorization (ownership always visible, faction visibility, public visibility, scanner range) cannot be expressed through rooms alone.

#### Room Size is Logical, Not Spatial

Rooms have no spatial extent. Adding an entity to a room is a logical grouping operation, not a position-based one. The server must still track entity positions and manually manage room membership based on spatial checks (which entity is inside which system radius).

### 12.6 Evaluation: Should Sidereal Adopt Rooms?

#### Option 1: Rooms as Coarse Solar System Filter (Hybrid)

Use rooms for macro-scale visibility (which systems are relevant to which clients) and keep the current immediate visibility system for micro-scale visibility (distance/scanner/authorization within systems).

**Architecture:**

1. Each solar system is a `Room` entity
2. A 1 Hz system checks player distance to each solar system and manages room membership via `RoomEvent` triggers
3. Entities get room membership when spawned based on their `SolarSystemId`
4. Entities transiting between systems get room membership updated
5. Deep-space entities (no `SolarSystemId`) go into a global "deep space" room that all players are in, OR bypass rooms via the immediate API
6. The existing authorization-first visibility pipeline continues to run, but only on entities that pass the room prefilter

**Pros:**
- Eliminates the planned custom solar-system-aware preselection code
- Room reference counting handles boundary cases automatically
- Aligns with Lightyear's intended usage pattern
- Room membership changes trigger reliable spawn/despawn, giving the client explicit "system enter/exit" signals for free

**Cons:**
- **Two visibility systems running concurrently** — rooms handle coarse filtering, immediate handles fine. Both modify `ReplicationState`. Need to verify they compose correctly.
- **Composition concern**: Room-based visibility and immediate visibility both call `gain_visibility`/`lose_visibility` on `ReplicationState`. If rooms grant visibility (entity and client share a room) but immediate visibility revokes it (entity outside scanner range), which wins? In Lightyear's model, both systems write to the same `VisibilityState`. The last writer wins within a frame. This creates ordering-dependent behavior that is hard to reason about.
- **Room membership management overhead** — a new 1 Hz system to track which players are near which systems, plus entity room updates on spawn/move.
- **Deep-space entity handling** is awkward — entities between systems need a room or need to bypass the room system entirely.

**Verdict: Not recommended.** The composition problem between room-based and immediate visibility is the fundamental issue. Lightyear's two visibility modes are designed as alternatives, not layers. Using both simultaneously on the same entities requires careful orchestration to avoid one system's `gain_visibility` being overridden by the other's `lose_visibility` in the same tick.

#### Option 2: Rooms Only (Replace Custom Visibility)

Replace the entire custom visibility pipeline with rooms. Create fine-grained rooms (e.g., grid cells as rooms, or scanner-range "zones" as rooms).

**Verdict: Not feasible.** Sidereal's authorization model (owner/faction/public/scanner) cannot be expressed through binary room membership. Scanner range is per-player and dynamic. Rooms have no distance concept. This would require creating per-player rooms (defeating the purpose of shared rooms) or so many micro-rooms that the reference counting overhead exceeds the current direct approach.

#### Option 3: Rooms for Non-Spatial Global Visibility Only

Use rooms only for entities that should be visible to all clients regardless of distance (solar system metadata, galaxy map data, public infrastructure), and keep immediate visibility for all spatial entities.

**Architecture:**

1. One "galaxy constants" room containing solar system entities, faction definitions, etc.
2. All authenticated clients are added to this room
3. All spatially-positioned gameplay entities use the current immediate visibility pipeline unchanged

**Pros:**
- Very simple — just one room, no spatial tracking
- Replaces the current `PublicVisibility` component bypass with a clean room membership
- No composition concerns — room entities don't overlap with immediate-visibility entities

**Cons:**
- Marginal benefit — the current `PublicVisibility` bypass already works
- Adds a dependency on Lightyear's room system for a small number of entities

**Verdict: Low priority.** This is clean but offers minimal value over the current approach.

#### Option 4: No Rooms — Keep Current Architecture

Stay with the current immediate visibility system and implement the planned spatial optimizations (configurable cell size, solar-system-aware preselection) as custom code.

**Pros:**
- No new Lightyear dependencies to manage
- Full control over visibility semantics and composition
- Current system is well-understood and working
- Planned optimizations (cell size, preselection) are straightforward to implement
- No composition footgun between two visibility modes

**Cons:**
- Must implement solar-system-aware preselection manually
- Custom code must stay compatible with Lightyear version upgrades

**Verdict: Recommended.** The current immediate visibility system is well-matched to Sidereal's requirements. The planned spatial optimizations are simple to implement and avoid the composition complexity of mixing rooms with immediate visibility.

### 12.7 Recommendation

**Do not adopt Lightyear rooms for spatial partitioning.** The room system is designed for logical grouping (chat rooms, game lobbies, map zones with hard boundaries) where membership is binary and authorization is not distance-dependent. Sidereal's visibility requirements — dynamic scanner ranges, multi-policy authorization, continuous delivery scope — are fundamentally distance-based and policy-driven, which is exactly what the current immediate visibility approach handles.

**Instead, continue with the planned spatial optimizations:**

1. **Make cell size configurable** (`SIDEREAL_VISIBILITY_CELL_SIZE_M` env var, default 2,000m)
2. **Switch candidate mode default** from `full_scan` to `spatial_grid`
3. **Implement solar-system-aware preselection** as a pre-pass in the existing visibility pipeline (40 distance checks per player per second)
4. **Upgrade cell keys to `i64`** when f64 coordinates are adopted

These are incremental improvements to the existing pipeline that maintain full compatibility with the authorization-first visibility contract.

**Rooms remain available** for future non-spatial use cases if they arise (e.g., instanced content, private communication channels, lobby systems).

---

## 13. References

- `docs/features/prediction_runtime_tuning_and_validation.md`
- `docs/sidereal_design_document.md`
- `docs/features/visibility_replication_contract.md`
- `docs/features/galaxy_world_structure.md`
- `crates/sidereal-net/src/lightyear_protocol/registration.rs`
- `bins/sidereal-client/src/runtime/motion.rs`
- `bins/sidereal-client/src/runtime/transforms.rs`
- `bins/sidereal-client/src/runtime/replication.rs`
- `bins/sidereal-replication/src/replication/control.rs`
- `bins/sidereal-replication/src/replication/simulation_entities.rs`
- `bins/sidereal-replication/src/replication/visibility.rs`
- [Lightyear 0.26.4 docs](https://docs.rs/lightyear/0.26.4/lightyear/)

- `docs/features/prediction_runtime_tuning_and_validation.md`
- `docs/sidereal_design_document.md`
- `docs/features/visibility_replication_contract.md`
- `crates/sidereal-net/src/lightyear_protocol/registration.rs`
- `bins/sidereal-client/src/runtime/motion.rs`
- `bins/sidereal-client/src/runtime/transforms.rs`
- `bins/sidereal-client/src/runtime/replication.rs`
- `bins/sidereal-replication/src/replication/control.rs`
- `bins/sidereal-replication/src/replication/simulation_entities.rs`
- [Lightyear 0.26.4 docs](https://docs.rs/lightyear/0.26.4/lightyear/)

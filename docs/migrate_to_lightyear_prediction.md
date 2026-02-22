# Migration: Lightyear Native Prediction & Replication

Self-contained reference for any session implementing this migration. Includes current architecture, target architecture, Avian/Bevy integration requirements, exact code paths to change, known footguns, and the end-to-end data flows.

---

## 1. Version Context

| Dependency | Version |
|---|---|
| Bevy | 0.18 |
| Avian3D | 0.5.0 |
| Lightyear | 0.26.4 |
| Fixed timestep | 30 Hz (both server and client) |

Lightyear 0.26.x is split into many sub-crates. The relevant ones are:
- `lightyear` — umbrella re-export
- `lightyear_replication` — `Replicate`, `Replicated`, `NetworkVisibility`, authority, component registration
- `lightyear_prediction` — `Predicted`, rollback, `PredictionHistory`, `VisualCorrection`
- `lightyear_interpolation` — `Interpolated`, snapshot buffering
- `lightyear_frame_interpolation` — visual frame interpolation (render-tick smoothing between fixed ticks)
- `lightyear_avian3d` — `LightyearAvianPlugin`, Position/Rotation sync, lag compensation
- `lightyear_inputs` — `InputBuffer<T>`, tick-indexed input transport

---

## 2. What We Have Today (Current Architecture)

### 2.1 How Lightyear Is Used Now

Lightyear is used **only as a transport layer**. We use:
- `ServerPlugins` / `ClientPlugins` for raw UDP session management
- `register_message` / `MessageReceiver` / `ServerMultiMessageSender` for manual message passing
- Three custom channels: `ControlChannel` (auth/assets), `InputChannel` (client → server actions), `StateChannel` (server → client world state)

Everything above the wire is **custom**:
- **Server builds a `WorldStateDelta`** (JSON-serialized `Vec<WorldDeltaEntity>`) by reflecting every component on every entity every tick, then sends it per-client with visibility filtering.
- **Client deserializes `WorldStateDelta`**, manually inserts/updates components via `insert_registered_components_from_world_deltas`, and runs custom reconciliation.
- **Prediction** uses a manual `replay_predicted_state_from_authoritative()` function that re-computes flight forces with `compute_flight_forces()` and integrates with `v += (F/m)*dt` — this is **not Avian physics**, it is a standalone Euler integrator that can drift from the server's Avian simulation.

### 2.2 Current Server Pipeline (sidereal-replication/src/main.rs)

```
Startup:
  init_replication_runtime → hydrate_replication_world → hydrate_simulation_entities → start_lightyear_server

Update (every frame):
  ensure_server_transport_channels
  → cleanup_client_auth_bindings
  → receive_client_auth_messages          # JWT validation, bind RemoteId → player_entity_id
  → receive_client_view_updates
  → receive_client_asset_requests/acks
  → stream_bootstrap_assets_to_authenticated_clients
  → receive_client_inputs                 # MessageReceiver<ClientInputMessage> → push to ActionQueue
  → report_input_drop_metrics
  → process_bootstrap_ship_commands       # gateway bootstrap → spawn_simulation_entity

FixedUpdate (30 Hz), ordered by SiderealGamePlugin:
  [Before PhysicsSystems::StepSimulation]
    enforce_planar_ship_motion            # zero Z, clamp rotation
    validate_action_capabilities          # from SiderealGamePlugin
    process_flight_actions                # ActionQueue → FlightComputer state
    recompute_total_mass                  # also syncs Avian Mass + AngularInertia
    apply_engine_thrust                   # compute_flight_forces → Forces::apply_force/apply_torque

  [Avian StepSimulation runs here]

  [After PhysicsSystems::StepSimulation]
    stabilize_idle_motion                 # zero small residuals
    clamp_angular_velocity                # cap angular velocity

  [After PhysicsSystems::Writeback]
    sync_simulated_ship_components        # Position→PositionM, Rotation→HeadingRad, etc.
    update_client_controlled_entity_positions
    compute_controlled_entity_scanner_ranges
    collect_local_simulation_state        # ~200 lines: queries ALL components, builds WorldStateDelta per entity
    refresh_component_payloads_from_reflection  # uses Reflect to serialize component payloads to JSON
    broadcast_replication_state           # per-client visibility filter → ReplicationStateMessage → send
    flush_replication_persistence         # periodic graph DB write
    flush_player_runtime_view_state_persistence
```

**Key systems to DELETE**: `collect_local_simulation_state`, `refresh_component_payloads_from_reflection`, `broadcast_replication_state`, `sync_simulated_ship_components`.

**Key resources to DELETE**: `ReplicationOutboundQueue`, `LatestReplicationWorld`, `EntityPositionCache`, `ClientVisibilityHistory`.

### 2.3 Current Client Pipeline (sidereal-client/src/main.rs)

```
Physics mode (default: Predicted):
  - SiderealGameCorePlugin (component registration only, NO simulation systems)
  - PhysicsPlugins (Avian runs but controlled entity has no RigidBody in Predicted mode)

Update:
  send_lightyear_input_messages           # keyboard → ClientInputMessage → send on InputChannel

FixedUpdate:
  receive_lightyear_replication_messages  # MessageReceiver<ReplicationStateMessage> → deserialize WorldStateDelta
                                          # → spawn/update entities, insert components manually
  apply_controlled_reconciliation_fixed_step  # custom reconciliation:
                                              # calls replay_predicted_state_from_authoritative()
                                              # which manually re-computes forces & integrates (NOT via Avian)
                                              # then lerps/snaps InterpolationState
  refresh_predicted_input_history_state   # store current state in InputHistory for future reconciliation
  interpolate_controlled_transform        # lerp Transform between InterpolationState prev/current
  interpolate_remote_entities             # SnapshotBuffer-based interpolation for non-controlled entities
```

**Key types to DELETE**: `InterpolationState`, `DisplayVelocity`, `ReconciliationState`, `InputHistory`, `InputHistoryEntry`, `PendingControlledReconciliationState`, `PendingControlledState`, `SnapshotBuffer`, `EntitySnapshot`, `RemoteEntity`, `ClientPhysicsMode`.

**Key functions to DELETE**: `replay_predicted_state_from_authoritative()`, `apply_controlled_reconciliation_fixed_step()`, `refresh_predicted_input_history_state()`, `interpolate_controlled_transform()`, `interpolate_remote_entities()`, `receive_lightyear_replication_messages()` (replaced by Lightyear's built-in receive).

### 2.4 Current Protocol Types to DELETE (sidereal-net)

- `WorldStateDelta`, `WorldDeltaEntity`, `WorldComponentDelta` — replaced by Lightyear component replication
- `ReplicationStateMessage` — replaced by Lightyear replication messages
- `ClientInputMessage` — replaced by `InputBuffer<PlayerInput>`
- `StateChannel`, `InputChannel` — replaced by Lightyear's built-in replication and input channels
- `LightyearWireMessage` enum — no longer needed
- `encode_wire_message` / `decode_wire_message` — no longer needed

**KEEP**: `ControlChannel`, `ClientAuthMessage`, `ClientViewUpdateMessage`, `AssetRequestMessage`, `AssetAckMessage`, `AssetStreamManifestMessage`, `AssetStreamChunkMessage`, `PlayerRuntimeViewState`.

---

## 3. Target Architecture (After Migration)

### 3.1 Server End-to-End Flow

```
Startup:
  init_replication_runtime → hydrate → start_lightyear_server
  (entities spawned with Replicate + NetworkVisibility components)

Update:
  receive_client_auth_messages            # JWT validation, bind session → ControlledBy
  receive_client_view_updates             # camera state for persistence
  receive_client_asset_requests/acks      # asset streaming (unchanged)
  process_bootstrap_ship_commands         # spawn new ships with Replicate

FixedUpdate (30 Hz):
  [Lightyear delivers inputs from InputBuffer<PlayerInput> at correct tick]
  drain_inputs_to_action_queue            # NEW: read InputBuffer → push to ActionQueue
  [SiderealGamePlugin systems run (unchanged)]
    validate_action_capabilities
    process_flight_actions                # ActionQueue → FlightComputer
    recompute_total_mass                  # syncs Avian Mass + AngularInertia
    apply_engine_thrust                   # compute_flight_forces → Forces::apply_force
  [Avian StepSimulation]
  [Post-physics]
    stabilize_idle_motion
    clamp_angular_velocity
    update_visibility                     # NEW: scanner range → NetworkVisibility gain/lose per client
  [Lightyear automatically: detect changed replicated components → serialize → send to visible clients]

  flush_replication_persistence           # periodic graph DB write (reads from ECS, unchanged)
```

**What changed**: Lightyear handles the entire collect → serialize → per-client-filter → send pipeline. We only need to update `NetworkVisibility` based on scanner range. All the `collect_local_simulation_state` / `refresh_component_payloads_from_reflection` / `broadcast_replication_state` code is deleted.

### 3.2 Client End-to-End Flow

```
Startup:
  SiderealGamePlugin (FULL simulation systems, including flight, mass, etc.)
  PhysicsPlugins with disabled PhysicsTransformPlugin + PhysicsInterpolationPlugin
  LightyearAvianPlugin { replication_mode: Position, .. }

FixedUpdate (30 Hz):
  [Lightyear: check for new confirmed state from server]
  [If divergence detected:]
    Lightyear rolls back: snap Predicted entity to confirmed state
    Lightyear re-runs FixedMain N times (catching up from confirmed tick to current tick)
    Each re-run executes ALL FixedUpdate systems including:
      - validate_action_capabilities
      - process_flight_actions    (reads ActionQueue populated from InputBuffer history)
      - recompute_total_mass
      - apply_engine_thrust       (compute_flight_forces → Forces::apply_force)
      - [Avian StepSimulation]    (REAL physics, not manual Euler integration)
      - stabilize_idle_motion
      - clamp_angular_velocity
    Lightyear applies VisualCorrection (smooth blend from old predicted visual to new corrected)

  [Normal tick (no rollback):]
    write_input_to_buffer                 # keyboard → PlayerInput → InputBuffer::set(tick)
    drain_inputs_to_action_queue          # InputBuffer::get(tick) → ActionQueue
    [SiderealGamePlugin systems]
    [Avian StepSimulation]
    [Lightyear: save PredictionHistory for this tick]

PostUpdate:
  [LightyearAvianPlugin: Position → Transform sync]
  [FrameInterpolation: smooth between fixed ticks for render]
  [VisualCorrection: decay correction error]

Controlled entity: Predicted marker → rollback + resimulate through real Avian
Remote entities: Interpolated marker → Lightyear buffers snapshots → smooth interpolation
```

**What changed**: The client runs the SAME `SiderealGamePlugin` systems through REAL Avian physics. Rollback re-executes the actual physics pipeline, not a separate manual replay function. The entire class of "two physics sims fighting" bugs becomes structurally impossible.

---

## 4. Avian + Lightyear Integration: Critical Details

### 4.1 Required Avian Plugin Configuration

On BOTH server and client:
```rust
app.add_plugins(
    PhysicsPlugins::default()
        .with_length_unit(1.0)
        .build()
        .disable::<PhysicsTransformPlugin>()      // LightyearAvianPlugin handles this
        .disable::<PhysicsInterpolationPlugin>()   // FrameInterpolation handles this
);
```

On client only:
```rust
app.add_plugins(LightyearAvianPlugin {
    replication_mode: AvianReplicationMode::Position,  // replicate Position/Rotation, not Transform
    update_syncs_manually: false,
    rollback_resources: false,    // we use state replication, not deterministic
    rollback_islands: false,
});
```

### 4.2 Why AvianReplicationMode::Position

- `Position` and `Rotation` are smaller to serialize than `Transform`
- Avian operates on `Position`/`Rotation` internally; `Transform` is a view concern
- `LightyearAvianPlugin` handles the Position↔Transform sync automatically
- Prediction/rollback history stores `Position`/`Rotation` (cheaper per-tick snapshots)
- `VisualCorrection` can blend in either Position-space or Transform-space (the `PositionButInterpolateTransform` mode does this but has known child propagation issues; `Position` mode is the default and most tested)

### 4.3 Known Avian+Lightyear Footguns (from lightyear_avian3d source)

1. **Predicted entities**: Lightyear replicates `Position` as `Confirmed<Position>`. Receiving a confirmed update triggers an immediate rollback that inserts the corrected `Position`.

2. **Interpolated entities**: Do NOT add `RigidBody` to `Interpolated` entities. `RigidBody` auto-inserts `Position`/`Rotation`/`Transform` which will fight with Lightyear's interpolation and display the entity at `Transform::default()` until updates arrive. Interpolated entities should only have visual components (mesh, material) + `Position`/`Rotation` from Lightyear.

3. **Interpolated entities - Position/Rotation timing**: For interpolated entities, `Position` and `Rotation` might not both be present at the same time (if `Rotation` doesn't change frequently). Only add rendering components to interpolated entities when BOTH are present.

4. **Mass/Inertia must be set explicitly**: We discovered (and fixed) that Avian derives `ComputedMass` from `Collider` density if no `Mass` component is present. Our flight code computes forces assuming `TotalMassKg` (e.g. 15,000 kg) but Avian was integrating with collider-derived mass (~288 kg), causing ~52x force amplification. **Both server and client must have matching `Mass(total_mass)` and `AngularInertia` components.** The `recompute_total_mass` system already syncs these when mass changes. The `angular_inertia_from_size(mass, &size)` helper computes Avian-compatible 3D angular inertia from gameplay `SizeM`.

5. **Collider should match SizeM**: Collider half-extents should be derived from `SizeM` (`width/2, length/2, height/2`) not hardcoded. This is now implemented in both spawn paths.

### 4.4 System Scheduling with LightyearAvianPlugin

When `LightyearAvianPlugin` is added with `replication_mode: Position`, it configures:

```
RunFixedMainLoop (BeforeFixedMainLoop):
  PhysicsSystems::Prepare
    → TransformToPosition sync (before FrameInterpolation::Restore)

FixedPostUpdate:
  PhysicsSystems::StepSimulation
    → PredictionSystems::UpdateHistory      (save state for rollback)
    → FrameInterpolationSystems::Update     (save state for render interpolation)

PostUpdate:
  FrameInterpolationSystems::Interpolate    (visual interpolation between ticks)
    → RollbackSystems::VisualCorrection     (apply correction blend)
    → PhysicsSystems::Writeback             (Position → Transform)
    → TransformSystems::Propagate           (GlobalTransform propagation)
```

Our `SiderealGamePlugin` systems (`apply_engine_thrust` etc.) run in `FixedUpdate` ordered `.before(PhysicsSystems::StepSimulation)` and `.after(PhysicsSystems::StepSimulation)`. This is compatible — `LightyearAvianPlugin` configures `FixedPostUpdate` and `PostUpdate`, not `FixedUpdate`.

### 4.5 What Happens During Rollback

1. Lightyear detects divergence between confirmed state and `PredictionHistory` at the confirmed tick.
2. Snaps `Predicted` entity to confirmed state (overwrites `Position`, `Rotation`, `LinearVelocity`, etc.).
3. Re-runs `FixedMain` schedule N times (from confirmed tick to current tick).
4. Each re-run executes: our flight systems → Avian StepSimulation → stabilize/clamp → save to PredictionHistory.
5. After all re-runs, compute `VisualCorrection` = old_visual - new_visual. Smoothly decay over time.

**Critical requirement**: Every system that modifies physics state MUST be in `FixedUpdate` (or `FixedFirst`/`FixedLast`). Systems in `Update` or `PostUpdate` are NOT re-run during rollback.

**Performance note**: Rollback re-runs ALL `FixedMain` systems. Systems that are expensive but don't affect physics (e.g. `flush_replication_persistence`, `compute_controlled_entity_scanner_ranges`) should be guarded with `is_in_rollback()` or `DisabledDuringRollback` to skip them during rollback.

---

## 5. Component Registration for Lightyear

### 5.1 Which Components to Register

Every component that Lightyear needs to replicate or predict must be registered. Registration requires `Component + Clone + PartialEq + Debug + Serialize + Deserialize`.

**Replicated + Predicted (needed for rollback correctness)**:
| Component | Notes |
|---|---|
| `Position` | Via LightyearAvianPlugin registration |
| `Rotation` | Via LightyearAvianPlugin registration |
| `LinearVelocity` | Must register manually; needed for force calculation during rollback |
| `AngularVelocity` | Must register manually; needed for torque calculation during rollback |
| `FlightTuning` | Needed for `compute_flight_forces` during rollback |
| `MaxVelocityMps` | Needed for speed clamping during rollback |
| `TotalMassKg` | Needed for force calculation during rollback |
| `SizeM` | Needed for angular inertia calculation during rollback |
| `FlightComputer` | Controls throttle/yaw — server sets it, client needs it for rollback |
| `ActionQueue` | Populated from inputs; consumed by `process_flight_actions` |

**Replicated but NOT predicted (display/identity only)**:
| Component | Notes |
|---|---|
| `EntityGuid` | Identity, set once at spawn |
| `OwnerId` | Ownership identifier |
| `HealthPool` | Display in HUD |
| `HeadingRad` | May be eliminable once Position/Rotation replicate directly |
| `PositionM` | May be eliminable once Position replicates directly |
| `VelocityMps` | May be eliminable once LinearVelocity replicates directly |

**Post-migration simplification**: `PositionM`, `VelocityMps`, `HeadingRad` currently exist because our custom pipeline serialized them into `WorldStateDelta`. With Lightyear replicating `Position`/`Rotation`/`LinearVelocity` directly, these gameplay mirrors may be removable. The HUD and persistence systems that read them would need to read from Avian components instead (or keep thin sync systems).

### 5.2 Module Entities (Engine, FuelTank, FlightComputer-as-module)

Ship modules are separate ECS entities linked by `MountedOn { parent_entity_id: Uuid }`. They carry `Engine`, `FuelTank`, etc.

**Server**: modules must also have `Replicate` so they appear on the client. The `MountedOn` component must be registered for replication so the client can resolve parent relationships.

**Client rollback concern**: During rollback, `apply_engine_thrust` queries `Engine` and `FuelTank` modules via `MountedOn`. These module entities MUST exist on the client and have correct data for rollback to produce correct forces. Options:
- **Option A (recommended)**: Replicate module entities with `Predicted` marker. Their `FuelTank.fuel_kg` will be rolled back correctly.
- **Option B**: Replicate module entities without `Predicted` marker but use `DeterministicPredicted` so they exist during rollback but aren't checked for divergence.

### 5.3 Registration Pattern

```rust
// In a shared function called by both server and client setup:
fn register_sidereal_replication(app: &mut App) {
    // Avian components (handled by LightyearAvianPlugin for Position/Rotation)
    app.register_component::<LinearVelocity>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);
    app.register_component::<AngularVelocity>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);

    // Gameplay components for prediction
    app.register_component::<FlightComputer>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);
    app.register_component::<FlightTuning>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);
    app.register_component::<MaxVelocityMps>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);
    app.register_component::<TotalMassKg>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);
    app.register_component::<SizeM>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);
    app.register_component::<ActionQueue>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);

    // Display/identity components (replicated, not predicted)
    app.register_component::<EntityGuid>(ChannelDirection::ServerToClient);
    app.register_component::<OwnerId>(ChannelDirection::ServerToClient);
    app.register_component::<HealthPool>(ChannelDirection::ServerToClient);

    // Module components
    app.register_component::<MountedOn>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);
    app.register_component::<Engine>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);
    app.register_component::<FuelTank>(ChannelDirection::ServerToClient)
        .add_prediction(ComponentSyncMode::Full);
}
```

NOTE: The exact Lightyear 0.26 registration API may differ slightly. Consult the Lightyear examples and `AppComponentExt` trait for the precise method signatures. The key concept is: every component needs to declare its direction, whether it participates in prediction, and whether it participates in interpolation.

---

## 6. Input Model Migration

### 6.1 Current Input Flow
```
Client keyboard → EntityAction enum variants (ThrustForward, YawLeft, Brake, etc.)
  → Vec<EntityAction> → ClientInputMessage { player_entity_id, actions, tick }
  → send via ServerMultiMessageSender on InputChannel
Server: MessageReceiver<ClientInputMessage>
  → validate auth binding, rate limit, tick ordering
  → push each EntityAction into ActionQueue component on the controlled entity
```

### 6.2 Target Input Flow
```
Client keyboard → PlayerInput { actions: Vec<EntityAction> }
  → InputBuffer<PlayerInput>::set(current_tick, input)
  → Lightyear sends automatically (with redundancy: sends last N ticks per packet)
Server: InputBuffer<PlayerInput> component on the controlled entity
  → drain_inputs_to_action_queue system reads InputBuffer::get(current_tick)
  → pushes actions into ActionQueue
  → process_flight_actions consumes ActionQueue (unchanged)
```

### 6.3 PlayerInput Type Definition
```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct PlayerInput {
    pub actions: Vec<EntityAction>,
}
```

Register with Lightyear's input system. The `player_entity_id` is no longer part of the input message — Lightyear binds inputs to entities via `ControlledBy`/`Controlled` markers set during auth.

### 6.4 Auth Binding Changes

Currently: `AuthenticatedClientBindings` maps `client_entity → player_entity_id`, then `PlayerControlledEntityMap` maps `player_entity_id → entity`.

Target: After JWT validation, use Lightyear's `ControlledBy` marker on the ship entity to bind it to the client. Lightyear then routes inputs to the correct entity automatically.

---

## 7. Visibility Migration

### 7.1 Current Visibility
```
compute_controlled_entity_scanner_ranges   → ScannerRangeM per entity
collect_local_simulation_state             → builds full WorldStateDelta
broadcast_replication_state                → per-client:
  visibility_context_for_client()          → get scanner range, position
  apply_visibility_filter()                → filter WorldStateDelta by range
  → send filtered ReplicationStateMessage
  ClientVisibilityHistory                  → track gained/lost for despawn signals
```

### 7.2 Target Visibility
```
compute_controlled_entity_scanner_ranges   → ScannerRangeM per entity (unchanged)
update_network_visibility                  → NEW system (runs after scanner computation):
  for each client:
    for each replicated entity:
      if in range: state.gain_visibility(client_entity)
      if out of range: state.lose_visibility(client_entity)
  → Lightyear automatically handles spawn/despawn signals based on visibility changes
```

The `NetworkVisibility` component is added to each replicated entity at spawn. Visibility state is updated per-client on the `ReplicationState` component (which `NetworkVisibility` requires).

This replaces ~300 lines of custom visibility filtering, `WorldStateDelta` construction, and `ClientVisibilityHistory` tracking.

---

## 8. Migration Phases

### Phase 0: Foundation (Low Risk, No Behavior Change)

**Goal**: Wire Lightyear registration and `LightyearAvianPlugin` alongside existing custom code. Verify nothing breaks.

1. Add `lightyear_avian3d` to `Cargo.toml` with `3d` feature.
2. Create `register_sidereal_replication()` function (Section 5.3) and call it in both server and client setup.
3. On client: add `LightyearAvianPlugin` (Section 4.1). Disable `PhysicsTransformPlugin` and `PhysicsInterpolationPlugin`.
4. On server: `LightyearAvianPlugin` is NOT needed (server doesn't do prediction/interpolation). But verify Avian plugin disabling is compatible.
5. Define `PlayerInput` type (Section 6.3). Register with Lightyear input system.
6. Run all quality gates. Verify existing behavior unchanged.

**Exit criteria**: Server and client build and run identically to before. Lightyear registrations are present but inert (no `Replicate` components on entities yet).

### Phase 1: Server-Side Replication (Medium Risk)

**Goal**: Server uses `Replicate` + `NetworkVisibility` instead of custom `WorldStateDelta` pipeline.

1. Add `Replicate` component to ship entities in `spawn_simulation_entity()` and `hydrate_simulation_entities()`.
2. Add `Replicate` to module entities (Engine, FuelTank, FlightComputer-module).
3. Add `NetworkVisibility` to all replicated entities.
4. Write `update_network_visibility` system that calls `gain_visibility`/`lose_visibility` based on scanner range (replacing `apply_visibility_filter`).
5. Replace `receive_client_inputs` with `drain_inputs_to_action_queue` that reads from `InputBuffer<PlayerInput>`.
6. DELETE: `collect_local_simulation_state`, `refresh_component_payloads_from_reflection`, `broadcast_replication_state`, `sync_simulated_ship_components` (or keep a thin version that syncs PositionM/VelocityMps/HeadingRad for persistence if still needed).
7. DELETE resources: `ReplicationOutboundQueue`, `LatestReplicationWorld`, `EntityPositionCache`, `ClientVisibilityHistory`.

**Test**: Connect with a headless/debug client. Verify entities appear. Verify component values are correct. The real client will still use the old receive path until Phase 2.

**Exit criteria**: Server replicates via Lightyear. Old `WorldStateDelta` pipeline completely removed from server.

### Phase 2: Client Prediction (High Risk, Highest Value)

**Goal**: Client uses `Predicted`/`Interpolated` markers and Lightyear's rollback instead of custom reconciliation.

1. On client, add `SiderealGamePlugin` (NOT just `SiderealGameCorePlugin`). The full flight system pipeline must run in `FixedUpdate` for rollback to work.
2. When client's controlled entity is received via Lightyear replication, add `Predicted` marker. Also add `RigidBody::Dynamic`, `Collider`, `Mass`, `AngularInertia`, `LockedAxes`, etc. (the Avian physics components needed for real simulation).
3. When remote entities are received, add `Interpolated` marker. Do NOT add `RigidBody`.
4. Write `write_input_to_buffer` system: keyboard → `PlayerInput` → `InputBuffer::<PlayerInput>::set(tick)`.
5. Write `drain_inputs_to_action_queue` system (same as server): `InputBuffer::get(tick)` → `ActionQueue`.
6. Guard expensive non-physics systems with `is_in_rollback()` to skip during rollback re-runs.
7. DELETE: `apply_controlled_reconciliation_fixed_step`, `refresh_predicted_input_history_state`, `interpolate_controlled_transform`, `interpolate_remote_entities`, `receive_lightyear_replication_messages`, `enforce_controlled_planar_motion`.
8. DELETE types: `InterpolationState`, `DisplayVelocity`, `ReconciliationState`, `InputHistory`, `PendingControlledReconciliationState`, `SnapshotBuffer`, `RemoteEntity`, `ClientPhysicsMode`.
9. DELETE: `prediction.rs` → `replay_predicted_state_from_authoritative()` and all supporting types.
10. Tune `RollbackPolicy` thresholds (how much divergence triggers rollback) and `VisualCorrection` decay rate.

**Test**: Connect to server. Press W. Ship should accelerate at the correct rate (~9 m/s² with single engine, 15,000 kg). Turn. Corrections from server should be smooth. Disconnect server — client should extrapolate smoothly.

**Exit criteria**: All custom prediction/reconciliation code deleted. Controlled entity is fully predicted via Lightyear + Avian rollback. Remote entities interpolate smoothly.

### Phase 3: Protocol Cleanup (Low Risk)

1. DELETE from `sidereal-net`: `WorldStateDelta`, `WorldDeltaEntity`, `WorldComponentDelta`, `ReplicationStateMessage`, `ClientInputMessage`, `StateChannel`, `InputChannel`, `LightyearWireMessage`, `encode_wire_message`, `decode_wire_message`.
2. KEEP in `sidereal-net`: `ControlChannel`, auth messages, asset streaming messages, `PlayerRuntimeViewState`.
3. Remove `sidereal-runtime-sync` functions only used by the old pipeline (`insert_registered_components_from_world_deltas_filtered`, etc.) if no remaining callers.
4. Update `AGENTS.md` and `sidereal_design_document.md` to reflect new architecture.
5. Evaluate whether `PositionM`/`VelocityMps`/`HeadingRad` components can be removed (persistence and HUD would read from Avian components directly).

---

## 8.1 Migration Progress (2026-02-22)

This section tracks current in-repo progress so follow-up sessions can resume without re-discovery.

### Completed in this migration stream

- `sidereal-net` protocol split from monolith into:
  - `crates/sidereal-net/src/lightyear_protocol/channels.rs`
  - `crates/sidereal-net/src/lightyear_protocol/input.rs`
  - `crates/sidereal-net/src/lightyear_protocol/messages.rs`
  - `crates/sidereal-net/src/lightyear_protocol/registration.rs`
- Shared reusable `PlayerInput` added in `sidereal-net` with centralized axis-to-`EntityAction` mapping.
- Lightyear native input foundation wired in shared protocol registration:
  - workspace `lightyear` dependency now enables `input_native`
  - `register_lightyear_protocol()` now adds `InputPlugin::<PlayerInput>`
  - `PlayerInput` now derives `Reflect` and implements `MapEntities` for native input plugin compatibility
- Added shared Lightyear replication component registration in `sidereal-net` protocol setup:
  - `register_lightyear_protocol()` now also registers replicated gameplay components (identity, ownership, mounted/module, scanner/faction visibility, kinematic mirrors)
  - prediction-enabled component registration now includes key flight components (`FlightComputer`, `FlightTuning`, `MaxVelocityMps`, `SizeM`, `TotalMassKg`) plus movement mirrors (`PositionM`, `HeadingRad`)
- Client now wires Lightyear Avian integration:
  - added `LightyearAvianPlugin` with `AvianReplicationMode::Position`
  - client Avian plugin setup now disables `PhysicsTransformPlugin` and `PhysicsInterpolationPlugin` so sync/interpolation authority is owned by Lightyear Avian/frame-interpolation flow
- Replication server Avian setup now also disables `PhysicsTransformPlugin` and `PhysicsInterpolationPlugin` to keep physics sync behavior aligned with migration invariants.
- Client input mapping extracted from `bins/sidereal-client/src/main.rs` into:
  - `bins/sidereal-client/src/client/mod.rs`
  - `bins/sidereal-client/src/client/input.rs`
- Replication server split upfront (no new monolith growth):
  - `bins/sidereal-replication/src/replication/lifecycle.rs`
  - `bins/sidereal-replication/src/replication/hydration_parse.rs`
  - `bins/sidereal-replication/src/replication/input.rs`
  - `bins/sidereal-replication/src/replication/visibility.rs`
  - `bins/sidereal-replication/src/replication/legacy_state.rs`
  - `bins/sidereal-replication/src/replication/auth.rs`
  - `bins/sidereal-replication/src/replication/assets.rs`
  - `bins/sidereal-replication/src/replication/persistence.rs`
  - `bins/sidereal-replication/src/replication/physics_runtime.rs`
  - `bins/sidereal-replication/src/replication/runtime_state.rs`
  - `bins/sidereal-replication/src/replication/transport.rs`
  - `bins/sidereal-replication/src/replication/view.rs`
  - `bins/sidereal-replication/src/replication/mod.rs`
- `main.rs` now uses extracted module systems for transport/auth/input/assets/view/runtime-state instead of keeping those concerns inline.
- Server input cutover started:
  - removed scheduled `MessageReceiver<ClientInputMessage>` ingestion from replication update loop
  - added scheduled native drain `drain_native_player_inputs_to_action_queue` (reads Lightyear native `ActionState<PlayerInput>` path)
  - removed legacy server-side input rate-limiter state cleanup and related wiring
- Client input cutover started:
  - `send_lightyear_input_messages` no longer sends `ClientInputMessage`; it now writes native `ActionState<PlayerInput>`
  - input writes are scheduled in `FixedPreUpdate` under Lightyear `WriteClientInputs`
  - controlled entities are ensured to have `InputMarker<PlayerInput>`/`ActionState<PlayerInput>` at runtime
- Removed custom `InputChannel` transport wiring from client/server transport setup and sidereal-net protocol channel registration.
- Deleted stale legacy input protocol artifacts from `sidereal-net`:
  - removed `ClientInputMessage` type and its protocol registration/test usage
  - removed `InputChannel` channel type
- Began first prediction-tagging slice on client spawn paths:
  - controlled entities in predicted mode now receive `Predicted`
  - remote entities in predicted mode now receive `Interpolated`
- Removed client legacy prediction/interpolation pipeline from active runtime:
  - deleted `bins/sidereal-client/src/prediction.rs` and removed `mod prediction`
  - removed legacy systems (`apply_controlled_reconciliation_fixed_step`, `refresh_predicted_input_history_state`, `interpolate_controlled_transform`, `interpolate_remote_entities`) from schedules and code
  - removed legacy state/components (`InputHistory`, `ReconciliationState`, `PendingControlledReconciliation`, `SnapshotBuffer`, `InterpolationState`, legacy marker components)
  - controlled and remote entities now apply authoritative transform/velocity updates directly on replication message ingest
- Legacy outbound state pipeline is being consolidated into `replication/legacy_state.rs`:
  - moved `refresh_component_payloads_from_reflection`
  - moved `broadcast_replication_state`
  - moved `serialize_registered_components_for_entity`
  - moved environment helper `fullscreen_layer_delta`
  - moved `collect_local_simulation_state` finalization tail into `finalize_collected_simulation_state` (tick/queue/cache/persistence ingestion block)
  - moved ship delta assembly + dirty-state evaluation into `collect_ship_deltas`
  - moved hardpoint/module delta assembly block into `collect_attachment_deltas`
  - moved `collect_local_simulation_state` system orchestration into `replication/legacy_state.rs` (main entrypoint now only wires the system)
- Added replication runtime cutover switch `SIDEREAL_REPLICATION_NATIVE_WORLD_SYNC`:
  - when enabled, the legacy `WorldStateDelta` collect/serialize/broadcast fixed-update chain is disabled via schedule `run_if`
  - default behavior remains legacy-enabled until native world replication flow is fully wired
  - bootstrap-spawned ships now attach Lightyear `Replicate::to_clients(NetworkTarget::All)` + `NetworkVisibility` when the cutover flag is enabled
  - bootstrap-spawned attached module entities now also attach Lightyear `Replicate::to_clients(NetworkTarget::All)` + `NetworkVisibility` when the cutover flag is enabled
  - hydration-spawned ships, hardpoints, and modules now attach Lightyear `Replicate::to_clients(NetworkTarget::All)` + `NetworkVisibility` when the cutover flag is enabled
- Added native visibility runtime system wiring in `bins/sidereal-replication/src/replication/visibility.rs`:
  - new `update_network_visibility` system now updates per-client `ReplicationState` via `gain_visibility`/`lose_visibility` for `NetworkVisibility` entities
  - native cutover mode now runs a dedicated post-physics fixed-update chain for controlled-position updates, scanner-range updates, and network visibility updates
  - visibility checks currently preserve ownership/public/faction sharing and scanner-range distance gates under the native path
- Removed legacy world-delta pipeline from active server runtime schedule:
  - replication no longer schedules `collect_local_simulation_state`, `refresh_component_payloads_from_reflection`, or `broadcast_replication_state`
  - post-physics fixed update now runs native path directly (`sync_simulated_ship_components` + position/scanner updates + `update_network_visibility`)
  - `process_bootstrap_ship_commands` and hydration/spawn flows now always attach `Replicate::to_clients(NetworkTarget::All)` + `NetworkVisibility` (legacy non-native spawn mode removed)
- Deleted legacy replication implementation module `bins/sidereal-replication/src/replication/legacy_state.rs`.
- Removed legacy world-delta queue/cache runtime scaffolding from replication entrypoint (`ReplicationOutboundQueue`, `QueuedReplicationDelta`, `LatestReplicationWorld`, `EntityPositionCache`) and simplified `ReplicationRuntime` to persistence-only runtime needs.
- Simplified replication persistence module by removing unused legacy world-delta flush path (`flush_replication_persistence`), keeping player runtime view persistence path only.
- Simplified replication visibility helper module (`bins/sidereal-replication/src/visibility.rs`) to native-runtime requirements only (client registry + controlled-position map + default range), dropping legacy world-delta filtering helpers.
- Asset bootstrap streaming no longer depends on legacy `LatestReplicationWorld` + world-delta visibility filtering. It now derives required assets from always-required defaults plus explicit client requests/dependency expansion.
- Reflection-driven component serialization helper moved from entrypoint into `replication/legacy_state.rs` (`serialize_registered_components_for_entity`) to keep legacy state concerns co-located.
- Lifecycle module now also owns connection lifecycle logging (`log_replication_client_connected`), reducing entrypoint event handling.
- Hydration graph-component decode helpers moved into `replication/hydration_parse.rs` (owner/faction/flight/mass/scanner parsing), reducing duplication and entrypoint scope.
- Removed stale legacy input/rate-limiter tests from `bins/sidereal-replication/src/main.rs` test module so replication test targets compile against the native input path.
- Client now runs native world-sync runtime intake directly:
  - added `adopt_native_lightyear_replicated_entities` to classify/tag Lightyear `Replicated` entities into existing runtime roles (`ControlledEntity` / `RemoteVisibleEntity`) and register runtime hierarchy IDs without manual world-delta spawns
  - added `sync_display_velocity_from_replicated_motion` so runtime velocity HUD reads replicated physics velocity (`LinearVelocity`) with mirror fallback during transition
- Client native world-sync now runs unconditionally (legacy world-delta ingest removed):
  - deleted `receive_lightyear_replication_messages` and supporting world-delta decode/merge helpers from `bins/sidereal-client/src/main.rs`
  - update schedules now drive world entity adoption/motion from native replicated components only
  - client transport no longer provisions `StateChannel` receivers
- Protocol cleanup progressed:
  - removed `ReplicationStateMessage` and `StateChannel` from `sidereal-net` lightyear protocol registration/types
  - removed server-side `StateChannel` transport wiring from replication transport setup
- Phase-3 protocol cleanup for legacy world-delta types is now complete in `sidereal-net`:
  - moved `WorldStateDelta`, `WorldDeltaEntity`, and `WorldComponentDelta` into `sidereal-persistence` (persistence/test-only ownership)
  - updated replication/persistence/runtime-sync/shard test imports to consume delta types from `sidereal-persistence`
  - removed legacy `LightyearWireMessage` + `encode_wire_message` / `decode_wire_message` helpers from `sidereal-net`
- Client transform bridge remnant has been reduced:
  - replaced `sync_native_replicated_motion_from_components` with `sync_display_velocity_from_replicated_motion`
  - client no longer writes `Transform` from `PositionM`/`HeadingRad`; transform ownership stays with replicated Avian/Lightyear physics components
  - `DisplayVelocity` now reads `LinearVelocity` first, with `VelocityMps` fallback while mirror readers still exist
- Client predicted-entity bootstrap now supports rollback-capable physics:
  - removed `SIDEREAL_CLIENT_NATIVE_PREDICTION` opt-in gate; predicted mode now always applies native `Predicted`/`Interpolated` markers
  - controlled predicted ships now insert Avian runtime physics components on adopt (`RigidBody::Dynamic`, `Collider`, `Mass`, `AngularInertia`, `Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`, planar locks + damping)
  - predicted physics insertions use replicated `SizeM`/`TotalMassKg`/`PositionM`/`HeadingRad`/`VelocityMps` when present, with corvette defaults as fallback at first adoption tick
- Client mirror fallback reduction progressed:
  - `adopt_native_lightyear_replicated_entities` now prefers replicated Avian `Position`/`Rotation`/`LinearVelocity` for predicted-entity initialization and only falls back to mirror gameplay components when those are not yet present
  - `sync_display_velocity_from_replicated_motion` now reads `LinearVelocity` only (mirror velocity fallback removed)
  - `DisplayVelocity` is now attached at world-entity adoption so HUD velocity can be maintained consistently
- Client predicted bootstrap now runs without mirror gameplay component dependencies:
  - removed `PositionM`/`HeadingRad`/`VelocityMps` reads from predicted-entity adoption path
  - controlled predicted-ship adoption is deferred until replicated Avian `Position` + `Rotation` + `LinearVelocity` are present, avoiding mixed-state initialization
  - predicted insertions now use native replicated Avian motion state directly
- Added coverage for deferred predicted adoption guard:
  - extracted `should_defer_controlled_predicted_adoption(...)` helper in client runtime
  - added unit tests validating defer/proceed behavior for controlled vs non-controlled entities and Avian motion-component readiness
- Added runtime diagnostics for deferred controlled predicted adoption:
  - introduced `DeferredPredictedAdoptionState` resource to track active wait windows
  - adoption logs now emit throttle-limited warnings (1s cadence) when required replicated Avian motion components are missing
  - diagnostics clear automatically once controlled adoption succeeds
- Bootstrap/runtime guardrails now include adoption-delay failure signaling:
  - fixed replication-state readiness tracking so `replication_state_seen` is marked once replicated entities begin arriving
  - `watch_in_world_bootstrap_failures` now emits a user-visible warning dialog if controlled predicted adoption is stalled >4s while replication is active
  - warning payload includes waiting entity id, wait duration, and missing Avian component list
- Adoption-delay thresholds are now runtime-tunable for load testing:
  - added `PredictionBootstrapTuning` resource (`from_env`) to control defer warning timing/cadence and dialog threshold
  - env keys: `SIDEREAL_CLIENT_DEFER_WARN_AFTER_S` (default 1.0), `SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S` (default 1.0), `SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S` (default 4.0)
- Added aggregate delayed-adoption telemetry for live-session tuning:
  - deferred-adoption state now tracks resolved delay sample count, cumulative wait, and max wait
  - emits per-resolution info logs (`resolved after Xs`) and periodic summary logs (`samples / avg_wait / max_wait`)
  - summary cadence is tunable via `SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S` (default 30.0)
- Added periodic prediction runtime health summaries:
  - new `log_prediction_runtime_state` system emits interval logs for `world`/`replicated`/`predicted`/`interpolated`/`controlled` entity counts plus current deferred-waiting entity id
  - uses same summary interval tuning (`SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S`) so live load sessions can correlate adoption latency with runtime marker distribution
  - now emits anomaly warnings in predicted mode when replication is active but no controlled entity is present after threshold age, or when replicated entities exist but zero `Predicted` markers are attached
- Critical rollback-simulation wiring updated:
  - client now runs `SiderealGamePlugin` in predicted mode, so rollback replays full flight/mass gameplay systems instead of component-registration-only mode
- Client physics-mode simplification progressed:
  - removed `ClientPhysicsMode` enum/runtime branching from client runtime
  - production/default path is now always native predicted simulation; optional local simulation is a lightweight debug toggle (`LocalSimulationDebugMode`) sourced from `SIDEREAL_CLIENT_PHYSICS_MODE=local`
  - client startup now always installs `SiderealGamePlugin` (full gameplay systems)
- Expensive-system rollback cost reductions:
  - `recompute_total_mass` now early-outs before building inventory/module trees when no root entity is dirty (or missing initial mass), reducing fixed-step overhead during rollback resim frames
  - replication scanner range runtime (`compute_controlled_entity_scanner_ranges`) now exits during rollback via `is_in_rollback(...)` guard
- Prediction manager correction/rollback tuning is now explicit:
  - added `configure_prediction_manager_tuning` system to configure Lightyear `PredictionManager` on client creation
  - env keys: `SIDEREAL_CLIENT_MAX_ROLLBACK_TICKS` (default 100), `SIDEREAL_CLIENT_INSTANT_CORRECTION` (`false` => smooth/default correction policy, `true` => instant correction)
- Removed transitional velocity bridge:
  - deleted `DisplayVelocity` component and `sync_display_velocity_from_replicated_motion`
  - camera/HUD now read `LinearVelocity` directly from replicated Avian state
- Planar-enforcement evaluation complete:
  - `enforce_controlled_planar_motion` is now Local-debug-only (no longer runs in predicted production path)
  - predicted path relies on replicated Avian `LockedAxes` + physics state instead of extra per-tick transform coercion
- Runtime-sync dead API cleanup complete:
  - removed unused world-delta helper APIs from `sidereal-runtime-sync` (`extract_*_from_world_delta`, `insert_registered_components_from_world_deltas*`, `remove_runtime_entity`, `update_parent_link_from_properties`)
  - kept only actively used graph/hierarchy helpers
- World-delta persistence cleanup complete:
  - removed `WorldStateDelta`/`WorldDeltaEntity`/`WorldComponentDelta` from `sidereal-persistence` public API
  - replication state ingestion now uses `GraphDeltaBatch` (upserts/removals of `GraphEntityRecord`) instead of world-delta structs
  - bootstrap/runtime test fixtures now persist via `persist_graph_records` + `remove_graph_entities` (no `persist_world_delta` path remains)
- Server mirror-motion component cleanup complete:
  - replication ship spawn/hydration no longer inserts `PositionM`/`VelocityMps`/`HeadingRad`
  - `sync_simulated_ship_components` now mirrors Avian `Position`/`Rotation` directly into `Transform` only
  - Lightyear protocol registration no longer registers mirror motion components (`PositionM`, `VelocityMps`, `HeadingRad`)
  - removed legacy mirror motion component definitions from `sidereal-game` generated component registry/schema and from `CorvetteBundle`
- Documentation contract updates:
  - `AGENTS.md` now enforces graph-record persistence as canonical and forbids introducing new world-delta persistence paths
  - `docs/sidereal_design_document.md` updated to clarify JSON envelope helpers are persistence/test fixtures, not active runtime replication protocol
  - `docs/sidereal_design_document.md`/`AGENTS.md` now explicitly document Avian-authoritative motion and forbid reintroducing legacy gameplay motion mirror components
- `sidereal-net` dependency footprint cleanup complete:
  - `serde`, `serde_json`, `sidereal-game`, and `sidereal-asset-runtime` are now optional and gated behind `lightyear_protocol`
  - base `sidereal-net` crate builds with no required dependencies when `lightyear_protocol` is disabled
- Clippy quality gate blockers from migration slices are now resolved:
  - replaced `NetworkVisibility::default()` (unit-struct lint) with `NetworkVisibility`
  - added focused `#[allow(clippy::type_complexity)]` annotations on high-complexity ECS system signatures used in migration wiring
- Completed `sidereal-net` protocol boundary cleanup:
  - `sidereal-net/src/lib.rs` is now Lightyear-protocol-only (re-exports `lightyear_protocol` module, nothing else)
  - moved `NetEnvelope<T>`, `ChannelClass`, `encode_envelope_json`, `decode_envelope_json` into `sidereal-persistence/src/legacy_envelope.rs`
  - moved `PlayerRuntimeViewState` into `sidereal-persistence/src/lib.rs` (persistence-layer data model, not a network protocol type)
  - removed dead duplicate `WorldStateDelta`/`WorldDeltaEntity`/`WorldComponentDelta` definitions from `sidereal-net` (all consumers already imported from `sidereal-persistence`)
  - removed `sidereal-net` dependency from `sidereal-persistence/Cargo.toml` (breaks circular concern; persistence no longer depends on net)
  - removed unused `sidereal-net` dependency from `sidereal-runtime-sync/Cargo.toml`
  - removed unused `sidereal-core` dependency from `sidereal-net/Cargo.toml`
  - moved `envelope_codec` integration test from `sidereal-net/tests/` to `sidereal-persistence/tests/`
  - rewired all consumer imports: replication `state.rs`, `main.rs`, `auth.rs`, `runtime_state.rs`, `view.rs`, `lifecycle.rs` test

- Generic entity simulation state persistence restored:
  - added `serialize_entity_components_to_graph_records()` to `sidereal-runtime-sync` — uses `GeneratedComponentRegistry` + `ReflectComponent::reflect()` + `TypedReflectSerializer` to generically serialize any registered ECS component into `GraphComponentRecord` JSON, round-trippable with the existing `TypedReflectDeserializer` hydration path
  - added `flush_simulation_state_persistence` system in `replication/persistence.rs` — queries all `SimulatedControlledEntity` and their `MountedOn` module entities, serializes all registered components plus Avian `Position`/`Rotation`/`LinearVelocity` as graph properties, and persists via `GraphPersistence::persist_graph_records()`
  - persistence runs on a throttled tick interval (default 300 ticks = 10s at 30Hz, configurable via `SIDEREAL_PERSIST_INTERVAL_TICKS`)
  - wired into server `FixedUpdate` schedule after `sync_simulated_ship_components` chain
- Bootstrap/hydration module parity fix:
  - `bootstrap_starter_ship` now persists complete corvette module layout matching `corvette.rs` and `spawn_simulation_entity`: 1 flight computer + 2 engines (1.2M thrust each) + 2 fuel tanks
  - previously persisted a single engine with 280k thrust, causing hydration to restore a much weaker ship than runtime spawning created
  - includes mass/size components on hull record for proper hydration fallback

### Current phase status

- **Phase 0** — Complete.
  - `PlayerInput` foundation, code splitting guardrails, shared native input plugin registration are all done.
- **Phase 1** — Complete.
  - Native Lightyear replication/visibility runtime path is active; legacy server world-delta runtime path has been removed.
  - Native Lightyear input is now active end-to-end for production/consumption.
- **Phase 2** — Complete.
  - All legacy world-delta types, mirror motion components, and custom reconciliation/interpolation code removed.
  - Protocol boundary cleanup complete (`sidereal-net` is Lightyear-only, persistence types relocated).
  - Dependency gating complete (`sidereal-net` optional deps behind `lightyear_protocol`).
- **Phase 3** — Complete.
  - Generic entity simulation state persistence restored via reflection-based serialization.
  - Bootstrap/hydration module parity aligned with runtime spawning.
  - All quality gates passing (fmt, clippy, workspace check, WASM, Windows).

### Remaining work (tuning/validation only — no structural changes)

1. Complete Lightyear-native client prediction/interpolation behavior tuning (confirmed + predicted + interpolated) so rollback/interpolation correctness is fully validated under gameplay load.
2. Validate correction policy + rollback window settings (`SIDEREAL_CLIENT_MAX_ROLLBACK_TICKS`, `SIDEREAL_CLIENT_INSTANT_CORRECTION`) under real jitter and lock production defaults.
3. Run controlled load sessions (multi-client connect/disconnect + input bursts) and record baseline telemetry targets for `avg_wait_s`/`max_wait_s`, then lock recommended defaults for `SIDEREAL_CLIENT_DEFER_*`.

### Runtime tuning playbook

Use this sequence during live load checks:

1. Start with defaults:
   - `SIDEREAL_CLIENT_DEFER_WARN_AFTER_S=1.0`
   - `SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S=1.0`
   - `SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S=4.0`
   - `SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S=30.0`
2. Run 2+ concurrent clients with repeated connect/disconnect and immediate input bursts.
3. Watch logs for:
   - `predicted adoption delay summary` (samples/avg/max)
   - `prediction runtime summary` (replicated/predicted/interpolated/controlled counts)
   - anomaly warnings (`no controlled entity...`, `zero Predicted markers...`)
4. Tune thresholds:
   - if brief, harmless join delays create too many warnings, increase `...WARN_AFTER_S`/`...WARN_INTERVAL_S`
   - if real control gaps are missed, reduce `...DIALOG_AFTER_S` to surface earlier
5. Keep thresholds unchanged only if:
   - controlled entity appears quickly under expected load,
   - anomaly warnings are rare/absent,
   - summary `max_wait_s` stays within acceptable gameplay startup latency.

---

## 9. Risks and Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| Rollback re-runs ALL FixedMain systems | Perf: mass recompute, scanner calc etc. run N extra times | Guard with `is_in_rollback()` or `DisabledDuringRollback` |
| Module entities must exist on client for rollback | Correctness: `apply_engine_thrust` queries Engine/FuelTank via MountedOn | Replicate modules with `Predicted` marker so they exist during rollback |
| Lightyear component registration requires Clone+PartialEq+Debug | Build: some components may need trait derives added | Add derives to generated components in schema or by hand |
| Avian computed mass/inertia from Collider if Mass not set | Physics: 52x force amplification (already fixed) | Keep `Mass(total_mass)` + `angular_inertia_from_size()` on all physics entities. `recompute_total_mass` syncs these. |
| PositionM/VelocityMps/HeadingRad redundancy | Complexity: two representations of same data | Evaluate removal in Phase 3; persistence can read Position directly |
| WASM client parity | Requirement: WASM must build at every phase | Verify `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu` at each phase |
| Auth binding model change | Security: must not allow input spoofing | Map JWT-validated identity to Lightyear's ControlledBy; reject unauthed inputs |

---

## 10. Files to Modify (Reference)

**Server** (`bins/sidereal-replication/src/main.rs`, ~3,500 lines):
- Entity spawn functions: add `Replicate`, `NetworkVisibility`
- System schedule: remove custom replication pipeline, add `drain_inputs_to_action_queue`, `update_network_visibility`
- Remove ~800 lines of `collect_local_simulation_state`, `refresh_component_payloads_from_reflection`, `broadcast_replication_state`
- Remove ~100 lines of `receive_client_inputs` (replaced by Lightyear input)

**Client** (`bins/sidereal-client/src/main.rs`, ~3,900 lines):
- Plugin setup: add `SiderealGamePlugin`, `LightyearAvianPlugin`, disable Avian plugins
- Entity spawn: add `Predicted`/`Interpolated` markers, physics components on predicted entities
- Remove ~600 lines of custom reconciliation, interpolation, input sending
- Remove `ClientPhysicsMode` enum and collapse to predicted-default runtime with debug-only local toggle

**Client prediction** (`bins/sidereal-client/src/prediction.rs`, ~435 lines):
- DELETE entire file (or reduce to just the `PlayerInput` type definition)

**Protocol** (`crates/sidereal-net/src/lib.rs` + `lightyear_protocol.rs`, ~300 lines):
- Remove WorldStateDelta types, custom message types, custom channels
- Keep auth/asset messages and ControlChannel

**Game** (`crates/sidereal-game/src/`, unchanged):
- All flight, mass, action systems remain exactly as-is
- They run in `FixedUpdate` and are naturally compatible with Lightyear rollback

---

## 11. Validation Checklist

After each phase, verify:
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo check --workspace`
- [ ] `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
- [ ] `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`
- [ ] `cargo test -p sidereal-game -p sidereal-client`
- [ ] Server starts and accepts connections
- [ ] Client connects, receives entities
- [ ] Controlled ship accelerates at correct rate (~120 m/s² with current default corvette tuning)
- [ ] Controlled ship reaches max velocity 600 m/s without oscillation
- [ ] Controlled ship turns smoothly
- [ ] Controlled ship brakes to stop
- [ ] Second client sees first client's ship moving smoothly
- [ ] Server shutdown → client continues extrapolating gracefully

---

## 12. Enforceable Refactor Guardrails (Code Splitting + Generic Naming)

These rules are mandatory for this migration and align with `AGENTS.md` non-negotiables. They are not style suggestions.

### 12.1 Single-Responsibility File Split (Required)

Do not continue adding large mixed concerns to `bins/sidereal-client/src/main.rs` and `bins/sidereal-replication/src/main.rs`.

For each migration phase, extract code into focused modules with clear ownership:

- **Client**
  - `client/input.rs` (input buffering and intent mapping only)
  - `client/prediction.rs` (Lightyear prediction/rollback wiring only)
  - `client/interpolation.rs` (interpolated entity visuals only)
  - `client/replication.rs` (Lightyear replicated-entity hooks only)
  - `client/camera.rs` (camera behavior only)
  - `client/hud.rs` (HUD/UI read-model only)
  - `client/materials.rs` (shader/material update systems only)
  - `client/auth.rs` and `client/assets.rs` (control-channel flows only)

- **Replication server**
  - `replication/auth.rs` (session binding and auth only)
  - `replication/input.rs` (InputBuffer drain and validation only)
  - `replication/visibility.rs` (NetworkVisibility gain/lose only)
  - `replication/spawn.rs` and `replication/hydration.rs` (entity spawn/hydrate only)
  - `replication/persistence.rs` (persistence flush only)
  - `replication/assets.rs` (asset stream transport only)

If a file owns multiple unrelated domains, the phase is not complete.

### 12.2 Components/Types Must Be Split Out of Runtime Entrypoints

`main.rs` files must not remain the source of truth for most component/resource/type declarations.

- Move domain components/resources/events into crate-local modules (for example `components.rs`, `resources.rs`, `events.rs`) under client/replication subtrees.
- Keep `main.rs` as composition and wiring only (plugin registration, schedule wiring, app boot config).
- Shared gameplay components remain source-of-truth in `crates/sidereal-game` schema/generation path.

### 12.3 Generic Naming Rule (Strict)

Any system/resource/API that is not truly ship-only must use generic entity terminology.

- **Forbidden for generic behavior**: `Ship*`, `ship_*`, `*_ship_*`, names that imply ship-only semantics for cross-entity logic.
- **Required for generic behavior**: entity/runtime/visibility/control terminology (for example `ControlledEntityMap`, `EntityVisibilityState`, `RuntimeAuthorityBinding`).
- Ship-specific names are allowed only when behavior is physically tied to ship-only mechanics.

This applies to file names, type names, function names, system labels, and comments.

### 12.4 Migration PR Gate (Must Pass Before Phase Exit)

Each phase PR must include:

- A module split diff showing at least one domain extracted from each touched monolithic entrypoint.
- No new >300 line function added to entrypoint files.
- No new generic systems introduced with ship-specific naming.
- Updated docs when introducing new enforceable behavior (per `AGENTS.md` Section 7).

If any item fails, the phase is incomplete and should not be merged.

---

## 13. Runtime Invariants Learned From This Incident

These are migration-blocking invariants based on observed prediction/replication failures.

### 13.1 Single-Writer Motion Invariant

For a controlled predicted entity, only one fixed-tick path may write authoritative motion state (`Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`) in that mode.

- Predicted mode: reconciliation/rollback pipeline is the only writer.
- Local full-sim mode: physics simulation pipeline is the only writer.
- Visual-only interpolation may write render transforms, but must not feed back into simulation state.

### 13.2 Mass/Inertia Parity Invariant

Force/torque generation assumptions must match Avian integration assumptions every tick.

- If gameplay math uses `TotalMassKg` and size-derived inertia, Avian `Mass` and `AngularInertia` must exist and be synchronized for the same entity.
- Spawn and hydration paths must insert required Avian mass/inertia components immediately.
- Runtime mass recompute must update both gameplay and Avian mass/inertia representations in the same system pass.

### 13.3 Local Intent Ownership Invariant

Predicted local input intent must not be overwritten by replicated stale server intent.

- Replicated control-state components are for confirmed state comparison, not for replacing local pending intent on the controlled predicted entity.
- Input ownership remains bound to authenticated session identity and routed through the input buffer path.

### 13.4 Fixed-Time Contract Invariant

All simulation and prediction math must use fixed-step time; variable frame delta is render-only.

- Simulation systems in fixed schedules use fixed timestep resources.
- No gameplay force/integration path may read render-frame delta for simulation math.

### 13.5 Shader Source Parity Invariant

When shader logic is changed for streamed runtime usage, source parity must be maintained.

- Update both source and streamed cache shader paths in the same change, or centralize shader source generation so one source produces both outputs.
- Material uniform schemas must remain aligned with shader bindings at both paths.

### 13.6 Runtime Observability Gate

Sync/prediction changes are not complete without live verification of core runtime state.

- Verify controlled entity mass/inertia, velocity, heading, and server-message staleness in live runtime inspection.
- Validate no duplicate writers are mutating controlled motion state in the same fixed tick.
- Confirm client/server fixed-rate contract matches configured tick duration.

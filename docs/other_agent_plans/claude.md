# Starmux — Server-Authoritative Space Game Architecture

## 1. Executive Summary

Starmux is a server-authoritative multiplayer space game built on **Bevy 0.18 + Lightyear 0.26.4 + Avian3D 0.5** with graph-shaped persistence. The architecture enforces a strict one-way authority flow: **client intent → server simulation → replication → persistence**. Clients never write authoritative world state.

The vertical slice delivers:
- Two simultaneous clients, each predicting their own entity via Lightyear rollback/replay.
- Non-controlled entities rendered via server-snapshot interpolation only.
- Runtime control swapping (player ↔ drone/ship) with explicit request/ack/reject protocol.
- Server-side interest management with spatial indexing for scale to 100s clients / 1000s entities.
- Graph-record persistence with deterministic hydration of hierarchies.
- Native + WASM parity (same prediction semantics; transport boundary only).

---

## 2. Architecture Diagram

```
┌───────────────────────────────────────────────────────────────────────────┐
│                              CRATE MAP                                    │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                     starmux_protocol (lib)                          │  │
│  │  Components · Messages · Channels · StableId · Enums               │  │
│  └──────────────────────────┬──────────────────────────────────────────┘  │
│                ┌────────────┴────────────┐                                │
│  ┌─────────────▼──────────┐  ┌──────────▼─────────────┐                  │
│  │   starmux_server (bin)  │  │  starmux_client (bin)   │                  │
│  │                         │  │                         │                  │
│  │  sim::                  │  │  input::                │                  │
│  │    physics_step         │  │    capture_input        │                  │
│  │    flight_computer      │  │    send_input           │                  │
│  │    engine_fuel          │  │                         │                  │
│  │    force_apply          │  │  prediction::           │                  │
│  │                         │  │    predict_controlled   │                  │
│  │  control::              │  │    rollback (lightyear) │                  │
│  │    handle_request       │  │                         │                  │
│  │    ack_reject           │  │  interpolation::        │                  │
│  │    route_input          │  │    interp_remotes       │                  │
│  │                         │  │                         │                  │
│  │  visibility::           │  │  camera::               │                  │
│  │    spatial_index        │  │    follow_controlled    │                  │
│  │    interest_cull        │  │                         │                  │
│  │    authorize_redact     │  │  rendering::            │                  │
│  │                         │  │    sync_render_xforms   │                  │
│  │  persistence::          │  │                         │                  │
│  │    snapshot_world       │  └─────────────────────────┘                  │
│  │    hydrate_world        │                                              │
│  │    graph_records        │                                              │
│  └─────────────────────────┘                                              │
└───────────────────────────────────────────────────────────────────────────┘
```

### Data Flow

```
  CLIENT                           SERVER                        PERSISTENCE
  ──────                           ──────                        ───────────
  ┌──────────┐    InputMsg     ┌──────────────┐
  │ Capture  │ ──────────────► │ Route Input  │
  │  Input   │                 │  to Target   │
  └──────────┘                 └──────┬───────┘
                                      │
                               ┌──────▼───────┐
       ControlReq              │ ActionQueue  │
  ─────────────────────►       │ FlightComp   │
       ControlAck/Rej          │ Engine/Fuel  │
  ◄─────────────────────       │ Force Apply  │
                               └──────┬───────┘
                                      │
                               ┌──────▼───────┐
                               │ Avian3D      │
                               │ Physics Step │
                               └──────┬───────┘
                                      │
                               ┌──────▼───────┐    ┌────────────┐
                               │ Replicate    │───►│ Snapshot   │
                               │ (Lightyear)  │    │ Persist    │
                               └──────┬───────┘    └────────────┘
                                      │
                               ┌──────▼───────┐
                               │ Visibility   │
                               │ Interest Mgr │
                               └──────┬───────┘
                                      │
                  ┌────────────────────┤
                  ▼                    ▼
           Client A              Client B
       ┌───────────┐         ┌───────────┐
       │ Predicted  │         │ Predicted  │
       │ (own)      │         │ (own)      │
       │ Interpolated│        │ Interpolated│
       │ (remote)   │         │ (remote)   │
       └───────────┘         └───────────┘
```

---

## 3. Component Model

### 3.1 Stable Identity

```rust
/// Cross-boundary identity. Never use raw Bevy Entity across network.
#[derive(Component, Serialize, Deserialize, Clone, Hash, Eq, PartialEq)]
pub struct StableId(pub Uuid);

/// Account identity (auth layer). 1:1 with authenticated session.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct AccountId(pub Uuid);

/// Gameplay identity. Multiple characters per account possible.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct PlayerId(pub Uuid);
```

### 3.2 Control Chain

```
Camera  ──follows──►  PlayerEntity  ──controls──►  ControlledEntity
                      (self if free-roam)
```

```rust
/// Present on the player entity. Points to the currently controlled entity.
/// Free-roam: controlled_id == own StableId (self-control, never None).
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct ControlLink {
    pub controlled_id: StableId,
}

/// Marker: this entity is predicted locally (client-side only).
#[derive(Component)]
pub struct Predicted;

/// Marker: this entity is interpolated from server (client-side only).
#[derive(Component)]
pub struct Interpolated;

/// Server-side: which session owns this entity for input routing.
#[derive(Component)]
pub struct InputOwner {
    pub session_id: ClientId,
}
```

### 3.3 Physics / Motion (Avian3D native)

```rust
// These are Avian3D components used directly — NOT wrapped:
//   Position, Rotation, LinearVelocity, AngularVelocity,
//   ExternalForce, ExternalTorque, RigidBody, Collider, Mass
```

### 3.4 Gameplay Force Pipeline

```rust
/// Intent actions queued per-tick, consumed by FlightComputer.
#[derive(Component, Serialize, Deserialize, Clone, Default)]
pub struct ActionQueue {
    pub actions: Vec<FlightAction>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FlightAction {
    Thrust(Vec3),
    Rotate(Vec3),
    Boost,
    Cut,
}

/// Translates actions into desired force/torque given ship capabilities.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct FlightComputer {
    pub max_thrust: f32,
    pub max_torque: f32,
}

/// Engine state + fuel, consumed by force application.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct Engine {
    pub thrust_fraction: f32,
    pub fuel: f32,
    pub fuel_rate: f32,
}

/// Ship class / metadata.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct ShipClass {
    pub name: String,
    pub mass: f32,
}
```

### 3.5 Visibility / Interest

```rust
/// Server-side. Per-client camera position for interest culling.
#[derive(Component)]
pub struct ClientViewport {
    pub center: Vec2,      // XY top-down
    pub half_extent: Vec2, // visible half-size
    pub edge_buffer: f32,  // hysteresis buffer
}

/// Server-side. Visibility tier for an entity.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct VisibilityTier {
    pub scanner_floor: f32,       // e.g. 300m — always visible within
    pub faction_visible: bool,
    pub owner_always_visible: bool,
    pub public: bool,
}
```

### 3.6 Persistence

```rust
/// Marker: this entity should be persisted.
#[derive(Component)]
pub struct Persistent;

/// Relationship tracking for hierarchy persistence.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MountRelation {
    pub parent_id: StableId,
    pub slot: String,
}
```

---

## 4. Networking / Protocol Model

### 4.1 Channels

| Channel             | Delivery        | Direction      | Purpose                    |
|---------------------|-----------------|----------------|----------------------------|
| `InputChannel`      | Unreliable seq  | Client→Server  | Per-tick input             |
| `ControlChannel`    | Reliable ordered| Bidirectional  | Control request/ack/reject |
| `ReplicationChannel`| Lightyear managed| Server→Client | Entity state replication   |

### 4.2 Input Message

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InputMsg {
    pub tick: Tick,
    pub session_id: ClientId,
    pub target_id: StableId,     // must match server-side ControlLink
    pub actions: Vec<FlightAction>,
}
```

Server validates: `session_id` matches authenticated connection, `target_id` matches
server-authoritative `ControlLink` for that session's player entity. Mismatches are silently
dropped (no desync possible — server never applies unauthorized input).

### 4.3 Control Handoff Protocol

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ControlRequest {
    pub seq: u32,
    pub requestor_player_id: StableId,
    pub target_entity_id: StableId,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ControlResponse {
    Ack {
        seq: u32,
        new_controlled_id: StableId,
    },
    Reject {
        seq: u32,
        reason: ControlRejectReason,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ControlRejectReason {
    Unauthorized,
    TargetNotControllable,
    AlreadyControlled,
    TooFarAway,
}
```

**Client-side state machine:**
```
Idle ──(user request)──► PendingControl { seq, target }
  │                            │
  │     ◄──(ack seq match)─────┘  → apply new ControlLink, swap Predicted marker
  │     ◄──(reject seq match)──┘  → revert to previous, notify user
  │     ◄──(timeout)────────────┘  → revert, retry or notify
```

**Server-side flow:**
1. Receive `ControlRequest`.
2. Validate: does requestor own target? Is target controllable? Distance check?
3. If valid: update `ControlLink` on player entity, send `Ack`.
4. If invalid: send `Reject` with reason.
5. Input routing immediately uses new `ControlLink` after update.

---

## 5. Simulation / Prediction / Interpolation Pipeline

### 5.1 Bevy Schedule Plan

```
FixedUpdate (64 Hz default)
├── SystemSet::InputRouting        [server only]
│   ├── receive_client_inputs
│   └── route_input_to_action_queue   (validates session → ControlLink → target)
│
├── SystemSet::GameplayForce
│   ├── flight_computer_system        (ActionQueue → desired thrust/torque)
│   ├── engine_fuel_system            (clamp by fuel, consume fuel)
│   └── apply_forces_system           (write ExternalForce/ExternalTorque)
│
├── SystemSet::Physics                [Avian3D PhysicsSet — runs here]
│   └── (Avian integrated step)
│
├── SystemSet::PostPhysics
│   ├── clamp_velocities
│   └── clear_action_queues
│
├── SystemSet::ControlHandoff         [server only]
│   └── process_control_requests
│
├── SystemSet::Visibility             [server only]
│   ├── update_client_viewports
│   ├── spatial_index_rebuild         (incremental)
│   └── interest_cull_and_replicate
│
└── SystemSet::Persistence            [server only, every N ticks]
    └── snapshot_dirty_entities

Update (vsync / variable)
├── SystemSet::Interpolation          [client only]
│   └── interpolate_remote_entities   (Lightyear VisualInterpolation)
│
├── SystemSet::Camera                 [client only]
│   └── camera_follow_controlled      (reads Position, never writes sim state)
│
└── SystemSet::Render                 [client only]
    └── sync_render_transforms        (copy Position/Rotation → Transform)
```

### 5.2 Prediction (Client-Side)

Lightyear's prediction model:
1. Client sends `InputMsg` to server each fixed tick.
2. Client applies same input locally to predicted entity in `FixedUpdate`.
3. Server processes input, replicates authoritative state.
4. Client receives server state → Lightyear compares with predicted state.
5. If mismatch beyond threshold → rollback to server state, replay buffered inputs.

**Critical:** Only the entity with `Predicted` marker participates in local `FixedUpdate`
gameplay systems. All other entities skip gameplay force pipeline on client.

```rust
// Client FixedUpdate — predicted entity only
fn client_apply_local_input(
    mut q: Query<(&mut ActionQueue, &Predicted)>,
    input: Res<LocalInput>,
) {
    for (mut queue, _) in &mut q {
        queue.actions = input.actions.clone();
    }
}
```

### 5.3 Interpolation (Client-Side)

Remote entities use Lightyear's `VisualInterpolationPlugin`. The client receives periodic
server snapshots and smoothly interpolates between them. No local physics/gameplay systems
run for these entities.

```rust
// Interpolated entities: Position/Rotation are written ONLY by Lightyear interpolation.
// Client systems must NEVER write to these components on Interpolated entities.
```

### 5.4 Single-Writer Enforcement

| Entity Type | Who writes Position/Rotation | Who writes ExternalForce | Physics runs? |
|-------------|------------------------------|--------------------------|---------------|
| Predicted (client) | Avian physics (FixedUpdate) | Gameplay force pipeline | Yes |
| Interpolated (client) | Lightyear interpolation only | Nobody | No |
| Server entity | Avian physics (FixedUpdate) | Gameplay force pipeline | Yes |
| Camera | Never writes sim state | N/A | N/A |

---

## 6. Persistence / Hydration Pipeline

### 6.1 Graph Record Model

```rust
pub struct GraphEntityRecord {
    pub stable_id: Uuid,
    pub entity_type: String,            // "ship", "player", "asteroid", ...
    pub components: Vec<GraphComponentRecord>,
    pub relationships: Vec<GraphRelationship>,
}

pub struct GraphComponentRecord {
    pub component_type: String,         // fully qualified type name
    pub data: serde_json::Value,        // reflect + serde serialized
}

pub struct GraphRelationship {
    pub relation_type: String,          // "parent", "mount", "cargo", ...
    pub target_id: Uuid,
    pub metadata: serde_json::Value,
}
```

### 6.2 Snapshot Strategy

- **Dirty tracking:** Components with `Changed<T>` filter trigger persistence marking.
- **Batch writes:** Every N ticks (configurable, default 128 = ~2s at 64Hz), dirty entities
  are serialized to graph records and flushed to storage.
- **Storage backend:** Initially SQLite (single-file, WASM-compatible via wasm-sqlite).
  Designed for swap to PostgreSQL/SurrealDB via trait abstraction.

### 6.3 Hydration

1. Load `GraphEntityRecord`s from storage, sorted topologically by relationships.
2. Spawn entities in order: parents before children.
3. For each entity:
   a. Spawn with `StableId`.
   b. Deserialize and insert each `GraphComponentRecord` via reflection.
   c. Resolve `GraphRelationship` → Bevy parent/child hierarchy or `MountRelation`.
4. After all entities spawned: run a `validate_hierarchy` system to assert consistency.
5. Mark all entities clean (not dirty).

### 6.4 Account vs. Character Separation

```
Account (auth identity, not an ECS entity in game world)
  └── owns PlayerEntity (gameplay identity, ECS entity, persisted)
        ├── ControlLink { controlled_id }    ← persisted
        ├── Camera state (zoom, angle)       ← persisted on player entity
        ├── Selection state                  ← persisted on player entity
        └── controls → ShipEntity (via ControlLink)
```

Runtime/control/camera state lives on the `PlayerEntity` component bundle — never in
ad-hoc side tables or global resources.

---

## 7. Interest Management + Visibility Strategy

### 7.1 Three-Stage Pipeline (Server-Side)

```
Stage 1: Authorization
  - Can this client see this entity at all? (faction, ownership, public flag)
  - Reject: entity is completely hidden (e.g., cloaked enemy)

Stage 2: Delivery / Interest Narrowing
  - Is this entity within the client's viewport + edge buffer?
  - Spatial candidate selection via grid index (avoid O(C*E) scan)
  - Scanner floor: entities within 300m always pass

Stage 3: Payload Redaction
  - Strip sensitive components before replication (e.g., enemy fuel levels)
  - Send only public-facing components to non-owner clients
```

### 7.2 Spatial Index

```rust
/// Uniform grid for spatial candidate selection.
/// Cell size chosen to match typical viewport half-extent (~500m).
pub struct SpatialGrid {
    cell_size: f32,
    cells: HashMap<IVec2, Vec<Entity>>,
}

impl SpatialGrid {
    /// Rebuild incrementally: only update entities whose cell changed.
    pub fn update(&mut self, entity: Entity, old_cell: IVec2, new_cell: IVec2);

    /// Query all entities in cells overlapping the given AABB.
    pub fn query_aabb(&self, min: Vec2, max: Vec2) -> impl Iterator<Item = Entity>;
}
```

**Cost model:**
- Rebuild: O(moved entities) per tick, not O(all entities).
- Query per client: O(cells in viewport) — typically 4-16 cells.
- Total per tick: O(clients × cells_per_viewport) — with 100 clients × 16 cells = 1600 lookups.
- Much better than naive O(100 × 5000) = 500,000 comparisons.

### 7.3 Edge Buffer / Hysteresis

Entities entering the viewport are added when they cross `viewport + buffer` inward.
Entities are removed only when they cross `viewport + buffer` outward.
This prevents oscillation at viewport boundaries.

### 7.4 Defaults

| Parameter          | Default | Notes                               |
|--------------------|---------|-------------------------------------|
| Scanner floor      | 300m    | Always visible regardless of camera |
| Viewport buffer    | 100m    | Edge hysteresis                     |
| Grid cell size     | 500m    | Tuned to viewport                   |
| Cull check rate    | Every 4 ticks | ~16Hz, sufficient for interest |
| Max replicated/client | 256  | Hard cap to protect bandwidth       |

---

## 8. Test Plan + Instrumentation

### 8.1 Automated Test Scenarios

| # | Scenario | Setup | Expected Outcome |
|---|----------|-------|-------------------|
| 1 | Two-client connect | Launch server + 2 clients | Both see own entity predicted, other interpolated |
| 2 | Independent movement | Both clients thrust in opposite directions | Each sees instant local response; remote moves smoothly |
| 3 | Position convergence | Client A moves, stops | After interpolation delay (≤200ms), Client B shows A at server-authoritative position ±0.01 |
| 4 | Control swap | Client A requests control of drone entity | A's camera follows drone; A predicts drone; A's ship becomes interpolated for A |
| 5 | Control reject | Client A requests control of B's ship | Server sends Reject(Unauthorized); A remains controlling own ship |
| 6 | Visibility culling | Move Client A far from B | B's entity disappears from A's replication set |
| 7 | Visibility restore | Move Client A back near B | B's entity reappears; no duplication |
| 8 | Reconnect | Kill Client A, reconnect | A rebinds to same PlayerEntity via AccountId; no duplicate entities |
| 9 | No remote writes | Add system audit | Assert: no system in client writes Position/Rotation on Interpolated entities |
| 10 | Persistence round-trip | Save world, restart server, reconnect | All entities restored with correct hierarchies and control links |

### 8.2 Instrumentation

```rust
// Metrics to track (via bevy_diagnostic or custom):
- prediction_correction_count: Counter    // rollbacks per second
- prediction_error_magnitude: Histogram   // position delta at correction
- interpolation_buffer_depth: Gauge       // snapshot buffer size
- visibility_set_size: Histogram          // entities per client
- spatial_index_update_us: Histogram      // microseconds per rebuild
- persistence_flush_us: Histogram         // microseconds per snapshot batch
- control_handoff_latency_ms: Histogram   // request to ack time
- input_rtt_ms: Histogram                 // input round-trip estimate
```

---

## 9. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Avian3D determinism across platforms | Prediction divergence, excessive rollbacks | Use fixed timestep; monitor correction rate; tune threshold |
| Lightyear rollback perf with many predicted entities | Frame spikes | Predict only 1 entity (controlled); all others interpolated |
| Spatial grid hotspots (many entities in one cell) | Slow queries | Adaptive cell size or quadtree fallback; monitor cell population |
| WASM transport limitations | No UDP in browser | Lightyear supports WebTransport; fall back to WebSocket with jitter buffer |
| Persistence write storms | DB latency spikes | Dirty tracking + batched writes; async I/O channel |
| Control swap during rollback | Mismatch between predicted and server state | Freeze prediction during pending control swap; resume on ack |
| Entity ID collision | Corruption | UUID v7 (time-sortable, practically collision-free) |
| Interpolation visual glitch on spawn | Pop-in | Fade-in on first snapshot; don't render until 2+ snapshots buffered |

---

## 10. Anti-Pattern "Do Not Do This" List

### Transform Divergence (Primary Desync Source)

1. **DO NOT** let clients write `Position`/`Rotation` on any entity except via the
   prediction pipeline for the single controlled entity.

2. **DO NOT** run Avian physics on interpolated entities client-side.
   They must receive state only from Lightyear interpolation.

3. **DO NOT** use Bevy `Transform` as the source of truth for simulation.
   Avian uses `Position`/`Rotation`. `Transform` is render-only, derived in a sync system.

4. **DO NOT** send `Transform` over the network. Replicate `Position`, `Rotation`,
   `LinearVelocity`, `AngularVelocity` only.

5. **DO NOT** use `Entity` IDs across the network. Always use `StableId(Uuid)`.

6. **DO NOT** represent "no control target" as `None`/null. Free-roam is self-control:
   `ControlLink { controlled_id: own_stable_id }`.

7. **DO NOT** apply input without validating `session_id → ControlLink → target_id` chain
   on the server. This is the authorization boundary.

8. **DO NOT** predict remote entities. Even "just a little extrapolation" causes visible
   divergence when server state arrives.

9. **DO NOT** interpolate the predicted entity. It must run through the full prediction
   pipeline or you get input lag.

10. **DO NOT** write camera position back to simulation components. Camera reads sim state;
    it never writes it.

11. **DO NOT** persist raw `Entity` IDs or `ClientId`s. They are runtime-only.
    Persist `StableId`, `AccountId`, `PlayerId`.

12. **DO NOT** run visibility/interest checks every tick. Every 4 ticks is fine. But DO
    update spatial index positions every tick (cheap incremental update).

13. **DO NOT** replicate fuel/engine state to non-owner clients. This is a redaction target.

14. **DO NOT** allow `ControlRequest` to bypass the server. Client must wait for `Ack`
    before swapping local prediction target.

15. **DO NOT** use `.insert()` to overwrite replicated components on client. Lightyear
    manages these; manual writes cause rollback storms.

---

## 11. Scale Plan: 100s Clients / 1000s Entities

### Budgets (per server tick at 64Hz = 15.6ms budget)

| System | Budget | Strategy |
|--------|--------|----------|
| Input routing | 0.5ms | HashMap lookup by ClientId |
| Gameplay force pipeline | 2ms | Parallel query over entities with ActionQueue |
| Avian physics | 4ms | Spatial broad-phase built in; tune collider complexity |
| Spatial index update | 0.5ms | Incremental cell updates |
| Visibility cull (all clients) | 2ms | Grid query per client; run every 4 ticks |
| Replication serialization | 3ms | Delta compression (Lightyear built-in) |
| Persistence snapshot | 1ms | Amortized; async flush |
| **Total** | **~13ms** | **2.6ms headroom** |

### Scaling Levers

1. **Shard by region:** Spatial partitioning into server instances at ~2000 entity threshold.
2. **Reduce cull frequency:** Drop to every 8 ticks for distant clients.
3. **LOD replication:** Send only Position/Rotation for distant entities (skip velocity).
4. **Entity budget per client:** Hard cap at 256 replicated entities; prioritize by distance.
5. **Async persistence:** Move snapshot writes to dedicated thread/task.
6. **Physics islands:** Avian's built-in sleeping + islands reduce active body count.

---

## 12. Demo Runbook (2-Client Vertical Slice)

### Prerequisites

```bash
cargo install cargo-watch  # optional, for dev reload
```

### Build & Run

```bash
# Terminal 1: Server
cargo run --bin starmux_server

# Terminal 2: Client A
cargo run --bin starmux_client -- --name "Alice"

# Terminal 3: Client B
cargo run --bin starmux_client -- --name "Bob"
```

### Success Criteria Checklist

- [ ] Both clients connect and see their own ship (colored differently).
- [ ] WASD/arrow thrust: own ship responds instantly (prediction).
- [ ] Other player's ship moves smoothly with slight delay (interpolation).
- [ ] Open debug overlay (`F3`): shows prediction correction count, interp buffer depth.
- [ ] Press `Tab` to request control of a nearby drone entity.
- [ ] Camera snaps to drone; drone becomes predicted; original ship becomes interpolated.
- [ ] Press `Tab` again to release drone (self-control restored).
- [ ] Move far away: other player's ship disappears from replication set (visibility cull).
- [ ] Move back: other player's ship reappears without duplication.
- [ ] Kill client, restart: reconnects to same player entity, no duplicates.
- [ ] Server `Ctrl+C`: persistence snapshot saved. Restart server: world restored.

### Debug Commands (Server Console)

```
/status               — show connected clients + controlled entities
/visibility <client>  — dump visibility set for client
/metrics              — dump tick timing breakdown
/save                 — force persistence snapshot
/spawn drone <x> <y>  — spawn controllable drone at position
```

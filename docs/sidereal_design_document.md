# Sidereal 2: Unified Build Specification (Authoritative Single-File Blueprint)

Status: Draft build spec for rebuilding Sidereal from scratch with the refactored network architecture.
Date: 2026-02-18
Audience: engineering agents and maintainers implementing the full project without any other reference files.

## 1. Product Vision and Scope

Sidereal is a multiplayer top-down 3D space RPG with server-authoritative simulation, persistent world state, and capability-driven ECS gameplay.

Core game loop:

- Authenticate and enter a persistent universe.
- Pilot controllable ships via module-based systems (engines, fuel, flight computer, hardpoints).
- Observe and interact with entities under strict server-authorized visibility rules.
- Persist world evolution through authoritative shard simulation and durability pipelines.

Near-term execution focus (current):

- Deliver a deterministic vertical slice from register/login UI to in-world starter ship control.
- Keep transport and persistence contracts stable while this slice is hardened (`gateway -> replication bootstrap`, `shard -> replication delta ingress`, `client <- replication state`).
- Preserve native/WASM single-client parity gates even while native remains the primary runtime target.

Primary near-term target:

- stable realtime multiplayer with robust client prediction/reconciliation,
- scalable architecture for multi-shard and future MMO operation,
- content extensibility foundations (missions, scripting, procedural content, minimap/intel streams, asset streaming).

Technology baseline (target):

- Rust 2024 edition
- Bevy 0.18
- Avian3D compatible with Bevy 0.18
- Lightyear 0.26.x target for client-facing replication/prediction layer
- PostgreSQL with Apache AGE extension
- `serde` + `bincode` support for internal protocol encoding paths

## 2. Hard Invariants (Non-Negotiable)

1. Authority direction is one-way. Current simplified mode: `client input -> replication simulation -> persistence graph`. Future multi-shard mode: `client input -> shard simulation -> replication/distribution -> persistence graph`.
2. Clients send intent/input only; clients never authoritatively set transforms.
3. Cross-boundary identity is UUID/entity_id only; Bevy `Entity` is runtime-local and never persisted/transmitted.
4. Runtime shard memory is authoritative live state; DB is durability + startup hydration only.
5. Visibility and field-level data permissions are server-enforced before serialization.
6. Domain labels (`Ship`, `Asteroid`, etc.) classify entities; behavior is capability/component driven.
7. Registration creates starter world entities exactly once; login is auth-only.

## 3. Target Runtime Architecture (Refactor-Baked)

### 3.1 Service Topology

Control plane:

- `sidereal-orchestrator`: shard leases, split/merge policy, failover coordination.
- `sidereal-bg-sim`: low-fidelity economy/NPC progression and event generation.
- PostgreSQL + AGE: durable graph and auth/session persistence.

Data plane:

- `sidereal-replication`: current authoritative ECS + Avian simulation host, client-facing transport, visibility filtering, fan-out, and durability staging.
- `sidereal-shard` (N instances): reserved for future multi-shard split/ownership and lease routing.
- `sidereal-shard` and `sidereal-replication` expose `bevy_remote` protocol endpoints for authenticated runtime world inspection.

Entry/auth plane:

- `sidereal-gateway`: auth API, token lifecycle, account/session domain, registration flow.

Client plane:

- `sidereal-client` (single workspace member): realtime client with prediction/rollback/interpolation. Builds as a native binary and as a WASM `cdylib` library from the same source. See section 3.3 for architecture details.
- `sidereal-client` enables `bevy_remote` protocol for local/remote inspection tooling parity with server runtimes.
- Both the native binary target and the WASM library target must stay CI-green. Gameplay and simulation code is shared; only the transport adapter and platform init code differ.
- Client UI follows the design system documented in `docs/ui_design_guide.md` (space-themed aesthetic, consistent color palette, component patterns). Error handling uses persistent modal dialogs (`dialog_ui::DialogQueue`) for failures requiring user acknowledgment.

### 3.2 Networking Strategy

Primary direction:

- Native client uses low-latency transport and a replication framework layer (Lightyear-targeted design) for prediction/interpolation/rollback primitives.
- Replication transport layer is multi-protocol by design: native-first transport plus browser-capable transport path(s) for WASM clients.
- Browser/WASM direction prioritizes WebRTC data channels; WebSocket is allowed only as compatibility/fallback path due prior overhead observations.
- Shard<->Replication and control-plane contracts remain explicit `sidereal-net` messages (do not leak internal ECS types across services).

Migration rule:

- keep protocol boundaries explicit even when using framework plugins;
- avoid big-bang replacement of every channel at once.
- keep transport adapters thin so simulation/gameplay/prediction code is shared across native and WASM clients.
- current implementation baseline: replication and native client run Lightyear raw UDP link/session entities using shared `sidereal-net` protocol registration (`register_lightyear_protocol`), and replication runs the active Avian simulation loop. Gateway bootstrap control handoff remains a dedicated UDP control path.
- note on codec compatibility: Lightyear message payloads are bincode-encoded by default. Current shard/replication state messages carry `world_json` bytes (JSON-serialized `WorldStateDelta`) inside Lightyear envelopes because `serde_json::Value` in the world-delta schema is not directly bincode-deserializable (`AnyNotSupported`) in this phase.

### 3.3 WebRTC Transport Architecture (WASM/Browser Client)

#### Why WebRTC Data Channels

Browser security policy prohibits raw UDP from WASM contexts. WebRTC data channels are the correct equivalent:

- unordered/unreliable data channels behave like UDP (no head-of-line blocking on game state streams),
- ordered/reliable data channels behave like TCP (for session control and auth messages),
- natively supported in all modern browsers without HTTP/3 infrastructure,
- server-side implementation works entirely in Rust without a TLS reverse proxy in front of game data.

WebSocket is available as a fallback transport but imposes head-of-line blocking across all game state updates and is not the primary path. Agents must not default to WebSocket for new WASM transport work.

#### Connection Lifecycle (Signaling Flow)

WebRTC requires an out-of-band signaling exchange before any data channel is established. The signaling path is a short-lived WebSocket connection to the replication server, not a new service:

1. WASM client authenticates via gateway (`POST /auth/login`) and receives a JWT.
2. Client opens a WebSocket connection to the replication server signaling endpoint (`/rtc/signal`), presenting the JWT for auth-gating.
3. Client sends a JSON SDP offer over the signaling WebSocket.
4. Replication server responds with a JSON SDP answer.
5. Both sides exchange ICE candidates over the signaling WebSocket until ICE negotiation completes.
6. The WebRTC peer connection and named data channels are established.
7. The signaling WebSocket is closed; all subsequent game traffic flows exclusively over the data channels.

The signaling WebSocket is a bootstrapping channel only. It carries no game data. Closing it after the peer connection is live is expected behavior, not an error.

#### Data Channel Model

Two named data channels per client session:

| Channel name | Ordered | Reliable | Purpose                                                     |
|--------------|---------|----------|-------------------------------------------------------------|
| `ctrl`       | yes     | yes      | Session auth token delivery, channel negotiation, admin     |
| `game`       | no      | no       | Tick-indexed input snapshots, game state replication frames |

The `game` channel is intentionally unordered and unreliable. Loss tolerance is handled at the protocol level via tick-indexed input redundancy and the late/early window enforcement in the shard input pipeline. The server never waits on a dropped `game` channel frame.

#### ICE, STUN, and TURN Requirements

- A STUN server is required for ICE candidate gathering. In development, public STUN (e.g., `stun:stun.l.google.com:19302`) is acceptable.
- A TURN relay server is required in production to handle clients behind symmetric NAT. Without TURN, a meaningful fraction of browser clients cannot establish a direct peer connection.
- The replication server delivers its ICE server configuration (STUN URLs, TURN URLs, TURN credentials) to the client as part of the signaling handshake response, before the SDP offer is sent.
- TURN credentials should be time-limited and generated per-session (HMAC-based TURN credential pattern).

#### Server-Side Implementation

The replication server WebRTC listener:

- Exposes a WebSocket signaling endpoint (`/rtc/signal`), auth-gated by session JWT.
- Uses a Rust WebRTC library (e.g., `webrtc-rs` crate or Lightyear's WebRTC transport adapter, whichever is used by the chosen Lightyear version) to negotiate and accept incoming peer connections.
- Each accepted data channel pair is mapped to a `player_entity_id` and session, then routed identically to native UDP sessions in the replication input/state pipeline.
- No game logic path differs based on client transport type. Transport adapters are isolated to the connection layer; the shard and visibility systems are transport-agnostic.

Reference shape for the signaling message contract:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalMsg {
    IceServers { stun_urls: Vec<String>, turn_urls: Vec<String>, turn_credential: Option<TurnCredential> },
    Offer { sdp: String },
    Answer { sdp: String },
    IceCandidate { candidate: String, sdp_mid: Option<String>, sdp_m_line_index: Option<u16> },
    Error { reason: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TurnCredential {
    pub username: String,
    pub password: String,
}
```

#### WASM Client Integration

In the WASM build, the transport adapter:

- Uses browser WebRTC APIs via `web-sys` and `js-sys` bindings.
- Implements the same `ClientTransport` trait interface as the native UDP adapter so that gameplay, prediction, and reconciliation code remain completely shared.
- Is selected at runtime by `cfg(target_arch = "wasm32")`, not by a cargo feature flag. The Rust compiler sets `target_arch` automatically when building for the WASM target; no manual feature must be enabled.
- Lightyear's WASM-compatible WebRTC transport plugin is used if available in the targeted Lightyear version; otherwise a thin adapter wrapping `web-sys` `RtcPeerConnection` is acceptable at the transport boundary layer only.

#### Client Crate Architecture: Single Crate, Dual Targets

A Cargo feature flag (`wasm`) for the client is not the right model. `cfg(target_arch = "wasm32")` is set automatically by the Rust toolchain when targeting WASM and cannot be accidentally miscombined with native builds. Feature flags can.

The client is one workspace member (`bins/sidereal-client`) with both targets declared in its `Cargo.toml`:

```toml
[[bin]]
name = "sidereal-client"
path = "src/main.rs"           # native entry point

[lib]
name = "sidereal_client_web"
crate-type = ["cdylib", "rlib"]
path = "src/lib.rs"            # WASM entry point (wasm-bindgen init)
```

Platform-specific code (transport adapter selection, `wasm-bindgen` init, browser canvas setup) is gated by `#[cfg(target_arch = "wasm32")]`. All gameplay, prediction, reconciliation, UI, and ECS systems live in shared modules with no target conditional.

Native is the current primary delivery target, but WASM is still a parity-gated target. Any client-side gameplay/protocol/runtime behavior change is incomplete unless the WASM path is updated (or confirmed unaffected) in the same change.

Build commands:

```bash
# native
cargo build -p sidereal-client

# WASM (requires wasm-pack or cargo build with wasm32 target)
wasm-pack build bins/sidereal-client --target web --out-dir ../../dist/web
# or
cargo build -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
```

This replaces the previously listed `bins/sidereal-client-web` binary. There is no separate web binary; the WASM artifact comes from the library target of `bins/sidereal-client`.

## 4. Tick, Time, and Input Model

### 4.1 Simulation Timing

- Shard authoritative simulation tick: 30 Hz fixed timestep.
- Client render target: 60 Hz.
- Input pipeline is tick-indexed (not wall-clock approximated).

### 4.2 Authoritative Tick Metadata

All authoritative envelopes include:

- protocol version,
- shard/source,
- lease epoch,
- sequence number,
- authoritative tick.

Reference shape:

```rust
struct NetEnvelope<T> {
    protocol_version: u16,
    channel: ChannelClass,
    source_shard_id: i32,
    lease_epoch: u64,
    seq: u64,
    tick: u64,
    payload: T,
}
```

### 4.3 Input Contract

Use full per-tick input snapshots for deterministic replay and packet-loss self-healing.

Reference intent model:

```rust
struct PlayerInput {
    tick: u64,
    thrust_forward: bool,
    thrust_reverse: bool,
    yaw_left: bool,
    yaw_right: bool,
    actions: Vec<OneTimeAction>,
}
```

Server apply rule:

- input for tick N is consumed while simulating tick N,
- late window and early window enforcement are explicit,
- acked input tick is returned for pruning/replay.

## 5. Prediction, Reconciliation, Interpolation, Extrapolation

### 5.1 Controlled Entity

- Client predicts only controlled entity.
- Server state remains authoritative.
- On state receipt:
  1. compare against historical prediction at authoritative tick,
  2. rollback to authoritative state on divergence,
  3. replay unacked inputs forward,
  4. apply correction smoothing policy (no blind hard snap for normal error ranges).

Correction policy requirements:

- hard snap only for large divergence/teleport threshold,
- short-window blend for small/moderate divergence,
- velocity-adaptive thresholds and blend rates are allowed and expected.

### 5.2 Remote Entities

- no prediction for non-controlled entities,
- use snapshot buffer interpolation, not lerp-to-latest-target.

Required structure:

```rust
struct SnapshotBuffer {
    snapshots: VecDeque<(u64, Vec3, Quat)>, // (server_time_or_tick, pos, rot)
}
```

Render rule:

- render at `now - interpolation_delay` (for example ~100ms),
- find two bracketing snapshots and interpolate by exact ratio,
- if newest snapshot is slightly behind render time, allow bounded extrapolation cap.

### 5.3 Physics Parity Rule

- Shared deterministic movement/control logic lives in shared crates. Current baseline uses `sidereal-game` systems for action/fuel/thrust rules on both client and server, while `sidereal-sim-core` hosts pure deterministic helpers.
- Client/server step semantics must match (turn/thrust ordering, damping, timestep assumptions).
- Full client Avian prediction for controlled entity is a phased upgrade after baseline parity and stability metrics are acceptable.

## 6. Visibility and Data Permissions (Security-Critical)

Three-scope model:

1. World truth scope: full authoritative state (server only).
2. Authorization scope: what this player may know (ownership, attachments, scan grants).
3. Delivery scope: what this active client session receives now (focus radius/hysteresis/stream tier).

Enforcement order (mandatory):

1. Build candidate entity set (spatial query + owned/attached inclusion).
2. Resolve authorization scope per entity (ownership, attachment inheritance, active scan grants).
3. Resolve delivery scope per active stream/session.
4. Apply field-level redaction mask before serialization.
5. Emit additions/updates/removals for the session stream.

Rules:

- Owned entities and owned attachments: full detail for control UI.
- Non-owned authorized entities: redacted by field policy (physical/render-safe fields by default).
- Unauthorized entities: never serialized; explicit removal if previously visible.
- Authorization and delivery are not equivalent: a player can be authorized for data that the active stream does not currently deliver.
- Current default delivery behavior: focus stream does not automatically include all offscreen owned entities unless explicitly subscribed via additional stream policy.

Sensitive-data rule:

- cargo internals, hidden loadouts, subsystem internals, private transfer details remain omitted unless explicit gameplay grants allow exposure.
- transfer payload visibility follows grant/ownership policy: unrelated observers must not receive private transfer detail.

Multi-entity ownership rule:

- authorization aggregates across all owned entities, not only currently controlled ship.

Server-only invariant:

- client cannot self-upgrade visibility by local inference.

## 7. Scan Intel and Multi-Stream Model (Ground-Level Support)

### 7.1 Scan Intel Grants

Server-managed temporary grants keyed by:

- observer player,
- target entity,
- field scope,
- source,
- grant/expiry times.
- unique grant id.

Example field scopes:

- `physical_public`
- `combat_profile`
- `cargo_summary`
- `cargo_manifest`
- `systems_detail`

Grant lifecycle requirements:

- grant source is explicit (`active_scan`, `dock_access`, `boarding`, `allied_share`, etc.).
- grants are time-bounded unless explicitly revoked earlier.
- on expiry/revocation, visibility reverts immediately to baseline redacted policy.
- no grant may bypass ownership/authorization identity checks.

### 7.2 Stream Tiers

Support from ground level:

- `focus_stream`: high rate, local tactical detail.
- `strategic_stream`: lower-rate minimap contacts/coarse kinematics.
- `intel_stream`: event-driven grant results and revocations.

Stream security constraints:

- all streams are server-authoritative and permission-filtered.
- clients can subscribe only to allowed stream types; subscription does not bypass redaction.
- unauthorized fields are never placed on any stream payload (including minimap/strategic streams).

### 7.3 Spatial Indexing

Visibility query engine must support sublinear candidate lookup:

- phase-1 acceptable: uniform spatial hash grid,
- future: adaptive grid/quadtree for hotspot density.

Baseline spatial query behavior:

- include nearby cells for focus radius queries.
- include nearby cells for each owned scanner radius and union candidates.
- include owned/attachment descendants independently of spatial culling.

Expected complexity target:

- avoid full-world `O(total_entities)` scan per client per tick.
- practical target is `O(entities_in_candidate_cells + owned_descendants)`.

Metrics required:

- candidate count per client frame,
- included entity count,
- query time budget.

Optional later stream extensions:

- low-frequency fleet/strategic panels may be added as explicit subscriptions.
- extensions must still route through the same authorization + delivery + redaction pipeline.

## 8. ECS and Gameplay Composition Model

### 8.1 Philosophy

Sidereal uses Bevy ECS for both realtime simulation and authoritative state flow.

Core principles:

- Data-oriented composition over inheritance.
- Stable entity identity (`EntityGuid` + persistent IDs in storage).
- Authority is explicit (`ShardAssignment`, lease epoch in protocol).
- Ownership and control are explicit and separate concerns.
- Hot simulation state is in-memory ECS; durability is snapshot + event persistence.
- Behavior is capability-driven. Labels like `Ship`, `Missile`, `Asteroid` are domain tags, not simulation branches.

### 8.2 Current Implemented Components

Implemented today in `crates/sidereal-game/src/lib.rs`:

- `ShipTag`: optional domain tag for ship-class entities.
- `ModuleTag`: marks ship module entities.
- `EntityGuid(Uuid)`: stable global identifier.
- `DisplayName(String)`: canonical display name mirrored to graph `name` property.
- `PositionM(Vec3)`: world position in meters.
- `VelocityMps(Vec3)`: linear velocity in meters/second.
- `MassKg(f32)`: mass in kilograms.
- `SizeM { length, width, height }`: physical dimensions in meters.
- `CollisionAabbM { half_extents }`: collision shape metadata.
- `ShardAssignment(i32)`: authoritative shard assignment.
- `Hardpoint { hardpoint_id, offset_m }`: mount point metadata.
- `MountedOn { parent_entity, parent_entity_id, hardpoint_id }`: module-to-parent relation (parent is any host entity with hardpoints; UUID is the cross-boundary identity).
- `Engine { thrust_n, burn_rate_kg_s, thrust_dir }`: propulsion module.
- `FuelTank { fuel_kg }`: remaining fuel.
- `FlightComputer { profile, throttle }`: fly-by-wire/autopilot controller.
- `OwnerKind`, `OwnerId`: ownership identity for combat/economy attribution.
- `InstigatorEntityId`: explicit combat initiator tracing (who fired/caused action).
- `HealthPool`: durability component for interceptable/damageable entities.
- `BaseMassKg`, `CargoMassKg`, `ModuleMassKg`, `TotalMassKg`, `MassDirty`: cached mass pipeline.
- `Warhead`, `GuidanceComputer`, `DamageProfile`, `LifetimeTicks`: modular missile/projectile foundations.

Also wired today:

- Avian physics components: `RigidBody`, `Collider`, `LinearVelocity`, `Position`, `Rotation`.

Physics/render sync rule:

- Avian `Position`/`Rotation` is simulation-authoritative.
- Bevy `Transform`/`GlobalTransform` is mirrored from Avian each fixed tick for rendering/camera/plugin compatibility.
- `PositionM`/`VelocityMps` are also updated from authoritative Avian state for persistence/network consistency.

### 8.3 Core Planned Cross-Domain Components

Identity and authority:

- `AuthorityShardId`
- `LeaseEpoch`
- `ReplicationVersion`

Ownership/control/security:

- `OwnerKind` (`Player | Faction | World | Unowned`)
- `OwnerId`
- `ControllerId`
- `AccessRights`
- `FactionAffiliation`

Spatial and visibility:

- `RegionId`
- `InterestRadius`
- `VisibilityMask`
- `SensorProfile`
- `Signature` (thermal/radar/emission)

Economy/inventory:

- `CargoSlots`
- `CargoMassKg`
- `InventoryLedgerRef`
- `MarketListingRef`
- `CurrencyWallet`

Combat:

- `HealthPool`
- `ArmorProfile`
- `ShieldProfile`
- `WeaponMount`
- `ProjectileProfile`
- `LifetimeTicks`
- `InstigatorEntityId`
- `Warhead`
- `GuidanceComputer`

World simulation:

- `OrbitalBody`
- `GravitySource`
- `ResourceField` (asteroid yields, gas density)
- `BackgroundSimProxy`

Power and utility simulation (planned):

- `PowerProducer` (engines, reactors, solar arrays).
- `PowerConsumer` (shields, computers, tractor beams, scanners, weapons).
- `BatteryBank` (stored energy buffer).
- `PowerBus` (distribution limits and priorities).
- `FuelConsumer` (engines, missiles).
- `DockingPort`, `DockedTo`
- `Scanner`, `RemoteBeaconEmitter`

### 8.4 Relationship Model

Use plain ECS references and composition:

- Parent host entity stores mass/physics/authority.
- Module entities reference parent via `MountedOn`.
- Hardpoint IDs define deterministic attachment points.
- Hardpoints are the universal attachment mechanism for engines, guns, computers, shields, tractor beams, cargo modules, and missile subsystems.
- Replication sends flattened deltas; storage rebuilds hierarchy by IDs.
- Cross-boundary relationship identity uses UUID fields (`parent_entity_id`), never Bevy `Entity` IDs.

Modularity rule:

- Behavior should live in module components.
- Example: missile explosion logic belongs to `Warhead`; guidance logic belongs to `GuidanceComputer`; thrust/fuel logic belongs to `Engine` + `FuelTank`.
- No gameplay system should branch by "is ship" when capability components are sufficient.

### 8.5 Archetype Examples

#### 8.5.1 Asteroid

- `EntityGuid`
- `PositionM`, `VelocityMps`
- `MassKg`, `SizeM`
- `CollisionAabbM` or sphere collider
- `HealthPool`
- `ResourceField` (ore type, richness)
- `ShardAssignment`, `RegionId`

Behavior:

- Mineable resource depletion via events.
- Optional rigid body for collisions.

#### 8.5.2 Bullet (kinetic)

- `EntityGuid`
- `PositionM`, `VelocityMps`
- `MassKg`
- `OwnerKind`, `OwnerId`
- `InstigatorEntityId`
- `ProjectileProfile` (damage, caliber)
- `HealthPool` (optional, if interceptable)
- `LifetimeTicks`
- `ShardAssignment`

Behavior:

- Short-lived, high-rate entities.
- Usually no inventory persistence.

#### 8.5.3 Missile

- `EntityGuid`
- `PositionM`, `VelocityMps`
- `MassKg`
- `OwnerKind`, `OwnerId`
- `InstigatorEntityId`
- `Engine`, `FuelTank`
- `GuidanceComputer` (optional: omit for dumb-fire)
- `Warhead`
- `DamageProfile`
- `HealthPool`
- `LifetimeTicks`
- `ShardAssignment`

Behavior:

- Server-authoritative homing/thrust.
- Detonation event on proximity/hit.

#### 8.5.4 Cargo Container

- `EntityGuid`
- `PositionM`, `VelocityMps`
- `MassKg`, `SizeM`
- `CollisionAabbM`
- `CargoSlots` or `InventoryLedgerRef`
- `OwnerKind`, `OwnerId`
- `ShardAssignment`

Behavior:

- Can be dropped, tractored, looted, transferred.

#### 8.5.5 Space Station

- `EntityGuid`
- `PositionM`
- `MassKg`, `SizeM`
- `CollisionAabbM`
- `DockingPorts`
- `MarketListingRef`
- `OwnerKind`, `OwnerId`
- `FactionAffiliation`
- `ShardAssignment`, `RegionId`

Behavior:

- Mostly static/kinematic body.
- Strong economy and mission hooks.

#### 8.5.6 Planet

- `EntityGuid`
- `PositionM`
- `SizeM` (radius representation)
- `GravitySource`
- `OrbitalBody`
- `AtmosphereProfile` (optional)
- `RegionId`

Behavior:

- Usually not dynamic rigid body.
- Influences nearby entity trajectory and sensors.

#### 8.5.7 Star

- `EntityGuid`
- `PositionM`
- `GravitySource`
- `RadiationProfile`
- `OrbitalBody` (for system model)

Behavior:

- Dominant gravity and environmental hazard source.

#### 8.5.8 Starter Craft Example (Current Seed)

The seeded prototype ship (`Prospector-14`) composes:

- Hull entity: ship tags, physics, mass/size, shard assignment.
- Module entities:
  - one `FlightComputer`
  - two `Engine + FuelTank`
- Hardpoints:
  - `computer_core`
  - `engine_left_aft`
  - `engine_right_aft`

This is the baseline pattern for all future player/NPC craft.

### 8.6 Event and Persistence Mapping

Guideline:

- Persist durable domain events (ownership, inventory, economy, destruction).
- Snapshot hot kinematics periodically.
- Rebuild runtime ECS world from snapshot + event replay.

Example durable events:

- `EntitySpawned`
- `ModuleAttached`
- `FuelConsumed`
- `ProjectileHit`
- `CargoTransferred`
- `EntityDestroyed`

### 8.7 Coding Conventions for Components

- Unit suffixes required (`M`, `Mps`, `Kg`, `N`, `Ticks`).
- Components should be small and single-purpose.
- Keep protocol DTOs in `sidereal-net`, not in gameplay component modules.
- Keep persistence row models in `sidereal-persistence`, not in gameplay modules.

### 8.8 Dynamic vs Generated Stats (Mass Best Practice)

For values like dynamic total mass, use cached derived components, not live recompute on every use.

Recommended pattern:

1. Store source components (`BaseMassKg`, `CargoMassKg`, `ModuleMassKg` on modules).
2. Mark parent entity `MassDirty` when cargo/modules change.
3. Recompute once in a system (`TotalMassKg`) and clear `MassDirty`.
4. Physics systems read `TotalMassKg` only.

Current v3 runtime behavior:
- Replication hydration rebuilds persisted parent/child hierarchy links into Bevy transform hierarchy using persisted `parent_entity_id`.
- Hardpoints are hydrated as normal entities and linked into the hierarchy, so child transforms inherit parent transforms and local offsets.
- `recompute_total_mass` derives `CargoMassKg`, `ModuleMassKg`, and `TotalMassKg` from inventories + mounted module trees and synchronizes Avian mass at runtime.
- Runtime hydration applies all registered generated component envelopes via reflection (`AppTypeRegistry` + `TypedReflectDeserializer` + `ReflectCommandExt::insert_reflect`) so newly registered persistable components hydrate without per-component manual insertion code.
- Runtime persistence emission refreshes component payloads from reflected ECS state (`TypedReflectSerializer` over registered generated component kinds), so newly registered persistable components are included in outgoing/pending persistence payloads without per-component manual serialization wiring.

Why:

- avoids repeated hot-path aggregation
- deterministic and replication-friendly
- scales better with many modules/cargo items

### 8.9 Avian Sync Contract

- Avian `Position`/`Rotation` is simulation-authoritative.
- Mirror into Bevy `Transform`/`GlobalTransform` each fixed tick.
- Mirror to network/persistence-facing kinematic components consistently.
- Avian runtime-only transient components (contacts/manifolds, solver caches, sleeping-island internals, broadphase internals, other non-durable physics runtime artifacts) are not persisted.
- Durable gameplay state required after restart must be represented in persistable ECS components outside Avian runtime internals.

### 8.10 Capability Rules (Must Hold)

Any entity with:

- `HealthPool` can be damaged and destroyed.
- `Engine` + `FuelTank` can accelerate and can run out of fuel.
- Power components can produce, consume, and store energy.
- `ShieldProfile` can trade power for shield mitigation.
- `FlightComputer` or AI computer can accept intent actions.
- Scanner/beacon components can extend fog-of-war visibility.
- Hardpoints can mount detachable modules, including cargo containers.

### 8.11 Action Routing System

The action routing system provides a modular, capability-driven approach to entity input handling and behavior execution.

#### Architecture Flow

```
Player Input (Keys/Mouse/Gamepad)
    ↓
Bindings → EntityAction enum (ThrustForward, FireWeapon, etc.)
    ↓
NetworkMessage/LocalQueue → ActionQueue component on controlled entity
    ↓
validate_action_capabilities system (warns if entity can't handle action)
    ↓
Component-specific handlers route actions to modules
    ↓
Module components check constraints (fuel, power, cooldown)
    ↓
Module components apply effects via Avian physics (Forces.apply_force(), etc.)
```

#### Core Components

**`EntityAction` enum** (in `crates/sidereal-game/src/actions.rs`):
- High-level intent actions (not raw physics)
- Examples: `ThrustForward`, `ThrustReverse`, `YawLeft`, `YawRight`, `FirePrimary`, `ActivateShield`
- Extensible for new actions without touching input layer

**`ActionQueue` component**:
- Attached to entities that receive actions
- Holds `Vec<EntityAction>` for current tick
- Cleared/drained each frame

**`ActionCapabilities` component**:
- Declares which `EntityAction`s an entity can handle
- Used for validation and UI hints
- Example: A ship with engines can handle `ThrustForward`/`YawLeft`, but a cargo container cannot

#### Example: Flight Control Chain

1. **Input Layer**: Player presses `W` → bindings produce `EntityAction::ThrustForward`
2. **Network/Local**: Action sent to controlled entity's `ActionQueue`
3. **FlightComputer Handler** (`process_flight_actions` system):
   - Reads `ActionQueue`, matches flight-related actions
   - Updates `FlightComputer.throttle` to 1.0
4. **Engine Handler** (`apply_engine_thrust` system):
   - Queries all `Engine` modules mounted on entities with `FlightComputer`
   - For each engine:
     - Check `FuelTank.fuel_kg > 0.0`
     - If yes: compute thrust force, drain fuel, accumulate force
     - If no: log fuel exhaustion, skip
   - Aggregate all engine forces in parent entity's local space
   - Rotate to world space via `Transform.rotation`
   - Apply via Avian's `Forces.apply_force(force_world)` query helper
5. **Avian Integration**: Forces are integrated by Avian's physics step into velocity/position changes

#### Design Invariants

- **No direct velocity manipulation**: Always use `Forces.apply_force()` / `Forces.apply_torque()` so Avian handles mass/inertia/damping correctly
- **Capability-driven**: Components declare support for actions; no global "is this a ship?" checks
- **Fuel/power/constraints at handler level**: Input layer doesn't know about fuel; `Engine` component does
- **Shared for player input, AI commands, scripted sequences**: Same `EntityAction` API works for all command sources
- **Module hierarchy respected**: Engines mounted via `MountedOn` component, forces applied to parent entity
- **Deterministic**: Action processing and force application run in `FixedUpdate` schedule

#### Future Extensions

- **Weapon actions**: `FirePrimary`/`FireSecondary` → `WeaponMount` handler → projectile spawn + ammo drain
- **Shield actions**: `ActivateShield` → `ShieldProfile` handler → power drain + damage mitigation
- **Utility actions**: `ActivateTractor`, `ActivateScanner` → respective component handlers
- **Autopilot/AI**: AI systems produce `EntityAction`s instead of raw input, same pipeline

#### Implementation Notes

**Files:**
- `crates/sidereal-game/src/actions.rs`: `EntityAction` enum, `ActionQueue`, `ActionCapabilities`
- `crates/sidereal-game/src/flight.rs`: `process_flight_actions`, `apply_engine_thrust`
- `crates/sidereal-game/src/lib.rs`: System registration in `FixedUpdate` schedule

**System ordering:**
```rust
FixedUpdate::chain(
    validate_action_capabilities,  // Warn about unsupported actions
    process_flight_actions,         // Actions → FlightComputer state
    apply_engine_thrust,            // FlightComputer → Engine forces
)
```

This runs before Avian's `PhysicsSet::StepSimulation`, ensuring forces are ready for integration.

### 8.12 Component Source-of-Truth and Generation Plan

Root rule:

- Gameplay component definitions are centralized in core (`crates/sidereal-game`) and treated as source-of-truth for runtime, replication, and persistence mapping.

Generation direction:

- Define component schemas in `crates/sidereal-game/schema/components/` (one schema per component family).
- Generate Rust component definitions/registrations into `crates/sidereal-game/src/generated/components.rs`.
- Generated output must include derives/metadata needed for:
  - Bevy ECS registration,
  - `Reflect`,
  - serde (`Serialize`/`Deserialize`),
  - stable persistence metadata (`component_kind`, type path envelope key).

Why this is done at grass roots:

- prevents drift between ECS runtime types and graph persistence mapping,
- guarantees new components are persistable/hydratable by default unless explicitly runtime-only,
- creates one extensible path for ships/hardpoints/modules/inventory/combat/scripting-facing components.

Non-generated exceptions:

- Truly runtime/transient components (for example Avian internal runtime state, caches, ephemeral prediction-only helpers) remain hand-authored and explicitly marked non-persisted.

Persistence contract for gameplay ECS components:

- Every gameplay-relevant component that crosses runtime boundaries must support `Reflect` + `Serialize` + `Deserialize`.
- Persisted component payloads are stored using reflect envelopes keyed by stable Rust type path.
- New persistable components must include graph mapping tests and hydration roundtrip tests in the same change.

### 8.13 Next ECS Implementation Steps

1. Add ownership/control components and validation systems.
2. Add `HealthPool`, `ProjectileProfile`, `LifetimeTicks` for combat baseline.
3. Add inventory/cargo components with persistence mapping.
4. Add gravity/orbital components for system-scale simulation.
5. Define archetype bundles per entity class for consistent spawning.

## 9. Shared Simulation Core (`sidereal-sim-core`)

Purpose:

- prevent client/server drift by centralizing deterministic rule math reused by shard and client prediction.

Must contain only pure deterministic logic:

- input normalization/state transitions,
- control integration helpers,
- fuel/power gating math,
- deterministic step helpers.

Must not contain:

- ECS queries/resources,
- Avian world/contacts,
- transport/persistence/auth code,
- authority decisions.

Current migrated examples:

- signed axis mapping,
- input edge/state transitions,
- planar integration helpers,
- yaw-rate helpers,
- stop/reset semantics,
- shared network-input mapping adapter crate.

Parity policy:

- maintain golden-vector tests for deterministic behavior.

## 10. Persistence and Graph Model

### 10.1 Storage Domains

Relational domain:

- `accounts`, `refresh_tokens`, `password_reset_tokens`, operational metadata.

Graph domain (AGE graph `sidereal`):

- world entities/components/hardpoints/ownership/inventory relationships.

Snapshot markers:

- periodic checkpoint metadata for recovery workflows.

### 10.2 Postgres + AGE Boot Requirements

Database container requirements:

- run PostgreSQL image with Apache AGE support,
- ensure `age` extension exists,
- load AGE and ensure graph `sidereal` exists at startup.

Runtime init requirements (service startup):

- `CREATE EXTENSION IF NOT EXISTS age;`
- `LOAD 'age';`
- `SET search_path = ag_catalog, "$user", public;`
- ensure graph exists (`ag_catalog.create_graph('sidereal')` if missing),
- reset search_path to `public` afterward.

### 10.3 Graph Identity Rules

- one logical entity_id => one graph node.
- AGE persistence uses `:Entity` as the canonical graph label; additional gameplay classifications are stored in `sidereal_labels` and rehydrated into runtime label sets.
- component ids are stable (for example `<entity_id>::<component_kind>`).

### 10.4 Canonical Graph Shape

Labels:

- `Entity`, `Ship`, `Component`, `Hardpoint`, `InventorySlot`, `Item`, `Faction`, `Player`

Edges:

- `(:Entity)-[:HAS_COMPONENT]->(:Component)`
- `(:Component)-[:MOUNTED_ON]->(:Hardpoint)`
- `(:Entity)-[:HAS_HARDPOINT]->(:Hardpoint)`
- `(:Entity)-[:HAS_CHILD]->(:Entity)` (entity hierarchy / Bevy children relationship)
- `(:Entity)-[:OWNS]->(:Entity|:Item|:InventorySlot)`
- `(:InventorySlot)-[:CONTAINS]->(:Item)`

### 10.5 Persistence Write Flow

1. Shard emits authoritative world deltas.
2. Replication ingests and persists at configured cadence (`REPLICATION_PERSIST_INTERVAL_S`, default `15s`), with immediate flush for removals/critical durability events.
3. Snapshot markers written periodically.
4. Critical events are durability candidates for replay semantics.

### 10.6 Recovery/Hydration

- startup hydration only,
- on startup, replication hydrates its read-model ECS state from graph persistence before serving client-facing state streams,
- no periodic DB overwrite into live shard entities,
- runtime remains shard-authoritative.

### 10.7 Generalized Component Persistence Rules

- Persistence mapping must be generalized for broad component families (ships, hardpoints, mounted modules like engines/flight computers/shield generators, inventory, ownership, hierarchy).
- Hierarchy/modularity must preserve parent-child and mount relationships in graph edges so hydration can rebuild ECS relationships deterministically.
- Internal network codecs may use `bincode` (or JSON where configured), but persisted graph payload contracts remain serde-based and backward-compatible.

## 11. Auth and Identity at the Core

### 11.1 Auth Principles

- Email/password login only.
- Access token: JWT HS256.
- Refresh/reset tokens: opaque random values stored hashed.
- Password hashing: Argon2.

### 11.2 Auth API Surface

Gateway endpoints:

- `GET /health`
- `POST /auth/register`
- `POST /auth/login`
- `POST /auth/refresh`
- `POST /auth/password-reset/request`
- `POST /auth/password-reset/confirm`
- `GET /auth/me`
- `GET /world/me` (JWT-authenticated player world bootstrap snapshot for client login handoff)
  - includes starter ship movement tuning required for client/shared module wiring (for example `engine_max_accel_mps2`, `engine_ramp_to_max_s`)
- `GET /assets/stream/{asset_id}` (JWT-authenticated streaming asset endpoint for client cache population)
- Asset bootstrap metadata is delivered on the authenticated replication/control channel (not HTTP asset file endpoints).
- Current scaffold behavior: password reset request returns a reset token in response for local/dev flow verification; production delivery should move to out-of-band mail/SMS and stop returning raw tokens.

### 11.3 Registration and Starter Ship Bootstrap

Required lifecycle:

1. gateway creates account record and `player_entity_id` (`player:<account_uuid>`),
2. gateway requests replication bootstrap command,
3. replication persists bootstrap receipt and applies bootstrap idempotently (`account_id` unique; duplicate commands are recorded but not re-applied),
4. replication performs world bootstrap in graph if player owns none (current scaffold creates starter `Ship` metadata with `asset_id=corvette_01` plus persisted engine tuning fields consumed by client/shared movement modules),
5. login does not create gameplay entities.

This keeps auth as entry authority and world bootstrap in replication-owned world pipeline.

### 11.4 Session to Gameplay Identity

- all gameplay routing derives from authenticated `player_entity_id` claim,
- replication binds transport session identity (`RemoteId`/peer) to authenticated `player_entity_id` and rejects mismatched input claims from client packets,
- entitlement/ownership loading is graph-based,
- controlled entity selection must remain ownership-authorized.

## 12. Asset Delivery and Streaming (Ground Support)

Presentation/data split:

- gameplay packets carry `asset_id` and physical dimensions,
- actual bytes are streamed from backend to client over authenticated game channels (not served as standalone HTTP files).

Required behavior:

- placeholder-first rendering,
- async swap on asset resolution,
- soft-fail (missing asset never crashes gameplay),
- cache + refcount + TTL + LRU + memory budget.
- disk cache on client with checksum/version validation and stale-asset replacement.
- client cache format is a single local PAK data file plus companion index/metadata file (MMO-style launcher/runtime cache model).

Asset manager defaults and scope:

- default asset root is `./data`,
- existing GLTF models in `./data/models` are first-class managed assets,
- manager covers models, textures, audio, shaders, scripting logic bundles, and other content blobs,
- client connection requirements stay minimal: connect/authenticate, then receive required assets and scripted logic streams from backend.

Stream protocol requirements:

- server emits asset catalog/version metadata on session bootstrap,
- client requests missing or stale `asset_id`s by checksum/version,
- backend streams assets in chunked frames with resumable offsets,
- each asset includes `asset_version` + `sha256` and optional parent version for delta/chunk reuse.
- server-side cache generation scripts are not part of runtime behavior; cache assembly is performed client-side from streamed chunks.

Versioning and invalidation requirements:

- authoritative invalidation is content-hash driven (`sha256`) and versioned (`asset_version`),
- backend asset updates must trigger client refresh on next sync window or explicit push invalidation,
- client keeps an on-disk cache index keyed by `asset_id` -> `{asset_version, sha256, size, pak_offset, pak_length, last_used_at}`.

Do not leak gameplay internals through asset metadata.

## 13. Procedural Asteroids (Ground Support)

Current universe baseline:

- seeded asteroid field with varied size/mass/health and collision dimensions.

Ground-level support requirements:

- stable asteroid identity->seed mapping,
- authoritative size/collision remains server truth,
- visuals can be generated/procedural on client without altering gameplay authority,
- asset pipeline must allow procedural registration/caching/LOD.

Recommended direction:

- CPU deterministic base mesh generation + optional GPU detail materials.

## 14. Scripting and Modding (Ground Support)

Scope policy:

- scripting for content-level systems (missions, dialogue, high-level AI behavior, event composition),
- core authority systems remain Rust/ECS only.

Not scriptable:

- authoritative physics,
- networking/replication protocol internals,
- client prediction core,
- trust/security enforcement.

Recommended engine integration:

- Lua scripting bridge (`bevy_mod_scripting`) with sandboxing and explicit API surface.

Architecture:

- script runtime calls controlled Rust APIs that operate on ECS data,
- scripts never bypass authority or permission boundaries.

### 14.1 Flight Computer and Scripting Model

Flight-computer direction:

- `FlightComputer` remains a core ECS component in `sidereal-game` data model.
- Flight-computer behavior is split into:
  - deterministic control pipeline in Rust (authoritative application of thrust/rotation intent),
  - optional script policy layer that decides high-level intent.

Script boundary for flight computers:

- scripts may read approved ECS/query views and emit intent-level outputs only (for example target throttle/steering profile/mode),
- scripts never directly mutate authoritative transforms, velocities, ownership, replication envelopes, or persistence state.

Operational model:

- authoritative script execution is server-side; clients can run non-authoritative mirrors for UX only.
- script callbacks are budgeted/sandboxed and failure-contained; on failure/time budget exceed, runtime falls back to deterministic Rust defaults.
- scripted flight-computer profiles are content assets (streamed/versioned like other script bundles), while ECS component state remains persistable through the same reflect+serde graph contract.

## 15. Multi-Shard and Handoff Design

Shard responsibilities:

- own bounded regions,
- simulate entities in region,
- prepare/commit handoff at boundaries.

Handoff primitives (must exist as explicit contracts):

- `HandoffPrepare`
- `HandoffAck`
- `HandoffCommit`
- epoch/lease validation on transfer.

Replication responsibilities in multi-shard:

- aggregate world state for client views,
- route client inputs to owning shard,
- compute visibility across shard boundaries,
- hide shard ownership details from clients.

## 16. Crate and Folder Structure (Authoritative Plan)

Workspace crates:

- `crates/sidereal-core`: IDs, shared constants/types, zero DB/network runtime logic.
- `crates/sidereal-sim-core`: deterministic shared simulation math.
- `crates/sidereal-input-map`: mapping network input models into sim-core controls.
- `crates/sidereal-net`: envelopes/messages/serialization contracts.
- `crates/sidereal-game`: ECS components/systems/gameplay logic.
- `crates/sidereal-persistence`: schema init, graph/relational persistence, hydration, replay utilities.

Binaries:

- `bins/sidereal-shard`
- `bins/sidereal-replication`
- `bins/sidereal-gateway`
- `bins/sidereal-orchestrator`
- `bins/sidereal-bg-sim`
- `bins/sidereal-tools`

Client (single crate, dual targets):

- `bins/sidereal-client`: the client workspace member. Produces a native binary via `[[bin]]` and a WASM `cdylib` library via `[lib]`. There is no separate `sidereal-client-web` crate; the WASM artifact is the library target of this crate. See section 3.3 for rationale and build commands. Platform branching is done with `cfg(target_arch = "wasm32")`, never with cargo feature flags.

Folder layout guidance:

- `docs/`: design/ADRs/protocol/runtime defaults/UI design system.
- `docker/` + `docker-compose.yaml`: local infra.
- `data/`: dev asset and DB data mounts.
- `scripts/`: repeatable local orchestration/dev flows.

## 17. Coding and Engineering Standards

### 17.1 Dependency and Workspace Discipline

- dependency versions align in root `[workspace.dependencies]`.
- avoid per-crate version drift.

### 17.2 Naming and Units

- explicit unit suffixes (`_m`, `_kg`, `_hz`, `_ms`, `_tick`),
- metric units only,
- snake_case modules/functions, PascalCase types.

### 17.3 Code Splitting Standard

- target module/file size around <=300 lines where practical,
- split by domain responsibility,
- no god modules combining protocol + ECS + persistence,
- wire DTOs, ECS components, and persistence models stay in separate crates/modules.

### 17.4 Boundary Rules

- no persistence/network code in core gameplay components unless via explicit boundary adapters,
- no Bevy `Entity` crossing service boundaries,
- protocol changes require synchronized updates to spec docs and tests.
- client platform/network adapters (native vs browser) must not fork gameplay/sim logic; keep shared code in common crates.

### 17.5 Operational Discipline

- structured logs include shard/tick/entity context where relevant,
- critical retries are explicit and observable,
- no silent authority fallbacks.
- runtime inspection endpoints (`bevy_remote`) must be auth-gated and enabled for shard/replication/client from day 0 scaffolding.

## 18. Runtime Defaults and Config Surface

Core defaults:

- sim tick: 30 Hz,
- render target: 60 Hz,
- snapshot send baseline: 20 Hz,
- interpolation buffer: ~100 ms,
- replication persist interval: 15 s,
- snapshot marker interval: 15 s.

Current notable env vars:

- `SIM_TICK_HZ`
- `REPLICATION_SEND_HZ`
- `REPLICATION_UDP_BIND` default: `0.0.0.0:7001` (Lightyear raw UDP server bind on replication)
- `REPLICATION_UDP_ADDR` default: `127.0.0.1:7001` (target addr for shard/native Lightyear clients)
- `SHARD_UDP_BIND` default: `127.0.0.1:7002` (Lightyear shard client local bind)
- `CLIENT_UDP_BIND` default: `127.0.0.1:7003` (Lightyear native client local bind)
- `SIDEREAL_CLIENT_HEADLESS` default: unset/false (`1`/`true` runs native client in transport-only headless mode for integration harnesses)
- `REPLICATION_PERSIST_INTERVAL_S`
- `SNAPSHOT_INTERVAL_S`
- `REPLICATION_DATABASE_URL` default: `postgres://sidereal:sidereal@127.0.0.1:5432/sidereal`
- `GATEWAY_BIND` default: `127.0.0.1:8080`
- `GATEWAY_DATABASE_URL` default: `postgres://sidereal:sidereal@127.0.0.1:5432/sidereal`
- `GATEWAY_JWT_SECRET` required; minimum length 32 chars
- `GATEWAY_ACCESS_TOKEN_TTL_S` default: `900`
- `GATEWAY_REFRESH_TOKEN_TTL_S` default: `2592000`
- `GATEWAY_RESET_TOKEN_TTL_S` default: `3600`
- `GATEWAY_BOOTSTRAP_MODE` default: `direct` (`udp` enables fire-and-forget replication control handoff instead)
- `GATEWAY_REPLICATION_CONTROL_UDP_BIND` default: `0.0.0.0:0` (gateway local UDP bind for bootstrap handoff send)
- `GATEWAY_*` visibility and delta thresholds
- `SIDEREAL_ASSET_ROOT` default: `./data`
- `SIDEREAL_ASSET_CACHE_DIR` default: `./client_cache/assets`
- `SIDEREAL_ASSET_CACHE_PAK_PATH` default: `./client_cache/assets/assets.pak`
- `SIDEREAL_ASSET_CACHE_INDEX_PATH` default: `./client_cache/assets/assets_index.bin`
- `SIDEREAL_ASSET_STREAM_CHUNK_BYTES`
- `SIDEREAL_CLIENT_TRANSPORT_NATIVE`
- `SIDEREAL_CLIENT_TRANSPORT_WEB` (target value direction: `webrtc` first, optional `websocket` fallback)
- `REPLICATION_CONTROL_UDP_BIND` / `REPLICATION_CONTROL_UDP_ADDR`
- `SIDEREAL_RTC_SIGNAL_BIND`: replication server WebSocket signaling endpoint bind address (e.g., `0.0.0.0:9003`).
- `SIDEREAL_STUN_URLS`: comma-separated STUN server URLs delivered to clients during signaling (e.g., `stun:stun.l.google.com:19302`).
- `SIDEREAL_TURN_URLS`: comma-separated TURN server URLs (production required for symmetric NAT clients).
- `SIDEREAL_TURN_CREDENTIAL_TTL_S`: lifetime in seconds for HMAC-based TURN per-session credentials (default: `3600`).

## 19. Local Development Setup (From Scratch)

### 19.1 Start PostgreSQL + AGE

```bash
docker compose up -d postgres
```

If `5432` is already in use:

```bash
SIDEREAL_PG_PORT=55432 docker compose up -d postgres
```

Container uses `apache/age:latest` and mounts:

- `./data/postgresql:/var/lib/postgresql`
- `./docker/init:/docker-entrypoint-initdb.d`

Reference compose shape:

```yaml
services:
  postgres:
    image: apache/age:latest
    environment:
      POSTGRES_DB: sidereal
      POSTGRES_USER: sidereal
      POSTGRES_PASSWORD: sidereal
    ports:
      - "${SIDEREAL_PG_PORT:-5432}:5432"
    volumes:
      - ./data/postgresql:/var/lib/postgresql
      - ./docker/init:/docker-entrypoint-initdb.d:ro
```

### 19.2 Verify AGE + Graph

```bash
docker exec sidereal-postgres psql -U sidereal -d sidereal -c "LOAD 'age'; SET search_path = ag_catalog, public; SELECT * FROM cypher('sidereal', \$\$ MATCH (n) RETURN count(n) \$\$) AS (count agtype);"
```

### 19.3 Run core services (example)

```bash
REPLICATION_UDP_BIND=0.0.0.0:7001 cargo run -p sidereal-replication
SHARD_UDP_BIND=127.0.0.1:7002 REPLICATION_UDP_ADDR=127.0.0.1:7001 cargo run -p sidereal-shard
cargo run -p sidereal-gateway
```

### 19.4 Formatting/Lint/Compile baseline

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

### 19.5 Asset Cache Runtime Behavior (Local Dev)

Expected behavior:

- user installs/runs small client binary,
- client authenticates and receives asset catalog/version metadata from backend,
- missing/stale assets are streamed and written into local `assets.pak` with index updates,
- subsequent launches reuse on-disk cache and only fetch invalidated/new assets.

## 20. Decision Register Carry-Forward (Relevant Locked Decisions)

Carry these as active constraints:

- DR-002 Canonical tick/time model.
- DR-003 Channel and protocol semantics with versioned envelopes.
- DR-004 visibility policy (authorization vs delivery scopes).
- DR-005 graph-native world persistence direction.
- DR-007 auth/session and trust boundary model.
- DR-013 one-way runtime authority direction.
- DR-014 UUID identity boundary.
- DR-015 register-only bootstrap behavior.
- DR-016 Avian->Transform sync contract.
- DR-017 staged prediction scope.
- DR-018 high-velocity reconciliation hardening direction.
- DR-019 staged UDP codec migration compatibility.

Open decisions to resolve early in a fresh rebuild:

- DR-001 handoff contract specifics,
- DR-006 recovery/failover semantics,
- DR-008 economy consistency model,
- DR-009 physics authority classing,
- DR-010 observability baseline,
- DR-011 final client scope freeze,
- DR-012 bootstrap/workspace operational boundaries.

## 21. Example Contracts and Snippets (Reference)

Network envelope:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetEnvelope<T> {
    pub protocol_version: u16,
    pub channel: ChannelClass,
    pub source_shard_id: i32,
    pub lease_epoch: u64,
    pub seq: u64,
    pub tick: u64,
    pub payload: T,
}
```

Input snapshot (current protocol style):

```rust
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct InputSnapshot {
    pub thrust_forward: bool,
    pub thrust_reverse: bool,
    pub yaw_left: bool,
    pub yaw_right: bool,
    pub stop_requested: bool,
    pub reset_requested: bool,
}
```

Visibility pseudo-code:

```rust
fn compute_visibility(player: &PlayerState, entity: &EntityState) -> Visibility {
    if player.owns(entity.id) || player.owns_parent_of(entity.id) {
        return Visibility::Full;
    }
    if player.any_scanner_authorizes(entity.id) {
        return Visibility::Redacted;
    }
    Visibility::None
}
```

Startup AGE ensure pseudo-flow:

```rust
sqlx::query("CREATE EXTENSION IF NOT EXISTS age;").execute(conn).await?;
sqlx::query("LOAD 'age';").execute(conn).await?;
sqlx::query("SET search_path = ag_catalog, \"$user\", public;").execute(conn).await?;
// ensure graph sidereal exists
sqlx::query("SET search_path = public;").execute(conn).await?;
```

## 22. Implementation Sequence (Rebuild Order)

1. Bootstrap workspace + crates + strict boundaries.
2. Implement auth/session domain in gateway and relational schema.
3. Implement AGE schema init + graph persistence primitives.
4. Implement shard authoritative fixed-step sim and ECS baseline.
5. Implement replication ingest/persist and visibility filtering contracts.
6. Implement client with tick-indexed input, controlled prediction, remote interpolation buffer.
7. Implement scan grants + multi-stream visibility.
8. Add asset streaming manager with placeholder-first policy.
9. Add scripting bridge and procedural asteroid pipeline behind feature flags.
10. Implement orchestrator and multi-shard handoff semantics.

## 23. Acceptance Criteria for "Same Stage as Current, Refactor-Baked"

- Auth/register/login/refresh/reset flows operational.
- Registration creates playable starter ship world state through replication-owned bootstrap path.
- Single-shard authoritative movement with stable local control and no frequent correction snaps at high speed.
- Remote entities rendered via snapshot-buffer interpolation.
- Server-authorized redaction working (owned vs non-owned visibility differences).
- Graph persistence + startup hydration functioning with no periodic DB->live overwrite.
- Shared gameplay logic (`sidereal-game`) is used by both server and client paths so intent validation/fuel/engine behavior remain aligned; `sidereal-sim-core` remains available for pure deterministic helper math.
- Asset IDs delivered with placeholder fallback and no gameplay impact from missing assets.
- Native and WASM clients both build in CI with shared gameplay code and transport-specific adapters only at boundary layers; WASM CI validation includes WebGPU-enabled build settings.
- Baseline docs/decisions/coding standards synchronized with implementation.

## 24. Compact Glossary

- `Authority`: the service that is allowed to decide final world state.
- `Shard`: authoritative simulation process for one or more regions.
- `Replication`: client-facing state distribution and durability staging service.
- `Lease Epoch`: monotonic ownership version used to prevent dual authority.
- `Prediction`: client local simulation ahead of confirmed server state.
- `Reconciliation`: correction process when authoritative state diverges from prediction.
- `Interpolation`: rendering entities between buffered snapshots from the past.
- `Extrapolation`: bounded forward estimate when no newer snapshot is available.
- `Authorization Scope`: data a player is allowed to know.
- `Delivery Scope`: subset of authorized data actively sent in a given stream.
- `Hydration`: loading durable graph state into runtime ECS state.
- `Reflect Envelope`: persisted component payload shape keyed by Rust type path.

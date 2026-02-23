# Sidereal v3 Design Document

Status: Active architecture and gameplay specification  
Audience: engineers and maintainers

## 1. Product and Gameplay Focus

Sidereal is a server-authoritative multiplayer space RPG built around:

- deterministic fixed-step simulation,
- capability-driven Bevy ECS gameplay,
- persistent world state,
- smooth client prediction/interpolation for responsive control.

Core player loop:

1. Authenticate and enter the world.
2. Control a modular ship (flight computer + engines + fuel + hardpoints).
3. Observe and interact with nearby entities under server-enforced visibility.
4. Persist state changes through replication-owned durability pipelines.

## 2. Hard Rules

1. Authority is one-way: `client input -> replication simulation -> persistence`.
2. Clients send intent only; clients never authoritatively set world transforms/state.
3. Cross-boundary identity is UUID/entity-id only; runtime Bevy `Entity` ids never cross service boundaries.
4. Runtime simulation state is authoritative in memory; persistence is durability/hydration.
5. Visibility and redaction are server-side concerns before serialization.
6. Behavior is capability-driven; labels like "Ship" are descriptive, not branching logic.
7. Motion authority uses Avian components directly (`Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`); legacy gameplay mirror motion components are not used.

## 3. Runtime Architecture

### 3.1 Services

- `sidereal-gateway`: auth and identity lifecycle.
- `sidereal-replication`: authoritative simulation host, visibility, client fanout, persistence staging.
- PostgreSQL + AGE: persistence.
- `sidereal-client`: native + WASM targets from one crate.

### 3.2 Current Networking

- Lightyear is the active runtime framework for:
  - replication,
  - prediction/rollback,
  - interpolation,
  - native input transport.
- Runtime protocol traffic is bincode-driven through Lightyear registrations.
- Legacy JSON envelope helpers are persistence/test fixtures only.

### 3.3 WASM Transport Direction (Future)

WASM client direction remains WebRTC-first:

- reliable ordered channel for session/control traffic,
- unreliable unordered channel for realtime gameplay traffic.

WebSocket may exist only as explicit fallback.  
Gameplay/simulation systems remain shared between native and WASM; only transport adapter code differs at the boundary.

## 4. Bevy ECS Gameplay Model

### 4.1 ECS Principles

- Composition over inheritance.
- Generic entity terminology for generic systems.
- Domain behavior through components/capabilities, not hardcoded entity classes.
- Shared gameplay logic in `crates/sidereal-game` is source-of-truth for runtime behavior.

### 4.2 Core Gameplay Components (Current)

Identity and ownership:

- `EntityGuid`
- `DisplayName`
- `OwnerId`
- `FactionId`, `FactionVisibility`, `PublicVisibility`
- `ShardAssignment`

Ship/modularity:

- `Hardpoint`
- `MountedOn`
- `FlightComputer`
- `Engine`
- `FuelTank`
- `ActionQueue`

Physics/mass:

- Avian: `RigidBody`, `Collider`, `Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`, `Mass`, `AngularInertia`, `LockedAxes`
- Gameplay: `MassKg`, `BaseMassKg`, `CargoMassKg`, `ModuleMassKg`, `TotalMassKg`, `MassDirty`, `SizeM`

Visibility/scanning:

- `ScannerRangeM`
- `ScannerComponent`
- `ScannerRangeBuff`

### 4.3 Capability Rules

Any entity with:

- `Engine` + `FuelTank` can generate thrust.
- `FlightComputer` can consume flight intent.
- `HealthPool` can be damaged/destroyed.
- scanner components can participate in visibility extension.
- hardpoints and mount links can host modular behavior.

### 4.4 Hierarchy and Relationships

- Parent-child and mount relationships must persist and hydrate deterministically.
- `MountedOn.parent_entity_id` is canonical across boundaries.
- Bevy hierarchy is rebuilt from persisted relationships on hydration.

## 5. Simulation, Tick, and Prediction

### 5.1 Timing Contract

- Fixed simulation tick: 30 Hz.
- Gameplay physics and prediction logic run in fixed schedules.
- Frame-time deltas are render/UI only, never authoritative simulation math.

### 5.2 Input Contract

Client writes per-tick `PlayerInput` intent:

```rust
pub struct PlayerInput {
    pub actions: Vec<EntityAction>,
}
```

Server input routing is bound to authenticated session identity and controlled entity mapping.

### 5.3 Prediction and Interpolation

Controlled entity:

- runs Lightyear predicted mode (`Predicted`),
- rollback rewinds to confirmed state and resimulates with shared gameplay + Avian physics,
- correction policy is tunable via client env vars.

Remote entities:

- run Lightyear interpolation (`Interpolated`) + frame interpolation for smooth rendering.

### 5.4 Rollback Performance Expectations

- rollback can re-run fixed-step systems multiple ticks,
- expensive non-authoritative systems should guard with rollback checks where appropriate,
- authoritative flight/mass systems must still run during rollback.

## 6. Persistence and Hydration

### 6.1 Canonical Shape

Persistence uses graph records:

- `GraphEntityRecord`
- `GraphComponentRecord`
- relationship edges for parent-child and modular mounts

World-delta legacy persistence shapes are not used.

### 6.2 Runtime Persistence Flow

1. Replication sim updates ECS state.
2. Persistence snapshot system serializes registered durable components.
3. Upserts/removals are persisted in graph form.
4. Startup hydration reconstructs ECS entities/components/relationships deterministically.

### 6.3 Persistence Boundaries

- Avian transient internals are not persisted.
- Durable gameplay state required after restart must be represented in persistable gameplay components.
- New persistable gameplay components must support `Reflect` + serde and include roundtrip coverage.

## 7. Visibility and Data Permissions

Server enforces three scopes:

1. world truth,
2. authorization scope,
3. delivery scope.

Rules:

- unauthorized data is never serialized,
- ownership/faction/public policies are applied server-side,
- visibility is computed over entities generically (not ship-only assumptions).
- client visibility is server-decided; clients cannot self-upgrade visibility by local inference/culling tricks.

### 7.1 Scope Definitions

- `world truth`: authoritative shard/replication runtime state for all entities/components.
- `authorization scope`: what the player is allowed to know at all (ownership, attachments, scanner reach, scan grants, faction/public policy).
- `delivery scope`: what the active client session receives right now (camera/focus culling and stream policy) from the authorized set.

A client may be authorized for more than it currently receives on a given stream.

### 7.2 Authorization and Fog-of-War Contract

- Scanner range is server-enforced fog-of-war for non-owned entities.
- Default scanner authorization floor is `300m` around the player's controlled-entity observer position.
- Scanner authorization aggregates over all owned entities (not only the currently controlled entity), including valid ownership/attachment chains.
- Non-public entities outside active scanner authorization must not be delivered.
- Visibility exceptions are explicit and server-enforced:
  - entities owned by the player are always authorized,
  - entities marked `PublicVisibility` are authorized as policy allows,
  - entities marked `FactionVisibility` are authorized to matching factions.
- Unauthorized entities previously delivered must be removed via authoritative removal flow.

### 7.3 Sensitive Data Rule and Redaction

- Physical presence visibility does not imply internal state visibility.
- By default, non-owned observed entities expose physical/render-safe data only (for example position, velocity, orientation, render/body identifiers).
- Sensitive internals must be omitted unless explicitly authorized:
  - cargo manifests and transfer details,
  - private subsystem internals/loadouts,
  - hidden operational state.
- Redaction is applied server-side before transport encoding on every stream.

### 7.4 Scan Intel Grant Model

- Deep intel is unlocked by explicit, temporary server-side scan grants.
- A grant binds observer, target, field scope, source, and expiry.
- Initial field scopes include:
  - `physical_public`,
  - `combat_profile`,
  - `cargo_summary`,
  - `cargo_manifest`,
  - `systems_detail`.
- Final payload masking is computed by server policy:
  - base authorization,
  - active grants for `(observer, target)`,
  - resulting field redaction mask.
- Grant expiry or revocation must immediately restore redacted output.

### 7.5 Camera-Centered Delivery Contract (Required)

- In addition to scanner/authorization visibility, replication delivery must apply camera-centered network culling as a client optimization filter.
- Client `ClientViewUpdateMessage.camera_position_m` is the culling anchor for delivery scope, not a persistence-only field.
- For top-down gameplay, camera delivery culling uses XY coordinates only (`x`, `y`); `z` is not part of the culling decision.
- The server must avoid delivering replication updates for entities outside the camera delivery volume that the client cannot render.
- Camera delivery culling includes an additional configurable edge buffer radius beyond the visible viewport bounds so fast-moving entities do not snap/pop in at the boundary.
- Camera-centered delivery culling must never bypass authorization/ownership/faction/public visibility policy; it is an additional narrowing filter only.

### 7.6 Stream Tiers (Direction)

- Visibility and redaction policy is shared across streams; streams differ by rate/radius/detail.
- `focus_stream`: high-rate, local gameplay fidelity.
- `strategic_stream`: lower-rate, wider-radius minimap/contact picture with coarse kinematics.
- `intel_stream`: event-driven scan/intel grant updates with only grant-authorized fields.
- Client subscription to additional streams must not widen authorization rules.

### 7.7 Spatial Query and Scaling Requirements

- Visibility selection must use spatial indexing/query acceleration, not full-world scans per client tick.
- Spatial queries must include:
  - nearby cells for focus/delivery radii,
  - owned/scanner-derived authorization coverage.
- Keep explicit performance telemetry for visibility queries:
  - candidates per frame,
  - included entities per frame,
  - query time budget per client.

### 7.8 Scale and Control-Swap Readiness (Current vs Target)

**Is the current network system robust enough for thousands of entities, multiple owners, and players swapping which ship they control?**

**No.** The current implementation is suitable for small sessions (handful of players, tens of entities). The following gaps must be closed for the target scale and control model.

**Visibility and scale**

- **Current:** `update_network_visibility` does a full scan: for each client, it iterates every replicated entity and evaluates range/ownership/faction. Complexity is O(clients × entities) per fixed tick.
- **Target (see 7.7):** Spatial indexing (e.g. grid or BVH) so visibility uses “nearby cells” and owned/scanner-derived coverage, not a full-world scan. Without this, thousands of entities and many clients will not be viable.

**Observer and scanner aggregation**

- **Current:** One observer position per client, from a single “controlled” entity (and an OwnerId-based fallback). Scanner range is taken from that entity only.
- **Target (see 7.2):** Scanner authorization should aggregate over *all* owned entities (e.g. all ships the player owns), not only the currently controlled one. Observer/visibility logic should support multiple observer points or an aggregated range per client.

**Control swap (player changes which ship they control)**

- **Current:** The server stores `last_controlled_entity_id` from `ClientViewUpdateMessage` and persists it, but **input routing is not driven by it**. Input is routed only via Lightyear’s `ControlledBy`, which is bound once at auth/bootstrap to a single ship. `PlayerControlledEntityMap` holds one entity per `player_entity_id`. There is no path to “swap control” to another owned ship.
- **Target:** When the client sends a view update with a new `controlled_entity_id` that is an entity owned by that player, the server must:
  - Update which entity has `ControlledBy` for that client (remove from old ship, insert on new ship),
  - Keep observer/visibility and `PlayerControlledEntityMap` (or equivalent) in sync with the current controlled entity,
  - Persist the new “current controlled” choice so it survives reconnect/restart.

**Multiple ships per player**

- **Current:** A player can own multiple ships (same `OwnerId`), but only one is the “controlled” ship in `PlayerControlledEntityMap`, and only that one receives input. No support for switching which of the player’s ships is controlled.
- **Target:** Support multiple owned entities per player and a clear “current controlled” entity that can change over time (control swap above), with visibility/input/observer all consistent with that choice.

**Summary**

- **Thousands of entities, multiple players:** Not robust until visibility uses spatial indexing and possibly replication culling/LOD as described in 7.5–7.7.
- **Players swapping between ships they own:** Not implemented; requires server-side handling of `controlled_entity_id` to update `ControlledBy`, observer, and controlled-entity mapping.

## 8. Auth and Session Identity

- Gateway owns auth lifecycle (`register/login/refresh/reset`).
- Replication binds session transport identity to authenticated `player_entity_id`.
- Input packets with mismatched identity claims are rejected.
- Gameplay control selection remains ownership-authorized.

## 9. Asset Streaming

- Asset delivery is stream-based through backend/client runtime channels.
- Client uses local cache (`assets.pak` + index metadata) with checksum/version invalidation.
- Missing assets must fail soft (no gameplay crash).
- No standalone HTTP asset-file serving for gameplay runtime paths.

## 10. Client Platform Model

- One client crate (`bins/sidereal-client`) with:
  - native `[[bin]]` target,
  - WASM `[lib]` target.
- Platform branching is `cfg(target_arch = "wasm32")` only.
- Native and WASM gameplay behavior stay in lockstep; transport adapters are platform-specific boundary code.

## 11. Engineering Boundaries

- Keep gameplay logic in shared crates, not duplicated across client/server binaries.
- Keep entrypoints focused on wiring/plugin composition.
- Split mixed domains into focused modules (input, visibility, persistence, auth, rendering, etc.).
- Do not reintroduce legacy world-delta or legacy gameplay mirror-motion pathways.

## 12. Operational Validation Baseline

Minimum checks for significant runtime changes:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

If client behavior/protocol/prediction changes:

```bash
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

---

For migration history and tuning backlog details, see `docs/migrate_to_lightyear_prediction.md`.  
This document is the current-state architecture contract.

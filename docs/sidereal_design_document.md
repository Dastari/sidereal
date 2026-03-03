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

1. Authenticate an account.
2. Select a character (player entity) and explicitly request Enter World.
3. Control a modular ship (flight computer + engines + fuel + hardpoints).
4. Observe and interact with nearby entities under server-enforced visibility.
5. Persist state changes through replication-owned durability pipelines.

## 2. Hard Rules

1. Authority is one-way: `client input -> replication simulation -> persistence`.
2. Clients send intent only; clients never authoritatively set world transforms/state.
3. Cross-boundary identity is UUID/entity-id only; runtime Bevy `Entity` ids never cross service boundaries.
4. Runtime entity GUIDs must be globally unique across entity families (player/ship/module/hardpoint). Do not reuse the same GUID for different entity categories.
5. Persistence/hydration must fail closed on runtime GUID collisions. Persistence batches with duplicate runtime GUIDs are rejected, and hydration aborts when collisions are detected in stored graph records.
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
- Production native runtime transport is currently UDP (`UdpIo` / `ServerUdpIo`).
- Browser/WASM transport is not yet implemented end-to-end; WebRTC-first remains the accepted target direction.

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

Visual identity (2D migration path):

- `VisualAssetId` (entity-generic sprite asset identity)
- `SpriteShaderAssetId` (optional sprite pixel-shader asset identity)

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

### 4.5 Possible Future Gameplay Systems (Planning)

The following are non-exhaustive candidate systems for future phases.  
These are directional planning notes and do not override phase gating or enforceable rules.

- **Control and intent**
  - control handoff validation/ack flow,
  - intent queue conflict resolution and stale-intent pruning,
  - capability-based action rejection with explicit reasons.
- **Flight and propulsion**
  - fuel request/allocation policy across multiple tanks,
  - engine degradation/failure effects,
  - collision-aware correction tuning for predicted controlled entities.
- **Hierarchy and modular runtime**
  - parent-link and hardpoint occupancy validation,
  - module attach/detach transitions with deterministic hierarchy rebuild,
  - module disable/destroy propagation into parent capabilities.
- **Combat and survivability**
  - weapon fire intent -> projectile spawn/authority routing,
  - hit resolution and damage pipeline (shield/armor/hull),
  - destroy/disable lifecycle state transitions.
- **Sensors and visibility**
  - scanner contribution aggregation and dynamic range buffs,
  - faction/public visibility policy expansion and redaction,
  - delivery-scope throttling under load.
- **Economy/inventory progression**
  - inventory transfer validation and ownership checks,
  - cargo mass coupling to runtime physics updates,
  - persistent progression mutations on player-scoped entities.

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

### 5.2.1 Control and Camera Chain (Normative)

Authoritative runtime chain:

1. `camera <- player entity <- controlled entity (optional)`

Rules:

1. `ControlledEntityGuid = Some(target)`:
- action routing target is controlled entity by default,
- player entity follows controlled entity position/state,
- camera follows player entity.
2. `ControlledEntityGuid = Some(self player guid)`:
- action routing target is player entity,
- player movement acceptor handles free-roam actions (WASD),
- camera follows player entity.
3. Detached free-camera is an explicit camera mode:
- enabled/disabled by explicit client mode switch,
- gameplay movement intent emission is suppressed while detached (camera-only pan),
- detached mode does not redefine server-authoritative control routing semantics.

Single-writer motion principle:

1. Controlled mode: controlled entity simulation writes controlled motion; player-follow system writes player anchor.
2. Uncontrolled mode: player movement system writes player motion.
3. Camera systems never write authoritative simulation motion state.

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
- Custom gameplay component definitions live in `crates/sidereal-game/src/components/` as individual component files and are registered through the shared game component registry.
- Persisted/replicated component metadata is declared with `#[sidereal_component(kind = \"...\", persist = bool, replicate = bool, visibility = [...])]`; visibility defaults to owner-only when omitted.
- Visibility policy metadata supports multiple scopes (`[OwnerOnly, Faction, Public]`) so the server can enforce field delivery by authorization policy rather than client-side assumptions.
- Detailed authoring workflow and examples live in `docs/component_authoring_guide.md`.

## 7. Visibility and Data Permissions

Implementation contract for contributors: `docs/features/visibility_replication_contract.md`.

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

Pipeline contract:

1. Authorization decides entitlement (security gate).
2. Delivery narrows authorized data for efficiency (interest management gate).
3. Payload redaction enforces component/field disclosure policy (serialization gate).

Implementation note:
- A performance-oriented candidate preselection step (for example spatial nearby-cell query) may run before full authorization evaluation.
- Such preselection is an optimization input only and must be fail-closed safe:
  - it must never be treated as authorization by itself,
  - it must not exclude entities that policy requires to be considered (ownership/public/faction/scan-grant exceptions),
  - final outbound delivery remains a strict narrowing of authorization.

### 7.2 Authorization and Fog-of-War Contract

- Scanner range is server-enforced fog-of-war for non-owned entities.
- Default scanner authorization floor is `300m` around the player's character observer position (player entity/camera context).
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

- **Current runtime modes:** `update_network_visibility` has a pluggable candidate stage:
  - default: `full_scan` (O(clients × entities)),
  - opt-in: `spatial_grid` (uniform-grid candidate preselection + policy exception bypass paths).
- **Safety rule:** candidate preselection is optimization-only; ownership/public/faction/scanner exceptions must still be considered even if an entity misses candidate preselection.
- **Target (see 7.7):** move production/default operation to spatial indexing (and later LOD/culling tiers) with telemetry-backed tuning.

**Observer and scanner aggregation**

- **Current:** One observer position per client from persisted player runtime camera state (`position_m`/`Transform.translation` on the player entity), with scanner-source union over owned entities.
- **Target (see 7.2):** Keep scanner authorization aggregated over *all* owned entities (e.g. all ships the player owns), with observer/visibility logic supporting multiple observer points or equivalent aggregated coverage per client.

**Control swap (player changes which ship they control)**

- **Current:** Implemented via persisted `controlled_entity_guid` on the player entity. Client sends `ClientControlRequestMessage { player_entity_id, controlled_entity_id, request_seq }`; server validates ownership and updates `ControlledBy` plus `PlayerControlledEntityMap`.
- **Rule:** Control handoff is explicit request/response:
  - success: `ServerControlAckMessage { player_entity_id, request_seq, controlled_entity_id }`,
  - failure: `ServerControlRejectMessage { player_entity_id, request_seq, reason, authoritative_controlled_entity_id }`.
  Client clears pending control only on matching ack/reject. Free-roam is self-control (`controlled_entity_guid = player guid`), not null control.
- **Camera/anchor contract:** camera always follows the player entity. When controlled target is not self, server continuously anchors player transform to the controlled entity.

**Multiple ships per player**

- **Current:** A player can own multiple ships; only the server-authoritative controlled ship receives input. Player can switch control among owned ships; missing targets resolve to player self-control.
- **Target:** Keep this model while extending scanner/visibility to aggregate over all owned entities at scale.

**Summary**

- **Thousands of entities, multiple players:** Not robust under `full_scan`; use `spatial_grid` (then next-stage index/LOD improvements) for large sessions as described in 7.5–7.7.
- **Players swapping between ships they own:** Implemented with server-side ownership validation and persisted player runtime state.

## 8. Auth and Session Identity

- Gateway owns auth lifecycle (`register/login/refresh/reset`).
- Registration must create and persist account + default character player entity + starter corvette graph records in durability storage.
- Register/login are auth-only and must not implicitly bind a runtime world session.
- Runtime bootstrap handoff from gateway to replication is explicit `Enter World` behavior and must be idempotent per `player_entity_id`.
- `Enter World` requests must ensure runtime presence/bind for the selected character on every reconnect attempt; idempotency must not prevent reconnect rebind when runtime entities are missing.
- Player-specific runtime/persistent data is player-entity scoped. Authoritative control state persists via `controlled_entity_guid` on the player entity; score, quest progression, and other character-local settings persist on the player entity in graph persistence.
- Account identity is an auth container and external reference. An account may own multiple player entities (characters); `player_entity_id` selects which character/session identity is bound for runtime control.
- Replication binds session transport identity to authenticated `player_entity_id`.
- Client world entry state transition is `Auth -> CharacterSelect -> WorldLoading -> InWorld`; transition to `InWorld` occurs only after replication session-ready bind acknowledgment for the selected `player_entity_id` plus replicated player-entity presence on client.
- Input packets with mismatched identity claims are rejected.
- Gameplay control selection remains ownership-authorized.
- Runtime systems must fail closed on ownership/identity mismatches (reject and log) rather than silently creating replacement state.

## 9. Asset Streaming

- Asset delivery is stream-based through backend/client runtime channels.
- Current client cache backend is file-tree based under `data/cache_stream/**`.
- `assets.pak` + index metadata remains the target cache shape; rollout is tracked in `docs/features/asset_delivery_contract.md`.
- Starter corvette runtime visual default is defined via gameplay component `VisualAssetId("corvette_01")`; default stream source maps that asset ID to `sprites/ships/corvette.png` (served from `ASSET_ROOT`, typically `data/sprites/ships/corvette.png` in local development).
- Optional per-entity sprite shader is driven by `SpriteShaderAssetId`; baseline streamed shader asset ID is `sprite_pixel_effect_wgsl` (`shaders/sprite_pixel_effect.wgsl`) and binds to client runtime shader path `data/cache_stream/shaders/sprite_pixel_effect.wgsl`.
- Missing assets must fail soft (no gameplay crash).
- Transitional note: gateway still exposes `/assets/stream/{asset_id}`; runtime asset consumption is stream-first and standalone HTTP serving is slated for removal from gameplay paths.

## 10. Client Platform Model

- One client crate (`bins/sidereal-client`) with:
  - native `[[bin]]` target,
  - WASM `[lib]` target.
- Platform branching is `cfg(target_arch = "wasm32")` only.
- Native and WASM gameplay behavior stay in lockstep; transport adapters are platform-specific boundary code.
- Native renderer backend selection uses `SIDEREAL_CLIENT_WGPU_BACKENDS` first, then `WGPU_BACKEND`, then defaults to `PRIMARY` backends (`VULKAN | METAL | DX12 | BROWSER_WEBGPU`) when unset.
- Native multi-instance safety: non-headless clients use a local process guard; if another client instance is already active, the new instance forces `WgpuSettings.force_fallback_adapter = true` to reduce multi-instance GPU-driver crash risk on low-end/older adapters.
- `SIDEREAL_CLIENT_FORCE_SOFTWARE_ADAPTER` explicitly overrides multi-instance fallback behavior (`1`/`true` forces software adapter; `0`/`false` disables software fallback and uses hardware adapter selection).
- Native primary window is user-resizable with enforced minimum logical size `960x540`; resize/minimize transitions treat non-positive viewport dimensions as non-renderable for fullscreen backdrop/material uniform updates.

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

For prediction runtime tuning and validation backlog details, see `docs/features/prediction_runtime_tuning_and_validation.md`.  
This document is the current-state architecture contract.

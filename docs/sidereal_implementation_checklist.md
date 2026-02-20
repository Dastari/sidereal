# Sidereal v3 Implementation Checklist

Status: Planned implementation guide
Date: 2026-02-19
Primary spec: `docs/sidereal_design_document.md`

## Architecture Note (Updated 2026-02-19)

**Simplified Architecture:** Shard servers are reserved for future multi-shard spatial partitioning. Current implementation consolidates all simulation into the replication server for faster iteration and simpler deployment.

```
Client ←→ Gateway (HTTP/REST: auth, /world/me, /assets/stream)
Client ←→ Replication (Lightyear UDP/WebRTC: game input/state)
Replication ←→ Database (persistence)
```

Replication server responsibilities:
- Client connections (Lightyear transport)
- Avian physics simulation
- EntityAction routing to entity ActionQueues
- FlightComputer → Engine → Forces pipeline
- Visibility filtering
- State broadcast to clients
- Database persistence

The `bins/sidereal-shard` binary exists but is not currently used. See `bins/sidereal-shard/README.md` for future multi-shard plans.

## How to Use This Checklist

- Complete phases in order unless a dependency says otherwise.
- Every completed item must include:
  - code change,
  - unit tests for the touched system,
  - integration test updates where cross-service behavior is affected,
  - docs updates if protocol/runtime behavior changed.

## Current Focus (Vertical Slice)

- [x] Registration to playable world path is deterministic:
  - [x] `POST /auth/register` creates account and dispatches bootstrap command once.
  - [x] Active bootstrap dispatcher path creates starter ship world state for that account (`direct` by default; optional `udp` handoff to replication).
  - [x] `GET /world/me` returns starter ship + asset descriptors without manual retries.
- [ ] Native client login/register UX closes loop:
  - [x] Auth UI works end-to-end (register/login/forgot).
  - [x] Dialog UI system implemented for persistent error/warning/info modals.
  - [x] Error handling uses dialog system (world load failures, protocol errors).
  - [x] Asset stream fetch succeeds for `corvette_01` + starfield shader assets.
  - [ ] Player enters world and can thrust/turn/logout with HUD telemetry visible.
- [x] Client auth/in-world lifecycle uses state-scoped entity cleanup (Bevy state transitions, not manual despawn loops).
- [x] Required Components baseline added for core generated gameplay components.
- [x] Fallible-query pattern audit completed for active runtime systems touched in current vertical slice.
- [ ] Transport remains parity-gated during this vertical slice:
  - [ ] Lightyear replication/native-client e2e transport test stays green.
  - [ ] Native and WASM client compile checks stay green on every change.
- [ ] Visibility/scan-intel grass-roots foundations are enforced in all new network payload work:
  - [ ] Apply authorization-scope vs delivery-scope separation before serialization.
  - [ ] Apply field-level redaction masks server-side (never trust client-side filtering).
  - [ ] Ensure grant expiry/revocation paths immediately revert to redacted baseline.

## Target Gameplay Loop (Acceptance Criteria)

- [ ] Launch native client and immediately see auth UX with these flows: register, login, forgot-password request, forgot-password confirm.
- [ ] Registering a brand-new account creates exactly one starter player ship through replication bootstrap.
- [ ] Starter ship uses `corvette_01` model and includes baseline gameplay components (`EntityGuid`, `DisplayName`, `PositionM`, `VelocityMps`, `HealthPool`, `FlightComputer`, `Hardpoint`/module attachments as applicable).
- [ ] On successful auth, client calls `/world/me` and receives starter ship state + required asset descriptors.
- [ ] Client requests and receives streamed asset bytes for `corvette_01` and starfield shader assets from backend stream endpoints.
- [ ] Entering world shows a top-down camera view (camera projection decision documented: orthographic vs perspective) with shader-driven starfield running as a camera-attached background fed by client camera/ship motion (velocity + acceleration) uniforms.
- [ ] Player can thrust/turn, receives authoritative updates, and logout returns cleanly to auth state.
- [ ] In-world HUD shows at least coordinates, velocity, and health from authoritative/state-reconciled data.
- [ ] Visibility/data-permission pipeline is enforced for gameplay replication payloads before network serialization.
- [ ] Native and WASM builds both compile with WebGPU enabled for `sidereal-client`.

## Phase 0: Repo Foundations

- [ ] Create/confirm workspace crate boundaries exactly as defined in `docs/sidereal_design_document.md`.
- [ ] Align dependencies under root `[workspace.dependencies]`.
- [ ] Enforce lint gates (`fmt`, `clippy -D warnings`, `check`) in CI.
- [ ] Add `bevy_remote` dependency and shared config scaffolding for `sidereal-shard`, `sidereal-replication`, and `sidereal-client`.
- [ ] Define auth model + bind defaults for runtime inspection endpoints (disabled unauthenticated access by default).
- [ ] Set up `crates/sidereal-client` as the single client workspace member with both a `[[bin]]` (native, `src/main.rs`) and a `[lib]` with `crate-type = ["cdylib", "rlib"]` (WASM, `src/lib.rs`). Do not create a separate `sidereal-client-web` crate. Platform branching is `cfg(target_arch = "wasm32")`; no cargo feature flag for WASM.
- [ ] Add both native and WASM build checks to CI from day 0: `cargo check -p sidereal-client` and `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`. The WASM target must never be left broken between PRs, and WASM validation must include WebGPU support.
- [ ] Define shared-code boundary rules so native and WASM clients reuse gameplay/prediction/sim crates and differ only at platform/network adapter layers.
- [ ] Enforce "native-primary, WASM-parity" change discipline: every client behavior/protocol/runtime change must include a WASM impact update in the same PR (code + tests + docs, or an explicit no-impact note).
- [ ] Add component source-of-truth scaffolding in `crates/sidereal-game`: schema directory, generated output location, and registry wiring conventions.
- [ ] Add baseline test harness structure:
  - `crates/*/tests/` for crate unit/integration tests,
  - `tests/` for cross-service flow tests.

Unit tests required:

- [ ] `sidereal-core`: ID helpers, constant invariants.
- [ ] `sidereal-net`: envelope encode/decode and backward compatibility decode tests.
- [ ] `sidereal-sim-core`: deterministic golden-vector tests.

## Phase 1: Database + Persistence Base (Postgres + AGE)

- [ ] Bring up PostgreSQL with AGE via `docker compose`.
- [ ] Implement schema ensure for relational auth tables.
- [ ] Implement AGE bootstrap (`CREATE EXTENSION`, `LOAD 'age'`, graph ensure).
- [ ] Implement graph persist primitives (`persist_graph_records`, removals, load/hydrate records).
- [ ] Implement snapshot marker writes.
- [ ] Implement replication startup hydration from graph persistence before serving client-facing replication streams.
- [ ] Enforce component persistence contract: gameplay components are `Reflect` + serde and stored via reflect envelopes keyed by stable type path.
- [ ] Define and test explicit non-persisted Avian runtime-only component exclusions; persist mirrored durable gameplay components instead.
- [ ] Persist and hydrate Bevy hierarchy (`Children`/parent-child) and module mount relationships (hardpoints/modules) through graph edges.

Unit tests required:

- [ ] SQL/query builder helpers (escaping, AGType parse, property serialization).
- [ ] Graph record transformation tests (world delta -> graph records).
- [ ] Hydration mapping tests for reflect envelope decode.
- [ ] Component roundtrip coverage for ship/module hierarchies (ship -> hardpoint -> mounted module).
- [ ] Explicit tests for non-persisted Avian runtime-only component exclusions.

Integration tests required:

- [ ] Service boot ensures schema/graph on empty DB.
- [x] Persist then hydrate roundtrip preserves IDs/components (`crates/sidereal-persistence/tests/lifecycle.rs`).

## Phase 2: Auth Core (`sidereal-gateway`)

- [x] Implement register/login/refresh/reset/me endpoints.
- [x] Implement Argon2 password hashing and JWT issuance.
- [x] Implement hashed refresh token storage and rotation behavior.
- [x] Implement account -> `player_entity_id` mapping.
- [x] Implement registration bootstrap command handoff to replication (not direct ad-hoc world writes).

Unit tests required:

- [x] Password hash/verify tests.
- [x] JWT claim encode/decode and expiry tests.
- [x] Refresh token hash/validation tests.
- [x] Request validation tests (email/password constraints).

Integration tests required:

- [x] register -> login -> refresh -> me happy path.
- [x] login does not create gameplay entities.
- [x] register creates starter world state once only.

## Phase 3: Replication Server Simulation (`sidereal-replication`)

**NOTE:** Replication server now handles all simulation (no separate shard servers in v3 simplified architecture).

### ⚠️ Breaking Changes to Fix

The protocol was updated to remove shard-specific messages. The following files need updates:

1. `bins/sidereal-replication/src/main.rs`:
   - Remove `ShardStateMessage` import (line 17)
   - Remove `receive_shard_state` system (line 388) - no longer needed
   - `ClientInputMessage` now has `actions: Vec<EntityAction>` instead of `thrust`/`turn` fields

2. `bins/sidereal-shard/src/main.rs`:
   - This binary is deprecated for now (see `bins/sidereal-shard/README.md`)
   - Keep it buildable as a minimal placeholder until multi-shard work begins

3. `bins/sidereal-client/src/main.rs`:
   - Update any test code that creates `ClientInputMessage` to use new format:
     ```rust
     ClientInputMessage {
         player_entity_id: "...".to_string(),
         actions: vec![EntityAction::ThrustForward],
         tick: 42,
     }
     ```

### Current Status (2026-02-19)

**✅ Completed:**
- [x] Network protocol updated (`ClientInputMessage` with `Vec<EntityAction>`, `ReplicationStateMessage`)
- [x] Dependencies added (`avian3d`, `sidereal-game`)
- [x] Action routing system implemented in `sidereal-game` (EntityAction enum, ActionQueue, FlightComputer, Engine)
- [x] Avian force-based physics working in test environment
- [x] Bootstrap command handling (creates starter ships in database)
- [x] Visibility filtering (`bins/sidereal-replication/src/visibility.rs`)

**🚧 In Progress - Critical Path to Playable Loop:**

1. **Add Avian Physics to Replication** (`bins/sidereal-replication/src/main.rs`)
   - [x] Add `PhysicsPlugins::default().with_length_unit(1.0)` to app
   - [x] Insert `Gravity(Vec3::ZERO)` resource
   - [x] Add `SiderealGamePlugin` (brings action systems: `validate_action_capabilities`, `process_flight_actions`, `apply_engine_thrust`)
   - [x] Set fixed timestep resource to 30Hz (`Time<Fixed>::from_hz(30.0)`)

2. **Hydrate Entities from Database on Startup** (`bins/sidereal-replication/src/main.rs`)
   - [x] Add startup hydration path that loads graph records and spawns simulation ship entities
   - [x] Spawn entities with action pipeline components (`ActionQueue`, `ActionCapabilities`, `FlightComputer`)
   - [x] Add Avian body components (`RigidBody::Dynamic`, `Collider`, `LinearVelocity`, `AngularVelocity`, damping)
   - [x] Map `player_entity_id` to spawned ship entities for input routing

3. **Route Client Input to Entity ActionQueues** (`bins/sidereal-replication/src/main.rs`)
   - [x] Receive `ClientInputMessage` from clients
   - [x] Look up entity by `player_entity_id`
   - [x] Push `actions` to entity's `ActionQueue` component
   - [x] System: `receive_client_inputs` populates `ActionQueue`

4. **Broadcast Updated State to Clients** (`bins/sidereal-replication/src/main.rs`)
   - [x] Collect simulated entity state from replication runtime
   - [x] Serialize to `WorldStateDelta` (positions, velocities, health, owner metadata)
   - [x] Apply visibility filtering (already implemented in `visibility.rs`)
   - [x] Send `ReplicationStateMessage` to connected clients via Lightyear

### Architecture Flow

```
[Database]
    ↓ (startup hydration)
[Replication ECS World]
    ↓
[Client connects] → [ClientInputMessage] → [ActionQueue on entity]
    ↓
[FixedUpdate 30Hz]:
  validate_action_capabilities
  process_flight_actions (Actions → FlightComputer state)
  apply_engine_thrust (FlightComputer → Engine → Avian Forces)
  [Avian PhysicsSchedule runs]
    ↓
[Entity positions/velocities updated]
    ↓
[serialize_world_state] → [visibility_filter] → [ReplicationStateMessage]
    ↓
[Clients receive and render]
```

Unit tests required:

- [ ] Input routing to correct entity ActionQueue
- [ ] Action validation for entities without capabilities
- [ ] Physics determinism (same input → same output)
- [ ] Visibility filtering respects ownership
- [ ] State serialization includes all required components

Integration tests required:

- [ ] Full loop: client input → physics → state broadcast
- [ ] Multiple clients don't see each other's private data
- [ ] Entity persistence survives replication restart

## Phase 4: Client-Side Implementation (`sidereal-client`)

**NOTE:** With simplified architecture, client connects directly to replication server (not separate shard).

### Current Status

**✅ Completed:**
- [x] Native client scaffold with auth UI
- [x] Lightyear transport to replication server
- [x] Avian physics added to client for prediction
- [x] Action system architecture designed

**🚧 Next Steps - Client Input Loop:**

1. **Capture Keyboard Input → EntityAction** (`bins/sidereal-client/src/main.rs`)
   - [ ] Add system to read `ButtonInput<KeyCode>` in `Update` schedule
   - [ ] Map keys to `EntityAction`:
     - `W` → `EntityAction::ThrustForward`
     - `S` → `EntityAction::ThrustReverse`  
     - `A` → `EntityAction::YawLeft`
     - `D` → `EntityAction::YawRight`
   - [ ] Accumulate actions for current tick
   - [ ] On key release, send neutral actions (`ThrustNeutral`, `YawNeutral`)

2. **Send EntityActions to Replication** (`bins/sidereal-client/src/main.rs`)
   - [ ] Create `ClientInputMessage` with:
     - `player_entity_id` (from `/world/me` response)
     - `actions: Vec<EntityAction>` (accumulated this tick)
     - `tick: u64` (client tick counter)
   - [ ] Send via Lightyear `MessageSender<ClientInputMessage>`
   - [ ] Use `InputChannel` (unordered/unreliable)
   - [ ] Send every tick (30Hz or 60Hz, redundancy is okay)

3. **Receive World State from Replication** (`bins/sidereal-client/src/main.rs`)
   - [ ] Receive `ReplicationStateMessage` via Lightyear
   - [ ] Deserialize `world_json` → `WorldStateDelta`
   - [ ] For each `WorldDeltaEntity`:
     - If entity doesn't exist locally: spawn it
     - If entity exists: update components (position, velocity, health)
     - If `removed == true`: despawn entity
   - [ ] System: `receive_replication_state` → spawns/updates entities

4. **Render HUD** (`bins/sidereal-client/src/main.rs`)
   - [ ] Query player's controlled entity
   - [ ] Extract position, velocity, health from components
   - [ ] Display in Bevy UI text:
     ```
     Position: (X, Y, Z)
     Velocity: V m/s
     Health: HP / MAX_HP
     ```
   - [ ] Update every frame in `Update` schedule

5. **Client-Side Prediction (Optional for MVP, Recommended)**
   - [ ] Apply same action systems locally (`process_flight_actions`, `apply_engine_thrust`)
   - [ ] Run local Avian simulation
   - [ ] When server state arrives:
     - Compare with predicted state at that tick
     - If divergence > threshold: rollback to server state, replay unacked inputs
     - Smooth correction over time
   - [ ] Use `bins/sidereal-client/src/prediction.rs` (needs Avian integration)

### Data Flow

```
[Player presses W]
    ↓
[Input system] → EntityAction::ThrustForward
    ↓
[ClientInputMessage { actions: [ThrustForward], tick: 42 }]
    ↓
[Lightyear send to Replication]
    ↓
[Replication receives, routes to entity ActionQueue]
    ↓
[Replication physics runs, updates entity]
    ↓
[ReplicationStateMessage { world_json: {...}, tick: 42 }]
    ↓
[Client receives, deserializes WorldStateDelta]
    ↓
[Update entity position/velocity components]
    ↓
[Render at new position, HUD shows updated values]
```

Integration tests required:

- [ ] Input roundtrip: keyboard → server → position change → HUD update
- [ ] Multiple keys pressed simultaneously → multiple actions sent
- [ ] Network lag doesn't cause input loss (redundant sends)
- [ ] Entity spawning/despawning from server state
- [ ] Visibility filtering hides other players' private data

## Phase 5: Polish and Optimization

Unit tests required:

- [ ] Prediction replay queue pruning by ack tick.
- [ ] Correction thresholds/blend policy logic.
- [ ] Snapshot bracketing interpolation math.
- [ ] Extrapolation cap behavior tests.
- [ ] `bevy_remote` endpoint wiring and auth/config guard tests.

Integration tests required:

- [ ] local controlled movement remains stable under simulated latency/jitter.
- [ ] remote entities remain smooth and do not lerp-chase target jumps.

## Phase 6: Visibility, Permissions, and Scan Intel

- [ ] Implement three-scope model in runtime code paths.
- [ ] Enforce field-level redaction server-side before serialization.
- [ ] Implement temporary scan-intel grants and expiry/revocation.
- [ ] Add stream tiers (`focus`, `strategic`, `intel`) scaffolding.
- [ ] Add/optimize spatial indexing for visibility candidate queries.

Unit tests required:

- [ ] Ownership and attachment authorization rules.
- [ ] Grant scope merge/resolution logic.
- [ ] Redaction of sensitive fields by default.
- [ ] Revocation/expiry behavior.

Integration tests required:

- [ ] unauthorized observers never receive restricted fields.
- [ ] authorized scan grants temporarily expose only allowed scopes.

## Phase 7: Asset Streaming Foundation

- [x] Deliver `asset_id` metadata with entity state where applicable.
- [ ] Implement client placeholder-first rendering.
- [x] Implement backend->client asset stream bootstrap (no HTTP file serving).
- [x] Set default asset root to `./data` and ingest existing GLTFs from `./data/models`.
- [ ] Enforce starter-loop asset contract:
  - [ ] `corvette_01` model stream includes all dependent asset payloads required for first render.
  - [ ] Starfield shader payloads are versioned and streamable through the same backend channel contract.
  - [ ] `/world/me` asset descriptors remain sufficient for on-demand fetch without out-of-band asset discovery.
- [ ] Stream all required content classes through asset manager (`models`, `textures`, `audio`, `shaders`, `scripting logic bundles`, misc blobs).
- [ ] Implement cache refcount + TTL + LRU budget eviction.
- [ ] Persist client cache on disk as single `assets.pak` + companion index with resumable downloads/chunks.
- [ ] Implement `asset_version` + `sha256` validation and stale-cache replacement on backend updates.
- [ ] Add failure dedupe and fallback visuals.
- [ ] Ensure runtime does not rely on server-side scripts to generate client cache artifacts; client assembles cache from streamed asset chunks.

Unit tests required:

- [ ] cache hit/miss/refcount accounting.
- [ ] eviction ordering and budget enforcement.
- [ ] checksum mismatch and version bump invalidation logic.
- [ ] chunk reassembly/resume correctness for interrupted transfers.
- [ ] pak index offset/length integrity and lookup correctness.
- [ ] missing asset fallback behavior.

Integration tests required:

- [ ] entity remains renderable with placeholder when asset fetch fails.
- [ ] asset swap-in works without gameplay state disruption.
- [ ] backend asset update causes client to receive refreshed bytes and pak/index update.

## Phase 8: Procedural Asteroids and Content Extensibility

- [ ] Implement deterministic asteroid seed mapping from entity identity.
- [ ] Implement procedural mesh/material pipeline hooks (feature-flagged).
- [ ] Keep collision/size authoritative on server data.

Unit tests required:

- [ ] seed determinism tests.
- [ ] generated mesh parameter validity tests (bounds/vertex counts).

Integration tests required:

- [ ] procedural visuals do not alter authoritative collision semantics.

## Phase 9: Scripting Bridge (Feature-Flagged)

- [ ] Introduce script runtime crate and safe API boundary.
- [ ] Expose content-level hooks (missions/dialogue/high-level AI).
- [ ] Keep core authority systems non-scriptable.
- [ ] Add sandboxing constraints and script error containment.
- [ ] Implement flight-computer script policy bridge: scripts output intent-level commands only; Rust applies deterministic authoritative controls.
- [ ] Add deterministic fallback behavior when script execution fails or exceeds budget.

Unit tests required:

- [ ] API boundary permission tests.
- [ ] script execution timeout/error isolation tests.
- [ ] deterministic bridge behavior for key script callbacks.
- [ ] flight-computer script boundary tests (no direct transform/velocity/ownership mutation API access).

Integration tests required:

- [ ] scripted mission flow interacts with ECS through approved API only.

## Phase 10: Multi-Shard and Orchestrator

- [ ] Implement lease model and epoch guards.
- [ ] Implement handoff prepare/ack/commit protocol.
- [ ] Implement replication routing for cross-shard visibility/input.
- [ ] Implement failover/recovery state transitions.

Unit tests required:

- [ ] lease epoch conflict handling.
- [ ] handoff state machine transitions.
- [ ] route selection by authority ownership.

Integration tests required:

- [ ] entity handoff continuity under movement across boundary.
- [ ] no dual-authority updates after handoff commit.

## Cross-Cutting Test Matrix (Must Exist)

- [ ] Unit tests in each crate touched by a feature.
- [ ] Deterministic fixture tests for all shared sim-core mechanics.
- [ ] Protocol compatibility tests for codec/version migration.
- [ ] Cross-service integration tests for auth->bootstrap->control->persist.
- [ ] Soak test scenario for high-speed flight jitter regression detection.

## Definition of Done (Per Feature PR)

- [ ] Implementation complete and behind correct service boundaries.
- [ ] Unit tests added/updated in each affected system.
- [ ] Integration tests updated where flow spans services.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo check --workspace` passes.
- [ ] Docs updated (`docs/sidereal_design_document.md`, protocol/design docs, runtime defaults, decision register if architectural).

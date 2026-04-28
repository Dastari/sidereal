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
7. Motion authority for physics entities uses Avian components directly (`Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`); legacy gameplay mirror motion components are not used.
8. Static non-physics world entities use `WorldPosition` / `WorldRotation`; Avian transform components are reserved for actual physics/simulation participants.

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
- Browser/WASM transport now targets a WebTransport-first browser boundary through Lightyear-compatible adapters, with WebSocket allowed only as an explicit fallback.
- The WASM client still does not implement the full native runtime, but it now shares the fixed-step gameplay core bootstrap with native instead of being a completely separate render-only shell.
- Native client currently reaches `InWorld` and renders replicated entities, but in-world controls and motion stability remain unresolved; native control/prediction stabilization is the immediate runtime priority before more WASM parity validation work resumes.
- Gateway HTTP must answer browser CORS preflight for local dashboard/client origins. The runtime default allows `http://localhost:3000` and `http://127.0.0.1:3000`; set comma-separated `GATEWAY_ALLOWED_ORIGINS` when the browser host origin differs.
- Gateway and replication tracing output is written to both the console and workspace-relative `./logs/`, using a fresh timestamped log file for each process start.

Update note (2026-03-11):
- Replication and gateway startup configuration is no longer intended to be Makefile-only. Both binaries now accept CLI arguments for their core runtime configuration, with precedence `CLI > env > built-in default`.
- Local-dev built-in defaults now align with the long-standing non-debug Makefile defaults:
  - replication UDP bind: `0.0.0.0:7001`
  - replication WebTransport bind: `0.0.0.0:7003`
  - replication control UDP bind: `127.0.0.1:9004`
  - replication health bind: `127.0.0.1:15716`
  - gateway HTTP bind: `0.0.0.0:8080`
  - asset root: `./data`
  - scripts root: `./data/scripts`
  - gateway allowed origins: `http://localhost:3000,http://127.0.0.1:3000`
- BRP remains opt-in and loopback-only. Those defaults were not changed to always-on runtime behavior.

Update note (2026-04-24):
- Gateway world-entry responses should advertise a client-reachable native replication endpoint through `REPLICATION_UDP_PUBLIC_ADDR` / `--replication-udp-public-addr`; do not advertise `127.0.0.1:7001` to clients on another host.
- Native clients now guard the common remote-host misconfiguration case: when a remote `GATEWAY_URL` is used but the advertised/fallback UDP endpoint resolves to loopback, the client derives the replication host from the gateway URL and keeps the advertised UDP port, logging the rewrite. This is a compatibility fallback, not a substitute for correct public server configuration.
- Native clients also choose a wildcard local UDP bind (`0.0.0.0:0`) by default when the resolved replication target is remote, while preserving loopback bind defaults for local replication and respecting explicit `CLIENT_UDP_BIND` configuration.
- Native impact: remote Windows/native clients can connect when the gateway and replication server share a host but the gateway still advertises the local-dev loopback default. WASM impact: no change; browser clients still require explicit WebTransport address and certificate digest.

### 3.2.1 Server-Only Admin Spawn Control Path (Current)

Server-authoritative entity spawning for dashboard/dev tooling uses a dedicated gateway-admin path:

1. Gateway endpoint: `POST /admin/spawn-entity`.
2. Caller must present a valid gateway access token with role `admin`, `dev_tool`, or `developer`, `session_context.mfa_verified=true`, and scope `admin:spawn`.
3. Gateway forwards a control command to replication over the replication control channel.
4. Replication validates:
   - canonical `player_entity_id` UUID,
   - allowed `bundle_id` from Lua bundle registry,
   - allowed override keys/shape/size.
5. Replication executes Lua bundle spawn via `bundles/entity_registry.lua` (`build_graph_records` path), enforces `owner_id` server-side, persists graph records, hydrates runtime entities, and lets normal replication/owner-manifest flows publish results.

Security rules:

1. Game client transport is never allowed to issue spawn commands.
2. Caller-supplied owner overrides are ignored/replaced by server-authoritative target player id.
3. Spawn requests are audit-logged with actor, target player, bundle, and spawned entity id.

### 3.2.2 Bevy Remote Inspection (Current)

- `bevy_remote` is available for client/replication inspection in development.
- Current hardening policy is loopback-only bind.
- A BRP auth token is still required in config, but it is not yet the primary network security boundary.
- Non-loopback BRP exposure is not allowed until an authenticated HTTP gate exists in front of the endpoint.

### 3.3 WASM Transport Direction (Current)

WASM client direction is WebTransport-first:

- Lightyear browser transport uses WebTransport as the primary runtime lane.
- Gateway auth/bootstrap/asset payloads remain authenticated HTTP, not replication transport.
- WebSocket may exist only as an explicit fallback path; it is not the default browser runtime transport.

Gameplay/simulation systems remain shared between native and WASM; only transport and browser I/O adapters differ at the boundary.
Live browser parity validation beyond the current bootstrap state is temporarily deferred while native in-world control and correction issues are being stabilized.

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

- `VisibilityRangeM`
- `ScannerComponent`
- `VisibilityRangeBuffM`

Visual identity (2D migration path):

- `VisualAssetId` (entity-generic sprite asset identity)
- `SpriteShaderAssetId` (optional sprite pixel-shader asset identity)
- Render composition is migrating to Lua-authored render-layer definitions and rule-based world-layer assignment executed through a fixed set of generic client material schemas; see `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`.
- Runtime shader/material ownership follows a family taxonomy rather than one Rust material type per effect; see `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`.

### 4.2.1 Render Layer Contract (Planned Direction)

The generic 2D render composition direction is:

1. Default non-fullscreen entities render in the main world layer.
2. Lua-authored rules may redirect entities to other world layers by labels/archetype/component presence.
3. Fullscreen background and foreground layers are authored separately from generic gameplay spawn paths.
4. Camera-scoped post-process passes are authored separately from world-layer assignment.
5. Layer depth/parallax is render-derived only; it must not mutate authoritative entity positions or other simulation motion state.

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

- Fixed simulation tick: 60 Hz.
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
Authoritative replication input is carried by Sidereal's authenticated realtime input lane; Lightyear native input remains client-local prediction support and native-client protocol compatibility, not the server's authoritative input source.
Authoritative realtime input snapshots are short-lived: the replication server expires them after `REPLICATION_REALTIME_INPUT_TIMEOUT_SECONDS` (default `0.35s`) so stale held input cannot persist across focus loss or background throttling.

2026-04-24 update: `ClientRealtimeInputMessage` carries the server-issued `control_generation` lease observed by the client. The replication server rejects realtime input whose generation does not match the currently authoritative player lease so delayed packets from a previous controlled entity cannot apply intent to a newly controlled target.

2026-04-27 update: native clients keep Lightyear native input for local `ActionState<PlayerInput>` and rollback replay history, but Lightyear input-based rollback is disabled because replication does not run Lightyear's native server input receiver. Authoritative reconciliation is server state rollback/correction from Sidereal realtime input.

2026-04-27 update: `ServerSessionReadyMessage` includes the server's current `control_generation` and authoritative controlled target. A reconnecting client must adopt that lease before sending realtime input; otherwise the server will reject input as stale generation.

2026-04-27 update: realtime input snapshots are discarded when they expire, carry a stale control generation, or target an entity the player no longer controls. Client disconnect notify also clears per-player realtime input tick/rate/latest-state resources. This keeps latest-intent state from surviving disconnects or control handoffs as reusable input residue.

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

2026-04-27 update: self-controlled player anchors may be non-physics entities with `LinearVelocity` but no Avian `RigidBody`. Shared fixed-step player movement must therefore integrate their `Position`/`Transform` directly, while physics-backed controlled entities continue to leave integration to Avian.

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
- There is no hidden `ShipTag` or player-observer baseline visibility floor. Visibility-range capability must be authored explicitly in data/components.
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
  - owned/visibility-range-derived authorization coverage.
- Keep explicit performance telemetry for visibility queries:
  - candidates per frame,
  - included entities per frame,
  - query time budget per client.

### 7.8 Scale and Control-Swap Readiness (Current vs Target)

**Is the current network system robust enough for thousands of entities, multiple owners, and players swapping which ship they control?**

**No.** The current implementation is suitable for small sessions (handful of players, tens of entities). The following gaps must be closed for the target scale and control model.

**Visibility and scale**

- **Current runtime modes:** `update_network_visibility` has a pluggable candidate stage:
  - default: `spatial_grid` (uniform-grid candidate preselection + policy exception bypass paths),
  - fallback: `full_scan` (O(clients × entities), useful for debug/validation).
- **Safety rule:** candidate preselection is optimization-only; ownership/public/faction/scanner exceptions must still be considered even if an entity misses candidate preselection.
- **Target (see 7.7):** move production/default operation to spatial indexing (and later LOD/culling tiers) with telemetry-backed tuning.

**Observer and scanner aggregation**

- **Current:** One observer position per client from persisted player runtime camera state (`position_m`/`Transform.translation` on the player entity), with visibility-source union over owned entities.
- **Target (see 7.2):** Keep scanner authorization aggregated over *all* owned entities (e.g. all ships the player owns), with observer/visibility logic supporting multiple observer points or equivalent aggregated coverage per client.

**Control swap (player changes which ship they control)**

- **Current:** Implemented via persisted `controlled_entity_guid` on the player entity. Client sends `ClientControlRequestMessage { player_entity_id, controlled_entity_id, request_seq }`; server validates ownership and updates `ControlledBy` plus `PlayerControlledEntityMap`.
- **Rule:** Control handoff is explicit request/response:
  - success: `ServerControlAckMessage { player_entity_id, request_seq, control_generation, controlled_entity_id }`,
  - failure: `ServerControlRejectMessage { player_entity_id, request_seq, control_generation, reason, authoritative_controlled_entity_id }`.
  Client clears pending control only on matching ack/reject. Free-roam is self-control (`controlled_entity_guid = player guid`), not null control.
  `control_generation` is the server-issued lease generation for the currently authoritative target; clients must key bootstrap/handoff state off that generation instead of inventing a local sequence from clone discovery alone.
  Realtime input must include the same `control_generation`; stale-generation input is rejected before it can update or drain from the latest-input snapshot.
  2026-04-27 update: a hydrated authoritative control lease is generation 1 before the first client handoff request; the first target change after startup must therefore acknowledge generation 2, and handoff updates write a targeted player-entity persistence snapshot for `controlled_entity_guid`. Graph persistence treats `last_tick` as a stale-write guard so older queued snapshots cannot overwrite newer control state.
  2026-04-27 update: graph schema/write operations are serialized in-process because Apache AGE graph mutations are not safe for concurrent targeted control snapshots and broad world snapshots against the same graph. Critical control persistence must not spawn a stampede of simultaneous AGE writes.
- **Camera/anchor contract:** camera always follows the player entity. When controlled target is not self, server continuously anchors player transform to the controlled entity.

**Multiple ships per player**

- **Current:** A player can own multiple ships; only the server-authoritative controlled ship receives input. Player can switch control among owned ships; missing targets resolve to player self-control.
- **Target:** Keep this model while extending scanner/visibility to aggregate over all owned entities at scale.

**Summary**

- **Thousands of entities, multiple players:** Not robust under `full_scan`; use `spatial_grid` (then next-stage index/LOD improvements) for large sessions as described in 7.5–7.7.
- **Players swapping between ships they own:** Implemented with server-side ownership validation and persisted player runtime state.

## 8. Auth and Session Identity

- Gateway owns auth lifecycle (`register/login/refresh/reset`).
- 2026-04-26 target update: gateway auth is being expanded to own dashboard account sessions, SMTP-backed password reset/email login, TOTP MFA, scoped JWT claims, refresh-token rotation, JWKS, and explicit character creation. The implementation plan is `docs/plans/gateway_dashboard_auth_character_flow_plan_2026-04-26.md`, with decision detail `docs/decisions/dr-0036_gateway_account_auth_dashboard_and_character_creation.md`.
- 2026-04-26 implementation update: gateway v1 email login now supports one-time code and magic-link token requests/verification, v1 password reset request delivers by email without returning raw reset tokens, and delivery can run in `GATEWAY_EMAIL_DELIVERY=noop|log|smtp` mode. SMTP mode requires `GATEWAY_SMTP_RELAY`, `GATEWAY_SMTP_USERNAME`, `GATEWAY_SMTP_PASSWORD`, and `GATEWAY_SMTP_FROM`; challenge links use `GATEWAY_PUBLIC_BASE_URL`, and delivery throttles use `GATEWAY_EMAIL_RESEND_COOLDOWN_S` plus `GATEWAY_EMAIL_MAX_PER_EMAIL_PER_HOUR`.
- 2026-04-26 implementation update: gateway v1 TOTP enrollment supports `/auth/v1/mfa/totp/enroll` and `/auth/v1/mfa/totp/verify`, returning a provisioning URI and QR SVG and storing encrypted pending/verified TOTP secrets. `GATEWAY_AUTH_SECRET_KEY_B64` should be a 32-byte base64 key in production; `GATEWAY_TOTP_ISSUER`, `GATEWAY_TOTP_STEP_S`, `GATEWAY_TOTP_DIGITS`, `GATEWAY_TOTP_ALLOWED_DRIFT_STEPS`, and `GATEWAY_TOTP_ENROLLMENT_TTL_S` configure the primitive.
- 2026-04-26 implementation update: gateway v1 password login returns a persisted TOTP login challenge for accounts with verified TOTP, `/auth/v1/login/challenge/totp` consumes successful challenges, and issued access tokens include defaulted `scope` plus `session_context` claims. TOTP-authenticated tokens set `auth_method=password_totp`, `mfa_verified=true`, and `mfa_methods=["totp"]`.
- 2026-04-26 implementation update: native/WASM game-client login uses the v1 password/TOTP challenge flow. Legacy `/auth/login` must not issue direct tokens for accounts with verified TOTP.
- 2026-04-26 implementation update: game-client auth UI is login-only. Registration and password reset request/confirm are dashboard web flows; native clients open dashboard `/forgot-password` via `SIDEREAL_DASHBOARD_URL` (default `http://127.0.0.1:3000`). Legacy gateway `/auth/password-reset/*` routes are removed in favor of `/auth/v1/password-reset/*`.
- 2026-04-26 implementation update: `/auth/v1/mfa/totp/verify` returns fresh MFA-verified tokens after enrollment, and dashboard `/mfa-setup` uses those tokens to let newly bootstrapped admin/dev accounts complete authenticator setup before entering guarded dashboard routes.
- 2026-04-26 implementation update: gateway stores account roles/scopes in `auth_account_roles` and `auth_account_scopes`; issued access tokens include persisted roles, a space-delimited `scope` string, and `session_context.active_scope`. Gateway admin spawn and script-management endpoints now require admin/dev role, verified MFA, and route-specific scopes (`admin:spawn`, `scripts:read`, `scripts:write`).
- 2026-04-26 implementation update: dashboard `/login` proxies gateway login/register/TOTP challenge completion and stores gateway tokens in an encrypted HttpOnly `sidereal_dashboard_auth` cookie using `SIDEREAL_DASHBOARD_SESSION_SECRET`. The pathless dashboard route uses a TanStack Router `beforeLoad` auth guard, and dashboard API handlers now use the gateway-backed admin/MFA/scope guard.
- 2026-04-26 implementation update: first administrator setup is gateway-owned through `/auth/v1/bootstrap/status` and `/auth/v1/bootstrap/admin`. Bootstrap requires `GATEWAY_BOOTSTRAP_TOKEN`, is eligible only while the database has no bootstrap state and no admin/dev role, records completion in `auth_bootstrap_state`, and is surfaced by dashboard `/setup` before normal login.
- 2026-04-26 implementation update: authenticated dashboard root `/` is now the `My Account` character-selection surface. Account character list/create/delete/reset operations are gateway-owned and documented with the reusable layout contract in `docs/features/account_character_selection_layout_contract.md`.
- 2026-04-26 implementation update: regular authenticated dashboard users may access only `/` for My Account character management. All other dashboard tool routes remain admin-only and require admin/dev/developer role, verified MFA, `dashboard:access`, and route-specific scopes where applicable.
- 2026-04-26 implementation update: gateway world entry now returns fresh character-scoped tokens whose `player_entity_id` claim and `session_context.active_character_id` match the selected character. The native client uses those tokens for replication auth and renders character display names from gateway character summaries in the character-select roster.
- Registration creates account/auth state only; it must not create a default character or starter-world graph records after the `DR-0036` migration lands.
- Explicit character creation creates and persists the account-owned character/player entity and starter graph records in durability storage.
- Public dashboard/web registration is the account creation surface; the game client supports login and character selection/creation, but not public account registration.
- Dashboard admin access uses gateway account sessions with admin/dev role, route-specific scopes, and verified MFA. The legacy standalone dashboard admin password is superseded by the `DR-0036` target.
- Register/login are auth-only and must not implicitly bind a runtime world session.
- Runtime bootstrap handoff from gateway to replication is explicit `Enter World` behavior and must be idempotent per `player_entity_id`.
- Runtime replication auth must use the character-scoped token returned by `Enter World`, not the account login token.
- `Enter World` requests must ensure runtime presence/bind for the selected character on every reconnect attempt; idempotency must not prevent reconnect rebind when runtime entities are missing.
- Player-specific runtime/persistent data is player-entity scoped. Authoritative control state persists via `controlled_entity_guid` on the player entity; score, quest progression, and other character-local settings persist on the player entity in graph persistence.
- Account identity is an auth container and external reference. An account may own multiple player entities (characters); `player_entity_id` selects which character/session identity is bound for runtime control.
- Replication binds session transport identity to authenticated `player_entity_id`.
- 2026-04-28 update: replication auth fails closed when the selected player entity is missing its persisted `AccountId` binding or the binding does not match the authenticated account subject. Replication must reject the session with `ServerSessionDeniedMessage` rather than patching account ownership at runtime, because missing bindings are persistence/hydration faults and must not be hidden during session bind.
- Replication must validate character-scoped world tokens for session bind. The `DR-0036` target replaces shared symmetric gateway JWT secret validation with asymmetric JWT/JWKS validation.
- Replication auth denial must be explicit: invalid player ids, rejected tokens, ownership mismatches, and temporarily unavailable player runtime entities return `ServerSessionDeniedMessage` rather than only logging and dropping the request.
- Client world entry state transition is `Auth -> CharacterSelect -> WorldLoading -> AssetLoading -> InWorld`; replication session-ready bind acknowledgment for the selected `player_entity_id` is the gate that starts bootstrap-required asset validation/download, and transition to `InWorld` occurs only after session-ready, required asset validation/download, and replicated player-entity presence on client.
- Input packets with mismatched identity claims are rejected.
- Gameplay control selection remains ownership-authorized.
- Runtime systems must fail closed on ownership/identity mismatches (reject and log) rather than silently creating replacement state.

## 9. Asset Delivery

- Asset definitions are authored in Lua runtime asset registry scripts and compiled into authoritative catalog metadata.
- Rust runtime code must not hardcode concrete gameplay asset IDs, filenames, shader names, material names, sprite names, or audio names.
- Each published asset version has an immutable generated `asset_guid`; payload download route is authenticated gateway HTTP `GET /assets/<asset_guid>`.
- Client startup receives server-authoritative asset manifest metadata (required assets and optional full catalog) including `asset_id`, `asset_guid`, checksum, and fetch URL.
- Client world entry lifecycle includes a dedicated `AssetLoading` state between `WorldLoading` and `InWorld`; required assets must validate/download before `InWorld`.
- Runtime missing assets are fetched lazily by `asset_guid` when new `asset_id` references appear in replicated data.
- Target cache shape remains `assets.pak` + `assets.index`; rollout and schema details are tracked in `docs/features/asset_delivery_contract.md`.
- Missing assets must fail soft (no gameplay crash).
- Shader asset metadata should evolve toward domain/signature/schema compatibility metadata rather than singleton hard-coded runtime role dispatch; details live in `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md` and `docs/features/asset_delivery_contract.md`.

## 10. Client Platform Model

- One client crate (`bins/sidereal-client`) with:
  - native `[[bin]]` target,
  - WASM `[lib]` target.
- Platform branching is `cfg(target_arch = "wasm32")` only.
- Native and WASM gameplay behavior stay in lockstep; transport adapters are platform-specific boundary code.
- Native renderer backend selection uses `SIDEREAL_CLIENT_WGPU_BACKENDS` first, then `WGPU_BACKEND`, then defaults to `PRIMARY` backends (`VULKAN | METAL | DX12 | BROWSER_WEBGPU`) when unset.
- Native client startup does not perform multi-instance lock/tracking; separate local client processes are treated identically.
- `SIDEREAL_CLIENT_FORCE_SOFTWARE_ADAPTER` is explicit opt-in only (`1`/`true` forces software adapter; unset or `0`/`false` keeps hardware adapter selection).
- Native primary window is user-resizable with enforced minimum logical size `960x540`; resize/minimize transitions treat non-positive viewport dimensions as non-renderable for fullscreen backdrop/material uniform updates.
- 2026-03-11 update: native client runtime configuration now supports command-line overrides as well as environment variables. `sidereal-client --help` is the canonical discovery surface for native launch options, and CLI flags take precedence over env vars for the current process.
- 2026-03-11 update: env-driven debug toggles and diagnostic kill-switch startup flags were removed from the native client startup surface. Native startup config is now limited to real transport/render/bootstrap/runtime tuning inputs rather than debug-only launch switches.
- 2026-03-11 update: native client bootstrap now initializes runtime resources by domain (transport, asset runtime, control/prediction, diagnostics, tactical/UI, scene/render), and shared replication/control scheduling is composed once before headless-vs-interactive divergences are applied. This keeps entrypoint ownership closer to documented domain boundaries without introducing a native-only runtime fork.
- 2026-03-14 update: native windowed client now runs the Bevy/winit event loop in continuous mode even while unfocused. Native impact: alt-tabbed local clients keep ticking prediction/replication/UI maintenance instead of dropping into Bevy's default low-power unfocused mode, which reduces oversized rollback aborts and stale delta-cache mismatches after refocus. WASM impact: no WASM impact; browser focus/background throttling behavior remains platform-managed.

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

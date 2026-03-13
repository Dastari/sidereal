# Rust Codebase Audit Report

Date: March 7, 2026
Scope: Rust workspace only
Prompt source: `docs/audits/rust_codebase_audit_prompt.md`

## 1. Executive Summary

The codebase has a coherent server-authoritative core, but several active runtime paths still violate the repo's own contracts. The biggest issues are not style-level problems; they are contract breaches:

1. WASM parity is not real yet. The WASM client is a render-only scaffold while the docs and contributor contract require one co-maintained runtime.
2. `bevy_remote` auth is configured but not enforced. The token is validated for presence, then discarded into an unused resource while the HTTP endpoint is still exposed.
3. Asset delivery is still partly implemented as a transitional local-file/runtime-fallback system rather than the documented generated-catalog + immutable payload pipeline.
4. Visibility and scanner logic still contain ship-specific baseline behavior that conflicts with the generic-entity contract.
5. The client bootstrap path can force `AssetLoading` complete after timeout/stall, which contradicts the documented loading barrier.

The architecture is still defensible overall in a few places:

- Shared gameplay logic in `crates/sidereal-game` is the right direction and materially better than duplicating movement/physics rules across runtimes.
- Graph-record persistence and shared hydration helpers are aligned with the stated persistence model.
- Input/session binding on the replication side is stricter than many early multiplayer codebases and should be kept.
- Visibility stage ordering is mostly explicit and the code shows deliberate fail-closed intent.

## 2. Architecture Findings

### Finding A1: WASM parity contract is currently broken
- Severity: Critical
- Classification: architecture, correctness, maintainability
- Priority: must fix
- Why it matters:
  The repo contract says native and WASM are co-maintained and share the runtime except at the transport boundary. The actual WASM target does not include auth flow, prediction, replication, assets, UI state, or shared gameplay/plugin composition. That makes every client change effectively native-first in practice.
- Evidence:
  - `AGENTS.md:44`
  - `AGENTS.md:45`
  - `AGENTS.md:58`
  - `bins/sidereal-client/src/platform/wasm.rs:6`
  - `bins/sidereal-client/src/platform/wasm.rs:15`
  - `bins/sidereal-client/Cargo.toml:16`
  - `bins/sidereal-client/Cargo.toml:27`
- Details:
  `wasm.rs` only boots `DefaultPlugins` plus `RenderPlugin` and logs `"wasm scaffold booted"`. The WASM target dependencies also omit `sidereal-game`, `sidereal-net`, `sidereal-runtime-sync`, `sidereal-asset-runtime`, and Lightyear. This is a direct code/doc divergence, not an inference.
- Recommendation:
  Collapse the client into one shared runtime plugin stack and move only the transport/file-fetch boundary behind `cfg(target_arch = "wasm32")`. If the repo intentionally no longer requires parity, the docs must be changed immediately, but the current codebase and AGENTS contract clearly say the opposite.

### Finding A2: `bevy_remote` is not actually auth-gated
- Severity: High
- Classification: security, correctness
- Priority: must fix
- Why it matters:
  The repo explicitly requires BRP inspection endpoints to be auth-gated. The current implementation only validates that a token exists in env, then stores it in a resource that nothing checks. If BRP is enabled on a non-loopback bind, this is an exposed inspection surface.
- Evidence:
  - `AGENTS.md:63`
  - `crates/sidereal-core/src/remote_inspect.rs:53`
  - `crates/sidereal-core/src/remote_inspect.rs:54`
  - `bins/sidereal-client/src/platform/native/remote.rs:15`
  - `bins/sidereal-client/src/platform/native/remote.rs:21`
  - `bins/sidereal-replication/src/replication/lifecycle.rs:67`
  - `bins/sidereal-replication/src/replication/lifecycle.rs:72`
- Details:
  The comment in `remote_inspect.rs` says security is enforced by the auth token requirement, but neither runtime attaches middleware/headers/checks to `RemoteHttpPlugin`. This finding is directly proven by the code I reviewed; I did not find any token enforcement path.
- Recommendation:
  Do not expose `RemoteHttpPlugin` directly without a gate in front of it. Either:
  1. bind strictly to loopback and remove the misleading auth-token contract, or
  2. front the endpoint with an authenticated Axum layer / reverse proxy / custom BRP adapter that actually validates the token.

### Finding A3: Asset delivery implementation still leaks authoring paths and local-file assumptions
- Severity: High
- Classification: architecture, correctness, maintainability
- Priority: must fix
- Why it matters:
  The asset contract says `source_path` is authoring-time only, must not cross client-facing runtime protocols, and payload delivery should come from generated immutable catalog metadata. The current gateway manifest sends `source_path` as `relative_cache_path`, and gateway payload serving still reads directly from the authoring tree.
- Evidence:
  - `docs/features/asset_delivery_contract.md:41`
  - `docs/features/asset_delivery_contract.md:87`
  - `docs/features/asset_delivery_contract.md:94`
  - `bins/sidereal-gateway/src/api.rs:303`
  - `bins/sidereal-gateway/src/api.rs:308`
  - `bins/sidereal-gateway/src/api.rs:342`
  - `bins/sidereal-gateway/src/api.rs:344`
  - `bins/sidereal-gateway/src/api.rs:461`
  - `bins/sidereal-gateway/src/api.rs:466`
- Details:
  This is not just an implementation detail mismatch. It means the client protocol currently exposes authoring layout, and the "catalog" is recomputed ad hoc by reading source files on request rather than serving a generated/published artifact.
- Recommendation:
  Introduce an actual published asset catalog artifact and published payload storage abstraction. The client manifest should carry runtime cache metadata, not authoring `source_path`. Remove direct source-tree streaming from the gateway path once the generated catalog exists.

### Finding A4: Client bootstrap can bypass the documented asset-loading barrier
- Severity: High
- Classification: correctness, architecture
- Priority: must fix
- Why it matters:
  The asset delivery contract says the client transitions to `InWorld` only after required assets validate. The watchdog currently flips bootstrap completion to true after timeout/stall in "degraded mode", which defeats the documented barrier and makes failures non-deterministic.
- Evidence:
  - `docs/features/asset_delivery_contract.md:156`
  - `docs/features/asset_delivery_contract.md:165`
  - `bins/sidereal-client/src/runtime/bootstrap.rs:91`
  - `bins/sidereal-client/src/runtime/bootstrap.rs:95`
  - `bins/sidereal-client/src/runtime/bootstrap.rs:189`
  - `bins/sidereal-client/src/runtime/bootstrap.rs:191`
  - `bins/sidereal-client/src/runtime/replication.rs:258`
- Details:
  `transition_asset_loading_to_in_world` trusts `AssetBootstrapRequestState.completed`, but the watchdog mutates `LocalAssetManager.bootstrap_phase_complete` to force forward progress. The user-facing dialogs acknowledge degraded mode, but the contract does not.
- Recommendation:
  Keep the watchdog, but change it to fail closed for required assets. Use degraded mode only for optional post-world assets, or update the docs if that policy is intentional. Right now the code and contract disagree.

## 3. Bevy / ECS Findings

### Finding B1: Visibility/scanner logic still has ship-only baseline behavior
- Severity: High
- Classification: architecture, maintainability
- Priority: must fix
- Why it matters:
  The contributor contract explicitly forbids ship-specific visibility assumptions. The runtime still uses `ShipTag` as an implicit scanner capability and adds a fixed ship scanner bonus.
- Evidence:
  - `AGENTS.md:37`
  - `bins/sidereal-replication/src/replication/visibility.rs:685`
  - `bins/sidereal-replication/src/replication/visibility.rs:686`
  - `bins/sidereal-replication/src/replication/visibility.rs:710`
  - `bins/sidereal-replication/src/replication/runtime_state.rs:11`
  - `bins/sidereal-replication/src/replication/runtime_state.rs:97`
  - `bins/sidereal-replication/src/replication/runtime_state.rs:127`
- Details:
  This is a direct code/doc divergence. The visibility code comments describe `ShipTag baseline scanner` as normative runtime behavior.
- Recommendation:
  Remove `ShipTag` from scanner derivation. Scanner capability should come only from generic components such as `ScannerComponent`, `ScannerRangeBuff`, and mount aggregation. If ships need a starter scanner, author it as a component bundle in Lua/bootstrap data.

### Finding B2: Client rendering and tactical/UI paths still embed game-specific asset IDs and rules
- Severity: Medium
- Classification: architecture, maintainability
- Priority: should fix
- Why it matters:
  The long-term direction is a generic runtime with content authored in Lua. The client still hardcodes space-specific shader IDs, fullscreen shader routing, asteroid generator IDs, and default ship icon fallbacks.
- Evidence:
  - `AGENTS.md:61`
  - `bins/sidereal-client/src/runtime/shaders.rs:8`
  - `bins/sidereal-client/src/runtime/shaders.rs:38`
  - `bins/sidereal-client/src/runtime/backdrop.rs:211`
  - `bins/sidereal-client/src/runtime/backdrop.rs:232`
  - `bins/sidereal-client/src/runtime/backdrop.rs:253`
  - `bins/sidereal-client/src/runtime/visuals.rs:62`
  - `bins/sidereal-client/src/runtime/visuals.rs:415`
  - `bins/sidereal-client/src/runtime/tactical.rs:292`
  - `bins/sidereal-client/src/runtime/ui.rs:483`
- Details:
  Some of this is clearly migration code, but it is still on active paths. The current client knows concrete asset IDs like `asteroid_wgsl`, `asteroid_texture_red_png`, `starfield_wgsl`, `space_background_wgsl`, and `map_icon_ship_svg`.
- Recommendation:
  Push this routing into data:
  1. render-layer/material schema declares the material family,
  2. asset catalog declares compatibility/signature metadata,
  3. entity/tactical presentation defaults come from authored data, not hardcoded fallback IDs.

### Finding B3: Large client/server runtime files are still accumulating mixed concerns
- Severity: Medium
- Classification: maintainability, architecture
- Priority: should fix
- Why it matters:
  The repo contract explicitly says large runtime refactors must split mixed concerns into domain modules. Several files are already large enough that review, ownership, and regression isolation are poor.
- Evidence:
  - `AGENTS.md:56`
  - `bins/sidereal-client/src/runtime/visuals.rs` (~1742 lines)
  - `bins/sidereal-client/src/runtime/backdrop.rs` (~1391 lines)
  - `bins/sidereal-client/src/runtime/ui.rs` (~1663 lines)
  - `bins/sidereal-client/src/runtime/auth_net.rs` (~948 lines)
  - `bins/sidereal-replication/src/replication/scripting.rs` (~1854 lines)
  - `bins/sidereal-replication/src/replication/visibility.rs` (~1350 lines)
  - `crates/sidereal-persistence/src/lib.rs` (~1388 lines)
- Details:
  This is not merely aesthetic. The large files mix orchestration, protocol mapping, data validation, ECS mutation, and feature-specific rules. That increases accidental coupling and makes it harder to enforce the generic/runtime split.
- Recommendation:
  Split by domain, not by arbitrary size:
  - client visuals: streamed sprites, planet visuals, weapon FX, fullscreen layers
  - client auth/assets: gateway auth, enter-world, bootstrap manifest, runtime lazy fetch
  - replication scripting: catalog sync, entity registry, asset registry, world-init config, Lua bindings
  - persistence: schema/bootstrap, graph entity persistence, relationship persistence, script catalog persistence

## 4. Rendering Findings

### Finding R1: Shader fallback strategy is still local-source driven and role-hardcoded
- Severity: Medium
- Classification: architecture, performance, maintainability
- Priority: should fix
- Why it matters:
  The current shader path undermines the "Lua-authored/generated catalog" direction. It also creates two parallel runtime sources of truth: streamed shader content and compiled-in `include_str!` WGSL.
- Evidence:
  - `bins/sidereal-client/src/runtime/shaders.rs:38`
  - `bins/sidereal-client/src/runtime/shaders.rs:108`
  - `bins/sidereal-client/src/runtime/shaders.rs:129`
  - `bins/sidereal-client/src/runtime/shaders.rs:150`
  - `bins/sidereal-client/src/runtime/shaders.rs:171`
- Details:
  Some fallback is reasonable, but the current implementation is still oriented around a fixed list of material roles known to Rust. That is defensible only as a short-lived migration layer. The codebase no longer treats it as obviously temporary.
- Recommendation:
  Keep a minimal "known generic material families" registry in Rust, but load the family-to-asset mapping entirely from the catalog/render-layer schema. Reduce hardcoded fallback WGSL to a single emergency shader per family, not per named content case.

## 5. Physics / Avian2D Findings

### Finding P1: No major Avian misuse was obvious in the authoritative core; this area is comparatively healthy
- Severity: Low
- Classification: architecture
- Priority: optional improvement
- Why it matters:
  Not every subsystem needs churn. The code generally respects fixed-tick simulation, uses Avian authoritative motion components, disables interpolation where appropriate on the server, and keeps gameplay systems scheduled around physics stages.
- Evidence:
  - `bins/sidereal-replication/src/main.rs:44`
  - `bins/sidereal-replication/src/plugins.rs:41`
  - `crates/sidereal-game/src/lib.rs:117`
  - `crates/sidereal-game/src/lib.rs:136`
- Details:
  This is one of the stronger parts of the codebase.
- Recommendation:
  Keep the current single-writer/fixed-step discipline. Focus refactor effort elsewhere first.

## 6. Networking / Lightyear Findings

### Finding N1: Input/session binding is defensible and should be kept
- Severity: Low
- Classification: architecture
- Priority: keep
- Why it matters:
  The replication input path correctly rejects mismatched claimed player IDs and normalizes controlled-entity routing around the authenticated binding. That is good authoritative hygiene.
- Evidence:
  - `bins/sidereal-replication/src/replication/input.rs:194`
  - `bins/sidereal-replication/src/replication/input.rs:224`
  - `bins/sidereal-replication/src/replication/input.rs:320`
  - `bins/sidereal-replication/src/replication/input.rs:372`
- Details:
  I did not find a stronger issue here than the already-documented mismatch-acceptance behavior for authoritative targets, which looks intentional and defensible as a robustness measure.
- Recommendation:
  Preserve the authenticated-session binding model while cleaning up adjacent client/WASM parity issues.

## 7. Scripting / Lua Findings

### Finding S1: Runtime scripting is powerful but still bundled with too many unrelated concerns
- Severity: Medium
- Classification: maintainability, architecture
- Priority: should fix
- Why it matters:
  `bins/sidereal-replication/src/replication/scripting.rs` is doing catalog load/persist/reload, Lua bundle parsing, render graph validation, asset registry ingestion, world defaults, and helper utilities in one place. That is too much responsibility for the file that defines live scripting behavior.
- Evidence:
  - `bins/sidereal-replication/src/replication/scripting.rs:1`
  - `bins/sidereal-replication/src/replication/scripting.rs` (~1854 lines)
- Details:
  This is less about raw length and more about mixing runtime scripting, content authoring contracts, persistence sync, and validation plumbing into one module.
- Recommendation:
  Split into:
  - `script_catalog`
  - `bundle_registry`
  - `asset_registry`
  - `world_init_config`
  - `lua_validation`
  - `spawn_graph_records`

## 8. Persistence / Data Flow Findings

### Finding D1: Cache implementation diverges from the documented MMO-style cache contract
- Severity: Medium
- Classification: architecture, maintainability
- Priority: should fix
- Why it matters:
  The contract says the client cache target shape is `assets.pak` + `assets.index` + `assets.tmp`. The code currently writes individual cached payload files under `data/cache_stream/<relative_cache_path>` and uses `index.json`.
- Evidence:
  - `docs/features/asset_delivery_contract.md:184`
  - `docs/features/asset_delivery_contract.md:197`
  - `AGENTS.md:62`
  - `crates/sidereal-asset-runtime/src/lib.rs:48`
  - `crates/sidereal-asset-runtime/src/lib.rs:49`
  - `bins/sidereal-client/src/runtime/auth_net.rs:353`
  - `bins/sidereal-client/src/runtime/assets.rs:107`
- Details:
  This is another direct code/doc divergence. The current implementation is simpler and acceptable as a temporary dev cache, but it is not the documented cache model.
- Recommendation:
  Either:
  1. implement the documented pak/index cache, or
  2. rewrite the contract to describe the actual current file-per-asset cache and mark pak/index as future work.

### Finding D2: Native client HTTP/gateway flows are built on blocking reqwest threads and cannot carry forward to parity cleanly
- Severity: Medium
- Classification: architecture, maintainability, performance
- Priority: should fix
- Why it matters:
  The client spins ad hoc OS threads for auth requests, enter-world, bootstrap asset download, and runtime asset fetches. That is workable for native prototypes, but it duplicates state-machine logic, does not map cleanly to WASM, and sidesteps Bevy task/runtime patterns.
- Evidence:
  - `bins/sidereal-client/src/runtime/auth_net.rs:145`
  - `bins/sidereal-client/src/runtime/auth_net.rs:313`
  - `bins/sidereal-client/src/runtime/auth_net.rs:376`
  - `bins/sidereal-client/src/runtime/assets.rs:293`
- Details:
  This is directly proven for the native path. The broader claim that it blocks clean WASM parity is partly inference, but it is a strong inference given the separate WASM scaffold and `reqwest::blocking` native-only dependency layout.
- Recommendation:
  Move gateway HTTP and runtime asset fetch orchestration behind a platform adapter built on async tasks/channels, with native and wasm transport/fetch backends sharing the same client state machine.

## 9. Redundancy / Dead Code Findings

### Finding X1: There are still placeholder and migration-style paths on active runtime codepaths
- Severity: Low
- Classification: cleanup, maintainability
- Priority: optional improvement
- Why it matters:
  The codebase still contains temporary comments and placeholder semantics in live modules, which is fine for short migrations but should not become permanent architecture.
- Evidence:
  - `bins/sidereal-client/src/runtime/visuals.rs:151`
  - `bins/sidereal-client/src/runtime/pause_menu.rs:1`
  - `bins/sidereal-replication/src/replication/assets.rs:1`
  - `bins/sidereal-client/src/platform/wasm.rs:16`
- Details:
  None of these are individually severe, but together they indicate transitional code sticking around longer than intended.
- Recommendation:
  Track each migration shim with an owning issue or decision note and remove the ones that no longer pay for themselves.

## 10. Startup / Main Loop Flow Maps

### Gateway startup and main loop

Directly observed flow:

1. `bins/sidereal-gateway/src/main.rs` initializes tracing.
2. Loads `AuthConfig` and connects `PostgresAuthStore`.
3. Ensures auth/persistence schema.
4. Chooses bootstrap dispatcher from `GATEWAY_BOOTSTRAP_MODE`:
   - UDP handoff via `UdpBootstrapDispatcher`, or
   - direct bootstrap dispatcher.
5. Constructs `AuthService`.
6. Binds Axum listener and serves routes from `app_with_service`.

Operational loop:

1. Axum handles auth routes (`/auth/register`, `/auth/login`, `/auth/refresh`, reset routes).
2. `/auth/me` and `/auth/characters` resolve authenticated account/character state.
3. `/world/enter` validates ownership and dispatches bootstrap command to replication.
4. `/admin/spawn-entity` validates admin/dev role and forwards server-side spawn command.
5. `/admin/scripts/*` exposes script catalog workflows.
6. `/assets/bootstrap-manifest` and `/assets/{asset_guid}` serve asset metadata/payloads.

Inference:
The gateway is currently both auth boundary and temporary asset-catalog materializer. That second role looks transitional rather than final architecture.

### Replication server startup and main loop

Directly observed flow:

1. `bins/sidereal-replication/src/main.rs` creates a Bevy `App` with `MinimalPlugins` and `ScheduleRunnerPlugin`.
2. Adds `AssetPlugin`, `ScenePlugin`, logging, `SiderealGamePlugin`, Avian physics, Lightyear server plugins, and protocol registration.
3. Disables hierarchy rebuild on the server runtime.
4. Forces fixed time to 60 Hz.
5. Initializes resources for visibility, simulation entities, auth, persistence, control, runtime scripting, owner manifest, tactical lane, and lifecycle.
6. Registers plugins for lifecycle, auth, input, control/combat, visibility, persistence, runtime scripting, and bootstrap bridge.

Operational loop:

1. `Startup` hydrates simulation entities and starts the Lightyear UDP server.
2. `Startup` also starts replication control listener for bootstrap/admin commands.
3. `Update` drains transport/control/input/auth/persistence metrics and bootstrap command ingestion.
4. `FixedUpdate` runs:
   - authoritative gameplay and physics-prep systems from `sidereal-game`,
   - server input routing,
   - control/combat side effects,
   - runtime scripting intent application,
   - visibility/tactical/owner-manifest streaming,
   - persistence dirty marking and flush.

### Client startup and main loop

Native path, directly observed:

1. `bins/sidereal-client/src/main.rs` dispatches to `platform::native::run()`.
2. `platform::native::run()` builds either:
   - a headless minimal app, or
   - a full Bevy app with window/render/material/audio/UI plugins.
3. Adds Avian physics, `SiderealGameCorePlugin`, Lightyear client plugins, Lightyear Avian plugin, protocol registration, remote-inspect config, fixed time, and a large set of client resources.
4. Registers client plugins for bootstrap, transport, replication adoption, prediction, visuals, UI, cameras, controls, tactical map, and logout.

Native operational loop:

1. `Auth` state handles gateway login/register/reset.
2. `CharacterSelect` chooses `player_entity_id`.
3. `WorldLoading` starts Lightyear transport and waits for session-ready + player entity presence.
4. `AssetLoading` waits for bootstrap asset flow.
5. `InWorld` runs prediction, replication adoption, transforms, controls, visuals, tactical UI, owner manifest, and rendering.

WASM path, directly observed:

1. `bins/sidereal-client/src/platform/wasm.rs` only boots Bevy render plugins and logs a scaffold message.
2. No auth, no transport, no asset state machine, no gameplay runtime, no UI state machine.

### Data / authority / persistence / replication / asset delivery / scripting / rendering flow

Current flow, combining direct evidence with limited inference:

1. Auth/account lifecycle begins at gateway.
2. Gateway validates JWT/account ownership and issues bootstrap command for a specific `player_entity_id`.
3. Replication receives bootstrap/admin control command, hydrates or spawns graph-backed runtime entities, and binds authenticated client session to canonical player identity.
4. Client sends intent only through Lightyear input/control channels.
5. Replication runs authoritative gameplay, Avian physics, visibility, tactical/owner lanes, and persistence staging.
6. Persistence stores authoritative graph entity/component records and relationships in Postgres/AGE.
7. Asset authority currently originates from Lua asset registry data, but gateway still derives runtime catalog entries directly from source files at request time.
8. Client asset bootstrap and lazy runtime fetch both use gateway HTTP, then local cache + runtime shader/visual attachment.
9. Runtime scripting on replication loads Lua handlers, snapshots a script-visible world view, emits intents/events, and feeds those intents back into authoritative Rust systems.
10. Client rendering is mostly data-driven at the component level, but many material/shader/content dispatch decisions remain hardcoded in Rust.

## 11. Prioritized Remediation Plan

1. Fix the BRP security hole first. Either auth-gate it for real or remove the false claim that it is gated.
2. Re-establish an honest client platform model. Either implement shared native/WASM runtime parity or explicitly downgrade the docs and contracts. The current middle state is misleading.
3. Normalize asset delivery against the contract:
   - stop leaking `source_path`,
   - stop rebuilding catalog from source files on request,
   - choose either documented pak/index cache or documented file-per-asset cache,
   - remove hardcoded asset/shader identifiers from active runtime paths.
4. Remove ship-specific scanner baseline behavior from visibility/runtime-state and move any default scanner behavior into authored components/bundles.
5. Delete the forced-success bootstrap path for required assets or update the contract to match it.
6. Split the largest runtime modules by domain before adding more features to them.
7. Replace native blocking thread-based HTTP helpers with a platform-adapted shared async/task layer that can be used by both native and WASM.

## 12. Final Assessment

The project has a solid authoritative simulation spine, but the repo is carrying too many "temporary but active" client/content/runtime shims at once. The main risk is not that the current native path fails immediately; the risk is that more features will continue to accumulate on top of mismatched contracts, especially around WASM parity, asset delivery, and data-driven content boundaries.

## 13. Workspace / Runtime Catalog Appendix

This appendix is intentionally responsibility-focused. It catalogs the active workspace pieces and the main Bevy runtime plugins, systems, and resources rather than attempting to enumerate every helper symbol.

### 13.1 Workspace binaries, crates, and libraries

- `bins/sidereal-gateway`
  Responsibility: HTTP gateway for auth, account/character lifecycle, explicit world entry, admin spawn/script endpoints, and current asset manifest/payload serving.
  Classification: active runtime.

- `bins/sidereal-replication`
  Responsibility: authoritative simulation host, Lightyear server, visibility, control/input routing, tactical/owner lanes, runtime scripting integration, and persistence staging.
  Classification: active runtime.

- `bins/sidereal-client`
  Responsibility: native game client runtime plus wasm entrypoint.
  Classification: active runtime, but wasm path is currently scaffold/placeholder.

- `crates/sidereal-game`
  Responsibility: shared gameplay component registry, core gameplay systems, mass/flight/combat/hierarchy logic, render-layer validation helpers.
  Classification: active shared gameplay library.

- `crates/sidereal-net`
  Responsibility: shared protocol types, channels, messages, IDs, and Lightyear registration wiring.
  Classification: active shared networking library.

- `crates/sidereal-persistence`
  Responsibility: graph persistence model, AGE/Postgres persistence helpers, script catalog persistence, graph entity/component records.
  Classification: active shared persistence library.

- `crates/sidereal-runtime-sync`
  Responsibility: hydration/serialization glue between persisted graph records and runtime ECS entities/components, runtime entity hierarchy maps.
  Classification: active shared runtime-sync library.

- `crates/sidereal-scripting`
  Responsibility: Lua sandbox setup, script loading/validation helpers, Lua asset registry parsing, JSON/Lua conversion utilities.
  Classification: active shared scripting library.

- `crates/sidereal-core`
  Responsibility: auth claims, gateway DTOs, bootstrap wire payloads, remote inspect config, shared low-level core types/helpers.
  Classification: active shared core library.

- `crates/sidereal-asset-runtime`
  Responsibility: client-side asset catalog/cache index data structures, checksum/version helpers, current cache-index file helpers.
  Classification: active shared asset library, current implementation still transitional relative to docs.

- `crates/sidereal-component-macros`
  Responsibility: gameplay component macro support for shared component registration/metadata generation.
  Classification: active build-time/library support.

- `crates/sidereal-shader-preview`
  Responsibility: shader-preview support crate in the workspace.
  Classification: tooling/support library.

### 13.2 Replication server Bevy plugins

- `ReplicationLifecyclePlugin`
  Responsibility: startup hydration, server startup, replication control listener bootstrap, connection/link observers.
  Classification: active runtime.

- `ReplicationAuthPlugin`
  Responsibility: receives and applies client auth/session binding messages.
  Classification: active runtime.

- `ReplicationInputPlugin`
  Responsibility: drains realtime input into authoritative action queues before physics preparation.
  Classification: active runtime.

- `ReplicationControlPlugin`
  Responsibility: control synchronization plus combat side effects that follow physics writeback.
  Classification: active runtime.

- `ReplicationVisibilityPlugin`
  Responsibility: controlled-transform sync, observer anchor updates, scanner range computation, visibility updates, tactical lane, owner manifest lane.
  Classification: active runtime.

- `ReplicationPersistencePlugin`
  Responsibility: persistence worker startup, dirty marking, simulation-state flush.
  Classification: active runtime.

- `ReplicationRuntimeScriptingPlugin`
  Responsibility: script snapshot refresh, interval/event execution, script intent application before authoritative gameplay systems.
  Classification: active runtime.

- `ReplicationBootstrapBridgePlugin`
  Responsibility: post-buffer application of pending control bindings between bootstrap/runtime state and replicated entities.
  Classification: active runtime.

### 13.3 Main replication systems and resources

- `lifecycle::start_lightyear_server`
  Responsibility: spawn and start the replication Lightyear UDP server entity.

- `lifecycle::ensure_server_transport_channels`
  Responsibility: ensure all required message channels exist on connected client transports.

- `lifecycle::ensure_entity_scoped_replication_groups`
  Responsibility: normalize default Lightyear replication groups to per-entity groups.

- `lifecycle::disconnect_idle_clients`
  Responsibility: disconnect dead/idle clients to stop wasting authoritative server fanout.

- `auth::receive_client_auth_messages`
  Responsibility: bind authenticated session identity to player entity and runtime client mapping.

- `input::receive_latest_realtime_input_messages`
  Responsibility: validate, rate-limit, canonicalize, and retain only the latest input per player.

- `input::drain_native_player_inputs_to_action_queue`
  Responsibility: move latest player intent into the authoritative `ActionQueue` of the current control target.

- `visibility::update_network_visibility`
  Responsibility: perform authorization/delivery visibility decisions and update network visibility state.

- `visibility::receive_client_local_view_mode_messages`
  Responsibility: ingest per-client view mode/range preferences.

- `runtime_state::update_client_observer_anchor_positions`
  Responsibility: maintain player observer-anchor world positions for visibility logic.

- `runtime_state::compute_controlled_entity_scanner_ranges`
  Responsibility: compute effective scanner range for controlled entities from scanner-capable modules and current baseline logic.

- `simulation_entities::*`
  Responsibility: bootstrap/hydration, controlled-entity transform sync, pending control bindings, planarity enforcement, player/runtime entity maps.

- `persistence::flush_simulation_state_persistence`
  Responsibility: stage authoritative runtime state to graph persistence.

- `runtime_scripting::*`
  Responsibility: build script world snapshot, run script handlers, and apply script intents.

- Major replication resources:
  - `ClientVisibilityRegistry`: client -> player visibility binding.
  - `VisibilityScratch`: per-tick visibility working set/cache.
  - `ClientObserverAnchorPositionMap`: player observer anchor positions.
  - `VisibilityRuntimeConfig`: runtime visibility settings.
  - `AuthenticatedClientBindings`: authoritative client/session/player binding.
  - `PlayerControlledEntityMap`: authoritative control target per player.
  - `PlayerRuntimeEntityMap`: player entity -> runtime ECS entity map.
  - `LatestRealtimeInputsByPlayer`: latest accepted input snapshot per player.
  - `PersistenceWorkerState` and related persistence state resources: background persistence orchestration.
  - `ScriptWorldSnapshot` and `ScriptEventQueue`: runtime scripting working state.

### 13.4 Client Bevy plugins

- `ClientBootstrapPlugin`
  Responsibility: state initialization, UI bootstrap, transport boot on world entry, world scene setup.
  Classification: active runtime.

- `ClientTransportPlugin`
  Responsibility: transport channel readiness, disconnect handling, auth/session-ready/session-denied transport message flow.
  Classification: active runtime.

- `ClientReplicationPlugin`
  Responsibility: replicated entity adoption, transform sync, asset lazy fetch polling, control/view sync, owner manifest and tactical intake, world-loading transitions.
  Classification: active runtime.

- `ClientPredictionPlugin`
  Responsibility: input send path plus local predicted action application for the controlled entity.
  Classification: active runtime.

- `ClientVisualsPlugin`
  Responsibility: runtime render-layer sync, streamed visuals, planet visuals, thruster visuals, tracer/spark effects.
  Classification: active runtime.

- `ClientLightingPlugin`
  Responsibility: world and local-light collection/update paths for runtime visuals.
  Classification: active runtime.

- `ClientUiPlugin`
  Responsibility: auth UI, character select, asset loading UI, tactical/owner/debug overlays, pause/logout and interaction flow.
  Classification: active runtime.

- `ClientDiagnosticsPlugin`
  Responsibility: diagnostics/debug-oriented client runtime support.
  Classification: active runtime/tooling support.

### 13.5 Main client systems and resources

- `transport::ensure_lightyear_client_system`
  Responsibility: spawn/recreate the Lightyear client transport entity.
  Classification: active runtime, native-specific today.

- `transport::ensure_client_transport_channels`
  Responsibility: ensure all required Lightyear channels are present on the client transport.

- `auth_net::poll_gateway_request_results`
  Responsibility: advance auth/enter-world state machine from gateway HTTP responses.

- `auth_net::poll_asset_bootstrap_request_results`
  Responsibility: apply asset bootstrap manifest/download results into client asset state.

- `input::send_lightyear_input_messages`
  Responsibility: emit canonical per-tick realtime input messages from local player intent.

- `replication::adopt_native_lightyear_replicated_entities`
  Responsibility: convert replicated entities into local runtime entity categories and registry entries.
  Classification: active runtime, naming indicates native-specific/transitional shape.

- `replication::transition_world_loading_to_in_world`
  Responsibility: move from `WorldLoading` to `AssetLoading` once session-ready/player-presence conditions are met.

- `replication::transition_asset_loading_to_in_world`
  Responsibility: move from `AssetLoading` to `InWorld` once bootstrap completes.

- `assets::queue_missing_catalog_assets_system`
  Responsibility: discover referenced but missing assets and start background runtime fetches.
  Classification: active runtime, native-specific implementation today.

- `assets::poll_runtime_asset_http_fetches_system`
  Responsibility: complete runtime lazy fetches and update local cache/runtime shader state.

- `bootstrap::watch_in_world_bootstrap_failures`
  Responsibility: bootstrap watchdog, timeout/stall/error dialogs, current degraded-mode behavior.

- `visuals::*`
  Responsibility: attach/update streamed visuals, planet layers, thruster plumes, tracers, impact sparks, duplicate suppression.

- `backdrop::*`
  Responsibility: fullscreen layers, post-process renderables, starfield/space background materials.

- Major client resources:
  - `ClientSession`: current gateway/auth/session-facing client state.
  - `CharacterSelectionState`: selected/available character state.
  - `SessionReadyState`: current session-ready binding.
  - `LocalPlayerViewState`: controlled entity, selected entity, detached camera state.
  - `LocalAssetManager`: asset bootstrap/catalog/cache readiness state.
  - `RuntimeAssetHttpFetchState`: in-flight runtime asset fetches.
  - `BootstrapWatchdogState`: watchdog state for bootstrap stall/failure detection.
  - `DeferredPredictedAdoptionState`: controlled predicted adoption delay tracking.
  - `OwnedAssetManifestCache`: cached owner-manifest read model.
  - `TacticalFogCache` and `TacticalContactsCache`: client tactical read models.
  - `RuntimeEntityHierarchy`: runtime entity GUID -> ECS entity mapping.
  - `RemoteEntityRegistry`: client-side registry of adopted remote/runtime entities.

### 13.6 Shared gameplay Bevy plugins, systems, and resources

- `SiderealGameCorePlugin`
  Responsibility: register gameplay component/reflection metadata only.
  Classification: active shared runtime support.

- `SiderealGamePlugin`
  Responsibility: full gameplay plugin for authoritative runtime, including system wiring for flight, combat, mass, hierarchy rebuild, and player-follow logic.
  Classification: active shared gameplay runtime.

- Main shared gameplay systems:
  - `process_character_movement_actions`: apply character movement intent.
  - `process_flight_actions`: apply flight-computer action interpretation.
  - `apply_engine_thrust`: authoritative thrust/torque application.
  - `recompute_total_mass`: recompute mass from gameplay state.
  - `sync_mounted_hierarchy`: deterministic mount/hierarchy rebuild.
  - `bootstrap_ship_mass_components`: mass bootstrap for current ship-tagged entities.
  - `bootstrap_collision_profiles_from_aabb`: collision-profile bootstrap helper.
  - `bootstrap_root_dynamic_entity_colliders`: collider bootstrap helper.
  - `bootstrap_weapon_cooldown_state`, `tick_weapon_cooldowns`, `process_weapon_fire_actions`, `resolve_shot_impacts`, `apply_damage_from_shot_impacts`: current shared combat loop.
  - `stabilize_idle_motion`, `clamp_angular_velocity`, `sync_player_to_controlled_entity`: post-physics motion stabilization/follow behavior.

- Shared gameplay resources:
  - `GeneratedComponentRegistry`: source-of-truth generated gameplay component metadata.
  - `HierarchyRebuildEnabled`: gate for local Bevy hierarchy rebuild in a given runtime.

### 13.7 Tooling, tests, and transitional pieces worth calling out

- `bins/sidereal-client/src/platform/wasm.rs`
  Responsibility: current wasm scaffold entry only.
  Classification: scaffold/placeholder.

- `bins/sidereal-replication/src/replication/assets.rs`
  Responsibility: no-op replication asset init hook left in place for wiring stability.
  Classification: transitional/migration code.

- `bins/sidereal-client/src/runtime/dev_console.rs`
  Responsibility: desktop/client diagnostics and logging support.
  Classification: tooling-oriented active runtime support, partly native-specific.

- `bins/*/tests`, `crates/*/tests`
  Responsibility: unit/integration coverage for auth, lifecycle, protocol, persistence, gameplay, and visibility paths.
  Classification: test-only.

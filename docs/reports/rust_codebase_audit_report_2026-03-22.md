# Rust Codebase Audit Report

Date: 2026-03-22
Status: New audit report for current Rust workspace state.
Scope: Rust workspace only. Dashboard/TypeScript code was not audited except where Rust runtime contracts clearly depend on it.

## 1. Executive Summary

The repo has a defensible high-level direction: server authority is explicit, runtime identity is UUID-based, replication startup disables Bevy hierarchy leakage, and authenticated player binding checks in replication control are materially stronger than the typical "trust the claimed player id" shortcut. Those are worth keeping.

The main problems are not at the top-level architecture layer. They are in the active runtime edges where the implementation still carries content-specific assumptions, oversized hot-path modules, duplicated spatial/visibility work, and compatibility-era behavior that now conflicts with the repo's own rules. The most important issues are:

1. fixed-step gameplay code still reads generic `Time` instead of fixed-step time in an authoritative flight path,
2. multiple runtime services still infer "ship/player/asteroid/planet" semantics from labels, names, bundle classes, and generator ids rather than content-authored metadata/components,
3. the rendering and visibility paths are still too monolithic and heuristic-heavy for the stated long-term Lua-authored runtime direction.

## 2. Architecture Findings

### Finding A1: Authoritative flight thrust still reads generic `Time` in fixed simulation

- Severity: High
- Classification: correctness
- Priority: must fix
- Why it matters: `apply_engine_thrust` runs in `FixedUpdate`, but it reads `Res<Time>` instead of `Res<Time<Fixed>>`. The project contract explicitly says simulation/prediction math must use fixed-step time resources only. This is exactly the kind of drift that later breaks prediction, replayability, and cross-runtime parity.
- Exact refs:
  - `crates/sidereal-game/src/flight.rs:162-175`
  - `bins/sidereal-client/src/runtime/app_setup.rs:191-209`
  - `crates/sidereal-game/src/lib.rs:153-166`
- Evidence: `apply_engine_thrust` computes `dt` from `time.delta_secs()` while being scheduled in authoritative fixed-step pipelines on both client prediction and replication.
- Recommendation: change the system signature to `Res<Time<Fixed>>` and audit the rest of `sidereal-game` authoritative systems for the same rule. Make the fixed-time rule mechanically enforced by a small helper wrapper or lint/test pattern.

### Finding A2: Runtime still hardcodes ship/player semantics instead of using generic capability/content metadata

- Severity: High
- Classification: architecture
- Priority: must fix
- Why it matters: the docs and AGENTS rules repeatedly push generic entity terminology and Lua/content-authored behavior. The implementation still hardcodes "ship" in gateway starter-world creation, tactical classification, owner manifest classification, and health explorer rendering. That keeps Sidereal tied to one game shape and creates scattered logic that must be updated every time content evolves.
- Exact refs:
  - `bins/sidereal-gateway/src/auth/starter_world.rs:99-131`
  - `bins/sidereal-gateway/src/auth/starter_world.rs:171-179`
  - `bins/sidereal-replication/src/replication/tactical.rs:93-105`
  - `bins/sidereal-replication/src/replication/owner_manifest.rs:38-47`
  - `bins/sidereal-replication/src/replication/health.rs:840-919`
- Evidence:
  - Starter world rejects non-`"ship"` bundle classes and resolves control by looking for a `"Ship"` label.
  - Tactical and owner-manifest kinds are derived from `"Ship"`/`"Player"` labels.
  - Health/world-map rendering falls back to substring matching on `planet`, `star`, `sun`, and `asteroid`.
- Doc divergence: this conflicts with the repo rule to use generic entity terminology where behavior is not inherently ship-only, and with the long-term Lua-authored content direction in `docs/sidereal_design_document.md` and `docs/prompts/rust_codebase_audit_prompt.md`.
- Recommendation: move control-target resolution, tactical kind, manifest kind, and map glyph/icon classification onto explicit components or script-authored metadata. The gateway should ask for a control-capable entity, not a `"Ship"` label. Replication-side tactical/manifest/health code should consume canonical metadata, not infer from labels/names.

### Finding A3: Client shader/material resolution still uses content heuristics instead of authoritative catalog metadata

- Severity: High
- Classification: architecture
- Priority: should fix
- Why it matters: the client still decides asteroid and planet shader roles from generator ids and component presence, which is brittle and directly undermines the stated shader-family/Lua-authored asset direction. This creates hidden coupling between content naming and renderer behavior.
- Exact refs:
  - `bins/sidereal-client/src/runtime/shaders.rs:795-860`
  - `crates/sidereal-game/src/components/ballistic_weapon.rs:23-35`
  - `crates/sidereal-game/src/components/tactical_presentation_defaults.rs:42-56`
- Evidence:
  - asteroid shader selection keys off `procedural_sprite.generator_id == "asteroid_rocky_v1"`,
  - planet visual resolution keys off `planet_settings.is_some()`,
  - several default asset identities are still embedded directly in Rust.
- Doc divergence: this conflicts with `docs/features/asset_delivery_contract.md` and the design-doc rule that concrete asset definitions should be Lua/catalog-authored instead of hardcoded in Rust runtime code.
- Recommendation: make the runtime consume explicit asset catalog metadata for shader family/domain/role and icon defaults. The renderer should not infer content roles from generator ids or special-case component presence once the catalog is available.

### Finding A4: Mass bootstrap remains ship-tag-specific even though the mass system is otherwise generic

- Severity: Medium
- Classification: architecture
- Priority: should fix
- Why it matters: `recompute_total_mass` is written in a reasonably generic way, but the bootstrap path that ensures derived mass components exists only for `With<ShipTag>`. That means the codebase says "generic runtime mass" while the initialization path still means "ships only."
- Exact refs:
  - `crates/sidereal-game/src/mass.rs:232-266`
  - `crates/sidereal-game/src/lib.rs:140-156`
- Evidence: `bootstrap_ship_mass_components` explicitly queries `With<ShipTag>`.
- Recommendation: rename and generalize this to root dynamic entities that participate in runtime mass/inertia sync. If some entity families should be excluded, use an explicit component/rule for that instead of `ShipTag`.

## 3. Bevy / ECS Findings

### Finding B1: Active runtime modules are still monoliths in hot paths

- Severity: Medium
- Classification: maintainability
- Priority: should fix
- Why it matters: the repo already documents that large runtime refactors should be split into domain modules. The biggest active files remain large enough that ownership, scheduling, and regression reasoning are all harder than they need to be.
- Exact refs:
  - `bins/sidereal-replication/src/replication/visibility.rs` (3634 lines)
  - `bins/sidereal-client/src/runtime/visuals.rs` (3349 lines)
  - `bins/sidereal-client/src/runtime/ui.rs` (2608 lines)
  - `bins/sidereal-client/src/runtime/backdrop.rs` (2236 lines)
  - `bins/sidereal-gateway/src/auth/starter_world_scripts.rs` (1347 lines)
  - `crates/sidereal-persistence/src/lib.rs` (1519 lines)
- Evidence: these are all active runtime files, not dead tooling files.
- Recommendation: split by ownership boundary, not by arbitrary helper count. For example:
  - `visibility.rs`: cache prep, spatial index, landmark discovery, policy evaluation, membership streaming, metrics
  - `visuals.rs`: streamed sprite visuals, projectile effects, planet visuals, lifecycle cleanup, effect dispatch
  - `starter_world_scripts.rs`: script-catalog persistence, bundle loading, world-init decoding, runtime validation

### Finding B2: Legacy compatibility behavior is still present even though the repo now forbids compatibility shims

- Severity: Medium
- Classification: maintainability
- Priority: should fix
- Why it matters: the repo explicitly says early-development schema discipline is strict and not to add compatibility aliases/default backfills/migration shims. The codebase still carries both a no-op compatibility hook and a test that codifies backward-compatible envelope decoding.
- Exact refs:
  - `crates/sidereal-game/src/actions.rs:133-136`
  - `crates/sidereal-persistence/tests/envelope_codec.rs:6-12`
  - `crates/sidereal-persistence/tests/envelope_codec.rs:40-64`
- Evidence:
  - `validate_action_capabilities` is described as a legacy compatibility hook and does nothing.
  - `PayloadV1.stop_requested` uses `#[serde(default)]`, and the test explicitly asserts backward-compatible decode of missing fields.
- Doc divergence: this conflicts with the AGENTS rule that renamed/reshaped gameplay or script payloads should not carry compatibility shims and that local/dev databases should simply be reset.
- Recommendation: delete the no-op action hook unless a real validation system is about to land, and remove compatibility-oriented envelope tests from authoritative runtime protocols unless those protocols are intentionally versioned long-term.

## 4. Rendering Findings

### Finding R1: Rendering/runtime content defaults are still embedded in gameplay/runtime crates

- Severity: Medium
- Classification: architecture
- Priority: should fix
- Why it matters: the code still embeds content presets such as `corvette_ballistic_gatling` and default tactical icon ids in shared gameplay/runtime crates. That keeps content iteration in Rust instead of Lua/catalog data and makes testing content variants harder.
- Exact refs:
  - `crates/sidereal-game/src/components/ballistic_weapon.rs:23-35`
  - `crates/sidereal-game/src/components/tactical_presentation_defaults.rs:42-56`
- Recommendation: move these presets to Lua-authored bundle/content data and keep Rust focused on schema validation and runtime execution.

### Finding R2: Placeholder/partial UI paths are still active in the in-world client runtime

- Severity: Low
- Classification: cleanup
- Priority: optional improvement
- Why it matters: this is not a correctness bug, but it is active runtime scaffolding rather than finished behavior.
- Exact refs:
  - `bins/sidereal-client/src/runtime/pause_menu.rs:1`
  - `bins/sidereal-client/src/runtime/pause_menu.rs:199-200`
  - `bins/sidereal-client/src/runtime/visuals.rs:498`
- Evidence:
  - the pause menu file is explicitly labeled a settings placeholder,
  - clicking Settings only logs "not implemented",
  - `visuals.rs` carries a TODO for relevance-loss fade-out.
- Recommendation: either finish these paths or downgrade them into clearly isolated scaffolding modules so the main runtime path does not keep accumulating unfinished UI behavior.

## 5. Physics / Avian2D Findings

### Finding P1: The mass/inertia pipeline is directionally correct and should be kept

- Severity: Low
- Classification: architecture
- Priority: keep
- Why it matters: the runtime does a good job of mirroring gameplay total mass back into Avian `Mass` and `AngularInertia`, which is exactly what the repo rules require.
- Exact refs:
  - `crates/sidereal-game/src/mass.rs:59-118`
  - `crates/sidereal-game/src/mass.rs:153-212`
  - `crates/sidereal-game/src/world_spatial.rs:1-20`
- Note: this is one of the more defensible parts of the codebase. The issue is not the mass recomputation logic itself; it is the ship-specific bootstrap gate described in Finding A4.

## 6. Networking / Lightyear Findings

### Finding N1: Authenticated session binding checks are strong and should be preserved

- Severity: Low
- Classification: architecture
- Priority: keep
- Why it matters: the control path rejects mismatched claimed player ids instead of trusting client-provided identity. That is directly aligned with the authority rules in the design docs and AGENTS.
- Exact refs:
  - `bins/sidereal-replication/src/replication/control.rs:137-176`
- Note: this is the correct direction. Do not simplify this away when the control flow is later generalized.

## 7. Scripting / Lua Findings

### Finding S1: Starter-world and runtime content paths still mix script loading, persistence, validation, and gameplay assumptions in one gateway module

- Severity: Medium
- Classification: maintainability
- Priority: should fix
- Why it matters: `starter_world_scripts.rs` is effectively several subsystems in one file: script catalog caching, persistence seeding, bundle loading, graph record decoding, runtime validation, and world-init helpers. This makes it hard to evolve the Lua-content boundary cleanly.
- Exact refs:
  - `bins/sidereal-gateway/src/auth/starter_world_scripts.rs`
- Recommendation: split it into:
  - script catalog store/cache,
  - bundle/world-init loaders,
  - graph-record validators,
  - gateway-facing admin/service adapters.

## 8. Persistence / Data Flow Findings

### Finding D1: Static landmark discovery duplicates spatial indexing work instead of reusing the visibility index

- Severity: Medium
- Classification: performance
- Priority: should fix
- Why it matters: the visibility runtime already maintains `VisibilitySpatialIndex`, but `refresh_static_landmark_discoveries` rebuilds a separate `entities_by_cell` map and related lookup tables from scratch on each discovery pass. That is redundant work inside the largest active runtime file.
- Exact refs:
  - `bins/sidereal-replication/src/replication/visibility.rs:945-1055`
  - `bins/sidereal-replication/src/replication/visibility.rs:1542-1628`
- Evidence:
  - discovery allocates new `HashMap`s for landmark/entity position/cell state,
  - the same module already owns a maintained spatial index resource.
- Recommendation: make landmark discovery consume `VisibilitySpatialIndex` plus a landmark subset cache rather than rebuilding a temporary parallel index.

## 9. Redundancy / Dead Code Findings

### Finding X1: Some "cleanup later" behavior is no longer justified as temporary

- Severity: Low
- Classification: cleanup
- Priority: optional improvement
- Why it matters: the repo has already moved into explicit architecture enforcement mode. No-op compatibility hooks and placeholder UI are now liabilities, not harmless staging.
- Exact refs:
  - `crates/sidereal-game/src/actions.rs:133-136`
  - `bins/sidereal-client/src/runtime/pause_menu.rs:199-200`
- Recommendation: either complete these features or remove the dead-weight behavior.

## 10. Startup / Main Loop Flow Maps

### 10.1 Gateway startup and main loop

1. `bins/sidereal-gateway/src/main.rs` parses CLI/env config, opens the run log, initializes tracing, builds `AuthConfig`, connects to Postgres, ensures schema, constructs a bootstrap dispatcher, and builds `AuthService`.
2. It binds the Axum TCP listener and serves the router from `bins/sidereal-gateway/src/api.rs`.
3. The main loop is Axum request handling:
   - auth endpoints call into `AuthService`,
   - character/world entry endpoints return gateway DTOs and replication transport bootstrap config,
   - admin/script endpoints read or persist script catalog state,
   - asset endpoints materialize asset payloads by GUID from the runtime asset catalog.

### 10.2 Replication server startup and main loop

1. `bins/sidereal-replication/src/main.rs` parses CLI/env config, prepares file/log fanout, loads BRP config, determines headless/TUI mode, and builds a headless Bevy app.
2. The app wires:
   - `MinimalPlugins` with uncapped `Update` by default,
   - `SiderealGamePlugin`,
   - Avian physics,
   - Lightyear server plugins and native input protocol registration,
   - replication-specific plugins/resources from `bins/sidereal-replication/src/plugins.rs`.
3. Startup systems hydrate world state, start Lightyear transport, start health/control listeners, and optionally launch the TUI.
4. `Update` handles connection/session/auth/control/catalog-input housekeeping.
5. `FixedUpdate` runs authoritative gameplay, runtime scripting, visibility preparation/indexing, membership updates, tactical/owner-manifest streaming, and persistence flushing.

### 10.3 Client startup and main loop

1. `bins/sidereal-client/src/main.rs` dispatches to native or WASM entrypoints.
2. Native path (`bins/sidereal-client/src/platform/native/entry.rs`) creates either:
   - a headless transport app, or
   - a windowed Bevy app through `runtime::build_windowed_client_app`.
3. WASM path (`bins/sidereal-client/src/platform/wasm.rs`) builds the same shared client runtime shell with browser-specific transport/cache adapters.
4. `bins/sidereal-client/src/runtime/app_setup.rs` wires:
   - Avian physics + Lightyear client plugins,
   - shared gameplay core registration,
   - transport/auth/bootstrap resources,
   - asset runtime resources,
   - prediction/control resources,
   - visuals, lighting, UI, diagnostics plugins when not headless.
5. Runtime flow by schedule:
   - `Update`: auth/bootstrap transport, asset fetching/hot reload, UI/audio/message fanout,
   - `FixedUpdate`: predicted local gameplay action processing and motion updates,
   - `PostUpdate`: interpolation/correction recovery, camera sync, visual transforms, nameplates,
   - `Last`: fullscreen backdrop and debug overlay draws.

### 10.4 Cross-service data / authority / persistence / replication / asset / scripting / rendering flow

1. Gateway owns authentication and identity lifecycle.
2. After auth, gateway returns transport/bootstrap details so the client can join replication.
3. Replication binds transport peers to authenticated player ids and receives client intent only.
4. Replication runs authoritative simulation, runtime scripting, visibility evaluation, tactical/owner-lane generation, and persistence flush staging.
5. Persistence stores graph-shaped world state and feeds hydration on startup.
6. Gateway serves asset payload bytes over authenticated HTTP; replication does not stream asset payload bytes.
7. Client receives replicated world state and owner/tactical lanes through Lightyear, fetches cataloged asset payloads via gateway HTTP, and renders visuals locally.
8. Script-authored content currently enters through Lua bundle/world-init/catalog loaders, but several runtime decisions are still made by Rust heuristics instead of solely by script/catalog metadata.

## 11. Prioritized Remediation Plan

### Phase 1: Correctness and contract alignment

1. Convert all authoritative simulation math systems to `Time<Fixed>` where required, starting with `apply_engine_thrust`.
2. Remove or replace remaining compatibility-era no-op/default-backfill behavior that conflicts with the repo's strict schema discipline.

### Phase 2: Remove hardcoded content semantics

1. Replace `"Ship"`/`"Player"` label inference with explicit runtime metadata/components for control target classification, tactical kind, manifest kind, and health explorer glyphing.
2. Move content presets and icon defaults out of Rust and into Lua/catalog-authored content.

### Phase 3: Simplify hot paths

1. Refactor `visibility.rs` into separate modules and make landmark discovery reuse maintained spatial index data.
2. Split client visuals/backdrop/UI by concern and remove generator-id-based shader role inference in favor of catalog metadata.

### Phase 4: Clean service boundaries

1. Split gateway scripting/catalog code by persistence/cache/loading/validation responsibilities.
2. Keep the existing strong authenticated session binding checks and hierarchy-isolation behavior while doing the refactor.

## 12. Workspace / Runtime Catalog Appendix

### 12.1 Workspace crates and binaries

- `bins/sidereal-client`: active runtime. Native binary plus WASM library/entrypoint for the game client.
- `bins/sidereal-gateway`: active runtime. Auth, bootstrap DTOs, admin/script APIs, asset HTTP delivery.
- `bins/sidereal-replication`: active runtime. Authoritative simulation host, visibility, tactical/owner lanes, persistence flushing, health/TUI.
- `crates/sidereal-game`: active runtime shared gameplay/components/systems source of truth.
- `crates/sidereal-net`: active runtime shared protocol/messages/Lightyear registration.
- `crates/sidereal-persistence`: active runtime persistence and graph record handling.
- `crates/sidereal-scripting`: active runtime scripting loaders/validators/decoders.
- `crates/sidereal-asset-runtime`: active runtime asset catalog/materialization helpers.
- `crates/sidereal-runtime-sync`: active runtime shared runtime-sync data structures.
- `crates/sidereal-core`: active runtime shared config/logging/auth/bootstrap DTO utilities.
- `crates/sidereal-ui`: active runtime client UI theme/layout/widget primitives.
- `crates/sidereal-audio`: active runtime shared audio registry/catalog validation.
- `crates/sidereal-component-macros`: active build-time proc-macro crate for gameplay component metadata.
- `crates/sidereal-shader-preview`: tooling/runtime-support hybrid. WASM shader preview support crate; not part of the core game runtime loop.

### 12.2 Major Bevy plugin groups and runtime responsibilities

#### Gateway

- No Bevy runtime here. Main active runtime is Axum routing in `bins/sidereal-gateway/src/api.rs`.
- Transitional/scaffold notes:
  - in-memory auth/persister paths exist for app construction/testing,
  - starter-world and script-catalog logic are active but still mixed with migration-era assumptions.

#### Replication

- `SiderealGamePlugin`: active runtime. Shared authoritative gameplay systems and component registration.
- `PhysicsPlugins`: active runtime. Avian authoritative physics.
- `ServerPlugins` + Lightyear input protocol registration: active runtime. Replication transport and protocol plumbing.
- `ReplicationLifecyclePlugin`: active runtime. Startup hydration, transport startup, health startup, connection observers.
- `ReplicationDiagnosticsPlugin`: active runtime. Health snapshots, world explorer/map snapshots, admin reset execution.
- `ReplicationInputPlugin`: active runtime. Native input drain into authoritative action queues.
- `ReplicationControlPlugin`: active runtime. Control-role reconciliation and combat replication follow-through.
- `ReplicationVisibilityPlugin`: active runtime. Transform sync, observer anchors, visibility range computation, cache/index refresh, membership updates, streaming.
- `ReplicationPersistencePlugin`: active runtime. Persistence worker startup, dirty marking, flush scheduling.
- `ReplicationRuntimeScriptingPlugin`: active runtime. Script snapshot refresh, event/interval execution, intent application.
- TUI (`bins/sidereal-replication/src/tui.rs`): active runtime in non-headless interactive sessions, but still partly transitional due its size and mixed concerns.

#### Client

- `SiderealGameCorePlugin`: active runtime. Component registration only, without authoritative server simulation.
- `ClientPlugins` + `LightyearAvianPlugin` + `FrameInterpolationPlugin`: active runtime. Transport/prediction/interpolation pipeline.
- `ClientBootstrapPlugin`: active runtime. Auth/bootstrap/startup transitions.
- `ClientTransportPlugin`: active runtime. Transport session/input/output handling.
- `ClientReplicationPlugin`: active runtime. Replicated state handling and entity sync.
- `ClientPredictionPlugin`: active runtime. Prediction/correction ownership and motion pipeline.
- `ClientVisualsPlugin`: active runtime. World visuals, effects, sprite/material handling.
- `ClientLightingPlugin`: active runtime. Lighting and fullscreen/background visual support.
- `ClientUiPlugin`: active runtime. HUD, menus, nameplates, dialogs.
- `ClientDiagnosticsPlugin`: active runtime. Debug overlay and diagnostics surfaces.
- `ExplosionDistortionPostProcessPlugin`: active runtime when not headless.
- Native platform adapters: active runtime on native only.
- WASM platform adapters: active runtime on browser only.

### 12.3 Major resources and systems by service

#### Replication active runtime resources

- Auth/session: `AuthenticatedClientBindings`, `PlayerRuntimeEntityMap`, activity/order tracking.
- Visibility: `VisibilityEntityCache`, `VisibilityClientContextCache`, `VisibilityMembershipCache`, `VisibilitySpatialIndex`, `VisibilityScratch`, `ClientObserverAnchorPositionMap`, runtime/metrics resources.
- Streaming: tactical stream state, owner manifest stream state.
- Persistence: worker handles/queues/metrics.
- Health/TUI: shared health/world snapshots and log fanout buffers.

#### Client active runtime resources

- Session/bootstrap: `ClientSession`, disconnect/logout state, auth sync/watchdogs.
- Assets: local asset manager, runtime asset dependency/hot-reload/fetch state, owned manifest cache.
- Prediction/control: motion ownership reconcile state, control request state, local player view state, prediction tuning.
- Tactical/UI: nameplate registries, fog/contact caches, tactical UI state, dialog queue, pause menu state.
- Visuals/rendering: fullscreen world data, starfield/camera motion state, shader assignment state.
- Audio: audio runtime backend/catalog/settings/state resources.

### 12.4 Likely transitional or migration-heavy areas

- `bins/sidereal-replication/src/replication/visibility.rs`
- `bins/sidereal-client/src/runtime/shaders.rs`
- `bins/sidereal-client/src/runtime/visuals.rs`
- `bins/sidereal-gateway/src/auth/starter_world.rs`
- `bins/sidereal-gateway/src/auth/starter_world_scripts.rs`
- `crates/sidereal-persistence/tests/envelope_codec.rs`

These are the areas where current behavior most clearly still reflects transition logic rather than the final intended architecture.

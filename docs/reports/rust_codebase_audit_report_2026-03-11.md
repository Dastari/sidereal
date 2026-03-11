# Rust Codebase Audit Report

Date: 2026-03-11
Scope: Rust workspace
Prompt source: `docs/prompts/rust_codebase_audit_prompt.md`

Update note (2026-03-11):
- This report re-audits the live codebase after the March 10 audits.
- Two previously severe findings no longer hold: `cargo clippy --workspace --all-targets -- -D warnings` now passes, and `sidereal_core::SIM_TICK_HZ` is aligned at 60 Hz.
- Runtime asset bootstrap and lazy asset fetch are also now bounded-parallel rather than single-file serialized.

## 1. Executive Summary

The codebase is still strongest where the project has been strictest: authenticated authority, fixed-step simulation, UUID-based boundary contracts, shared native/WASM client bootstrapping, and server-owned persistence all remain directionally correct and should be kept.

The main risks have shifted. The current problems are less about blatant contract violations and more about hot-path architecture and transitional complexity that is staying alive too long:

1. Replication visibility is still the dominant cross-stack risk. It uses a spatial-grid candidate phase, but the runtime still rebuilds large scratch state and then iterates replicated entities against clients every fixed tick in [`bins/sidereal-replication/src/replication/visibility.rs:667`](bins/sidereal-replication/src/replication/visibility.rs:667).
2. The native client runtime remains too monolithic. App wiring, prediction bootstrap, replication adoption, render-layer maintenance, visuals, backdrop, UI, and diagnostics still span a small number of very large modules.
3. The rendering/content pipeline is still partly generic in data and partly hardcoded in Rust. Shader slot inference and some fullscreen/world visual decisions still know too much about current Sidereal content.
4. Asset delivery implementation still diverges from the documented `assets.pak` + `assets.index` cache contract. The live code still uses loose published files plus a JSON index.
5. Runtime scripting and duplicate-visual suppression still rebuild or reevaluate more state than they should on active runtime paths.

## 2. Architecture Findings

### Finding A1: Replication visibility still scales as a large per-tick whole-runtime pass
- Severity: Critical
- Type: architecture, performance
- Priority: must fix
- Why it matters:
  The server already moved away from naive full-world candidate selection, but the actual visibility update path still rebuilds large scratch maps, recomputes per-client candidate sets, runs landmark discovery, and then loops replicated entities against clients every fixed tick. That is the highest-leverage architecture problem in the workspace because it degrades replication cadence, client smoothness, and future scale at the same time.
- Exact references:
  - [`docs/sidereal_design_document.md:404`](docs/sidereal_design_document.md:404)
  - [`docs/sidereal_design_document.md:425`](docs/sidereal_design_document.md:425)
  - [`bins/sidereal-replication/src/replication/visibility.rs:667`](bins/sidereal-replication/src/replication/visibility.rs:667)
  - [`bins/sidereal-replication/src/replication/visibility.rs:815`](bins/sidereal-replication/src/replication/visibility.rs:815)
  - [`bins/sidereal-replication/src/replication/visibility.rs:913`](bins/sidereal-replication/src/replication/visibility.rs:913)
  - [`bins/sidereal-replication/src/replication/visibility.rs:1180`](bins/sidereal-replication/src/replication/visibility.rs:1180)
- Concrete recommendation:
  Split visibility into persistent indexed state plus smaller dirty updates. Keep spatial-grid candidate narrowing, but stop rebuilding everything every tick. Move landmark discovery to a lower-frequency or dirty-triggered lane. Long-term, the apply phase should operate on changed client/entity regions, not the whole replicated population each tick.

### Finding A2: The native client runtime is still too monolithic to be safely evolved
- Severity: High
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The client has plugin names, but much of the real ownership still sits in large cross-domain files. `native/mod.rs` still owns major runtime composition and resource insertion, `plugins.rs` still wires most hot-path scheduling, and `visuals.rs` / `backdrop.rs` each combine multiple unrelated domains. This is not just style drift; it directly raises regression risk because lifecycle resets, schedule ordering, and runtime ownership are hard to review in isolation.
- Exact references:
  - [`bins/sidereal-client/src/native/mod.rs:100`](bins/sidereal-client/src/native/mod.rs:100)
  - [`bins/sidereal-client/src/native/mod.rs:169`](bins/sidereal-client/src/native/mod.rs:169)
  - [`bins/sidereal-client/src/native/plugins.rs:29`](bins/sidereal-client/src/native/plugins.rs:29)
  - [`bins/sidereal-client/src/native/plugins.rs:109`](bins/sidereal-client/src/native/plugins.rs:109)
  - [`bins/sidereal-client/src/native/visuals.rs:1`](bins/sidereal-client/src/native/visuals.rs:1)
  - [`bins/sidereal-client/src/native/backdrop.rs:1`](bins/sidereal-client/src/native/backdrop.rs:1)
- Concrete recommendation:
  Split by ownership boundary, not by file size alone:
  - prediction/adoption
  - runtime render-layer registry/assignment
  - streamed sprite attachment
  - planet/fullscreen/post-process rendering
  - HUD/tactical/nameplate UI
  The app entrypoint should mostly compose smaller domain plugins rather than directly owning dozens of resources.

### Finding A3: Render-layer and shader selection are still only partially data-driven
- Severity: High
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The project direction is explicit: Lua-authored render layers and generic shader families should increasingly own content specifics. The current runtime still infers concrete content slots in Rust by layer name and known content families. That is workable as migration code, but it is still active runtime ownership of game-specific rendering decisions.
- Exact references:
  - [`docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`](docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md)
  - [`docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`](docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md)
  - [`bins/sidereal-client/src/native/shaders.rs:640`](bins/sidereal-client/src/native/shaders.rs:640)
  - [`bins/sidereal-client/src/native/backdrop.rs:85`](bins/sidereal-client/src/native/backdrop.rs:85)
  - [`bins/sidereal-client/src/native/visuals.rs:198`](bins/sidereal-client/src/native/visuals.rs:198)
- Concrete recommendation:
  Reduce Rust-side named slot inference. Keep Rust responsible for shader-family/domain plumbing and fallback loading, but move concrete selection and compatibility decisions fully behind authored metadata where possible.

### Finding A4: Asset cache implementation still diverges from the documented contract
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The docs still describe the target cache shape as `assets.pak` + `assets.index`, but the live implementation still uses generated loose files under `published_assets/` on the gateway and `data/cache_stream/index.json` on the client/shared runtime. This is a code/doc divergence in an operationally important area.
- Exact references:
  - [`docs/sidereal_design_document.md:474`](docs/sidereal_design_document.md:474)
  - [`docs/features/asset_delivery_contract.md:226`](docs/features/asset_delivery_contract.md:226)
  - [`crates/sidereal-asset-runtime/src/lib.rs:190`](crates/sidereal-asset-runtime/src/lib.rs:190)
  - [`crates/sidereal-asset-runtime/src/lib.rs:256`](crates/sidereal-asset-runtime/src/lib.rs:256)
  - [`bins/sidereal-client/src/native/assets.rs:267`](bins/sidereal-client/src/native/assets.rs:267)
  - [`bins/sidereal-gateway/src/api.rs:401`](bins/sidereal-gateway/src/api.rs:401)
- Concrete recommendation:
  Either implement the documented packed-cache format or explicitly downgrade the docs to describe the current loose-file cache as the active contract. Do not leave this as an implicit migration forever.

### Finding A5: Runtime scripting still rebuilds broad world snapshot state every fixed tick
- Severity: Medium
- Type: performance, maintainability
- Priority: should fix
- Why it matters:
  `refresh_script_world_snapshot()` clears and rebuilds the full script-visible entity map every fixed tick, and `run_script_intervals()` clones that map into an `Rc<HashMap<...>>` before iterating handlers. For the current content scale this may be acceptable, but it is still broad work in the authoritative fixed-tick path.
- Exact references:
  - [`bins/sidereal-replication/src/plugins.rs:123`](bins/sidereal-replication/src/plugins.rs:123)
  - [`bins/sidereal-replication/src/replication/runtime_scripting.rs:228`](bins/sidereal-replication/src/replication/runtime_scripting.rs:228)
  - [`bins/sidereal-replication/src/replication/runtime_scripting.rs:258`](bins/sidereal-replication/src/replication/runtime_scripting.rs:258)
  - [`bins/sidereal-replication/src/replication/runtime_scripting.rs:390`](bins/sidereal-replication/src/replication/runtime_scripting.rs:390)
- Concrete recommendation:
  Move toward dirty or scoped script snapshots, or at least split the snapshot by concern so interval/event handlers do not require a fresh full entity map each fixed tick.

### Finding A6: Duplicate predicted/interpolated visual suppression is still active transitional runtime code
- Severity: Medium
- Type: maintainability, performance
- Priority: should fix
- Why it matters:
  The client still has a whole subsystem dedicated to tracking duplicate entity GUID groups, choosing a winner, and hiding losers. The implementation is incremental rather than a naive full scan, but it still means duplicate presentation remains an active hot-path concern instead of being resolved closer to adoption/handoff boundaries.
- Exact references:
  - [`bins/sidereal-client/src/native/visuals.rs:374`](bins/sidereal-client/src/native/visuals.rs:374)
  - [`bins/sidereal-client/src/native/visuals.rs:591`](bins/sidereal-client/src/native/visuals.rs:591)
  - [`bins/sidereal-client/src/native/replication.rs:471`](bins/sidereal-client/src/native/replication.rs:471)
  - [`bins/sidereal-client/src/native/replication.rs:797`](bins/sidereal-client/src/native/replication.rs:797)
- Concrete recommendation:
  Make duplicate resolution a narrower lifecycle concern around replicated adoption/control-handoff instead of a persistent render-maintenance concern.

### Finding A7: The core authority and timing model is still defensible and should be preserved
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  The workspace still correctly centers on authenticated session binding, server-owned simulation, fixed-step timing, and shared core gameplay code. This is the right spine to keep while simplifying the surrounding runtime.
- Exact references:
  - [`bins/sidereal-replication/src/main.rs:84`](bins/sidereal-replication/src/main.rs:84)
  - [`bins/sidereal-replication/src/plugins.rs:56`](bins/sidereal-replication/src/plugins.rs:56)
  - [`bins/sidereal-replication/src/replication/input.rs:231`](bins/sidereal-replication/src/replication/input.rs:231)
  - [`bins/sidereal-client/src/client_core.rs:4`](bins/sidereal-client/src/client_core.rs:4)
  - [`bins/sidereal-client/src/wasm.rs:37`](bins/sidereal-client/src/wasm.rs:37)
- Concrete recommendation:
  Do not relax server authority or split shared client/runtime code again while addressing the current hot-path issues.

## 3. Bevy / ECS Findings

### Finding B1: Large runtime files are now the main consistency smell in the codebase
- Severity: Medium
- Type: maintainability
- Priority: should fix
- Why it matters:
  The codebase no longer looks inconsistent because of basic style or warning discipline. The bigger consistency problem is that a handful of large files now carry too many responsibilities and therefore force inconsistent local patterns inside themselves.
- Exact references:
  - [`bins/sidereal-client/src/native/visuals.rs`](bins/sidereal-client/src/native/visuals.rs)
  - [`bins/sidereal-client/src/native/backdrop.rs`](bins/sidereal-client/src/native/backdrop.rs)
  - [`bins/sidereal-client/src/native/debug_overlay.rs`](bins/sidereal-client/src/native/debug_overlay.rs)
  - [`bins/sidereal-replication/src/replication/visibility.rs`](bins/sidereal-replication/src/replication/visibility.rs)
- Concrete recommendation:
  Split by query/mutation boundary and runtime responsibility. Avoid continuing to add new behavior to these files.

### Finding B2: `ClientDiagnosticsPlugin` is still a scaffold plugin
- Severity: Low
- Type: cleanup
- Priority: optional improvement
- Why it matters:
  An empty active plugin is small, but it is still runtime-facing scaffolding and adds noise when reading the client composition.
- Exact references:
  - [`bins/sidereal-client/src/native/plugins.rs:572`](bins/sidereal-client/src/native/plugins.rs:572)
- Concrete recommendation:
  Either delete it until it has real ownership or move actual diagnostics ownership into it.

## 4. Rendering Findings

### Finding R1: Fullscreen/render-layer migration remains partially transitional
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  Lua-authored `RuntimeRenderLayerDefinition` is live, but `FullscreenLayer` compatibility still exists in active fullscreen sync. That means the migration is not yet cleanly finished.
- Exact references:
  - [`bins/sidereal-client/src/native/backdrop.rs:85`](bins/sidereal-client/src/native/backdrop.rs:85)
  - [`bins/sidereal-client/src/native/backdrop.rs:141`](bins/sidereal-client/src/native/backdrop.rs:141)
  - [`docs/features/visibility_replication_contract.md:49`](docs/features/visibility_replication_contract.md:49)
- Concrete recommendation:
  Decide whether legacy `FullscreenLayer` is still required. If not, remove it and update docs in the same change.

### Finding R2: The render-layer architecture is directionally correct and should be kept
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  Runtime layer validation, phase/domain checks, and authored-rule compilation are good structural decisions for this project. The issue is not the existence of render layers; it is the residual hardcoded content logic around them.
- Exact references:
  - [`crates/sidereal-game/src/render_layers.rs`](crates/sidereal-game/src/render_layers.rs)
  - [`bins/sidereal-client/src/native/render_layers.rs:20`](bins/sidereal-client/src/native/render_layers.rs:20)
- Concrete recommendation:
  Preserve the architecture. Simplify the transitional Rust-side content routing around it.

## 5. Physics / Avian2D Findings

### Finding P1: No major Avian misuse stood out in the active server-authoritative path
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  The authoritative flow still uses fixed schedules, server-side physics ownership, and direct Avian components without reintroducing the old gameplay mirror-motion lane.
- Exact references:
  - [`bins/sidereal-client/src/native/mod.rs:169`](bins/sidereal-client/src/native/mod.rs:169)
  - [`bins/sidereal-replication/src/main.rs:98`](bins/sidereal-replication/src/main.rs:98)
  - [`docs/sidereal_design_document.md:33`](docs/sidereal_design_document.md:33)
- Concrete recommendation:
  Keep current motion authority discipline. The bigger wins are elsewhere.

## 6. Networking / Lightyear Findings

### Finding N1: Native/WASM shared bootstrap direction is now materially healthier
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  The WASM client now boots through the shared windowed client builder rather than a separate render shell. That materially reduces future parity cost and is worth preserving.
- Exact references:
  - [`bins/sidereal-client/src/wasm.rs:37`](bins/sidereal-client/src/wasm.rs:37)
  - [`bins/sidereal-client/src/native/mod.rs:278`](bins/sidereal-client/src/native/mod.rs:278)
- Concrete recommendation:
  Continue keeping transport/platform differences at the boundary only.

## 7. Scripting / Lua Findings

### Finding S1: Gateway and replication still each own neighboring pieces of script/bootstrap orchestration
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The code is better shared than before, but gateway and replication still each carry surrounding orchestration for catalog loading, world-init/bootstrap, and runtime validation. This is not a correctness bug today, but it keeps project-wide content contracts spread across services.
- Exact references:
  - [`bins/sidereal-gateway/src/api.rs:548`](bins/sidereal-gateway/src/api.rs:548)
  - [`bins/sidereal-gateway/src/auth/service.rs`](bins/sidereal-gateway/src/auth/service.rs)
  - [`bins/sidereal-replication/src/replication/assets.rs:94`](bins/sidereal-replication/src/replication/assets.rs:94)
  - [`bins/sidereal-replication/src/replication/simulation_entities.rs:578`](bins/sidereal-replication/src/replication/simulation_entities.rs:578)
- Concrete recommendation:
  Keep moving shared content/catalog/world-init logic downward into shared crates. The service binaries should be thinner orchestration layers.

## 8. Persistence / Data Flow Findings

### Finding D1: Persistence shape and UUID boundary discipline are still good and should be retained
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  Graph records, reflect-backed component serialization, and UUID-only cross-boundary identity are consistent with the documented design and reduce long-term migration risk.
- Exact references:
  - [`crates/sidereal-persistence/src/lib.rs`](crates/sidereal-persistence/src/lib.rs)
  - [`crates/sidereal-runtime-sync/src/lib.rs`](crates/sidereal-runtime-sync/src/lib.rs)
  - [`docs/sidereal_design_document.md:24`](docs/sidereal_design_document.md:24)
- Concrete recommendation:
  Preserve this contract while simplifying runtime orchestration around it.

## 9. Redundancy / Dead Code Findings

### Finding X1: Transitional code is no longer the whole story, but it still clusters in client rendering and duplicate-handling paths
- Severity: Low
- Type: cleanup, maintainability
- Priority: optional improvement
- Why it matters:
  The workspace has clearly improved since March 10, but transitional logic remains concentrated in the same client render/presentation areas. That is the cleanup surface to keep reducing after the main visibility work.
- Exact references:
  - [`bins/sidereal-client/src/native/backdrop.rs:141`](bins/sidereal-client/src/native/backdrop.rs:141)
  - [`bins/sidereal-client/src/native/visuals.rs:374`](bins/sidereal-client/src/native/visuals.rs:374)
  - [`bins/sidereal-client/src/native/replication.rs:433`](bins/sidereal-client/src/native/replication.rs:433)
- Concrete recommendation:
  Treat these as explicit follow-up workstreams rather than letting them accumulate by default.

## 10. Startup / Main Loop Flow Maps

### 10.1 Gateway startup and main loop

1. `bins/sidereal-gateway/src/main.rs` parses CLI/env, initializes tracing, connects Postgres, ensures auth schema, constructs `AuthService`, binds TCP, and serves the Axum router.
2. `bins/sidereal-gateway/src/api.rs` owns HTTP endpoints for auth, world entry, script admin, bootstrap manifest, and asset payload fetch.
3. `AuthService` validates identity/ownership, issues tokens, dispatches world-entry bootstrap commands, and gates admin/script routes.
4. Asset manifest and asset fetch routes load the active script-authored asset registry, derive runtime catalog metadata, and stream authenticated asset payloads over HTTP.

### 10.2 Replication server startup and main loop

1. `bins/sidereal-replication/src/main.rs` parses CLI/env, configures logging/BRP/TUI, creates a Bevy `App`, adds `SiderealGamePlugin`, Avian, Lightyear server plugins, and replication plugins.
2. Startup systems hydrate persisted simulation entities, start the Lightyear transport, start health services, and start the UDP replication-control listener.
3. `Update` handles transport/message wiring, auth/control/input ingress, asset catalog polling, bootstrap entity commands, and health/logging.
4. `FixedUpdate` runs runtime scripting, authoritative input drain, gameplay/physics, replication visibility, owner/tactical/asset fanout, and persistence dirty/flush work.

### 10.3 Client startup and main loop

1. Native `main.rs` calls `native::run()`. WASM `main.rs` calls `wasm::run()`, which uses the shared windowed client builder.
2. `bins/sidereal-client/src/native/mod.rs` composes Bevy default/runtime plugins, Lightyear client/prediction plugins, Avian integration, runtime resources, and client-specific plugins.
3. Startup/OnEnter systems build UI cameras, auth UI, world scene cameras, and bootstrap state.
4. `Update` handles transport/auth messages, replicated entity adoption, asset dependency and fetch maintenance, render-layer registry/assignment sync, fullscreen/world visual sync, lighting, UI, tactical overlays, and pause/logout behavior.
5. `PostUpdate` applies interpolation/correction-adjacent transform recovery, camera follow, visual transform updates, and debug snapshotting before transform propagation.

### 10.4 Cross-service authority, persistence, replication, asset delivery, scripting, and rendering flow

1. Gateway authenticates account identity and validates chosen `player_entity_id`.
2. Gateway dispatches authoritative world-entry/bootstrap intent to replication.
3. Replication hydrates/owns runtime ECS state, accepts authenticated realtime input, runs fixed-step simulation, visibility, scripting intents, and persistence staging.
4. Replication publishes state over Lightyear and sends control/owner/tactical/asset-catalog side messages.
5. Gateway serves bootstrap manifests and authenticated `/assets/<asset_guid>` payloads; replication does not stream asset bytes.
6. Client receives replicated entities, adopts runtime clones, derives render-layer assignments, resolves cached/lazy asset bytes, installs shaders/materials, and renders camera-relative 2D visuals.

## 11. Prioritized Remediation Plan

1. Rework replication visibility first. This is the biggest shared correctness/performance risk and the most likely root cause behind downstream smoothness complaints.
2. Split the client hot path into smaller ownership modules, starting with `visuals.rs`, `backdrop.rs`, `plugins.rs`, and replicated-adoption logic.
3. Finish the rendering data-ownership migration by reducing Rust-side named shader/layer slot inference.
4. Reconcile asset-cache docs and implementation so contributors stop working against two different contracts.
5. Narrow runtime scripting snapshot work and duplicate-visual maintenance after the visibility refactor lands.

## 12. Workspace / Runtime Catalog Appendix

### 12.1 Workspace crates and binaries

- `bins/sidereal-client`: active runtime binary/lib; native and WASM client bootstrap, transport, rendering, UI, prediction, and replicated adoption. Some transitional client/runtime code remains active.
- `bins/sidereal-replication`: active runtime binary; authoritative simulation, visibility, scripting, persistence staging, replication delivery, health/TUI.
- `bins/sidereal-gateway`: active runtime binary/lib; auth, account/session lifecycle, world-entry bootstrap dispatch, script admin, bootstrap manifest, authenticated asset serving.
- `crates/sidereal-game`: active shared runtime library; gameplay components, generated registry, fixed-step gameplay systems, render-layer validation, mass/flight/combat helpers.
- `crates/sidereal-net`: active shared runtime library; Lightyear protocol registration/types.
- `crates/sidereal-core`: active shared runtime library; core constants, auth claims, DTOs, logging helpers, BRP config.
- `crates/sidereal-persistence`: active shared runtime library; AGE/Postgres graph persistence and script-catalog persistence helpers.
- `crates/sidereal-runtime-sync`: active shared runtime library; runtime entity registry helpers and reflect component insertion/serialization helpers.
- `crates/sidereal-scripting`: active shared runtime library; Lua sandboxing, asset registry loading, render-layer/world-visual validation helpers.
- `crates/sidereal-asset-runtime`: active shared runtime library; runtime asset catalog generation, hashing, published path materialization, cache-index helpers. Current implementation is still transitional relative to the documented packed-cache target.
- `crates/sidereal-component-macros`: active build-time/shared library; gameplay component derive/attribute macros.
- `crates/sidereal-shader-preview`: tooling-only library; shader-preview support, not a main gameplay runtime.

### 12.2 Major runtime Bevy plugins, systems, and resources by service

#### Client

- Core plugins: `DefaultPlugins` or headless minimal set, `PhysicsPlugins` with transform/interpolation disabled, `ClientPlugins`, `LightyearAvianPlugin`, `FrameInterpolationPlugin`, `LightyearInputProtocolPlugin`, native `ClientInputPlugin`, `Material2dPlugin` family registrations, `SvgPlugin`, `FrameTimeDiagnosticsPlugin`.
- Client custom plugins: `ClientBootstrapPlugin`, `ClientTransportPlugin`, `ClientReplicationPlugin`, `ClientPredictionPlugin`, `ClientVisualsPlugin`, `ClientLightingPlugin`, `ClientUiPlugin`, `ClientDiagnosticsPlugin`.
- Major active resources:
  - transport/auth: `ClientSession`, `ClientNetworkTick`, `ClientInputAckTracker`, `ClientAuthSyncState`
  - prediction/control: `MotionOwnershipReconcileState`, `ClientControlRequestState`, `LocalPlayerViewState`, `DeferredPredictedAdoptionState`
  - assets/render: `LocalAssetManager`, `RuntimeAssetDependencyState`, `RuntimeAssetHttpFetchState`, `RuntimeShaderAssignments`, `RuntimeRenderLayerRegistry`, `RuntimeRenderLayerAssignmentCache`
  - scene/UI: `CameraMotionState`, `StarfieldMotionState`, `TacticalMapUiState`, `DebugOverlaySnapshot`
- Transitional/scaffold items worth calling out:
  - `DuplicateVisualResolutionState`: transitional runtime maintenance
  - `ClientDiagnosticsPlugin`: scaffold/placeholder
  - legacy fullscreen compatibility in backdrop sync: transitional runtime code

#### Replication

- Core plugins: `MinimalPlugins`, `AssetPlugin`, `ScenePlugin`, `SiderealGamePlugin`, Avian `PhysicsPlugins` with interpolation disabled, `ServerPlugins`, Lightyear native input protocol plugin.
- Replication custom plugins: `ReplicationLifecyclePlugin`, `ReplicationDiagnosticsPlugin`, `ReplicationAuthPlugin`, `ReplicationInputPlugin`, `ReplicationControlPlugin`, `ReplicationRuntimeScriptingPlugin`, `ReplicationVisibilityPlugin`, `ReplicationPersistencePlugin`, `ReplicationBootstrapBridgePlugin`.
- Major active resources:
  - auth/control: `AuthenticatedClientBindings`, `PlayerRuntimeEntityMap`
  - visibility: `ClientVisibilityRegistry`, `VisibilityRuntimeConfig`, `VisibilityScratch`, `VisibilityRuntimeMetrics`
  - scripting: `ScriptRuntime`, `ScriptWorldSnapshot`, `ScriptRuntimeMetrics`
  - persistence: `PersistenceDirtyState`, `PersistenceWorkerState`
  - diagnostics/ops: health snapshots, admin command bus, TUI log buffer

#### Gateway

- No Bevy runtime.
- Major active runtime modules:
  - `api.rs`: active HTTP routing and asset/bootstrap endpoints
  - `auth/service.rs`: active auth/account/session/bootstrap orchestration
  - `auth/store.rs`: active persistence boundary for account/auth records
  - script admin endpoints and runtime asset catalog cache: active runtime support logic

### 12.3 Test-only, tooling-only, scaffold, and transitional items

- `bins/sidereal-replication/src/tests/`: test-only
- `bins/sidereal-gateway/tests/`: test-only
- `crates/sidereal-shader-preview`: tooling-only
- `ClientDiagnosticsPlugin`: scaffold/placeholder
- duplicate visual suppression / winner selection: active transitional runtime code
- legacy fullscreen compatibility in backdrop sync: active transitional runtime code

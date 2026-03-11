# Rust Codebase Audit Report

Date: 2026-03-10
Scope: Rust workspace
Prompt source: `docs/prompts/rust_codebase_audit_prompt.md`

## 1. Executive Summary

The codebase is still strongest where the project has been strictest: authenticated server authority, fixed-step simulation, and the broad direction toward shared native/WASM client code are all visible in active runtime paths and should be kept. In particular, replication input is still bound to authenticated player identity rather than trusting claimed client IDs, the replication app still enforces a 60 Hz authoritative fixed step, and the WASM entrypoint now boots through the shared native client builder instead of a separate render-only shell.

The largest alignment drift is no longer in the authority model. It is in code quality discipline and ownership boundaries:

1. The workspace is out of alignment with its own mandatory quality gate because `cargo clippy --workspace --all-targets -- -D warnings` currently fails on active replication and client code.
2. A stale `SIM_TICK_HZ = 30` constant in shared core diverges from both the design docs and the actual 60 Hz runtime used by client and replication.
3. Client bootstrap/orchestration remains too monolithic and partly duplicated, especially in `bins/sidereal-client/src/native/mod.rs` and `bins/sidereal-client/src/native/plugins.rs`.
4. Gateway and replication still duplicate world-init/script-catalog validation logic that should now be shared.
5. The fullscreen/render-layer migration is only partially complete: runtime render-layer records are already authored from Lua, but legacy `FullscreenLayer` compatibility code and stale docs remain active.
6. Asset cache implementation still diverges from the documented pak/index contract.

Those are architecture and maintainability problems with direct correctness implications. Left alone, they will keep increasing the cost of every client, scripting, and asset-delivery change.

## 2. Architecture Findings

### Finding A1: Mandatory workspace quality gate is currently failing
- Severity: Critical
- Type: correctness, maintainability
- Priority: must fix
- Why it matters:
  `AGENTS.md` treats `cargo clippy --workspace --all-targets -- -D warnings` as a minimum completion gate. The repository is therefore not currently aligned with its own enforceable coding standard. This is not a style nit; it means stale dead code and complexity regressions are already accumulating in active runtime paths.
- Exact references:
  - `AGENTS.md`
  - `bins/sidereal-replication/src/replication/scripting.rs:1005`
  - `bins/sidereal-replication/src/replication/input.rs:373`
  - `bins/sidereal-replication/src/replication/runtime_state.rs:32`
  - `bins/sidereal-replication/src/replication/visibility.rs:562`
  - `bins/sidereal-replication/src/replication/visibility.rs:1609`
  - `bins/sidereal-client/src/native/backdrop.rs:502`
  - `bins/sidereal-client/src/native/backdrop.rs:580`
  - `bins/sidereal-client/src/native/backdrop.rs:1904`
  - `bins/sidereal-client/src/native/replication.rs:69`
  - `bins/sidereal-client/src/native/replication.rs:178`
  - `bins/sidereal-client/src/native/replication.rs:860`
  - `bins/sidereal-client/src/native/replication.rs:869`
  - `bins/sidereal-client/src/native/ui.rs:158`
  - `bins/sidereal-client/src/native/ui.rs:166`
  - `bins/sidereal-client/src/native/visuals.rs:1833`
  - `bins/sidereal-client/src/native/visuals.rs:2589`
- Evidence:
  A fresh run on 2026-03-10 failed on dead code, `too_many_arguments`, `type_complexity`, and `needless_option_as_deref` in active runtime modules.
- Concrete recommendation:
  Treat this as a short-term stabilization task, not a background cleanup. First remove dead code in replication scripting, then split oversized systems into parameter structs/helpers so Clippy is satisfied without blanket `allow` attributes.

### Finding A2: Shared simulation tick constant is stale and contradicts runtime behavior
- Severity: High
- Type: correctness, architecture
- Priority: must fix
- Why it matters:
  The docs and both primary runtimes use 60 Hz, but shared core still publishes 30 Hz as the canonical simulation rate. Any new code, tests, tooling, or protocol logic that consumes `sidereal_core::SIM_TICK_HZ` can silently diverge from the real runtime.
- Exact references:
  - `docs/sidereal_design_document.md:204`
  - `crates/sidereal-core/src/lib.rs:11`
  - `crates/sidereal-core/tests/id_helpers.rs:19`
  - `bins/sidereal-client/src/native/mod.rs:147`
  - `bins/sidereal-replication/src/main.rs:152`
- Concrete recommendation:
  Make `sidereal_core::SIM_TICK_HZ` the single authoritative value at 60, update the test, and replace local literal `60.0` inserts with conversions from the shared constant.

### Finding A3: Client bootstrap and runtime wiring remain too monolithic
- Severity: High
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The client has moved toward plugins, but major bootstrapping still happens in one large function that manually inserts a long list of resources and runtime toggles. That makes lifecycle ownership harder to reason about and increases the chance of missing state resets during auth/logout/world transitions.
- Exact references:
  - `AGENTS.md`
  - `bins/sidereal-client/src/native/mod.rs:103`
  - `bins/sidereal-client/src/native/mod.rs:147`
  - `bins/sidereal-client/src/native/mod.rs:157`
  - `bins/sidereal-client/src/native/mod.rs:208`
- Details:
  `configure_client_runtime()` is still doing physics setup, Lightyear setup, fixed-time setup, asset/cache adapter insertion, debug toggles, world-state resources, tactical resources, hierarchy resources, camera resources, and prediction tuning in one place.
- Concrete recommendation:
  Split initialization into domain-owned plugin/resource modules: transport/auth, asset bootstrap/cache, prediction/replication, world scene/render, tactical/UI, and diagnostics/debug. The app entrypoint should mostly compose those modules instead of owning their resources directly.

### Finding A4: Client plugin scheduling still contains duplicated headless/non-headless chains
- Severity: High
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  `ClientReplicationPlugin` duplicates large Update chains for headless and non-headless modes. This is brittle because the two paths can drift on ordering or feature coverage without an obvious compile-time signal.
- Exact references:
  - `bins/sidereal-client/src/native/plugins.rs:124`
  - `bins/sidereal-client/src/native/plugins.rs:162`
  - `bins/sidereal-client/src/native/plugins.rs:223`
  - `bins/sidereal-client/src/native/plugins.rs:260`
- Concrete recommendation:
  Build common system tuples once and gate only the genuinely different state-transition and logging behavior. If the headless path truly needs separate semantics, isolate that difference behind smaller helper plugins rather than duplicating the chain.

### Finding A5: Gateway and replication still duplicate script/world-init authority code
- Severity: High
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  Shared authoritative bootstrap logic should not exist in two large copies. Duplication here is especially risky because both sides validate graph records and world-init content that must stay schema-consistent.
- Exact references:
  - `bins/sidereal-gateway/src/auth/starter_world_scripts.rs:52`
  - `bins/sidereal-gateway/src/auth/starter_world_scripts.rs:85`
  - `bins/sidereal-gateway/src/auth/starter_world_scripts.rs:184`
  - `bins/sidereal-gateway/src/auth/starter_world_scripts.rs:244`
  - `bins/sidereal-replication/src/replication/scripting.rs:47`
  - `bins/sidereal-replication/src/replication/scripting.rs:80`
  - `bins/sidereal-replication/src/replication/scripting.rs:180`
  - `bins/sidereal-replication/src/replication/scripting.rs:185`
  - `docs/features/scripting_support.md:1660`
  - `docs/features/scripting_support.md:1908`
- Concrete recommendation:
  Move shared world-init script config loading, graph record decoding, and runtime render-layer validation into `sidereal-scripting` or another neutral shared crate. Keep gateway- and replication-specific error translation at the boundary only.

## 3. Bevy / ECS Findings

### Finding B1: Large client and replication modules still mix unrelated responsibilities
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  Several files have grown into domain mixtures rather than coherent modules. That makes code review and future ownership harder and is one reason Clippy complexity warnings are concentrating in the same hotspots.
- Exact references:
  - `bins/sidereal-client/src/native/visuals.rs`
  - `bins/sidereal-client/src/native/backdrop.rs`
  - `bins/sidereal-client/src/native/ui.rs`
  - `bins/sidereal-replication/src/replication/scripting.rs`
  - `bins/sidereal-replication/src/replication/visibility.rs`
- Inference:
  File size alone is not proof of a bug, but here it aligns with concrete complexity warnings and duplicated ownership logic, so the maintenance risk is directly evidenced.
- Concrete recommendation:
  Split these modules by domain mutation boundary, not by arbitrary line count. Example: backdrop selection vs fullscreen material attachment vs HTTP texture resolution should not all live in one file.

### Finding B2: Some ECS system signatures are carrying too much state directly
- Severity: Medium
- Type: maintainability
- Priority: should fix
- Why it matters:
  The repeated `too_many_arguments` and `type_complexity` failures indicate that several systems are expressing domain coupling directly through oversized system parameters instead of using smaller resources, helper structs, or `SystemParam` wrappers.
- Exact references:
  - `bins/sidereal-replication/src/replication/input.rs:373`
  - `bins/sidereal-client/src/native/backdrop.rs:580`
  - `bins/sidereal-client/src/native/replication.rs:860`
  - `bins/sidereal-client/src/native/ui.rs:158`
  - `bins/sidereal-client/src/native/visuals.rs:1833`
- Concrete recommendation:
  Introduce explicit `SystemParam` bundles for recurring query/resource sets and move pure data-shaping steps into ordinary Rust helpers. This will also make system ordering intent easier to see.

## 4. Rendering Findings

### Finding R1: Fullscreen render-layer migration is incomplete and docs are stale
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  Lua world init is already authoring `RuntimeRenderLayerDefinition` records for fullscreen layers, but the client still falls back to legacy `FullscreenLayer` components and the docs still describe fullscreen bootstrap as pending migration. That means the migration status is ambiguous in both code and documentation.
- Exact references:
  - `data/scripts/world/world_init.lua:11`
  - `data/scripts/world/world_init.lua:22`
  - `data/scripts/world/world_init.lua:33`
  - `bins/sidereal-client/src/native/backdrop.rs:259`
  - `bins/sidereal-client/src/native/backdrop.rs:282`
  - `docs/features/scripting_support.md:1658`
- Concrete recommendation:
  Decide whether the migration is complete enough to remove legacy fullscreen compatibility. If yes, delete the fallback path and update docs. If no, update docs to state exactly which runtime still depends on `FullscreenLayer` and why.

### Finding R2: Shader/material routing is still partly content-specific in Rust
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The long-term contract is generic runtime behavior with concrete content owned by Lua-authored catalogs. The active shader registry still exposes named handles and slots for specific content concepts such as starfield, asteroid sprite, and tactical map overlay.
- Exact references:
  - `bins/sidereal-client/src/native/shaders.rs:164`
  - `bins/sidereal-client/src/native/shaders.rs:166`
  - `bins/sidereal-client/src/native/shaders.rs:170`
  - `bins/sidereal-client/src/native/shaders.rs:172`
  - `bins/sidereal-client/src/native/shaders.rs:174`
  - `bins/sidereal-client/src/native/shaders.rs:178`
  - `bins/sidereal-client/src/native/shaders.rs:214`
- Concrete recommendation:
  Push concrete shader selection fully behind render-layer/catalog metadata. Rust should keep family/domain behavior and fallback loading, not a hardcoded map of current game content.

### Finding R3: WASM-specific fallback shader ownership is still embedded in the client runtime
- Severity: Low
- Type: architecture, maintainability
- Priority: optional improvement
- Why it matters:
  This is not inherently wrong, but it is another sign that rendering still contains platform/content exceptions in runtime code instead of a cleaner data-driven fallback story.
- Exact references:
  - `bins/sidereal-client/src/native/shaders.rs:54`
  - `bins/sidereal-client/src/native/shaders.rs:109`
  - `bins/sidereal-client/src/wasm.rs:40`
- Concrete recommendation:
  Keep the WASM fallback mechanism, but move fallback shader source ownership into a narrower platform adapter module or generated asset artifact so runtime rendering code stays generic.

## 5. Physics / Avian2D Findings

### Finding P1: No significant Avian authority misuse stood out in the active runtime
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  The main authority path is still using fixed-step physics, zero gravity for the intended space runtime, and server-side ordering around Avian `PhysicsSystems`. This is one area where the current architecture is defensible.
- Exact references:
  - `bins/sidereal-client/src/native/mod.rs:117`
  - `bins/sidereal-client/src/native/mod.rs:147`
  - `bins/sidereal-replication/src/main.rs:133`
  - `bins/sidereal-replication/src/main.rs:152`
  - `bins/sidereal-replication/src/main.rs:244`
- Concrete recommendation:
  Keep the current fixed-step/single-writer discipline. Refactoring effort is better spent on ownership boundaries and complexity reduction.

## 6. Networking / Lightyear Findings

### Finding N1: Authenticated session binding remains strong and should be preserved
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  The code still rejects or drops mismatched claimed player IDs and ties realtime input acceptance to authenticated player bindings. That is the correct server-authoritative stance.
- Exact references:
  - `bins/sidereal-replication/src/replication/auth.rs:287`
  - `bins/sidereal-replication/src/replication/auth.rs:331`
  - `bins/sidereal-replication/src/replication/input.rs:233`
  - `bins/sidereal-replication/src/replication/input.rs:261`
  - `bins/sidereal-replication/src/replication/input.rs:279`
- Concrete recommendation:
  Preserve this contract during future refactors. Do not simplify input routing in a way that trusts wire-claimed IDs.

### Finding N2: Client parity direction has improved and the earlier “render shell only” diagnosis no longer applies
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  The WASM path now boots through `build_windowed_client_app()` and uses shared client configuration. That reduces platform drift and is a meaningful improvement from the older state.
- Exact references:
  - `bins/sidereal-client/src/wasm.rs:40`
  - `bins/sidereal-client/src/wasm.rs:69`
- Concrete recommendation:
  Keep converging from this shared bootstrap path instead of reintroducing a separate browser-only client shell.

## 7. Scripting / Lua Findings

### Finding S1: World-init/render-layer ownership is split across mature Lua data and lingering Rust compatibility paths
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The repository direction is clear: concrete content authoring should increasingly live in Lua. That transition is underway, but runtime ownership is still split in a way that keeps old Rust-side assumptions alive.
- Exact references:
  - `data/scripts/world/world_init.lua:11`
  - `data/scripts/world/world_init.lua:44`
  - `bins/sidereal-client/src/native/backdrop.rs:259`
  - `bins/sidereal-client/src/native/backdrop.rs:297`
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
- Concrete recommendation:
  Finish the migration by making runtime render-layer records the only authoritative fullscreen layer source, then remove legacy component compatibility and update the feature docs in the same change.

## 8. Persistence / Data Flow Findings

### Finding D1: Asset cache implementation still diverges from the documented pak/index contract
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The docs specify an MMO-style `assets.pak` plus companion index. The actual implementation still uses loose files by content type and `index.json`. This is a direct code/doc divergence in a critical runtime contract.
- Exact references:
  - `docs/features/asset_delivery_contract.md:226`
  - `docs/features/asset_delivery_contract.md:227`
  - `docs/features/asset_delivery_contract.md:228`
  - `docs/features/asset_delivery_contract.md:234`
  - `crates/sidereal-asset-runtime/src/lib.rs:242`
  - `crates/sidereal-asset-runtime/src/lib.rs:256`
  - `crates/sidereal-asset-runtime/src/lib.rs:272`
  - `bins/sidereal-client/src/native/auth_net.rs:367`
  - `bins/sidereal-client/src/native/auth_net.rs:387`
- Concrete recommendation:
  Either implement the documented pak/index cache or revise the contract to describe the current loose-file cache shape as the intended design. Do not leave both stories active.

### Finding D2: Asset delivery dependency handling has improved and should be retained
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  This is a notable improvement since the prior audit. The gateway now computes dependency closure and derives catalog versioning from catalog content rather than a fixed placeholder.
- Exact references:
  - `crates/sidereal-asset-runtime/src/lib.rs:52`
  - `crates/sidereal-asset-runtime/src/lib.rs:144`
  - `bins/sidereal-gateway/src/api.rs:372`
  - `bins/sidereal-gateway/src/api.rs:376`
  - `bins/sidereal-gateway/src/api.rs:398`
- Concrete recommendation:
  Keep this direction and make sure docs and client assumptions match it.

## 9. Redundancy / Dead Code Findings

### Finding X1: Replication scripting still contains dead exported helpers
- Severity: Medium
- Type: cleanup, maintainability
- Priority: should fix
- Why it matters:
  The live Clippy failure on `load_world_init_config()` confirms that some replication scripting surface area no longer has a caller. In a large module, dead exported helpers are a warning sign that responsibilities have drifted.
- Exact references:
  - `bins/sidereal-replication/src/replication/scripting.rs:1005`
- Concrete recommendation:
  Delete the dead helper if no longer needed, or move the shared implementation into a shared crate and consume that from the actual call sites.

### Finding X2: Residual placeholders and TODOs remain in active client paths
- Severity: Low
- Type: cleanup
- Priority: optional improvement
- Why it matters:
  These are not severe individually, but they show parts of the client runtime are still carrying visible transitional work.
- Exact references:
  - `bins/sidereal-client/src/native/visuals.rs:323`
  - `bins/sidereal-client/src/native/pause_menu.rs:1`
  - `bins/sidereal-client/src/native/render_layers.rs:672`
  - `bins/sidereal-client/src/native/render_layers.rs:675`
- Concrete recommendation:
  Triage them explicitly as either near-term work or dead transitional scaffolding. Do not let low-grade placeholders accumulate indefinitely.

## 10. Startup / Main Loop Flow Maps

### 10.1 Gateway startup and main loop

1. `bins/sidereal-gateway/src/main.rs:15` initializes logging, auth config, and the Postgres-backed auth store.
2. `bins/sidereal-gateway/src/main.rs:38` selects UDP or direct bootstrap dispatch.
3. `bins/sidereal-gateway/src/main.rs:50` builds `AuthService` from config, store, and dispatcher.
4. `bins/sidereal-gateway/src/main.rs:61` binds the HTTP listener and serves the Axum app produced by `app_with_service(service)`.
5. Operationally, the gateway is an HTTP/auth/bootstrap broker. It authenticates accounts, exposes account/player APIs, returns asset bootstrap manifests, serves asset bytes, and hands clients the replication transport bootstrap information.

### 10.2 Replication server startup and main loop

1. `bins/sidereal-replication/src/main.rs:72` parses CLI and configures logs/BRP.
2. `bins/sidereal-replication/src/main.rs:109` creates the Bevy app with `MinimalPlugins`, `AssetPlugin`, `ScenePlugin`, `SiderealGamePlugin`, Avian physics, and Lightyear server plugins.
3. `bins/sidereal-replication/src/main.rs:151` forces authoritative fixed-step time to 60 Hz.
4. `bins/sidereal-replication/src/main.rs:189` initializes domain resources: admin, visibility, simulation entities, auth, assets, input, persistence, control, runtime state, scripting, runtime scripting, owner manifests, tactical, lifecycle, and health.
5. `bins/sidereal-replication/src/main.rs:207` registers high-level plugins for lifecycle, diagnostics, auth, input, control, runtime scripting, visibility, persistence, and bootstrap bridge.
6. `bins/sidereal-replication/src/main.rs:217` runs the Update-chain that services transport/auth/input/control/asset-catalog maintenance and replication-group housekeeping.
7. `bins/sidereal-replication/src/main.rs:243` runs fixed-step simulation work around Avian `PhysicsSystems`.

### 10.3 Client startup and main loop

1. Native startup flows through `bins/sidereal-client/src/native/mod.rs:103`, which configures Bevy, Avian client-side physics, Lightyear client plugins, fixed-step time, and a large set of runtime resources.
2. WASM startup now reuses the shared builder via `bins/sidereal-client/src/wasm.rs:40`, swapping only browser-specific HTTP/cache adapters and render/window setup.
3. `bins/sidereal-client/src/native/plugins.rs:36` bootstraps the app-state flow: auth, character select, world loading, asset loading, and in-world scene setup.
4. `bins/sidereal-client/src/native/plugins.rs:91` wires transport/message systems.
5. `bins/sidereal-client/src/native/plugins.rs:124` wires replication adoption, transform synchronization, control handover, owner manifests, tactical snapshots, and runtime asset fetch state.
6. `bins/sidereal-client/src/native/plugins.rs:300` wires prediction, input send, rollback, interpolation, camera, visuals, UI, and debug/tactical systems.

### 10.4 Cross-service data and authority flow

1. Account auth begins at the gateway. The gateway validates credentials/account ownership and returns tokens plus bootstrap metadata.
2. Asset delivery is gateway HTTP-based. The gateway builds a runtime asset catalog/manifest and serves required asset bytes to the client.
3. The client caches asset payloads locally, materializes runtime-readable files, and uses those for render/runtime asset attachment.
4. Enter-world/bootstrap hands the client replication transport details so it can connect to replication.
5. Replication binds transport peers to authenticated player entities and rejects mismatched claimed player IDs before accepting realtime input.
6. Client input is intent only. Replication owns authoritative simulation, control state, and visibility filtering.
7. Persistence remains graph-record oriented: authoritative world/player state is loaded, hydrated, and later persisted by the replication-side persistence flow.
8. Scripting currently affects bootstrap/world-init/content definitions and runtime authoring, but the ownership boundary is still partially split between shared Lua data and duplicated Rust validation/load code.
9. Rendering remains client-local and data-driven in direction, but still retains some content-specific routing and compatibility layers in Rust.

## 11. Prioritized Remediation Plan

### Must fix

1. Restore workspace quality-gate compliance by eliminating current Clippy failures in replication and client runtime code.
2. Correct `sidereal_core::SIM_TICK_HZ` to 60 and remove local literal drift.
3. Decide whether legacy fullscreen compatibility is still required. If not, delete it and update docs in the same change.

### Should fix

1. Extract shared world-init/script validation into a shared crate and remove gateway/replication duplication.
2. Break down client bootstrap/resource ownership into smaller domain plugins.
3. Collapse duplicated headless/non-headless replication chains in the client plugin graph.
4. Resolve the asset-cache contract mismatch by either implementing pak/index or rewriting the docs to the actual loose-file design.

### Optional improvements

1. Continue moving concrete shader/content routing out of Rust and into data.
2. Clean up low-value placeholders/TODOs in active client paths.
3. Narrow WASM rendering fallback ownership so platform-specific exceptions are more localized.

## 12. Workspace / Runtime Catalog Appendix

### 12.1 Workspace crates and binaries

- `crates/sidereal-component-macros`
  - Responsibility: proc macros for component authoring/registration.
  - Status: active build-time support.
- `crates/sidereal-core`
  - Responsibility: core shared types, protocol constants, gateway DTOs, logging helpers, remote inspect config.
  - Status: active runtime shared library.
- `crates/sidereal-net`
  - Responsibility: Lightyear protocol definitions and shared network messages.
  - Status: active runtime shared library.
- `crates/sidereal-game`
  - Responsibility: gameplay components, shared simulation logic, hierarchy/mass/runtime gameplay rules.
  - Status: active runtime shared library.
- `crates/sidereal-persistence`
  - Responsibility: graph persistence, database mapping, hydration/persistence helpers.
  - Status: active runtime shared library.
- `crates/sidereal-runtime-sync`
  - Responsibility: shared runtime synchronization helpers between services/targets.
  - Status: active runtime shared library.
- `crates/sidereal-asset-runtime`
  - Responsibility: generated/runtime asset catalog building, dependency closure, cache-path helpers, cache index helpers.
  - Status: active runtime shared library.
- `crates/sidereal-shader-preview`
  - Responsibility: shader-preview tooling/runtime support.
  - Status: tooling-oriented, but still built in workspace.
- `crates/sidereal-scripting`
  - Responsibility: Lua scripting runtime and shared scripting support.
  - Status: active runtime shared library.
- `bins/sidereal-gateway`
  - Responsibility: account auth, player bootstrap, asset manifest/asset byte HTTP delivery, replication bootstrap coordination.
  - Status: active runtime binary.
- `bins/sidereal-replication`
  - Responsibility: authoritative simulation, auth binding, input handling, visibility, persistence, scripting-driven world/bootstrap orchestration.
  - Status: active runtime binary.
- `bins/sidereal-client`
  - Responsibility: native binary plus WASM library target for auth UI, asset bootstrap, transport, replication client, rendering, prediction, tactical/UI.
  - Status: active runtime binary/lib with some transitional compatibility code.

### 12.2 Major Bevy plugins, systems, and resources by runtime

#### Gateway

- Primary runtime shape
  - `sidereal-gateway` is not a Bevy app. It is a Tokio/Axum service runtime.
  - Status: active runtime.
- Major runtime responsibilities
  - `AuthService`
  - `BootstrapDispatcher` implementations
  - `app_with_service(...)` Axum wiring

#### Replication server

- App-level plugins
  - `MinimalPlugins`
  - `AssetPlugin`
  - `ScenePlugin`
  - `SiderealGamePlugin`
  - `PhysicsPlugins`
  - `ServerPlugins`
  - `LightyearInputProtocolPlugin`
  - Status: active runtime.
- High-level replication plugins
  - `ReplicationLifecyclePlugin`
  - `ReplicationDiagnosticsPlugin`
  - `ReplicationAuthPlugin`
  - `ReplicationInputPlugin`
  - `ReplicationControlPlugin`
  - `ReplicationRuntimeScriptingPlugin`
  - `ReplicationVisibilityPlugin`
  - `ReplicationPersistencePlugin`
  - `ReplicationBootstrapBridgePlugin`
  - Status: active runtime.
- Major resource domains initialized in `init_resources(...)`
  - admin command/state
  - visibility state
  - simulation entity bootstrap state
  - auth bindings
  - asset/script catalog state
  - input buffers/metrics
  - persistence workers/state
  - control state
  - runtime state tracking
  - scripting/runtime scripting state
  - owner manifest state
  - tactical state
  - lifecycle/health state
  - Status: active runtime.
- Update-chain responsibilities
  - transport channel/message component readiness
  - client auth receive/cleanup
  - script catalog reload polling
  - realtime input receive
  - control request receive
  - local view mode receive
  - metrics reporting
  - bootstrap entity command processing
  - replication group maintenance
  - idle disconnect handling
  - Status: active runtime.
- FixedUpdate responsibilities
  - authoritative simulation and motion enforcement around physics
  - Status: active runtime.

#### Client

- App/bootstrap plugins
  - Avian `PhysicsPlugins`
  - `ClientPlugins`
  - `LightyearAvianPlugin`
  - `FrameInterpolationPlugin`
  - `LightyearInputProtocolPlugin`
  - `NativeClientInputPlugin` on non-WASM only
  - Status: active runtime.
- Client domain plugins
  - `ClientBootstrapPlugin`
  - `ClientTransportPlugin`
  - `ClientReplicationPlugin`
  - `ClientPredictionPlugin`
  - plus scene/UI/audio/visual/tactical modules wired beneath them
  - Status: active runtime, with some transitional duplication.
- Major client resource groups visible in bootstrap
  - session/auth state
  - transport/input ack/log state
  - asset/cache/bootstrap state
  - shader assignments
  - control/view-mode state
  - debug overlay and debug toggles
  - tactical caches and UI state
  - hierarchy/fullscreen/backdrop/camera state
  - prediction tuning and ownership tracking
  - Status: active runtime, but ownership is too centralized in one bootstrap function.
- Transitional/migration items
  - legacy `FullscreenLayer` fallback in backdrop sync
  - content-specific shader slot registry
  - loose-file asset cache contract despite pak/index docs
  - some placeholder client UI/TODOs
  - Status: likely transitional/migration code.

#### Shared/runtime-adjacent crates

- `sidereal-game`
  - Bevy gameplay plugin and shared systems/components.
  - Status: active runtime shared.
- `sidereal-asset-runtime`
  - runtime asset catalog, dependency expansion, cache index helpers.
  - Status: active runtime shared.
- `sidereal-scripting`
  - shared Lua runtime support.
  - Status: active runtime shared, but not yet the sole owner of world-init validation helpers.

## 13. Closing Assessment

The server-authoritative spine is still in good shape. The current misalignment is mostly about engineering discipline: stale quality gates, duplicated ownership logic, and partially completed migrations. Fixing those will do more for codebase health than adding new features on top of the current client/scripting/asset boundaries.

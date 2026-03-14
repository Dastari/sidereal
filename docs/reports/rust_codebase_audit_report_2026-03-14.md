# Rust Codebase Audit Report

Status note 2026-03-14: fresh audit generated from prompt-guided code inspection of prompt files, source-of-truth docs, and code. I did not inspect existing files under `docs/reports/` or `docs/plans/`.

## Executive Summary

The codebase has a defensible high-level direction: shared gameplay in `sidereal-game`, server-authoritative replication, graph persistence, Lua-authored content metadata, and a single shared client runtime for native and WASM. The largest issues are not the foundational architecture but the amount of transitional runtime machinery now required to keep that architecture behaving.

The strongest recurring problem is accumulated repair code. Client presentation, visibility delivery, tactical UI, persistence scanning, and some gateway data paths all show signs of patch-style layering: the runtime works by stacking corrective systems and caches rather than by reducing the number of active ownership paths and hot loops. That raises performance cost, increases mental overhead, and makes regressions more likely.

The most important corrective action is simplification, not another layer of local fixes. The major systems worth keeping are the shared component registry path, the generic render-layer direction, graph persistence as canonical shape, and HTTP asset delivery. The areas that most need restructuring are client visual ownership, replication visibility, and large hot-path systems that mix multiple responsibilities.

## Architecture Findings

### 1. Duplicate-visual suppression and transform repair indicate unresolved motion/presentation ownership

- Severity: Critical
- Class: architecture, performance, maintainability
- Priority: must fix
- Why it matters: the client is carrying multiple systems whose purpose is to recover from clone/adoption/interpolation instability instead of presenting one clean runtime model. That is both a performance cost and an architecture smell.
- Exact references:
  - `bins/sidereal-client/src/runtime/transforms.rs:123-181`
  - `bins/sidereal-client/src/runtime/transforms.rs:184-223`
  - `bins/sidereal-client/src/runtime/transforms.rs:225-283`
  - `bins/sidereal-client/src/runtime/visuals.rs:529-790`
  - `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:22-52`
- Concrete recommendation: collapse adoption and presentation ownership so only one clone type is eligible to render for a GUID at a time. Treat stale-transform recovery and duplicate-visual winner selection as temporary migration code with a removal plan.

### 2. Replication visibility is a monolithic hot path mixing too many responsibilities

- Severity: Critical
- Class: architecture, performance
- Priority: must fix
- Why it matters: candidate generation, spatial index refresh, disclosure sync, cache maintenance, policy evaluation, and membership diffing all happen in one large area of code and in one tightly coupled schedule flow. That makes correctness harder to reason about and scaling harder to improve.
- Exact references:
  - `bins/sidereal-replication/src/plugins.rs:146-190`
  - `bins/sidereal-replication/src/replication/visibility.rs:1397-1515`
  - `bins/sidereal-replication/src/replication/visibility.rs:1616-2326`
- Concrete recommendation: split visibility into incremental cache maintenance, per-client context derivation, candidate narrowing, disclosure sync, and final membership diff stages with explicit resource boundaries and budget telemetry.

### 3. "Map mode" is overloaded between visual transition state and replication delivery state

- Severity: Medium
- Class: architecture, maintainability
- Priority: should fix
- Why it matters: pressing `M` enables the tactical map overlay and camera transition, but replication-facing `ClientLocalViewMode::Map` is tied to a deeper zoom threshold. That can be a valid design if server-side strategic delivery is meant to begin only after the transition completes. The problem is that the code does not make that distinction obvious, so the same term appears to describe two different states.
- Exact references:
  - `bins/sidereal-client/src/runtime/control.rs:141-154`
  - `crates/sidereal-game/src/components/tactical_map_ui_settings.rs:44-49`
  - `bins/sidereal-client/src/runtime/ui.rs:517-533`
  - `bins/sidereal-replication/src/replication/visibility.rs:1171-1217`
- Concrete recommendation: make the contract explicit in code and docs. Either rename the replication-facing state to reflect "strategic zoom delivery mode" or otherwise document that pressing `M` starts a visual transition while server map-delivery mode begins only after the camera reaches the deeper threshold.

### 4. Tactical UI code is doing too much in too few hot functions

- Severity: High
- Class: maintainability, performance
- Priority: should fix
- Why it matters: tactical mode currently mixes state transitions, camera behavior, HUD ownership, marker spawn/update/despawn, cursor display, SVG cache usage, overlay material updates, and fog texture generation inside hot per-frame paths. This is difficult to test, difficult to optimize, and difficult to reason about.
- Exact references:
  - `bins/sidereal-client/src/runtime/ui.rs:517-760`
  - `bins/sidereal-client/src/runtime/ui.rs:1197-1415`
- Concrete recommendation: split tactical UI into transition/state systems, marker data prep, render-material sync, and fog/cache maintenance. Most of those should not share one frame-frequency lane.

### 5. Render-layer system is defensible, but still architecturally transitional

- Severity: Medium
- Class: architecture, performance
- Priority: should fix
- Why it matters: the data-driven render-layer path is a good fit for the stated Lua-authored direction, but the current implementation still depends on broad runtime scans and hot-path compilation/resolution work.
- Exact references:
  - `bins/sidereal-client/src/runtime/render_layers.rs:20-250`
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
- Concrete recommendation: keep the model, but move invalidation and rule compilation further away from normal frame execution. This is a simplification task, not a rollback of the architecture.

## Bevy / ECS Findings

### 6. Client runtime schedule shape is broad and expensive

- Severity: High
- Class: performance, maintainability
- Priority: should fix
- Why it matters: multiple large `Update` and `PostUpdate` chains touch rendering, replication, assets, control, camera, and UI every frame. This increases schedule cost and makes it difficult to isolate steady-state work from one-time lifecycle work.
- Exact references:
  - `bins/sidereal-client/src/runtime/app_setup.rs:160-243`
  - `bins/sidereal-client/src/runtime/plugins/replication_plugins.rs:39-99`
  - `bins/sidereal-client/src/runtime/plugins/presentation_plugins.rs:29-132`
  - `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:18-93`
- Concrete recommendation: draw harder boundaries between lifecycle systems, relevance/adoption systems, and steady-state presentation systems. Default more systems to event-driven or explicit state gating.

### 7. `configure_client_runtime` is becoming a monolithic wiring point

- Severity: Medium
- Class: maintainability
- Priority: should fix
- Why it matters: startup resource creation, physics setup, network plugins, audio systems, combat systems, and client plugin registration are all wired from one large function. The function is still coherent, but it is trending toward "everything important happens here."
- Exact references:
  - `bins/sidereal-client/src/runtime/app_setup.rs:108-244`
- Concrete recommendation: split runtime wiring into domain-specific setup functions or plugin groups with clear ownership, especially for transport, prediction/control, presentation, and tactical/UI.

## Rendering Findings

### 8. The code supports a more complicated render path than the current runtime stability warrants

- Severity: Medium
- Class: performance, maintainability
- Priority: should fix
- Why it matters: multiple cameras, multiple fullscreen/post-process layers, planet-body passes, and eight `Material2dPlugin`s are not inherently wrong, but they amplify the cost of every other hot-path problem.
- Exact references:
  - `bins/sidereal-client/src/runtime/app_builder.rs:24-36`
  - `bins/sidereal-client/src/runtime/scene_world.rs:68-208`
- Concrete recommendation: hold the current visual ambition, but avoid adding more pass diversity until frame-pacing and ownership issues are under control.

## Physics / Avian2D Findings

### 9. No major Avian misuse is obvious; the main problem is the amount of sync code wrapped around it

- Severity: Medium
- Class: architecture
- Priority: should fix
- Why it matters: disabling Bevy/Avian transform interpolation plugins and manually owning sync lanes can be valid in a server-authoritative client. The issue is that the surrounding corrective systems now do a lot of work to keep that model stable.
- Exact references:
  - `bins/sidereal-client/src/runtime/app_setup.rs:115-139`
  - `bins/sidereal-client/src/runtime/transforms.rs:123-283`
- Concrete recommendation: do not reintroduce ad hoc transform writers. Instead, reduce the number of runtime handoff states so the current manual sync model needs fewer safeguards.

## Networking / Lightyear Findings

### 10. Lightyear is being used seriously, but too much local repair code is compensating for runtime churn

- Severity: High
- Class: architecture, performance
- Priority: must fix
- Why it matters: the project is correctly using prediction, interpolation, frame interpolation, rollback visual correction ordering, and shared protocol registration. The problem is not that Lightyear is absent; the problem is that local runtime shape still creates enough churn to need extra suppression and repair layers.
- Exact references:
  - `bins/sidereal-client/src/runtime/app_setup.rs:124-139`
  - `bins/sidereal-client/src/runtime/plugins/ui_plugins/post_update.rs:22-40`
  - `bins/sidereal-client/src/runtime/visuals.rs:529-790`
- Concrete recommendation: simplify clone adoption and visibility/despawn churn before trying to tune Lightyear harder. The next win is architectural, not a transport knob.

## Scripting / Lua Findings

### 11. Lua-authored asset/render direction is sound, but runtime scripting snapshots rebuild broadly every tick

- Severity: Medium
- Class: performance, maintainability
- Priority: should fix
- Why it matters: the scripting direction is aligned with repo goals, but `refresh_script_world_snapshot` clears and rebuilds the script-visible entity map every run. That is simple and safe, but it scales poorly.
- Exact references:
  - `bins/sidereal-replication/src/replication/runtime_scripting.rs:227-256`
- Concrete recommendation: move toward incremental snapshot maintenance keyed by entity GUID changes, or at minimum gate rebuild frequency when script intervals do not need fresh full-world state.

## Persistence / Data Flow Findings

### 12. Persistence flush still does a full-entity fingerprint pass on every flush interval

- Severity: Medium
- Class: performance
- Priority: should fix
- Why it matters: the persistence path already tracks dirtiness, but it still iterates all persistent entities, serializes component records, computes fingerprints, and compares them. This is safe but expensive as world size grows.
- Exact references:
  - `bins/sidereal-replication/src/replication/persistence.rs:282-406`
- Concrete recommendation: preserve graph-record persistence shape, but move toward narrower dirty-component/entity accounting so full serialization is not the normal flush strategy.

### 13. Gateway data layer mixes async and blocking Postgres access styles

- Severity: Medium
- Class: maintainability
- Priority: optional improvement
- Why it matters: most auth-store operations use `tokio_postgres::Client`, but account creation and direct bootstrap dispatch jump to blocking `postgres::Client` / blocking graph persistence inside `spawn_blocking`. This inconsistency increases cognitive load and makes the data layer feel less intentional.
- Exact references:
  - `bins/sidereal-gateway/src/auth/store.rs:63-190`
  - `bins/sidereal-gateway/src/auth/bootstrap_dispatch.rs:105-161`
- Concrete recommendation: standardize the gateway’s persistence/data-access pattern. Blocking isolation is acceptable where necessary, but it should look deliberate and shared, not piecemeal.

## Redundancy / Dead Code Findings

### 14. Some compatibility and placeholder paths are still active with little apparent value

- Severity: Low
- Class: cleanup, maintainability
- Priority: optional improvement
- Why it matters: small pieces of no-op or placeholder runtime code are still scheduled or registered, which makes the active runtime harder to audit.
- Exact references:
  - `crates/sidereal-game/src/actions.rs:133-137`
  - `crates/sidereal-game/src/lib.rs:157-170`
  - `bins/sidereal-client/src/runtime/plugins/ui_plugins/diagnostics_plugin.rs:1-6`
  - `bins/sidereal-client/src/runtime/app_setup.rs:237-243`
- Concrete recommendation: remove or clearly label transitional runtime stubs. The empty diagnostics plugin and no-op action-capability validator are good candidates.

## Architecture Worth Keeping

- Shared gameplay/component source of truth in `crates/sidereal-game` is correct.
- Graph records as canonical persistence shape are correct and match repo rules.
- HTTP asset bootstrap and authenticated asset fetches through gateway are correct.
- Lua-authored render layers and asset metadata are directionally correct.
- One client crate with native/WASM targets is correct and aligns with the repo constraints.

## Startup / Main Loop Flow Maps

### Gateway startup and main loop

- `bins/sidereal-gateway/src/main.rs:16-70` parses CLI/env, prepares a timestamped log file, builds auth config, opens Postgres, ensures schema, chooses UDP or direct bootstrap dispatcher, creates `AuthService`, binds Axum, and serves HTTP.
- Runtime flow: HTTP routes authenticate users, issue tokens, expose character and world-entry flows, serve asset bootstrap manifest and asset bytes, and provide admin/script endpoints through the same service/router (`bins/sidereal-gateway/src/api.rs:73-105`, `372-460`).

### Replication server startup and main loop

- `bins/sidereal-replication/src/main.rs:41-167` parses config, configures logs and BRP, builds a headless Bevy app, installs `SiderealGamePlugin`, Avian physics, Lightyear server/input plugins, shared protocol, and replication domain plugins.
- `Update` is intentionally uncapped by default to avoid bunching network drain work (`bins/sidereal-replication/src/main.rs:84-96`).
- `FixedUpdate` owns authoritative simulation cadence through shared tick rate and domain plugins, including visibility, persistence, input, control, runtime scripting, tactical streaming, and lifecycle work (`bins/sidereal-replication/src/main.rs:192-240`, `bins/sidereal-replication/src/plugins.rs:146-190`).

### Client startup and main loop

- Native entry configures windowing, render plugin, logging, BRP, and either windowed or headless runtime (`bins/sidereal-client/src/platform/native/entry.rs:46-115`).
- `configure_client_runtime` installs physics, Lightyear client plugins, frame interpolation, shared protocol, fixed time, transport/assets/control/tactical resources, and runtime domain plugins (`bins/sidereal-client/src/runtime/app_setup.rs:108-244`).
- Runtime loop shape:
  - `Update`: replication adoption, asset dependency/fetch work, control sync, visuals/lights, audio
  - `FixedUpdate`: local motion/control/gameplay before and after physics
  - `PostUpdate`: hierarchy sync, interpolation-aware camera follow, visual transforms, UI placement
  - `Last`: fullscreen backdrop/debug draw finalization

### Cross-service data / authority / persistence / replication / asset / scripting / rendering flow

- Authority: gateway authenticates account and world entry, replication binds client identity to server-side player entity, replication sim owns authoritative state, client presents predicted/interpolated views.
- Persistence: replication serializes ECS entities/components into graph records and writes them through persistence worker batches (`bins/sidereal-replication/src/replication/persistence.rs:282-406`).
- Replication: server visibility decides delivery membership, Lightyear transports state, client adopts and presents cloned entities.
- Asset delivery: gateway builds runtime asset/audio catalogs from Lua-authored registry scripts and serves authenticated bootstrap manifests and asset fetches (`bins/sidereal-gateway/src/api.rs:372-460`, `564-690`).
- Scripting: script catalogs come from persistence-backed or disk-backed sources, replication runtime scripting builds a world snapshot and runs interval handlers, while Lua-authored registries feed asset/render behavior.
- Rendering: client render layers, streamed visual attachments, fullscreen/backdrop passes, tactical overlays, and interpolation-aware camera sync convert replicated world state into final 2D presentation.

## Prioritized Remediation Plan

1. Collapse client visual ownership and remove the need for duplicate-visual suppression and stalled-transform repair as normal runtime behavior.
2. Split replication visibility into smaller subsystems with narrower caches and explicit budgets.
3. Clarify the contract between tactical-map UI transition state and replication-side strategic delivery mode so the current threshold behavior is clearly intentional.
4. Break tactical UI/rendering into smaller systems and remove the per-frame fog-mask CPU rebuild.
5. Convert render-layer registry/assignment maintenance toward invalidation-driven work.
6. Narrow persistence and scripting hot paths so full-world scans are not the default steady-state pattern.
7. Remove placeholder and no-op runtime code that no longer earns its maintenance cost.

## Workspace / Runtime Catalog Appendix

### Workspace members

- `bins/sidereal-client`: active runtime. Native and WASM Bevy client, transport/bootstrap/prediction/presentation/tactical UI.
- `bins/sidereal-gateway`: active runtime. Auth service, world-entry API, asset bootstrap and authenticated asset delivery, admin/script endpoints.
- `bins/sidereal-replication`: active runtime. Authoritative simulation, Lightyear server, visibility, persistence, tactical and asset streaming, runtime scripting.
- `crates/sidereal-game`: active runtime. Core gameplay ECS components and systems shared across runtimes.
- `crates/sidereal-net`: active runtime. Shared Lightyear protocol/message registration.
- `crates/sidereal-core`: active runtime. Shared constants, DTOs, logging, remote inspect config, bootstrap wire types.
- `crates/sidereal-persistence`: active runtime. Graph persistence and schema helpers.
- `crates/sidereal-runtime-sync`: active runtime. Runtime entity hierarchy/sync support.
- `crates/sidereal-asset-runtime`: active runtime. Runtime asset catalog construction and asset materialization.
- `crates/sidereal-scripting`: active runtime. Lua loading, registry parsing, scripting support.
- `crates/sidereal-ui`: active runtime. Shared Bevy UI theme/widgets.
- `crates/sidereal-audio`: active runtime. Audio registry/runtime support and validation.
- `crates/sidereal-component-macros`: active runtime/tooling support. Shared component authoring macros.
- `crates/sidereal-shader-preview`: tooling-only. Shader preview support.

### Major active client plugins / systems / resources

- Core plugins:
  - `ClientPlugins`, `LightyearAvianPlugin`, `FrameInterpolationPlugin<Transform>`, `LightyearInputProtocolPlugin`, native `ClientInputPlugin` on non-WASM.
  - Responsibility: transport, prediction, interpolation, protocol wiring.
- Runtime domain plugins:
  - `ClientBootstrapPlugin`, `ClientTransportPlugin`, `ClientReplicationPlugin`, `ClientPredictionPlugin`, `ClientVisualsPlugin`, `ClientLightingPlugin`, `ClientUiPlugin`.
  - `ClientDiagnosticsPlugin`: scaffold/placeholder; currently empty.
- Major active resources:
  - transport/session resources in `init_transport_resources`
  - asset runtime resources in `init_asset_runtime_resources`
  - control/prediction resources in `init_control_and_prediction_resources`
  - tactical/UI resources in `init_tactical_resources`
  - scene/render resources in `init_scene_and_render_resources`
- Transitional client code:
  - duplicate-visual suppression
  - transform history bootstrap and stalled-transform recovery
  - render-layer registry/assignment hot-path maintenance

### Major active replication plugins / systems / resources

- `ReplicationLifecyclePlugin`: active runtime. Transport lifecycle and entity-scoped replication plumbing.
- `ReplicationDiagnosticsPlugin`: active runtime. Metrics and runtime health/logging support.
- `ReplicationAuthPlugin`: active runtime. Session/player binding and auth state.
- `ReplicationInputPlugin`: active runtime. Input receipt and routing.
- `ReplicationControlPlugin`: active runtime. Controlled-entity and control request handling.
- `ReplicationRuntimeScriptingPlugin`: active runtime. Interval-driven runtime scripting.
- `ReplicationVisibilityPlugin`: active runtime. Visibility cache, candidate generation, disclosure, membership updates.
- `ReplicationPersistencePlugin`: active runtime. Dirty tracking, flush batching, worker metrics.
- `ReplicationBootstrapBridgePlugin`: active runtime. Bootstrap command bridge.
- Major resources from `init_resources`:
  - visibility, simulation entity, auth, asset, input, persistence, control, runtime state, scripting, tactical, lifecycle, and health resources.
- Transitional/high-risk area:
  - `replication/visibility.rs` is active runtime but also clearly transitional in shape because it centralizes too many concerns in one file.

### Major active gateway modules

- `auth/*`: active runtime. Auth service, token issuance, account ownership, bootstrap dispatch.
- `api.rs`: active runtime. Route definitions, asset bootstrap manifest, authenticated asset fetch, admin/script endpoints.
- Transitional or inconsistent area:
  - mixed async and blocking database/persistence usage in auth store and direct bootstrap dispatch.

### Test-only / scaffold / transitional notes

- `ClientDiagnosticsPlugin`: scaffold/placeholder.
- `validate_action_capabilities`: likely legacy compatibility hook still scheduled.
- Duplicate-visual winner selection and transform recovery systems: active runtime, but clearly transitional/migration code rather than desirable end-state architecture.

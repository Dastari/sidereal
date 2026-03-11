# Rust Codebase Audit Report

Date: 2026-03-12
Scope: Rust workspace
Prompt source: `docs/prompts/rust_codebase_audit_prompt.md`

Update note (2026-03-12):
- This re-audits the live tree after the 2026-03-11 Rust audit and after the 2026-03-12 rendering audit.
- This pass was static only. I did not rely on `cargo clippy` or full workspace validation because the tree is being changed concurrently.
- The March 11 report's top visibility finding is now stale in its original form: visibility is no longer one monolithic scratch rebuild. The server now has a staged pipeline with persistent caches, a spatial index, landmark cadence control, and telemetry, but the final apply lane is still a major cost/risk.

## 1. Executive Summary

The codebase is still strongest where the project has been strictest: authenticated server authority, fixed-step simulation, UUID-based boundary contracts, shared client runtime composition, and graph-shaped persistence remain directionally correct and should be kept.

The highest-value problems have shifted:

1. A likely client-render correctness regression exists in the runtime shader assignment sync path: the dirty-marking system exists, but it does not appear to be scheduled anywhere, while the sync system now early-returns unless the state is dirty or the catalog reloads.
2. Replication visibility is materially improved compared with March 11, but the final per-entity/per-client membership apply loop is still the dominant remaining scale risk.
3. The native client runtime is still too monolithic, especially in rendering/presentation code.
4. Asset delivery code still diverges from the documented `assets.pak` / `assets.index` contract.
5. Transitional fullscreen/render-layer compatibility and broad runtime scripting snapshots are still alive longer than they should be.

## 2. Architecture Findings

### Finding A1: Visibility architecture is improved, but the final membership apply lane is still a high-risk hot path
- Severity: High
- Type: architecture, performance
- Priority: should fix
- Why it matters:
  The March 11 framing is no longer accurate. Visibility now has persistent caches and an ordered staged pipeline instead of one broad rebuild. That is real progress. The remaining issue is narrower: `update_network_visibility` still walks replicated entities and then iterates client visibility state to compute and diff desired membership. That keeps the most expensive part of the problem in the authoritative fixed-tick path.
- Exact references:
  - `bins/sidereal-replication/src/plugins.rs:103`
  - `bins/sidereal-replication/src/plugins.rs:120`
  - `bins/sidereal-replication/src/replication/visibility.rs:240`
  - `bins/sidereal-replication/src/replication/visibility.rs:682`
  - `bins/sidereal-replication/src/replication/visibility.rs:928`
  - `bins/sidereal-replication/src/replication/visibility.rs:1531`
  - `bins/sidereal-replication/src/replication/visibility.rs:1636`
  - `bins/sidereal-replication/src/replication/visibility.rs:2083`
  - `bins/sidereal-replication/src/replication/visibility.rs:2291`
- Concrete recommendation:
  Keep the staged design. Do not revert to broad rebuild logic. Next, isolate and profile only the membership apply lane, then move it toward changed-client / changed-region / changed-entity deltas rather than a full replicated-entity sweep each tick.

### Finding A2: The native client runtime is still overly monolithic
- Severity: High
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  Runtime composition is cleaner than before, but ownership is still concentrated in a few large files. `native/mod.rs` still inserts large resource families and wires fixed/update/render scheduling. `plugins.rs`, `visuals.rs`, `backdrop.rs`, and `debug_overlay.rs` still carry too many responsibilities at once. That makes schedule ordering and regression review harder than it should be.
- Exact references:
  - `bins/sidereal-client/src/native/mod.rs:109`
  - `bins/sidereal-client/src/native/mod.rs:187`
  - `bins/sidereal-client/src/native/mod.rs:236`
  - `bins/sidereal-client/src/native/plugins.rs:268`
  - `bins/sidereal-client/src/native/visuals.rs`
  - `bins/sidereal-client/src/native/backdrop.rs`
  - `bins/sidereal-client/src/native/debug_overlay.rs`
- Concrete recommendation:
  Split by runtime ownership boundary, not by arbitrary line count:
  - render-layer registry and assignment
  - streamed visuals
  - fullscreen/backdrop/post-process
  - tactical map and nameplates
  - debug/diagnostics
  - prediction/adoption/control handoff

### Finding A3: Asset cache implementation still diverges from the documented contract
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The docs still describe a packed cache model with `assets.pak`, `assets.index`, and transactional recovery. The live code still materializes loose files under `published_assets` and persists client/shared cache metadata in `data/cache_stream/index.json`. This is a meaningful contract divergence in a core operational subsystem.
- Exact references:
  - `docs/features/asset_delivery_contract.md:223`
  - `crates/sidereal-asset-runtime/src/lib.rs:190`
  - `crates/sidereal-asset-runtime/src/lib.rs:198`
  - `crates/sidereal-asset-runtime/src/lib.rs:256`
  - `bins/sidereal-gateway/src/api.rs:360`
  - `bins/sidereal-gateway/src/api.rs:404`
- Concrete recommendation:
  Either implement the documented packed-cache contract or update the docs to explicitly describe the current loose-file materialization model as the active contract. Do not keep both stories alive.

### Finding A4: Runtime scripting still rebuilds and clones broad world snapshot state on the fixed path
- Severity: Medium
- Type: performance, maintainability
- Priority: should fix
- Why it matters:
  `refresh_script_world_snapshot()` clears and rebuilds the entire GUID-indexed snapshot each tick, and `run_script_intervals()` clones it into an `Rc<HashMap<...>>` before iterating handlers. That is broad fixed-step work for a runtime that is supposed to stay authoritative and scalable.
- Exact references:
  - `bins/sidereal-replication/src/plugins.rs:220`
  - `bins/sidereal-replication/src/replication/runtime_scripting.rs:228`
  - `bins/sidereal-replication/src/replication/runtime_scripting.rs:258`
  - `bins/sidereal-replication/src/replication/runtime_scripting.rs:287`
- Concrete recommendation:
  Move toward dirty/scoped script visibility, or at minimum split snapshots by concern so interval handlers do not require a fresh full-entity world map every fixed step.

### Finding A5: The core authority and timing model remains defensible and should be preserved
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  Server authority, shared fixed tick, Lightyear protocol registration, and client/server separation of concerns still line up with the design docs and AGENTS constraints. The current problems are mostly around runtime maintenance and hot-path breadth, not a broken authority model.
- Exact references:
  - `bins/sidereal-replication/src/main.rs:84`
  - `bins/sidereal-replication/src/main.rs:120`
  - `bins/sidereal-client/src/native/mod.rs:194`
  - `bins/sidereal-client/src/native/mod.rs:224`
  - `bins/sidereal-gateway/src/main.rs:39`
- Concrete recommendation:
  Keep the authority flow and shared fixed-step model intact while simplifying the surrounding runtime.

## 3. Bevy / ECS Findings

### Finding B1: Runtime shader assignment dirty tracking appears unscheduled
- Severity: High
- Type: correctness, maintainability
- Priority: must fix
- Why it matters:
  `RuntimeShaderAssignmentSyncState` now gates `sync_runtime_shader_assignments_system()` behind `dirty || catalog_reloaded`. The dirty-marking system exists and watches exactly the right change/removal signals, but it does not appear to be scheduled anywhere in the client runtime. If so, runtime shader assignment changes after startup will stop propagating unless the asset catalog reloads. That is a live behavior regression, not just cleanup.
- Exact references:
  - `bins/sidereal-client/src/native/shaders.rs:316`
  - `bins/sidereal-client/src/native/shaders.rs:655`
  - `bins/sidereal-client/src/native/shaders.rs:721`
  - `bins/sidereal-client/src/native/plugins.rs:284`
  - search evidence: `rg -n "mark_runtime_shader_assignments_dirty_system|sync_runtime_shader_assignments_system" bins/sidereal-client/src/native -S`
- Concrete recommendation:
  Schedule `mark_runtime_shader_assignments_dirty_system` in the same runtime path as the sync system, before the sync system runs. Add a regression test proving that changes to `RuntimeRenderLayerDefinition`, `SpriteShaderAssetId`, `StreamedSpriteShaderAssetId`, `TacticalMapUiSettings`, `PlanetBodyShaderSettings`, or `ProceduralSprite` cause assignment recomputation without a catalog reload.

### Finding B2: Large files remain the main codebase consistency smell
- Severity: Medium
- Type: maintainability
- Priority: should fix
- Why it matters:
  The workspace no longer looks weak because of warning debt or gross style violations. The bigger consistency problem is that a few large files mix many domains, so each file develops its own local conventions and schedule assumptions.
- Exact references:
  - `bins/sidereal-client/src/native/visuals.rs` (2787 lines)
  - `bins/sidereal-client/src/native/backdrop.rs` (1962 lines)
  - `bins/sidereal-client/src/native/debug_overlay.rs` (1342 lines)
  - `bins/sidereal-replication/src/replication/visibility.rs` (2800 lines)
  - `bins/sidereal-client/src/native/plugins.rs` (589 lines)
  - `bins/sidereal-client/src/native/mod.rs` (451 lines)
- Concrete recommendation:
  Stop adding new behavior to these files except for extraction work. Treat them as decomposition targets, not extension points.

### Finding B3: `ClientDiagnosticsPlugin` is still an empty scaffold
- Severity: Low
- Type: cleanup
- Priority: optional improvement
- Why it matters:
  Empty runtime plugin shells make client composition harder to read and imply ownership that does not actually exist.
- Exact references:
  - `bins/sidereal-client/src/native/plugins.rs:585`
- Concrete recommendation:
  Either delete it until it owns real diagnostics behavior or move actual diagnostics ownership into it.

## 4. Rendering Findings

### Finding R1: Fullscreen/render-layer migration is still partially transitional
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The authored runtime render-layer model is live, but the fullscreen sync path still accepts legacy `FullscreenLayer` input and synthesizes `legacy:*` layer IDs. The scripting docs also still say world init seeds legacy fullscreen layers. That means the migration is not complete and transitional compatibility is still in active runtime code.
- Exact references:
  - `bins/sidereal-client/src/native/backdrop.rs:85`
  - `bins/sidereal-client/src/native/backdrop.rs:141`
  - `bins/sidereal-client/src/native/backdrop.rs:291`
  - `docs/features/scripting_support.md:1844`
  - `docs/features/scripting_support.md:1850`
- Concrete recommendation:
  Finish the migration to authored runtime render layers. Remove legacy fullscreen compatibility once the remaining world-init/bootstrap producers are updated in the same change.

### Finding R2: The render-layer architecture itself is still the right direction
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  Runtime layer definitions, phase/domain validation, and the shader-family model are a better long-term fit than hardcoded rendering branches. The issue is not the architecture. The issue is incomplete migration and residual content-specific logic around it.
- Exact references:
  - `bins/sidereal-client/src/native/render_layers.rs`
  - `bins/sidereal-client/src/native/shaders.rs`
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
  - `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`
- Concrete recommendation:
  Preserve the authored render-layer / shader-family direction and remove the remaining compatibility paths around it.

## 5. Physics / Avian2D Findings

### Finding P1: No major Avian misuse stood out in the active authority path
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  The active runtime still uses fixed schedules, explicit motion ownership discipline, and Avian authoritative motion components without reintroducing the old mirror-motion approach prohibited by AGENTS.
- Exact references:
  - `bins/sidereal-client/src/native/mod.rs:236`
  - `bins/sidereal-replication/src/plugins.rs:68`
  - `bins/sidereal-replication/src/main.rs:129`
- Concrete recommendation:
  Keep the current authority and mass/integration discipline. The larger wins are elsewhere.

## 6. Networking / Lightyear Findings

### Finding N1: Shared client runtime composition across native and WASM is materially healthier and should be kept
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  The client still routes both headless/windowed/native/WASM setups through a shared runtime configuration path rather than diverging into separate gameplay runtimes. That keeps later parity recovery tractable.
- Exact references:
  - `bins/sidereal-client/src/native/mod.rs:187`
  - `bins/sidereal-client/src/native/mod.rs:287`
  - `bins/sidereal-client/src/native/mod.rs:313`
  - `bins/sidereal-client/src/main.rs`
- Concrete recommendation:
  Continue keeping platform differences at the transport and platform-IO boundary only.

## 7. Scripting / Lua Findings

### Finding S1: Script/bootstrap ownership is improved, but world-init still carries legacy rendering assumptions
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  Shared extraction into `sidereal-scripting` is real progress, but the scripting support docs still explicitly call out legacy fullscreen seeding during world init. That means content/bootstrap is still encoding rendering migration debt.
- Exact references:
  - `docs/features/scripting_support.md:1844`
  - `docs/features/scripting_support.md:1847`
  - `docs/features/scripting_support.md:1850`
- Concrete recommendation:
  Use world-init/bootstrap cleanup to remove the last legacy fullscreen assumptions, not just the client-side compatibility consumer.

## 8. Persistence / Data Flow Findings

### Finding D1: UUID boundary and graph persistence shape remain solid
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  Nothing in this pass suggested a regression toward raw Bevy `Entity` leakage across service boundaries or toward side persistence shapes that violate the graph-record model.
- Exact references:
  - `crates/sidereal-persistence/src/lib.rs`
  - `crates/sidereal-runtime-sync/src/lib.rs`
  - `AGENTS.md`
- Concrete recommendation:
  Preserve the identity and persistence contract while simplifying runtime orchestration around it.

## 9. Redundancy / Dead Code Findings

### Finding X1: Transitional compatibility is now concentrated rather than everywhere, but it still needs explicit deletion work
- Severity: Medium
- Type: cleanup, maintainability
- Priority: should fix
- Why it matters:
  The codebase is cleaner than March 10 and March 11, but a recognizable class of transitional code remains clustered in rendering and presentation paths: legacy fullscreen support, empty diagnostics scaffolding, and broad duplicate/assignment management systems. That is now targeted cleanup work rather than a repo-wide problem.
- Exact references:
  - `bins/sidereal-client/src/native/backdrop.rs:291`
  - `bins/sidereal-client/src/native/plugins.rs:585`
  - `bins/sidereal-client/src/native/shaders.rs:655`
- Concrete recommendation:
  Treat these as explicit deletion/migration tasks with owners and follow them through to removal.

## 10. Startup / Main Loop Flow Maps

### Gateway startup and main loop

1. CLI/env config is parsed and applied in `bins/sidereal-gateway/src/main.rs:18`.
2. Timestamped tracing output is initialized in `bins/sidereal-gateway/src/main.rs:27`.
3. Auth config and Postgres backing store are created and schema-ensured in `bins/sidereal-gateway/src/main.rs:39`.
4. Gateway selects a bootstrap dispatcher mode and builds `AuthService` in `bins/sidereal-gateway/src/main.rs:49`.
5. Axum binds the TCP listener and serves the API in `bins/sidereal-gateway/src/main.rs:63`.
6. Runtime asset delivery happens through authenticated manifest and `/assets/<guid>` endpoints in `bins/sidereal-gateway/src/api.rs:360` and `bins/sidereal-gateway/src/api.rs:404`.

### Replication server startup and main loop

1. CLI/env, logging, BRP config, and headless/TUI mode are resolved in `bins/sidereal-replication/src/main.rs:41`.
2. The app is built with `MinimalPlugins`, `AssetPlugin`, `ScenePlugin`, `SiderealGamePlugin`, Avian physics, Lightyear server plugins, and protocol registration in `bins/sidereal-replication/src/main.rs:84`.
3. Shared fixed tick and runtime resources are inserted in `bins/sidereal-replication/src/main.rs:129` and `bins/sidereal-replication/src/main.rs:192`.
4. Runtime behavior is then divided into plugins:
   - lifecycle/auth/bootstrap
   - input/control/combat
   - runtime scripting
   - visibility/streaming
   - persistence/diagnostics
   Refs: `bins/sidereal-replication/src/main.rs:210`, `bins/sidereal-replication/src/plugins.rs:14`.
5. `Update` handles transport/auth/control/admin/catalog polling and persistence metrics in `bins/sidereal-replication/src/main.rs:220`.
6. `FixedUpdate` handles authoritative simulation, visibility, streaming, and persistence flush ordering in `bins/sidereal-replication/src/plugins.rs:64`, `bins/sidereal-replication/src/plugins.rs:103`, and `bins/sidereal-replication/src/plugins.rs:193`.

### Client startup and main loop

1. Native startup parses env/CLI and chooses headless vs windowed app construction in `bins/sidereal-client/src/native/mod.rs:348`.
2. Windowed mode installs Bevy default plugins, material plugins, SVG support, diagnostics, logging, and WGPU configuration in `bins/sidereal-client/src/native/mod.rs:313`.
3. Shared runtime wiring then installs Avian, Lightyear client plugins, protocol registration, fixed tick, transport resources, asset resources, prediction/control resources, tactical/UI resources, and scene/render resources in `bins/sidereal-client/src/native/mod.rs:187`.
4. Fixed-step gameplay and motion ownership systems are wired in `bins/sidereal-client/src/native/mod.rs:236`.
5. Client behavior is split into plugin groups:
   - bootstrap
   - transport
   - replication
   - prediction
   - visuals
   - lighting
   - UI
   - diagnostics scaffold
   Refs: `bins/sidereal-client/src/native/mod.rs:266`, `bins/sidereal-client/src/native/plugins.rs:30`.
6. In the visuals path, update-time work includes runtime shader assignment sync, render-layer registry sync, duplicate visual suppression, streamed visual attachment, fullscreen/backdrop sync, and effect pool maintenance in `bins/sidereal-client/src/native/plugins.rs:268`.

### Cross-service data / authority / persistence / asset / scripting / rendering flow

1. Gateway authenticates accounts/sessions, exposes authenticated bootstrap metadata, and serves asset manifests and asset payloads over HTTP.
2. Replication owns authoritative simulation and fixed-step gameplay. Client input flows into replication through authenticated Lightyear transport, is applied server-side, then replicated back outward.
3. Persistence remains server-owned and flushes after authoritative fixed-step work, not from the client.
4. Asset payload bytes do not travel over replication. Gateway serves the bootstrap manifest plus `/assets/<guid>` downloads; the client/runtime asset layer fetches, caches, and exposes bytes to render/runtime systems.
5. Runtime scripting currently lives on the replication fixed path and observes a GUID-indexed snapshot of script-visible entities.
6. Rendering remains client-only. Replication sends the data needed to decide visibility, render layers, and streamed visual bindings; the client resolves those into actual Bevy renderables/materials.

## 11. Prioritized Remediation Plan

1. Fix the shader assignment dirty-path regression.
   `mark_runtime_shader_assignments_dirty_system` should be scheduled immediately and covered by a regression test.
2. Profile and narrow the visibility apply loop.
   The architecture win is already in place; the next task is to stop the final membership diff from remaining an O(entities x clients) style fixed-step burden.
3. Resolve the asset cache contract divergence.
   Pick one truth: packed cache or documented loose-file cache. Align code and docs together.
4. Finish fullscreen/render-layer migration end to end.
   Remove legacy fullscreen producers and consumers in one coordinated pass.
5. Break up large client rendering/runtime modules.
   Prioritize `visuals.rs`, `backdrop.rs`, and the schedule-heavy parts of `native/mod.rs` / `plugins.rs`.
6. Reduce broad runtime scripting snapshot rebuilds.
   Prefer dirty/scoped snapshots or smaller script-facing views.

## 12. Workspace / Runtime Catalog Appendix

### Workspace crates and binaries

- `bins/sidereal-gateway`
  Active runtime. HTTP auth/bootstrap/asset delivery service.
- `bins/sidereal-replication`
  Active runtime. Authoritative simulation, replication, visibility, persistence flush, runtime scripting host.
- `bins/sidereal-client`
  Active runtime. Native binary plus WASM lib target for client transport, prediction, rendering, UI, and asset/runtime integration.
- `crates/sidereal-game`
  Active runtime shared core. Gameplay components, systems, render-layer types, and fixed-step gameplay logic.
- `crates/sidereal-net`
  Active runtime shared networking/protocol definitions and Lightyear registration helpers.
- `crates/sidereal-core`
  Active runtime shared constants, config, logging, BRP support, and common utilities.
- `crates/sidereal-persistence`
  Active runtime shared graph persistence model and Postgres/AGE integration helpers.
- `crates/sidereal-runtime-sync`
  Active runtime shared runtime sync/persistence bridge structures.
- `crates/sidereal-asset-runtime`
  Active runtime shared asset catalog, cache/index handling, hashing, and materialization helpers.
- `crates/sidereal-scripting`
  Active runtime shared scripting/catalog/world-init support used by gateway and replication.
- `crates/sidereal-component-macros`
  Build-time/runtime-support crate. Component authoring macros and registration support.
- `crates/sidereal-shader-preview`
  Tooling/runtime-support crate. Shader preview/support surface, not a main gameplay runtime.

### Major Bevy plugins, systems, and resources by runtime

#### Gateway

- Bevy runtime: none.
- Main active runtime responsibilities:
  - `AuthService`
  - bootstrap dispatchers
  - Postgres auth store
  - Axum API handlers for auth, bootstrap, scripts, and assets
- Classification:
  - active runtime

#### Replication server

- `ReplicationLifecyclePlugin`
  Active runtime. Startup hydration, Lightyear server start, health server start, connection/bootstrap observers.
- `ReplicationDiagnosticsPlugin`
  Active runtime. Health snapshots, world map/explorer snapshots, admin command handling.
- `ReplicationInputPlugin`
  Active runtime. Drains native inputs into authoritative action queues before physics prepare.
- `ReplicationControlPlugin`
  Active runtime. Control/combat side effects, projectile prespawn config, weapon event broadcasting.
- `ReplicationRuntimeScriptingPlugin`
  Active runtime. Script snapshot refresh, runtime catalog reload, interval/event execution.
- `ReplicationVisibilityPlugin`
  Active runtime. Staged visibility pipeline:
  transform sync, observer anchors, visibility ranges, entity cache, spatial index, landmark discovery, membership ensure/update, tactical/asset streaming.
- `ReplicationPersistencePlugin`
  Active runtime. Dirty marking, persistence flush after visibility update, end-of-step diagnostics.
- `ReplicationBootstrapBridgePlugin`
  Active runtime. Bootstrap bridge/message handling.
- Major resources:
  - visibility caches/index/config/telemetry
  - auth/client binding state
  - persistence worker state
  - runtime scripting catalog/runtime/metrics
  - tactical snapshot state
  - health snapshot resources
- Classification:
  - mostly active runtime
  - tests under `bins/sidereal-replication/src/tests/*` are test-only

#### Client

- `ClientBootstrapPlugin`
  Active runtime. Session/bootstrap/watchdog/menu-to-world transitions.
- `ClientTransportPlugin`
  Active runtime. Gateway auth/bootstrap HTTP flow, client transport connect/disconnect, raw link maintenance.
- `ClientReplicationPlugin`
  Active runtime. Replicated entity adoption, session-ready flow, remote state intake.
- `ClientPredictionPlugin`
  Active runtime. Local control intent, motion ownership, correction/reconcile handling.
- `ClientVisualsPlugin`
  Active runtime. Runtime shader assignment sync, render-layer sync/assignment, streamed visual attachment, duplicate suppression, weapon tracer/effect pools, backdrop sync.
- `ClientLightingPlugin`
  Active runtime. Lighting/post-process related runtime visuals.
- `ClientUiPlugin`
  Active runtime. HUD, tactical map, nameplates, dialogs, debug-facing UI systems.
- `ClientDiagnosticsPlugin`
  Scaffold/placeholder. Currently empty.
- Major resources inserted during shared runtime setup:
  - transport/session/auth/disconnect state
  - asset manager/catalog reload/dependency/fetch state
  - control/prediction/motion ownership state
  - tactical map, fog, contacts, nameplates, session-ready state
  - shader assignment state
  - render-layer registries/caches/perf counters
  - backdrop/fullscreen caches
- Classification:
  - active runtime in `native/*`
  - some paths are explicitly transitional/migration code, especially fullscreen compatibility and duplicate visual handling

#### Shared core crates

- `SiderealGamePlugin`
  Active runtime shared gameplay/plugin surface used by replication and client.
- Shared protocol and persistence crates
  Active runtime support, not scaffolding.
- `sidereal-shader-preview`
  Tooling/support, not part of the main client/server authority loop.


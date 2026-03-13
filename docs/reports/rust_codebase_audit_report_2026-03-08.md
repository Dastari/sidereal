# Rust Codebase Audit Report

Date: March 8, 2026
Scope: Rust workspace
Prompt source: `docs/audits/rust_codebase_audit_prompt.md`

## 1. Executive Summary

The codebase still has a solid server-authoritative spine: fixed-step simulation is explicit, replication input is bound to authenticated player identity, the BRP hardening policy now matches the docs, and the visibility runtime no longer appears to rely on the old hidden `ShipTag` baseline path. Those parts are defensible and should be kept.

The highest-risk issues are now concentrated in the client/content boundary rather than the authoritative core:

1. WASM parity is still not real in runtime terms. The browser target boots a render shell plus shared game core, but not auth, transport, replication, asset delivery, or the native plugin stack.
2. Asset dependency closure is not carried through the catalog/manifest/client state machine, even though the Lua registry already declares real dependencies on bootstrap-required shaders.
3. Rust runtime code still hardcodes concrete asset IDs and shader identities in active paths, including one live asset ID mismatch (`sprite_pixel_shader_wgsl` vs `sprite_pixel_effect_wgsl`).
4. Asset cache/versioning still diverges from the contract: the client writes loose files under `data/cache_stream/**`, uses `index.json`, and receives a constant catalog version string.

Those are architecture and correctness problems, not cleanup trivia. If they remain in place, every new client/content feature will keep adding native-only and hardcoded exceptions on top of a codebase that is supposed to be data-driven and parity-safe.

## 2. Architecture Findings

### Finding A1: WASM client is still scaffold-only at the runtime boundary
- Severity: Critical
- Type: architecture, correctness
- Priority: must fix
- Why it matters:
  The repo contract says the native and WASM clients are co-maintained and differ only at the platform boundary. The current WASM entrypoint still does not boot auth flow, transport, replication, asset loading, or the shared client plugin stack. That means parity failures are architectural, not incidental.
- Evidence:
  - `AGENTS.md:45`
  - `AGENTS.md:46`
  - `AGENTS.md:59`
  - `bins/sidereal-client/src/platform/wasm.rs:6`
  - `bins/sidereal-client/src/platform/wasm.rs:15`
  - `bins/sidereal-client/Cargo.toml:17`
  - `bins/sidereal-client/Cargo.toml:27`
  - `docs/plans/wasm_parity_implementation_plan.md:32`
- Details:
  The WASM target only adds `DefaultPlugins`, `RenderPlugin`, and `configure_shared_client_core()`, then logs a scaffold startup line. The target-specific dependency section for `wasm32` does not include `lightyear`, `sidereal-net`, `sidereal-runtime-sync`, or `sidereal-asset-runtime`.
- Recommendation:
  Unify app bootstrap so both targets use the same client plugin graph and state machine, with `cfg(target_arch = "wasm32")` limited to transport, HTTP, storage, and optional tooling adapters.

### Finding A2: Asset dependency closure is declared in Lua but dropped by the runtime protocol
- Severity: High
- Type: correctness, architecture
- Priority: must fix
- Why it matters:
  The asset contract requires dependency closure to be honored before attach. The Lua registry already declares bootstrap-required shaders that depend on non-bootstrap-required flare textures, but the generated catalog type, gateway DTOs, and client bootstrap flow do not carry or expand dependencies. This can produce valid checksums for the shader asset while still entering `InWorld` without its declared texture dependencies.
- Evidence:
  - `docs/features/asset_delivery_contract.md:89`
  - `docs/features/asset_delivery_contract.md:115`
  - `docs/features/asset_delivery_contract.md:190`
  - `data/scripts/assets/registry.lua:56`
  - `data/scripts/assets/registry.lua:68`
  - `crates/sidereal-asset-runtime/src/lib.rs:20`
  - `crates/sidereal-asset-runtime/src/lib.rs:48`
  - `crates/sidereal-core/src/gateway_dtos.rs:79`
  - `bins/sidereal-gateway/src/api.rs:286`
  - `bins/sidereal-client/src/runtime/auth_net.rs:380`
- Details:
  `expand_required_assets()` exists, but I did not find it used in the gateway manifest path or client bootstrap path. `RuntimeAssetCatalogEntry` and `AssetBootstrapManifestEntry` have no dependency field, so the dependency graph is lost before the client can honor it.
- Recommendation:
  Carry dependencies in the generated catalog and manifest, expand bootstrap-required closure on the server, and require the client to validate/fetch dependencies before marking the root asset ready.

### Finding A3: Active runtime still hardcodes concrete asset IDs, including one live ID mismatch
- Severity: High
- Type: correctness, architecture, maintainability
- Priority: must fix
- Why it matters:
  The asset delivery contract explicitly forbids hardcoded runtime asset IDs and requires Lua-authored catalog ownership. The current client still hardcodes shader asset IDs in active runtime paths, and one of them no longer matches the registry. That is both an architectural violation and a concrete correctness bug.
- Evidence:
  - `AGENTS.md:62`
  - `docs/features/asset_delivery_contract.md:42`
  - `docs/features/asset_delivery_contract.md:95`
  - `data/scripts/assets/registry.lua:87`
  - `bins/sidereal-client/src/runtime/shaders.rs:74`
  - `bins/sidereal-client/src/runtime/shaders.rs:97`
  - `bins/sidereal-client/src/runtime/shaders.rs:177`
  - `bins/sidereal-client/src/runtime/backdrop.rs:724`
  - `bins/sidereal-client/src/runtime/backdrop.rs:671`
  - `bins/sidereal-client/src/runtime/visuals.rs:56`
- Details:
  The registry defines `sprite_pixel_effect_wgsl`, but the runtime shader registry looks for `sprite_pixel_shader_wgsl`. The same file also hardcodes multiple named shaders instead of resolving runtime families from catalog metadata. Backdrop logic still hardcodes flare texture asset IDs.
- Recommendation:
  Move named asset selection fully into Lua-authored catalog/render-layer data. Rust should keep only generic shader family handling and one emergency fallback per family. Fix the `sprite_pixel_*` naming mismatch immediately.

### Finding A4: Gateway asset manifest versioning is a placeholder, not a real catalog version
- Severity: Medium
- Type: correctness, maintainability
- Priority: should fix
- Why it matters:
  The contract requires a generated catalog artifact and an active catalog version pointer. The gateway currently emits a fixed string for `catalog_version`, which prevents deterministic invalidation and makes observability/debugging weaker.
- Evidence:
  - `docs/features/asset_delivery_contract.md:118`
  - `docs/features/asset_delivery_contract.md:145`
  - `crates/sidereal-core/src/gateway_dtos.rs:90`
  - `bins/sidereal-gateway/src/api.rs:309`
- Details:
  `catalog_version` is always `"lua-registry-v1"`, regardless of which assets changed.
- Recommendation:
  Derive catalog version from the generated catalog artifact or a published active pointer and use it consistently across gateway, replication, and client cache invalidation.

## 3. Bevy / ECS Findings

### Finding B1: Large runtime modules still mix orchestration, protocol handling, and feature logic
- Severity: Medium
- Type: maintainability, architecture
- Priority: should fix
- Why it matters:
  Reviewability and regression isolation are getting worse because major runtime files still bundle multiple domains together. This conflicts with the repo rule to split large runtime refactors by domain instead of growing monolith entrypoints.
- Evidence:
  - `AGENTS.md:57`
  - `bins/sidereal-replication/src/replication/scripting.rs`
  - `bins/sidereal-replication/src/replication/visibility.rs`
  - `bins/sidereal-client/src/runtime/visuals.rs`
  - `bins/sidereal-client/src/runtime/backdrop.rs`
  - `bins/sidereal-client/src/runtime/ui.rs`
  - `crates/sidereal-persistence/src/lib.rs`
- Details:
  The current hot spots are not just long; they mix domain rules, ECS mutation, protocol translation, cache logic, render routing, and validation in one place.
- Recommendation:
  Split by domain responsibility:
  - client: asset bootstrap, transport/auth, world visuals, fullscreen/post-process, tactical UI
  - replication: script catalog loading, world-init data, entity spawn graph generation, visibility, tactical streaming
  - persistence: graph records, script catalog persistence, relationship persistence, startup/bootstrap

### Finding B2: Native-only HTTP/cache implementation is still embedded in active client systems
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The current client bootstrap and runtime fetch path still depends on `reqwest::blocking`, `std::fs`, and native local-path assumptions. This is one of the main reasons the WASM state machine is not actually shared.
- Evidence:
  - `docs/plans/wasm_parity_implementation_plan.md:35`
  - `docs/plans/wasm_parity_implementation_plan.md:40`
  - `bins/sidereal-client/src/runtime/auth_net.rs:345`
  - `bins/sidereal-client/src/runtime/assets.rs:273`
  - `bins/sidereal-client/Cargo.toml:20`
- Details:
  This is partly acknowledged in the parity plan, but it remains on active runtime code paths today.
- Recommendation:
  Introduce target-agnostic client boundary traits or plugins for HTTP, manifest fetch, and cache storage. Keep Bevy systems shared; swap only the adapter implementation per target.

## 4. Rendering Findings

### Finding R1: Runtime shader routing is still content-specific instead of family-driven
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The render-layer direction in the docs is generic family-based runtime shader ownership. The active shader registry still enumerates named content shaders and maps them to runtime kinds manually.
- Evidence:
  - `docs/features/asset_delivery_contract.md:180`
  - `bins/sidereal-client/src/runtime/shaders.rs:74`
  - `bins/sidereal-client/src/runtime/shaders.rs:165`
  - `bins/sidereal-client/src/runtime/shaders.rs:172`
  - `bins/sidereal-client/src/runtime/shaders.rs:183`
- Details:
  This is currently workable, but it keeps the client coupled to specific content IDs and duplicates authoring knowledge that should live in data.
- Recommendation:
  Convert the shader registry to generic family/signature handling, with Lua/catalog metadata selecting the concrete asset. Keep only emergency family fallbacks in Rust.

### Finding R2: Fullscreen/backdrop visual dependencies are still partially hardwired in Rust
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  Backdrop behavior still chooses specific flare texture asset IDs and named shader IDs directly in code. That works against the Lua-authored content direction and makes visual iteration harder.
- Evidence:
  - `AGENTS.md:62`
  - `bins/sidereal-client/src/runtime/backdrop.rs:671`
  - `bins/sidereal-client/src/runtime/backdrop.rs:682`
  - `bins/sidereal-client/src/runtime/backdrop.rs:704`
  - `bins/sidereal-client/src/runtime/backdrop.rs:724`
  - `bins/sidereal-client/src/runtime/backdrop.rs:746`
- Recommendation:
  Author backdrop texture/shader bindings in the render-layer/content schema, not in the Rust render system.

## 5. Physics / Avian2D Findings

### Finding P1: No major Avian misuse stood out in the authoritative runtime
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  Not every subsystem needs churn. Fixed-step ordering, explicit `PhysicsSystems` scheduling, and direct Avian authoritative motion components are stronger than the client/content boundary layers right now.
- Evidence:
  - `bins/sidereal-replication/src/main.rs:33`
  - `bins/sidereal-replication/src/plugins.rs:44`
  - `bins/sidereal-client/src/runtime/mod.rs:159`
- Recommendation:
  Keep the current fixed-step and single-writer motion discipline. Refactor effort is better spent elsewhere first.

## 6. Networking / Lightyear Findings

### Finding N1: Authenticated input/session binding remains a strong part of the architecture
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  The replication side still centers control/input routing on authenticated player binding rather than trusting claimed IDs from clients. That is correct for a server-authoritative game and should not be weakened during refactors.
- Evidence:
  - `AGENTS.md:41`
  - `bins/sidereal-replication/src/main.rs:81`
  - `bins/sidereal-replication/src/plugins.rs:37`
  - `bins/sidereal-replication/src/plugins.rs:54`
- Recommendation:
  Preserve the current authoritative routing model while cleaning up parity and asset-boundary problems.

## 7. Scripting / Lua Findings

### Finding S1: Asset registry authority is not fully respected by client/runtime code
- Severity: High
- Type: architecture, maintainability
- Priority: must fix
- Why it matters:
  The codebase direction is clear: concrete content should move into Lua-authored registries and generated catalogs. The registry is present, but the active runtime still keeps parallel Rust-side asset identity knowledge.
- Evidence:
  - `docs/features/scripting_support.md`
  - `data/scripts/assets/registry.lua:56`
  - `bins/sidereal-client/src/runtime/shaders.rs:74`
  - `bins/sidereal-client/src/runtime/backdrop.rs:671`
  - `bins/sidereal-client/src/runtime/visuals.rs:56`
- Details:
  This is broader than the single `sprite_pixel_*` mismatch. The runtime still “knows” specific content IDs that should be authored data.
- Recommendation:
  Treat the Lua asset registry plus generated catalog as the only source of concrete asset names. Rust should validate shape and perform family-level behavior only.

## 8. Persistence / Data Flow Findings

### Finding D1: Client cache implementation still diverges from the documented MMO-style cache contract
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  The docs define a `assets.pak` + `assets.index` + transactional temp file model. The current runtime still writes one file per asset under `data/cache_stream/**` and uses `index.json`. That is a direct code/doc divergence and means the published cache contract is not yet the real one.
- Evidence:
  - `AGENTS.md:63`
  - `docs/features/asset_delivery_contract.md:194`
  - `crates/sidereal-asset-runtime/src/lib.rs:208`
  - `bins/sidereal-client/src/runtime/auth_net.rs:324`
  - `bins/sidereal-client/src/runtime/auth_net.rs:359`
- Recommendation:
  Either finish the documented packed-cache design or explicitly downgrade the docs to describe the current loose-file cache as transitional. Right now the implementation and contract disagree.

### Finding D2: Gateway still rebuilds the runtime asset catalog on request
- Severity: Medium
- Type: performance, architecture
- Priority: should fix
- Why it matters:
  The docs describe a generated catalog artifact plus an active published pointer. The gateway still rebuilds the catalog from the Lua registry when handling manifest and asset requests, which keeps request-path work and failure modes tied to source-tree state.
- Evidence:
  - `docs/features/asset_delivery_contract.md:103`
  - `docs/features/asset_delivery_contract.md:118`
  - `bins/sidereal-gateway/src/api.rs:285`
  - `bins/sidereal-gateway/src/api.rs:324`
  - `bins/sidereal-gateway/src/api.rs:471`
- Details:
  This is partly acknowledged by the current implementation note in the docs, so this is not a hidden bug. It is still a meaningful transitional architecture risk.
- Recommendation:
  Precompute and publish the active catalog version outside the request path, then have gateway serve the published artifact and payload store directly.

## 9. Redundancy / Dead Code Findings

### Finding X1: A few transitional helper paths are present but not wired into the actual protocol flow
- Severity: Low
- Type: cleanup, maintainability
- Priority: optional improvement
- Why it matters:
  Leaving unused contract helpers around makes the code look more complete than it is and hides missing implementation work.
- Evidence:
  - `crates/sidereal-asset-runtime/src/lib.rs:48`
  - `bins/sidereal-client/src/runtime/shaders.rs:233`
- Details:
  `expand_required_assets()` exists but is not used in the manifest/bootstrap path. Shader direct-path fallback also appears to assume an alternate rooted location that does not line up with the normal streamed cache path.
- Recommendation:
  Either wire these helpers into the actual contract path or delete them until the contract is implemented end-to-end.

## 10. Startup / Main Loop Flow Maps

### 10.1 Gateway startup and main loop

1. `bins/sidereal-gateway/src/main.rs` initializes tracing, loads `AuthConfig`, connects `PostgresAuthStore`, ensures schema, and selects a bootstrap dispatcher.
2. It binds an Axum TCP listener and serves `app_with_service()`.
3. Active request loop responsibilities:
   - auth endpoints (`/auth/*`)
   - character listing and `POST /world/enter`
   - admin spawn/script endpoints
   - asset bootstrap manifest and authenticated `/assets/<asset_guid>` delivery
4. Current architecture note:
   - gateway still rebuilds the runtime asset catalog from the Lua registry inside request handling rather than serving a pre-published active catalog artifact.

### 10.2 Replication server startup and main loop

1. `bins/sidereal-replication/src/main.rs` loads BRP config, builds a headless Bevy app with `MinimalPlugins`, `ScheduleRunnerPlugin`, `SiderealGamePlugin`, Avian physics, and Lightyear `ServerPlugins`.
2. It registers protocol types, initializes replication resources, and adds domain plugins from `bins/sidereal-replication/src/plugins.rs`.
3. Startup systems:
   - hydrate simulation entities
   - start Lightyear server transport
   - start replication control listener
   - start persistence worker
4. Main loop shape:
   - `Update`: transport/channel maintenance, auth message drain, control/input requests, bootstrap entity commands, disconnect/cleanup
   - `FixedUpdate`: script runtime, authoritative gameplay/physics, visibility update, owner manifest/tactical streaming, persistence dirty-marking and flush

### 10.3 Client startup and main loop

1. Native entry (`bins/sidereal-client/src/runtime/mod.rs`) builds the full Bevy app, physics, Lightyear client plugins, remote inspection config, shared game core, resources, and native plugin stack.
2. WASM entry (`bins/sidereal-client/src/platform/wasm.rs`) currently does not do that; it boots rendering plus shared game core only.
3. Native plugin composition from `bins/sidereal-client/src/runtime/plugins.rs` groups runtime into:
   - bootstrap/state setup
   - transport/auth session flow
   - replication adoption and transform sync
   - prediction/input
   - visuals/lighting
   - UI and bootstrap watchdog behavior
4. Native client state machine:
   - `Auth -> CharacterSelect -> WorldLoading -> AssetLoading -> InWorld`
5. Main runtime loop:
   - `Update`: gateway request polling, session readiness, replication adoption, asset fetches, UI, visual attachment, camera/backdrop
   - `FixedPreUpdate`: input write
   - `FixedUpdate`: predicted local action queue and motion

### 10.4 Data / authority / persistence / replication / assets / scripting / rendering flow

1. Gateway authenticates the account and authorizes world entry for a specific player entity.
2. Gateway forwards bootstrap intent toward replication.
3. Replication binds transport session to authenticated `player_entity_id`, hydrates authoritative world state, runs Lua-scripted intent generation plus Rust authoritative gameplay, and persists graph records through the persistence worker.
4. Replication applies visibility/redaction, then streams entity replication, owner manifest data, and tactical deltas through Lightyear.
5. Gateway separately serves authenticated asset metadata and payloads over HTTP.
6. Native client uses gateway auth/bootstrap plus HTTP asset download, then adopts replicated entities, runs prediction/reconciliation locally, resolves visuals from replicated data and cached assets, and renders the world.
7. Rendering is currently split between:
   - generic ECS/gameplay state in shared crates
   - native-only runtime asset/cache/shader install logic
   - hardcoded named asset routing that should move into authored catalog/render-layer data

## 11. Prioritized Remediation Plan

1. Unify native and WASM client bootstrap around one runtime plugin graph and one client state machine.
2. Fix asset dependency closure end-to-end: registry -> generated catalog -> gateway manifest -> client bootstrap/runtime fetch.
3. Remove concrete asset IDs from active runtime code and repair the `sprite_pixel_effect_wgsl` naming mismatch immediately.
4. Replace placeholder catalog versioning with a real generated active version pointer.
5. Decide whether the cache contract is changing or the implementation is catching up, then make docs and code agree in the same change.
6. Split the largest mixed-responsibility runtime modules by domain before adding more client/content features on top.

## 12. Workspace / Runtime Catalog Appendix

### 12.1 Workspace crates and binaries

- `bins/sidereal-gateway`
  Responsibility: auth API, world-entry boundary, admin script/entity endpoints, asset manifest and HTTP payload serving.
  Classification: active runtime.
- `bins/sidereal-replication`
  Responsibility: authoritative simulation host, Lightyear server, visibility, tactical/owner streaming, persistence staging, script execution.
  Classification: active runtime.
- `bins/sidereal-client`
  Responsibility: player client.
  Classification: active runtime for native; WASM path is still scaffold/placeholder.
- `crates/sidereal-game`
  Responsibility: shared gameplay components, plugins, fixed-step gameplay systems, shared ECS contracts.
  Classification: active shared runtime.
- `crates/sidereal-net`
  Responsibility: protocol/message definitions and Lightyear registration helpers.
  Classification: active shared runtime.
- `crates/sidereal-persistence`
  Responsibility: graph-record persistence, schema/bootstrap, SQL/AGE interaction helpers.
  Classification: active shared runtime.
- `crates/sidereal-runtime-sync`
  Responsibility: runtime entity ID/GUID mapping, hierarchy/runtime sync helpers.
  Classification: active shared runtime.
- `crates/sidereal-asset-runtime`
  Responsibility: asset catalog/cache helper types, checksum/version helpers, runtime materialization.
  Classification: active shared runtime, but still transitional relative to the documented final cache/catalog model.
- `crates/sidereal-scripting`
  Responsibility: Lua registry/schema loading and validation helpers.
  Classification: active shared runtime/tooling boundary.
- `crates/sidereal-core`
  Responsibility: shared config/DTO/support code including remote-inspect env parsing.
  Classification: active shared runtime support.
- `crates/sidereal-component-macros`
  Responsibility: gameplay component proc macros and registration plumbing.
  Classification: active build-time support.
- `crates/sidereal-shader-preview`
  Responsibility: shader preview runtime used by dashboard tooling.
  Classification: tooling-focused; current runtime is scaffold-like.

### 12.2 Major Bevy plugins / systems / resources by runtime

#### Gateway

- Axum router in `bins/sidereal-gateway/src/api.rs`
  Responsibility: HTTP surface for auth, world entry, admin scripts/spawn, asset manifest, asset payloads.
  Classification: active runtime.

#### Replication runtime

- `SiderealGamePlugin`
  Responsibility: shared gameplay ECS and fixed-step systems.
  Classification: active runtime.
- `ReplicationLifecyclePlugin`
  Responsibility: hydration, server transport startup, control listener startup, replication connect observers.
  Classification: active runtime.
- `ReplicationAuthPlugin`
  Responsibility: auth message intake/binding.
  Classification: active runtime.
- `ReplicationInputPlugin`
  Responsibility: fixed-tick input drain to action queues.
  Classification: active runtime.
- `ReplicationControlPlugin`
  Responsibility: control-mode sync, combat message fanout, runtime script combat events.
  Classification: active runtime.
- `ReplicationVisibilityPlugin`
  Responsibility: observer positions, visibility range computation, network visibility, owner manifests, tactical snapshots.
  Classification: active runtime.
- `ReplicationPersistencePlugin`
  Responsibility: persistence worker startup, dirty marking, state flush.
  Classification: active runtime.
- `ReplicationRuntimeScriptingPlugin`
  Responsibility: script snapshot refresh, interval/event execution, intent application.
  Classification: active runtime.
- Key resources:
  `AuthenticatedClientBindings`, `ClientVisibilityRegistry`, `VisibilityScratch`, `PlayerRuntimeEntityMap`, persistence worker resources, tactical stream state.
  Classification: active runtime.

#### Client runtime

- `ClientBootstrapPlugin`
  Responsibility: app-state transitions, scene/bootstrap setup, headless bootstrap path.
  Classification: active runtime.
- `ClientTransportPlugin`
  Responsibility: Lightyear transport/session channel readiness, auth bind/session-ready message flow.
  Classification: active runtime.
- `ClientReplicationPlugin`
  Responsibility: replicated entity adoption, transform sync, owner manifest/tactical message intake, asset-loading transitions.
  Classification: active runtime.
- `ClientPredictionPlugin`
  Responsibility: input write and fixed-tick predicted local control.
  Classification: active runtime.
- `ClientVisualsPlugin`
  Responsibility: render-layer registry sync, streamed visual attachment, planet/weapon/tracer visuals, fullscreen/backdrop renderables.
  Classification: active runtime.
- `ClientLightingPlugin`
  Responsibility: world lighting state and local light collection.
  Classification: active runtime.
- `ClientUiPlugin`
  Responsibility: auth/character/world UI, tactical overlay, debug overlay, pause menu, bootstrap watchdog dialogs.
  Classification: active runtime.
- Key resources:
  `ClientSession`, `LocalAssetManager`, `RuntimeAssetHttpFetchState`, `SessionReadyState`, `LocalPlayerViewState`, `BootstrapWatchdogState`, tactical caches, prediction tuning resources.
  Classification: active runtime.

### 12.3 Transitional or placeholder items worth calling out

- `bins/sidereal-client/src/platform/wasm.rs`
  Responsibility: current browser entrypoint.
  Classification: scaffold/placeholder.
- `crates/sidereal-asset-runtime`
  Responsibility: shared asset runtime helpers.
  Classification: active but still transitional while cache/catalog publication remains incomplete.
- `crates/sidereal-shader-preview`
  Responsibility: shader preview support.
  Classification: tooling-only/scaffold-like runtime.

## 13. Validation Notes

- Manual source audit completed.
- `cargo check --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` were started, but both were blocked behind existing cargo package/build locks held by other active workspace builds (`cargo run`/cross-target builds already running in this repo). I could not complete those checks during this audit run.

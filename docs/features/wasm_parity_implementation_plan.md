# WASM Parity Implementation Plan

Status: Active execution plan  
Date: 2026-03-07  
Owners: Client/runtime team  
Primary references: `AGENTS.md`, `docs/sidereal_design_document.md`, `docs/sidereal_implementation_checklist.md`, `docs/features/asset_delivery_contract.md`, `docs/features/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`

## 1. Objective

Deliver a browser-hosted WASM client that runs the same gameplay-facing runtime as the native client, with target-specific differences limited to:

1. transport bootstrap and socket/data-channel bindings,
2. HTTP/browser I/O,
3. local cache/storage backend,
4. optional desktop-only tooling.

This is not "just add another Lightyear transport." The current native client still embeds native-only assumptions in transport, protocol registration, HTTP, asset cache, shader loading, and dependency wiring.

## 2. Current Baseline (Code-Accurate)

### 2.1 What already helps parity

- Client behavior is already decomposed into plugins under `bins/sidereal-client/src/native/plugins.rs`.
- Most gameplay/prediction logic is already shared via:
  - `crates/sidereal-game`
  - `crates/sidereal-net`
  - `crates/sidereal-runtime-sync`
- Some client code is already written with `cfg(target_arch = "wasm32")` branches inside shared modules, which is the right direction.

### 2.2 What is still native-only today

- `bins/sidereal-client/src/wasm.rs` is still scaffold-only and does not boot the gameplay runtime.
- `bins/sidereal-client/src/native/transport.rs` hardcodes UDP `UdpIo`, `LocalAddr`, and `PeerAddr`.
- `crates/sidereal-net/src/lightyear_protocol/registration.rs` hardwires `NativeInputPlugin<PlayerInput>` into shared protocol registration for both client and server.
- `bins/sidereal-client/src/native/auth_net.rs` uses:
  - `reqwest::blocking`,
  - `std::thread::spawn`,
  - `std::fs`,
  - local path assumptions for cache/bootstrap assets.
- `bins/sidereal-client/src/native/assets.rs` uses:
  - `reqwest::blocking`,
  - `std::thread::spawn`,
  - `std::fs`,
  - direct cache-path probing/writes.
- `bins/sidereal-client/src/native/shaders.rs` reads shader sources from local filesystem and compiled-in fallback WGSL sources.
- `bins/sidereal-client/Cargo.toml` keeps `lightyear`, `sidereal-game`, `sidereal-net`, `sidereal-runtime-sync`, and `sidereal-asset-runtime` behind `cfg(not(target_arch = "wasm32"))`.

### 2.3 Conclusion

The gap is not primarily gameplay logic. The gap is boundary architecture:

1. transport bootstrapping,
2. input-plugin registration,
3. HTTP/auth world-entry adapter,
4. runtime asset fetch adapter,
5. cache/storage adapter,
6. dependency and entrypoint wiring.

## 3. Non-Negotiable Constraints

- Keep gameplay/prediction/reconciliation ECS logic target-agnostic.
- Restrict platform branching to `cfg(target_arch = "wasm32")` only.
- Keep one client crate (`bins/sidereal-client`), not a split web client.
- Keep native and WASM state-machine semantics identical:
  - `Auth -> CharacterSelect -> WorldLoading -> AssetLoading -> InWorld`
- Keep identity/auth/input authority invariants identical across native and WASM.
- Keep browser-specific logic at the boundary only:
  - network adapter,
  - HTTP adapter,
  - local cache backend,
  - optional browser UX integration.

## 4. What Actually Has To Change

## 4.1 Transport is only one workstream

Transport work is necessary, but it is not enough. A parity implementation also requires:

1. protocol registration split,
2. client app bootstrap unification,
3. browser-safe HTTP/auth flow,
4. browser-safe asset/cache persistence,
5. removal of native-only dependency placement,
6. verification that rendering/asset/runtime code compiles for wasm.

## 4.2 File-level blockers already visible

- `bins/sidereal-client/src/wasm.rs`
  - currently does not build the real client runtime.
- `bins/sidereal-client/Cargo.toml`
  - shared runtime crates are unavailable to wasm.
- `bins/sidereal-client/src/native/transport.rs`
  - UDP-only transport bootstrap.
- `crates/sidereal-net/src/lightyear_protocol/registration.rs`
  - native input plugin wired in shared registration path.
- `bins/sidereal-client/src/native/auth_net.rs`
  - blocking/native HTTP and file I/O assumptions.
- `bins/sidereal-client/src/native/assets.rs`
  - blocking/native runtime asset fetch and filesystem cache.
- `bins/sidereal-client/src/native/shaders.rs`
  - native file reads for shader source resolution.
- `crates/sidereal-asset-runtime/src/lib.rs`
  - cache-index helpers are filesystem-only.

## 5. Implementation Phases

## 5.1 Phase 0: Lock the Browser Transport Contract

Purpose:
Prevent implementation churn around the wrong browser transport stack.

Required work:

- [ ] Confirm one accepted browser transport direction and remove contradictory wording across docs.
- [ ] Document channel mapping explicitly:
  - realtime gameplay lane,
  - control/session lane,
  - tactical/manifest lanes if they differ.
- [ ] Document whether fallback exists and under what conditions it is allowed.
- [ ] Add or update a decision record if transport wording is still mixed elsewhere.

Outputs:

- Updated transport wording in:
  - `AGENTS.md`
  - `docs/sidereal_design_document.md`
  - `docs/sidereal_implementation_checklist.md`
  - this plan

Exit criteria:

- One browser transport contract exists everywhere.

## 5.2 Phase 1: Build One Shared Client Runtime Entry

Purpose:
Stop treating WASM as a second app.

Required work:

- [ ] Extract common client app construction from `bins/sidereal-client/src/native/mod.rs` into shared entry helpers.
- [ ] Make the shared entry own:
  - core Bevy plugin wiring,
  - fixed timestep setup,
  - shared resources,
  - shared client plugins,
  - shared runtime state machine.
- [ ] Keep platform-specific entry files thin:
  - `native/mod.rs` sets native adapters/plugins,
  - `wasm.rs` sets wasm adapters/plugins,
  - both call the same shared app-builder.
- [ ] Remove the scaffold-only behavior from `wasm.rs`.

Concrete target shape:

- `build_client_app(platform: PlatformAdapters) -> App`
- `native::run()` provides native adapters
- `wasm::run()` provides wasm adapters

Exit criteria:

- `wasm.rs` boots the same app state machine as native, not a stub render app.

## 5.3 Phase 2: Fix Dependency Wiring So Shared Runtime Compiles on WASM

Purpose:
Make the shared runtime actually available to the wasm target.

Required work:

- [ ] Move shared runtime dependencies out of native-only sections in `bins/sidereal-client/Cargo.toml`:
  - `sidereal-game`
  - `sidereal-net`
  - `sidereal-runtime-sync`
  - `sidereal-asset-runtime`
  - `lightyear` with target-appropriate features
- [ ] Leave only truly native-only crates behind `cfg(not(target_arch = "wasm32"))`:
  - blocking HTTP client if still temporarily needed,
  - desktop-only logging/tooling,
  - native remote inspect flavor if wasm variant differs.
- [ ] Re-check all transitive client dependencies for wasm compatibility.
- [ ] If any dependency is not wasm-safe, isolate it behind a platform adapter instead of keeping the entire subsystem native-only.

Exit criteria:

- Shared client runtime compiles for `wasm32-unknown-unknown`.

## 5.4 Phase 3: Split Protocol Registration from Native Input Plugin Wiring

Purpose:
Right now the shared protocol layer is not truly shared.

Required work:

- [ ] Refactor `crates/sidereal-net/src/lightyear_protocol/registration.rs` into:
  - common message/channel/component registration,
  - client input plugin registration,
  - server input plugin registration if needed separately.
- [ ] Remove unconditional `NativeInputPlugin<PlayerInput>` from:
  - `register_lightyear_client_protocol`
  - `register_lightyear_server_protocol`
- [ ] Ensure the protocol crate only registers protocol state, not target-specific input adapters.
- [ ] Add target-boundary functions such as:
  - `register_lightyear_common_protocol(app)`
  - `register_lightyear_client_replication_components(app)`
  - `register_native_input_adapter(app)`
  - `register_browser_input_adapter(app)`

Why this matters:

The current setup assumes native keyboard/input plugin semantics inside shared registration. That is a direct parity blocker even before transport is solved.

Exit criteria:

- Common protocol registration is target-agnostic.

## 5.5 Phase 4: Implement Browser Transport Adapters

Purpose:
Replace the UDP-only client bootstrap with a wasm transport boundary.

Required work on client:

- [ ] Replace `bins/sidereal-client/src/native/transport.rs` with:
  - shared transport orchestration,
  - native UDP adapter,
  - wasm browser transport adapter.
- [ ] Preserve existing channel setup logic (`ensure_client_transport_channels`) as shared logic if Lightyear transport entity shape remains compatible.
- [ ] Add browser connect/disconnect/bootstrap path with the same runtime semantics:
  - connect transport,
  - authenticate session,
  - wait for session-ready,
  - continue to shared runtime flow.

Required work on replication server:

- [ ] Expose the accepted browser transport endpoint in replication.
- [ ] Keep authoritative simulation/persistence/visibility unchanged.
- [ ] Ensure session binding and player identity routing remain identical to native.

Likely server changes:

- additional transport listener/bootstrap config,
- transport-specific adapter plugin wiring,
- environment defaults for browser endpoint and any fallback.

Exit criteria:

- Browser client can connect to replication and receive authoritative state through Lightyear-compatible browser transport.

## 5.6 Phase 5: Replace Native HTTP/Auth Flow with Shared Adapter Boundary

Purpose:
The current auth/world-entry path is native-only even before gameplay starts.

Required work:

- [ ] Replace blocking HTTP in `bins/sidereal-client/src/native/auth_net.rs` with a platform adapter interface.
- [ ] Preserve current flow semantics:
  - login/register/password reset,
  - `/auth/me`,
  - `/auth/characters`,
  - `/world/enter`,
  - asset bootstrap manifest request.
- [ ] Native backend may temporarily keep an internal threaded implementation, but the shared ECS state machine must no longer know that.
- [ ] WASM backend must use browser-compatible async HTTP.

Recommended boundary:

- `GatewayApiAdapter`
  - `submit_auth(...)`
  - `submit_enter_world(...)`
  - `fetch_asset_bootstrap_manifest(...)`
  - `fetch_asset_bytes(...)`

Exit criteria:

- Auth/world-entry/bootstrap uses one shared state machine with platform-specific HTTP backends.

## 5.7 Phase 6: Replace Filesystem Asset Cache with Shared Storage Backend

Purpose:
Asset parity is impossible while cache logic assumes `std::fs`.

Required work:

- [ ] Extract cache-index operations from `crates/sidereal-asset-runtime/src/lib.rs` behind storage backends.
- [ ] Replace direct filesystem calls in:
  - `bins/sidereal-client/src/native/auth_net.rs`
  - `bins/sidereal-client/src/native/assets.rs`
  - `bins/sidereal-client/src/native/shaders.rs`
- [ ] Define a logical cache backend with identical semantics across platforms:
  - load/save cache index,
  - probe cached payload,
  - persist payload bytes,
  - resolve runtime-readable asset handle/path or memory blob.
- [ ] Native backend:
  - filesystem implementation.
- [ ] WASM backend:
  - browser storage backend such as IndexedDB or OPFS.

Important:

Do not fork the asset state machine. Only the persistence backend should differ.

Exit criteria:

- Bootstrap assets and runtime lazy-fetched assets are stored and resolved on both native and wasm with identical checksum/version semantics.

## 5.8 Phase 7: Make Shader and Render-Asset Resolution Browser-Safe

Purpose:
Rendering parity still depends on local shader/source path assumptions.

Required work:

- [ ] Replace direct local shader-source reads in `bins/sidereal-client/src/native/shaders.rs` with asset-backend-driven resolution.
- [ ] Ensure shader/material installation can consume cached bytes or validated runtime assets instead of assuming local files.
- [ ] Audit `bins/sidereal-client/src/native/platform.rs`, `dev_console.rs`, and any file-based debug helpers for wasm-incompatible behavior.
- [ ] Keep the same render-layer/material semantics across native and wasm.

Exit criteria:

- Runtime shader/material loading no longer depends on native filesystem access.

## 5.9 Phase 8: Input and UI Behavior Parity Validation

Purpose:
After transport/auth/storage compile, behavior still needs to match.

Required work:

- [ ] Verify input emission and local-intent ownership remain identical across native and wasm.
- [ ] Ensure control handoff, detached camera, tactical view-mode updates, and owner manifest handling are shared behavior.
- [ ] Ensure persistent error dialogs remain the same in wasm.
- [ ] Verify browser focus/blur and keyboard capture do not violate current input ownership assumptions.

Notes:

This is where browser-specific UX bugs appear even if transport is working.

Exit criteria:

- Native and wasm produce the same gameplay-facing state transitions and authoritative interaction patterns.

## 5.10 Phase 9: Add Explicit Parity Tests and Evidence

Purpose:
Parity will regress immediately if it is not tested.

Required work:

- [ ] Add unit tests for shared client state-machine logic.
- [ ] Add tests for protocol registration split so wasm path does not accidentally reintroduce native-only plugin wiring.
- [ ] Add tests around asset bootstrap/cache semantics at the adapter layer.
- [ ] Add integration coverage for:
  - authenticated session binding,
  - input routing invariants,
  - bootstrap/session-ready flow,
  - asset-loading gate before `InWorld`.
- [ ] Record native impact / wasm impact in any follow-up change.

Exit criteria:

- Parity-critical behavior is covered by automated checks and not just manual browser testing.

## 6. Suggested Work Breakdown by File

## 6.1 Client entry and dependency wiring

- `bins/sidereal-client/Cargo.toml`
- `bins/sidereal-client/src/main.rs`
- `bins/sidereal-client/src/native/mod.rs`
- `bins/sidereal-client/src/wasm.rs`

## 6.2 Protocol and input registration

- `crates/sidereal-net/src/lightyear_protocol/registration.rs`
- `bins/sidereal-client/src/native/input.rs`

## 6.3 Transport boundary

- `bins/sidereal-client/src/native/transport.rs`
- replication transport/server bootstrap files

## 6.4 Auth and asset HTTP boundary

- `bins/sidereal-client/src/native/auth_net.rs`
- `bins/sidereal-client/src/native/assets.rs`

## 6.5 Cache/storage backend

- `crates/sidereal-asset-runtime/src/lib.rs`
- client asset/cache helpers

## 6.6 Render/shader asset resolution

- `bins/sidereal-client/src/native/shaders.rs`
- any file-based render helper usage

## 7. Risks and Misconceptions

## 7.1 "Lightyear already supports browser transport, so parity is mostly done"

Not true in this repo's current state.

Even if Lightyear browser transport were dropped in tomorrow, the client would still fail parity because:

1. wasm does not build the real client runtime,
2. shared runtime crates are not available to the wasm target,
3. protocol registration still wires native input plugins,
4. auth/world-entry flow is native-only,
5. asset bootstrap and lazy fetch are native-only,
6. cache and shader loading are filesystem-based.

## 7.2 "The gameplay code itself is the problem"

Mostly false.

The authoritative gameplay core is already comparatively portable. The real work is at the target boundary and bootstrap layers.

## 7.3 "We can defer asset/cache parity until later"

Not if parity means real browser playability.

The project contract explicitly requires the same asset state machine and validation logic across native and wasm.

## 8. Definition of Done

- [ ] `bins/sidereal-client/src/wasm.rs` no longer boots a scaffold app.
- [ ] Native and wasm both use one shared client app-builder and one shared state machine.
- [ ] Shared runtime crates compile for wasm.
- [ ] Protocol registration is target-agnostic and no longer hardcodes native input plugin wiring.
- [ ] Browser transport is implemented at the boundary without gameplay-system forks.
- [ ] Auth/world-entry/bootstrap HTTP flow runs through a shared adapter boundary.
- [ ] Asset bootstrap and lazy fetch run through a shared storage/cache abstraction.
- [ ] Shader/render-asset resolution no longer depends on native filesystem access.
- [ ] Native and wasm parity-critical behaviors are covered by tests.
- [ ] Docs and code agree on current browser transport and parity scope.

## 9. Required Quality Gates

Run on completion:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

## 10. Recommended PR Sequence

1. Transport-contract cleanup and plan/doc sync.
2. Shared client app-builder extraction and wasm entry replacement.
3. Dependency reshaping and protocol-registration split.
4. Browser transport adapter and replication endpoint support.
5. Shared HTTP/auth adapter.
6. Shared asset/cache backend and shader-resolution refactor.
7. Parity validation/tests and cleanup of remaining native-only shims.

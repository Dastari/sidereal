# WASM Parity Implementation Plan

Status: Deferred after native runtime stabilization  
Date: 2026-03-07  
Owners: Client/runtime team  
Primary references: `AGENTS.md`, `docs/sidereal_design_document.md`, `docs/sidereal_implementation_checklist.md`, `docs/features/asset_delivery_contract.md`, `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`

## 0.0 Priority Status (2026-03-08)

- Native client now reaches in-world state and renders replicated ships.
- That native path is not yet playable: controls are currently non-functional and motion/correction still intermittently jumps.
- Because native control/prediction/camera stability is the immediate blocker, the remaining WASM parity work is intentionally back-burnered for now.
- This document remains the parked follow-up plan once native in-world behavior is stable again.

## 0.1 Current Progress Snapshot (2026-03-08)

- Runtime shader/material ABI hardcoding was reduced in the native client and catalog metadata now carries dependency/shader-family data.
- Player/starter bootstrap content is now Lua-authored in the active gateway path.
- Native auth/bootstrap/runtime-asset I/O now sits behind explicit client adapter resources plus native helper implementations instead of leaving `reqwest::blocking` and `std::fs` calls scattered through gameplay-facing systems.
- Shared protocol registration no longer injects native input plugins; native client/server now own that adapter choice explicitly.
- The client's wasm build no longer depends on Lua scripting, Postgres/persistence, or BRP just to compile shared runtime crates. Those boundaries are now feature-gated or target-gated out of the browser target.
- Native and wasm now build the same windowed client app shell through shared client-app construction instead of maintaining a separate wasm render stub.
- Browser/WASM now provides concrete gateway HTTP and cache adapters.
- Browser/WASM cache persistence now uses IndexedDB rather than `localStorage`, with an in-memory mirror for synchronous runtime asset reads on the render path.
- Browser/WASM runtime asset attachment now mounts streamed shaders, images, and SVGs from validated cached bytes instead of filesystem-style `AssetServer` paths.
- Gateway enter-world bootstrap now publishes replication transport metadata and replication can expose a WebTransport listener alongside native UDP.
- Local browser WebTransport validation depends on a certificate that satisfies browser `serverCertificateHashes` constraints: short-lived X.509v3 cert, ECDSA P-256 key, and SAN coverage for the advertised local host/IPs.
- Browser/WASM currently defaults `SIDEREAL_ENABLE_SHADER_MATERIALS=0` unless explicitly overridden so browser validation can continue while active browser-only WebGPU shader-material failures are isolated. Native and Windows keep shader materials enabled by default.

The remaining work is no longer architectural bootstrap. It is live end-to-end browser validation plus the longer-running cache-packaging follow-through tracked separately by the asset delivery contract. That work is currently deferred until native runtime stability is good enough to make parity validation meaningful.

## 0.2 Actual Remaining Work (2026-03-08)

The highest-signal remaining WASM parity tasks are now:

1. live browser validation for login -> character select -> world-entry -> asset-loading -> in-world,
2. parity validation for browser input ownership, focus/blur behavior, control handoff, owner-manifest flow, and persistent dialog UX,
3. automated coverage around the new protocol/transport/cache/runtime-asset adapter boundaries.

Separate but related follow-up:

4. the asset-delivery contract's physical packed-cache target (`assets.pak`-style native packaging) remains follow-up work, but it is no longer a blocker for functional browser parity because the current browser adapter already satisfies the required byte-backed cache/index semantics.
5. the client crate currently keeps its Cargo lib target `rlib`-only so native/Windows executable builds do not also try to link a Windows `cdylib`. Any future dedicated wasm packaging/export step must emit the browser artifact explicitly instead of assuming the default Cargo lib target always produces a `cdylib`.

The old dependency, bootstrap, and transport-contract blockers are resolved.

## 0.3 Resolved Contract Decisions (2026-03-08)

1. Browser transport direction is WebTransport-first across active project docs.
2. Browser runtime asset mounting is defined as byte-backed from validated cached/gateway payload bytes rather than filesystem-style `AssetServer` path loads.

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

### 2.2 What is still outstanding today

- `bins/sidereal-client/src/wasm.rs` now boots the shared gameplay runtime shell, but it still needs live browser validation against gateway + replication.
- `bins/sidereal-client/Cargo.toml`
  - intentionally keeps the shared lib target `rlib`-only for native/Windows build stability; any future wasm packaging/export flow must handle browser artifact emission explicitly.
- `bins/sidereal-client/src/native/remote.rs` remains native-only by design for now; BRP is intentionally absent from wasm.
- Browser transport/caching now compiles end to end, but parity-critical runtime behavior still needs integration coverage.
- The asset delivery contract’s long-term physical packed-cache target is still follow-up work; the current browser adapter exposes the required byte-backed/indexed semantics through platform storage.
- Browser/WASM currently runs with shader materials disabled by default as a temporary mitigation for browser-only WebGPU/Bevy fullscreen/material shader failures; use `SIDEREAL_ENABLE_SHADER_MATERIALS=1` only for focused browser shader debugging.

### 2.3 Conclusion

The remaining gap is no longer primary architecture work. The unresolved portion is validation and regression-proofing:

1. live browser proof that the full runtime flow works against gateway + replication,
2. parity checks for browser-specific input/focus behavior,
3. explicit test coverage for the new shared boundaries.

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

Current browser runtime note:

- Native and Windows render the streamed shader-material path successfully.
- Browser/WASM should currently be validated first for authoritative session flow, world entry, replication, and baseline scene/runtime behavior with shader materials disabled by default.
- Shader-material parity on browser remains active follow-up debugging behind the explicit env override.

## 4.2 File-level follow-up areas still worth checking

- `bins/sidereal-client/src/wasm.rs`
  - needs live browser validation against real gateway + replication endpoints.
- `bins/sidereal-client/src/native/input.rs`
  - needs explicit parity validation for browser focus/blur and local-intent ownership semantics.
- `bins/sidereal-client/src/native/dialog_ui.rs`
  - needs explicit confirmation that persistent dialog UX matches native behavior in browser runtime.
- `bins/sidereal-client/src/native/platform.rs`
  - still needs a cleanup audit for native-only helper behavior that may not belong in shared runtime code.
- `bins/sidereal-client/src/native/dev_console.rs`
  - still needs a wasm-compatibility audit for any file/device assumptions.
- `crates/sidereal-net/tests/lightyear_protocol.rs`
  - currently covers basic registration, but not the stronger regression cases around protocol/input split boundaries.

## 5. Implementation Phases

## 5.1 Phase 0: Lock the Browser Transport Contract

Purpose:
Prevent implementation churn around the wrong browser transport stack.

Required work:

- [x] Confirm one accepted browser transport direction and remove contradictory wording across docs.
- [ ] Document channel mapping explicitly:
  - realtime gameplay lane,
  - control/session lane,
  - tactical/manifest lanes if they differ.
- [x] Document whether fallback exists and under what conditions it is allowed.
- [x] Add or update a decision record if transport wording is still mixed elsewhere.

Outputs:

- Updated transport wording in:
  - `AGENTS.md`
  - `docs/sidereal_design_document.md`
  - `docs/sidereal_implementation_checklist.md`
  - this plan

Exit criteria:

- [x] One browser transport contract exists everywhere.

## 5.2 Phase 1: Build One Shared Client Runtime Entry

Purpose:
Stop treating WASM as a second app.

Required work:

- [x] Extract common client app construction from `bins/sidereal-client/src/native/mod.rs` into shared entry helpers.
- [x] Make the shared entry own:
  - core Bevy plugin wiring,
  - fixed timestep setup,
  - shared resources,
  - shared client plugins,
  - shared runtime state machine.
- [x] Keep platform-specific entry files thin:
  - `native/mod.rs` sets native adapters/plugins,
  - `wasm.rs` sets wasm adapters/plugins,
  - both call the same shared app-builder.
- [x] Remove the scaffold-only behavior from `wasm.rs`.

Concrete target shape:

- `build_windowed_client_app(...) -> App`
- `build_headless_client_app(...) -> App`
- `native::run()` provides native adapters and native-only diagnostics
- `wasm::run()` provides wasm adapters and browser render settings

Exit criteria:

- `wasm.rs` boots the same app state machine as native, not a stub render app.

## 5.3 Phase 2: Fix Dependency Wiring So Shared Runtime Compiles on WASM

Purpose:
Make the shared runtime actually available to the wasm target.

Required work:

- [x] Move shared runtime dependencies out of native-only sections in `bins/sidereal-client/Cargo.toml`:
  - `sidereal-game`
  - `sidereal-net`
  - `sidereal-runtime-sync`
  - `sidereal-asset-runtime`
  - `lightyear` with target-appropriate features
- [x] Leave only truly native-only crates behind `cfg(not(target_arch = "wasm32"))`:
  - blocking HTTP client if still temporarily needed,
  - desktop-only logging/tooling,
  - native remote inspect flavor if wasm variant differs.
- [x] Re-check all transitive client dependencies for wasm compatibility.
- [x] If any dependency is not wasm-safe, isolate it behind a platform adapter instead of keeping the entire subsystem native-only.

Exit criteria:

- [x] Shared client runtime dependencies compile for `wasm32-unknown-unknown`.
- [x] Shared gameplay-facing client runtime entry boots from the wasm target.

## 5.4 Phase 3: Split Protocol Registration from Native Input Plugin Wiring

Purpose:
Right now the shared protocol layer is not truly shared.

Required work:

- [x] Refactor `crates/sidereal-net/src/lightyear_protocol/registration.rs` into:
  - common message/channel/component registration,
  - client input plugin registration,
  - server input plugin registration if needed separately.
- [x] Remove unconditional `NativeInputPlugin<PlayerInput>` from:
  - `register_lightyear_client_protocol`
  - `register_lightyear_server_protocol`
- [x] Ensure the protocol crate only registers protocol state, not target-specific input adapters.
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

- [x] Replace `bins/sidereal-client/src/native/transport.rs` with:
  - shared transport orchestration,
  - native UDP adapter,
  - wasm browser transport adapter.
- [x] Preserve existing channel setup logic (`ensure_client_transport_channels`) as shared logic if Lightyear transport entity shape remains compatible.
- [x] Add browser connect/disconnect/bootstrap path with the same runtime semantics:
  - connect transport,
  - authenticate session,
  - wait for session-ready,
  - continue to shared runtime flow.

Required work on replication server:

- [x] Expose the accepted browser transport endpoint in replication.
- [x] Keep authoritative simulation/persistence/visibility unchanged.
- [x] Ensure session binding and player identity routing remain identical to native.

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

- [x] Replace blocking HTTP in `bins/sidereal-client/src/native/auth_net.rs` with a platform adapter interface.
- [x] Preserve current flow semantics:
  - login/register/password reset,
  - `/auth/me`,
  - `/auth/characters`,
  - `/world/enter`,
  - asset bootstrap manifest request.
- [x] Native backend may temporarily keep an internal threaded implementation, but the shared ECS state machine must no longer know that.
- [x] WASM backend must use browser-compatible async HTTP.

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

- [x] Extract cache-index operations from `crates/sidereal-asset-runtime/src/lib.rs` behind storage backends.
- [x] Replace direct filesystem calls in:
  - `bins/sidereal-client/src/native/auth_net.rs`
  - `bins/sidereal-client/src/native/assets.rs`
  - `bins/sidereal-client/src/native/shaders.rs`
- [x] Define a logical cache backend with identical semantics across platforms:
  - load/save cache index,
  - probe cached payload,
  - persist payload bytes,
  - resolve runtime-readable asset handle/path or memory blob.
- [x] Native backend:
  - filesystem implementation.
- [x] WASM backend:
  - browser storage backend providing the same byte/index semantics.

Important:

Do not fork the asset state machine. Only the persistence backend should differ.

Exit criteria:

- Bootstrap assets and runtime lazy-fetched assets are stored and resolved on both native and wasm with identical checksum/version semantics.

## 5.8 Phase 7: Make Shader and Render-Asset Resolution Browser-Safe

Purpose:
Rendering parity still depends on local shader/source path assumptions.

Required work:

- [x] Replace direct local shader-source reads in `bins/sidereal-client/src/native/shaders.rs` with asset-backend-driven resolution.
- [x] Ensure shader/material installation can consume cached bytes or validated runtime assets instead of assuming local files.
- [ ] Audit `bins/sidereal-client/src/native/platform.rs`, `dev_console.rs`, and any file-based debug helpers for wasm-incompatible behavior.
- [x] Keep the same render-layer/material semantics across native and wasm.

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

- [x] `bins/sidereal-client/src/wasm.rs` no longer boots a scaffold app.
- [x] Native and wasm both use one shared client app-builder and one shared state machine.
- [x] Shared runtime crates compile for wasm.
- [x] Protocol registration is target-agnostic and no longer hardcodes native input plugin wiring.
- [x] Browser transport is implemented at the boundary without gameplay-system forks.
- [x] Auth/world-entry/bootstrap HTTP flow runs through a shared adapter boundary.
- [x] Asset bootstrap and lazy fetch run through a shared storage/cache abstraction.
- [x] Shader/render-asset resolution no longer depends on native filesystem access.
- [ ] Native and wasm parity-critical behaviors are covered by tests.
- [x] Docs and code agree on current browser transport and parity scope.

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

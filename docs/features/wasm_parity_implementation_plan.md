# WASM Parity Implementation Plan

Status: Active execution plan  
Date: 2026-02-25  
Owners: Client/runtime team  
Primary references: `AGENTS.md`, `docs/sidereal_design_document.md`, `docs/sidereal_implementation_checklist.md`, `docs/features/asset_delivery_contract.md`

## 1. Objective

Deliver a browser-hosted WASM client that runs the same gameplay-facing runtime behavior as the native client, with platform differences constrained to transport/bootstrap and browser I/O boundaries.

## 2. Current Baseline (Code-Accurate)

- Native runtime remains monolithic in `bins/sidereal-client/src/native.rs` (~5.5k LOC).
- WASM runtime is scaffold-only in `bins/sidereal-client/src/wasm.rs` (startup log + render plugin only).
- Native transport is explicitly UDP (`UdpIo`) and replication server startup is explicitly UDP (`ServerUdpIo`).
- Client dependency wiring is native-heavy:
  - `sidereal-game`, `sidereal-net`, `sidereal-runtime-sync`, `sidereal-asset-runtime`, `lightyear`, and `reqwest` are currently in `cfg(not(target_arch = "wasm32"))`.
  - wasm target currently only wires `bevy_remote`.
- Protocol registration currently hardwires `lightyear` native input plugin (`NativeInputPlugin<PlayerInput>`), which blocks direct reuse for browser input transport wiring.
- Native asset cache/shader flow currently uses direct filesystem APIs (`std::fs`, local path probing/writes).
- Native auth/world-entry flow currently uses blocking HTTP client APIs (`reqwest::blocking`).
- `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu` passing today does not indicate runtime parity.

## 3. Non-Negotiable Constraints

- Keep gameplay/prediction/reconciliation ECS logic target-agnostic.
- Restrict platform branching to `cfg(target_arch = "wasm32")` only.
- Keep identity/auth/input authority invariants identical across native and WASM.
- Preserve streamed asset protocol and fail-soft behavior across targets.
- Keep native and WASM co-maintained in the same crate (`bins/sidereal-client`), not split into a separate web client crate.

## 4. Phase 0: Transport Contract Alignment (Gate)

Problem: source-of-truth docs currently use mixed browser transport wording (`WebRTC-first` vs `WebTransport-first` references).

- [ ] Align transport terminology across:
  - `AGENTS.md`
  - `docs/sidereal_design_document.md`
  - `docs/sidereal_implementation_checklist.md`
  - this plan
- [ ] Record accepted contract in `docs/decision_register.md` with a dedicated decision detail file under `docs/features/dr-XXXX_*.md` if needed.
- [ ] Explicitly document fallback behavior and which traffic classes use which channel semantics.

Exit criteria:
- One transport contract is documented everywhere with no contradictions.

## 5. Phase 1: Decompose Native Monolith into Shared Runtime Modules

Refactor `bins/sidereal-client/src/native.rs` into `bins/sidereal-client/src/client/` domain modules.

- [ ] Create module boundaries for:
  - app/plugin wiring
  - auth + session state machine
  - protocol/replication message wiring
  - prediction/reconciliation setup
  - entity projection/render adapters
  - asset stream/cache state machine
  - UI screens/dialog orchestration
- [ ] Keep `native.rs` as thin bootstrap/wiring entrypoint.
- [ ] Rewire `wasm.rs` to use the same shared module graph (initially with transport/auth/cache adapters as stubs where needed).
- [ ] Ensure no gameplay/prediction system duplication between native and WASM paths.

Exit criteria:
- Shared client runtime modules are used by both `native.rs` and `wasm.rs`.

## 6. Phase 2: Dependency and Protocol Registration Reshaping

### 6.1 Client crate dependency corrections

- [ ] Update `bins/sidereal-client/Cargo.toml` so shared runtime dependencies are available for wasm builds where required:
  - `sidereal-game`
  - `sidereal-net` (`lightyear_protocol`)
  - `sidereal-runtime-sync`
  - `sidereal-asset-runtime`
  - target-appropriate `lightyear` features
- [ ] Keep truly native-only dependencies behind `cfg(not(target_arch = "wasm32"))` (for example blocking HTTP clients or desktop-only tooling).

### 6.2 Protocol registration split

- [ ] Remove hard native-input coupling from `crates/sidereal-net/src/lightyear_protocol/registration.rs`:
  - separate message/component/channel registration from transport/input-plugin registration.
- [ ] Provide target-appropriate input transport plugin wiring at client boundary, not in shared protocol registration.

Exit criteria:
- Shared protocol registration compiles and runs on both native and wasm targets without gameplay `cfg` forks.

## 7. Phase 3: Browser Transport Boundary Implementation

### 7.1 Client transport bootstrap

- [ ] Add WASM transport bootstrap module(s) that create and connect Lightyear client entities for browser transport.
- [ ] Preserve native UDP bootstrap path unchanged (`UdpIo`, `LocalAddr`, `PeerAddr`) for non-wasm builds.

### 7.2 Replication service transport endpoint

- [ ] Add/enable replication server transport endpoint(s) required by accepted browser transport contract.
- [ ] Keep authoritative simulation, visibility, and persistence flow unchanged.
- [ ] Ensure session binding + authenticated player identity routing invariants are unchanged.

### 7.3 Config/runtime defaults

- [ ] Add explicit env/runtime config for browser transport endpoint(s) and fallback behavior.
- [ ] Update runbooks/README/dev targets as needed.

Exit criteria:
- Browser client can connect to replication and participate in authoritative protocol flow.

## 8. Phase 4: Browser-Safe Auth and Asset I/O Adapters

### 8.1 Auth/world-entry HTTP path

- [ ] Replace native blocking HTTP assumptions with adapter boundary:
  - native adapter may remain blocking/threaded if desired.
  - wasm adapter must use browser-compatible async HTTP APIs.
- [ ] Preserve exact auth/world-entry semantics (`/auth/*`, `/world/enter` flow).

### 8.2 Asset cache/storage adapter

- [ ] Extract asset cache persistence behind adapter trait(s):
  - native backend: filesystem (`data/cache_stream`, index file).
  - wasm backend: browser storage backend (IndexedDB/OPFS or equivalent) with same logical cache-index semantics.
- [ ] Keep streamed manifest/chunk protocol identical across targets.
- [ ] Keep checksum/version validation and fail-soft placeholders identical in behavior.

Exit criteria:
- Browser client uses streamed asset protocol with browser-safe persistence backend and parity cache semantics.

## 9. Phase 5: UI/Runtime Parity Hardening

- [ ] Ensure login/register/character-select/world-loading/in-world state transitions match native behavior in browser runtime.
- [ ] Ensure critical errors requiring acknowledgement use persistent dialog queue behavior in shared UI flow.
- [ ] Confirm camera/control/predicted-intent ownership rules match native behavior.
- [ ] Confirm detached camera and control handoff paths operate identically under browser transport latency.

Exit criteria:
- Gameplay-facing runtime flow is behaviorally parity-aligned across native and WASM.

## 10. Phase 6: Parity Tests and Validation Evidence

- [ ] Add/extend unit tests for shared client modules (transport-agnostic logic).
- [ ] Add transport boundary tests for browser path and fallback behavior.
- [ ] Add/extend integration tests for:
  - predicted local intent ownership (no replicated overwrite of pending local intent),
  - session/auth identity binding and rejection paths,
  - asset manifest/chunk/cache validation behavior.
- [ ] Capture explicit native impact / WASM impact notes in change records.

Exit criteria:
- Parity-critical behavior has automated coverage and documented evidence.

## 11. Required Quality Gates

Run on completion:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

## 12. Definition of Done

- [ ] `bins/sidereal-client/src/wasm.rs` no longer runs scaffold-only runtime.
- [ ] Shared runtime modules power both native and WASM entrypoints.
- [ ] Browser transport is implemented at boundary without gameplay-system forks.
- [ ] Browser-safe auth and asset cache adapters are implemented with parity semantics.
- [ ] Replication service exposes required browser transport endpoint(s) with unchanged authority invariants.
- [ ] Docs/decision records fully match implemented transport contract and runtime behavior.
- [ ] Required quality gates pass.

## 13. Suggested PR Sequence

1. Transport contract alignment + decision record.
2. Native monolith extraction to shared modules.
3. Dependency and protocol-registration split for wasm compatibility.
4. Browser transport endpoint/client bootstrap implementation.
5. Browser auth + asset storage adapters.
6. Parity tests, doc sync, and final quality-gate pass.

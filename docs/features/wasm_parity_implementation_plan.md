# WASM Parity Implementation Plan

Status: Active execution plan  
Date: 2026-02-24  
Owners: Client/runtime team  
Primary references: `AGENTS.md`, `docs/sidereal_design_document.md`, `docs/sidereal_implementation_checklist.md`, `docs/features/asset_delivery_contract.md`

## 1. Objective

Complete WASM client implementation so browser runtime behavior matches native runtime behavior for gameplay-facing systems, with platform differences isolated to transport/bootstrap boundaries.

## 2. Current Baseline

- Native client runtime is implemented in `bins/sidereal-client/src/native.rs`.
- WASM runtime is scaffold-only in `bins/sidereal-client/src/wasm.rs`.
- `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu` currently passes, but this does not indicate runtime parity.

## 3. Non-Negotiable Constraints

- Keep shared gameplay/prediction/reconciliation ECS logic target-agnostic.
- Restrict platform branching to `cfg(target_arch = "wasm32")` only.
- Keep browser transport WebRTC-first with explicit WebSocket fallback only.
- Keep identity/auth/input authority invariants identical across targets.
- Preserve asset-stream fail-soft and placeholder behavior across targets.

## 4. Execution Plan

## Phase 0: Documentation Alignment (Gate)

- [ ] Resolve and align transport terminology across docs where wording diverges.
- [ ] Update docs to a single browser transport contract (WebRTC-first + explicit fallback behavior).
- [ ] Record the final decision in `docs/decision_register.md` and a linked `docs/features/dr-XXXX_*.md` if required.

Exit criteria:
- No conflicting transport-direction wording remains across source-of-truth docs.

## Phase 1: Shared Client Runtime Extraction

- [ ] Split monolithic native entrypoint into focused modules under `bins/sidereal-client/src/client/`.
- [ ] Move shared app/plugin wiring, protocol registration, prediction/interpolation setup, and input tick flow into target-agnostic modules.
- [ ] Keep only transport adapter/bootstrap and platform IO in target-specific modules.

Exit criteria:
- Native and WASM both build against the same shared runtime module graph.

## Phase 2: Dependency and Target Wiring

- [ ] Ensure `sidereal-game`, `sidereal-net`, `sidereal-runtime-sync`, and required Lightyear client deps are available for wasm target wiring where needed.
- [ ] Preserve native-only deps behind `cfg(not(target_arch = "wasm32"))` with wasm-safe alternatives.
- [ ] Make crate target declarations explicit and consistent with project contract (native bin + wasm lib).

Exit criteria:
- No gameplay-facing dependency exists only for native unless explicitly platform-boundary-only.

## Phase 3: WASM Transport Boundary

- [ ] Implement browser transport bootstrap module(s) for WASM using the documented transport direction.
- [ ] Keep native UDP/raw path intact and unchanged in authority semantics.
- [ ] Bind session/auth identity and input routing invariants identically to native behavior.

Exit criteria:
- Browser client can join replication session and exchange authoritative protocol traffic through WASM transport path.

## Phase 4: Asset Streaming/Cache Parity

- [ ] Port shared asset state machine usage into WASM runtime path.
- [ ] Match placeholder/fail-soft behavior between native and WASM.
- [ ] Ensure cache validity semantics are identical (`asset_version` + checksum/hash behavior).

Exit criteria:
- Same logical asset message/state behavior for native and WASM clients.

## Phase 5: Parity Validation and Tests

- [ ] Add/extend unit tests for shared client runtime modules.
- [ ] Add integration tests for protocol/runtime parity behaviors (native vs wasm-targeted logic checks where feasible).
- [ ] Add focused tests for predicted local input ownership and replicated-control confirmation behavior.
- [ ] Add/extend asset parity tests for placeholder and swap-in semantics.

Exit criteria:
- Parity-critical behaviors have automated test coverage and pass.

## Phase 6: Quality Gates and Evidence

Run on completion:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

Evidence to capture in change notes:
- Build/test command outcomes.
- Confirmed parity behaviors.
- Explicit statement of native impact and WASM impact.

## 5. Definition of Done

- [ ] WASM no longer runs scaffold-only client runtime.
- [ ] Shared runtime logic is reused across native and WASM.
- [ ] Transport differences are boundary-only.
- [ ] Parity tests cover gameplay-facing behavior and asset flow.
- [ ] Docs and decision records reflect implemented reality.
- [ ] Required quality gates pass.

## 6. Work Tracking (Suggested PR Sequence)

1. Docs + decision alignment PR (Phase 0).
2. Shared runtime extraction PR (Phase 1).
3. WASM dependency/target wiring PR (Phase 2).
4. WASM transport bootstrap PR (Phase 3).
5. Asset parity PR (Phase 4).
6. Parity tests + final contract/doc sync PR (Phases 5-6).

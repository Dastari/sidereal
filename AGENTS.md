# AGENTS.md

Project operating contract for human and AI contributors working in this repository.

## 1. Scope and Intent

- This repo is rebuilding **Sidereal** from scratch as a server-authoritative multiplayer architecture.
- Work must follow the documented phased plan and invariants.
- Do not introduce ad-hoc architecture that conflicts with the design documents.

## 2. Source-of-Truth Documentation

- Primary architecture/spec: `docs/sidereal_design_document.md`
- Implementation sequencing/checklists: `docs/sidereal_implementation_checklist.md`
- UI design system and component patterns: `docs/ui_design_guide.md`
- Workspace test area guidance: `tests/README.md`
- Repo overview: `README.md`

If any code change conflicts with docs, update docs in the same change or stop and resolve ambiguity first.

## 3. Non-Negotiable Technical Rules

- Authority flow is one-way: `client input -> shard sim -> replication/distribution -> persistence`.
- Clients never authoritatively set world transforms/state.
- Keep identity crossing boundaries as UUID/entity IDs only (no raw Bevy `Entity` IDs over service boundaries).
- Keep shared simulation/prediction/gameplay logic in shared crates, not duplicated across client targets.
- Persistable gameplay ECS components must support `Reflect` + serde (`Serialize`/`Deserialize`) and be mappable to graph persistence with hydration roundtrip coverage.
- Gameplay component source-of-truth is core (`crates/sidereal-game`); new persistable component families must flow through the shared component registry/generation path rather than ad-hoc per-service definitions.
- Bevy hierarchy relationships (`Children`/parent-child) and modular mount relationships (for example hardpoints -> engines/shield generators/flight computers) must persist as graph relationships and hydrate back deterministically.
- Visibility/range logic must be generic over entities (not ship-only). Use `ScannerRangeM`/related generic components for dynamic sensor range behavior; do not hardcode ship-specific visibility assumptions.
- Replication input routing must be bound to authenticated session identity. Bind transport peer/session (`RemoteId`) to authenticated `player_entity_id` and reject mismatched claimed player IDs in subsequent input packets.
- Hydration/persistence must preserve hierarchy semantics: persist parent-child and mount relationships, then rebuild Bevy hierarchy deterministically during hydration so child transform offsets remain correct.
- Inventory-bearing entities must feed dynamic mass derivation (`CargoMassKg`/`ModuleMassKg`/`TotalMassKg`) and runtime physics mass updates so acceleration behavior reflects mounted modules and nested inventories.
- Avian runtime-only transient internals are excluded from persistence; durable gameplay state must be mirrored into persistable components.
- Native and WASM client builds are co-maintained; WASM is never a deferred concern. Both must build and pass quality gates at every change, not just when "the WASM phase" arrives.
- Native may be the current delivery priority, but that never relaxes WASM parity requirements; client behavior and protocol changes must keep WASM in lockstep in the same change.
- The client is one workspace member (`bins/sidereal-client`) with a native `[[bin]]` target and a WASM `[lib]` target. There is no separate `sidereal-client-web` crate.
- Use generic entity terminology in systems/resources/APIs that are not inherently domain-specific. Avoid naming generic runtime structures with `Ship*` prefixes (for example visibility maps, control maps, authority registries). Reserve ship-specific names only for truly ship-only behavior.
- Platform branching uses `cfg(target_arch = "wasm32")` only. Never use a cargo feature flag to gate native-vs-WASM code paths; `target_arch` is set automatically by the compiler and cannot be miscombined.
- WASM uses platform-specific network adapters only at the transport boundary. All gameplay, prediction, reconciliation, and ECS systems are shared and must compile for both targets without conditional compilation.
- Browser transport direction is WebRTC-first (unreliable/unordered data channels for game state, ordered/reliable channel for session control). WebSocket is allowed only as an explicit fallback. New WASM transport work must not default to WebSocket.
- Asset delivery is stream-based from backend to client; no standalone HTTP asset file serving.
- Client cache is MMO-style local cache: single `assets.pak` + companion index/metadata, with checksum/version invalidation.
- `bevy_remote` inspection endpoints for shard/replication/client must be auth-gated and follow project security defaults.

## 4. Implementation Workflow Requirements

- Implement in phase order from `docs/sidereal_implementation_checklist.md` unless dependency constraints require otherwise.
- For each feature change, include:
  - code updates,
  - unit tests in touched crates,
  - integration test updates if cross-service behavior changes,
  - doc updates for protocol/runtime/architecture changes.
- For new gameplay components, include persistence/hydration mapping updates (or explicit non-persisted runtime-only rationale) and tests in the same change.
- For scripting-connected components (for example `FlightComputer`), script APIs may emit intent only; scripts must not directly authoritatively mutate transforms/velocities/ownership or bypass Rust authority systems.
- Keep boundaries explicit between crates/services (no persistence/network leakage into gameplay core).
- When adding or changing client-side code: verify both native and WASM targets still build. If a change breaks the WASM target, fix it in the same PR before marking complete. Do not defer WASM build failures.
- WASM client validation must include WebGPU support in the build configuration (`bevy/webgpu`), not only default WASM feature sets.
- When changing client behavior, transport contracts, prediction/reconciliation flow, or client runtime defaults: update docs to note native impact and WASM impact (or explicitly state "no WASM impact").
- When adding a new client-side dependency: verify it is either WASM-compatible or correctly gated behind `cfg(not(target_arch = "wasm32"))` with a WASM-compatible alternative also provided.
- When adding or changing client UI: follow the design system specified in `docs/ui_design_guide.md`. Match existing color palette, spacing, and component patterns. Do not introduce new colors or patterns without updating the design guide first.
- For error handling in client: use persistent dialog UI (`dialog_ui::DialogQueue::push_error()`) for failures requiring user acknowledgment. Do not rely on console logs or ephemeral status text for critical errors.

## 5. Runtime and Environment Conventions

- Postgres + AGE local infra is defined in `docker-compose.yaml`.
- Initialization SQL for AGE/graph lives under `docker/init/`.
- Asset root default is `./data`.
- Follow runtime defaults and env vars listed in `docs/sidereal_design_document.md`.

## 6. Quality Gates (Minimum)

Before marking work complete, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

If client code was touched, also verify the WASM and Windows targets compile:

```bash
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

This requires the targets to be installed (`rustup target add wasm32-unknown-unknown x86_64-pc-windows-gnu`) and a MinGW cross-linker (`x86_64-w64-mingw32-gcc`). The workspace `.cargo/config.toml` configures the linker for the Windows GNU target. If a target toolchain is not installed in the local environment, note it in the change but do not skip the check in CI.

Run targeted tests for touched crates; run broader integration tests when flow boundaries are impacted.

## 7. Documentation Maintenance Rule (Enforceable)

When adding any new **critical or enforceable** behavior (security rule, protocol contract, transport rule, runtime default, operational requirement), you must:

1. Update the relevant docs under `docs/`.
2. Update this `AGENTS.md` if the new rule changes contributor/agent behavior or enforcement expectations.

Do not defer this to a later PR.

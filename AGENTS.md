# AGENTS.md

Project operating contract for human and AI contributors working in this repository.

## 1. Scope and Intent

- This repo is rebuilding **Sidereal** from scratch as a server-authoritative multiplayer architecture.
- Work must follow the documented phased plan and invariants.
- Do not introduce ad-hoc architecture that conflicts with the design documents.

## 2. Source-of-Truth Documentation

- Primary architecture/spec: `docs/sidereal_design_document.md`
- Decision register: `docs/decision_register.md`
- Component authoring workflow/macros: `docs/component_authoring_guide.md`
- Visibility/replication implementation contract: `docs/features/visibility_replication_contract.md`
- Asset delivery implementation contract: `docs/features/asset_delivery_contract.md`
- Lightyear upstream issue triage reference: `docs/features/lightyear_upstream_issue_snapshot.md`
- UI design system and component patterns: `docs/ui_design_guide.md`
- Repo overview: `README.md`

If any code change conflicts with docs, update docs in the same change or stop and resolve ambiguity first.

## 3. Non-Negotiable Technical Rules

- Authority flow is one-way: `client input -> shard sim -> replication/distribution -> persistence`.
- Clients never authoritatively set world transforms/state.
- Keep identity crossing boundaries as UUID/entity IDs only (no raw Bevy `Entity` IDs over service boundaries).
- Player runtime state (control target, selection target, focus target, camera position) must live on the persisted player ECS entity/components in graph persistence; do not add separate per-player SQL side tables for authoritative runtime state.
- Player-specific persistent progression/state (for example score, quest progression, character-local settings, local player data) must live on persisted player ECS entity/components, not account rows or ad-hoc side stores.
- Accounts are identity/auth containers and may own multiple player entities (characters). Runtime/session binding is account-authenticated but control/selection/progression state is player-entity scoped.
- Keep shared simulation/prediction/gameplay logic in shared crates, not duplicated across client targets.
- Persistable gameplay ECS components must support `Reflect` + serde (`Serialize`/`Deserialize`) and be mappable to graph persistence with hydration roundtrip coverage.
- Gameplay component source-of-truth is core (`crates/sidereal-game`); new persistable component families must flow through the shared component registry/generation path rather than ad-hoc per-service definitions.
- Custom gameplay components must be defined as individual files under `crates/sidereal-game/src/components/` (one primary component per file; tightly-coupled helper types may live alongside it) and re-exported via `components/mod.rs`.
- New persistable/replicated custom components must use `#[sidereal_component(kind = \"...\", persist = ..., replicate = ..., visibility = [...])]`; when `visibility` is omitted, owner-only is the default policy.
- Bevy hierarchy relationships (`Children`/parent-child) and modular mount relationships (for example hardpoints -> engines/shield generators/flight computers) must persist as graph relationships and hydrate back deterministically.
- Visibility/range logic must be generic over entities (not ship-only). Canonical runtime direction is `VisibilityRangeM` / `VisibilityRangeBuffM`; do not hardcode ship-specific visibility assumptions or hidden `ShipTag` baseline range behavior.
- Static non-physics world entities (for example planets, stars, and decorative celestial bodies) must use the generic `WorldPosition` / `WorldRotation` lane rather than Avian transform components unless they are actually simulated by physics.
- Visibility policy must preserve valid no-ship/no-engine states and support data-driven public/faction visibility (`PublicVisibility`, `FactionId`, `FactionVisibility`) without spawning fallback gameplay modules.
- Changes touching visibility, replication delivery, or redaction must follow `docs/features/visibility_replication_contract.md` and update it when behavior/policy changes.
- Replication input routing must be bound to authenticated session identity. Bind transport peer/session (`RemoteId`) to authenticated `player_entity_id` and reject mismatched claimed player IDs in subsequent input packets.
- Hydration/persistence must preserve hierarchy semantics: persist parent-child and mount relationships, then rebuild Bevy hierarchy deterministically during hydration so child transform offsets remain correct.
- Inventory-bearing entities must feed dynamic mass derivation (`CargoMassKg`/`ModuleMassKg`/`TotalMassKg`) and runtime physics mass updates so acceleration behavior reflects mounted modules and nested inventories.
- Avian runtime-only transient internals are excluded from persistence; durable gameplay state must be mirrored into persistable components.
- Native and WASM client builds are co-maintained; WASM is never a deferred concern. Both must build and pass quality gates at every change, not just when "the WASM phase" arrives.
- Native may be the current delivery priority, but that never relaxes WASM parity requirements; client behavior and protocol changes must keep WASM in lockstep in the same change.
- The client is one workspace member (`bins/sidereal-client`) with a native `[[bin]]` target and a WASM `[lib]` target. There is no separate `sidereal-client-web` crate.
- Use generic entity terminology in systems/resources/APIs that are not inherently domain-specific. Avoid naming generic runtime structures with `Ship*` prefixes (for example visibility maps, control maps, authority registries). Reserve ship-specific names only for truly ship-only behavior.
- Enforce single-writer motion ownership per runtime mode: for controlled predicted entities, only one fixed-tick pipeline may write authoritative motion state (`Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`). Visual interpolation/camera systems must not feed render transforms back into simulation state.
- Do not reintroduce legacy gameplay mirror motion components (`PositionM`/`VelocityMps`/`HeadingRad`) for runtime replication or simulation paths; use Avian authoritative motion components directly.
- Enforce gameplay-physics mass/inertia parity: whenever gameplay force/torque code depends on gameplay mass/inertia, Avian `Mass` and `AngularInertia` must be present and synchronized on the same entity from spawn/hydration onward and updated with runtime mass recomputation in the same simulation flow.
- Persist/hydrate runtime state using graph records (`GraphEntityRecord`/`GraphComponentRecord`) and relationships as the canonical persistence shape. Do not introduce new `WorldStateDelta`/`WorldDeltaEntity`/`WorldComponentDelta` persistence paths.
- Early-development schema discipline is strict: do not add legacy compatibility aliases/default backfills/migration shims for renamed or reshaped gameplay/script component payloads. Update all producers/consumers to one canonical schema and reset local/dev databases when schema changes.
- Predicted local input intent is client-owned: do not overwrite pending local control intent for the controlled predicted entity with replicated server intent state; replicated control state is for confirmation/correction flow, not local intent ownership.
- Simulation and prediction math must use fixed-step time resources only; frame-time deltas are render/UI-only and must not drive authoritative force/integration math.
- When changing streamed shaders/material bindings, keep source and streamed cache shader paths in schema parity in the same change (or generate both from one source) so runtime does not diverge by load path.
- Large runtime refactors must split mixed concerns into domain modules; avoid continuing monolithic growth in client/server entrypoints. Keep entrypoints focused on app wiring and plugin composition.
- Platform branching uses `cfg(target_arch = "wasm32")` only. Never use a cargo feature flag to gate native-vs-WASM code paths; `target_arch` is set automatically by the compiler and cannot be miscombined.
- WASM uses platform-specific network adapters only at the transport boundary. All gameplay, prediction, reconciliation, and ECS systems are shared and must compile for both targets without conditional compilation.
- Browser transport direction is WebTransport-first. WebSocket is allowed only as an explicit fallback. New WASM transport work must not default to WebSocket.
- Asset payload delivery is gateway HTTP-based via authenticated `/assets/<asset_guid>` fetches; replication transport must not stream asset payload bytes.
- Concrete asset definitions (asset IDs, filenames, shader/material/audio/sprite references, bootstrap-required sets, dependency metadata) are authored in Lua asset registry scripts and generated catalogs, not hardcoded in Rust runtime code.
- Client cache is MMO-style local cache: single `assets.pak` + companion index/metadata, with checksum/version invalidation.
- Browser/WASM runtime asset mounting must be byte-backed from the authenticated cache adapter or gateway fetch path; browser code must not rely on filesystem-style `AssetServer` paths such as `data/cache_stream/...`.
- `bevy_remote` inspection endpoints for shard/replication/client must be auth-gated and follow project security defaults. Until a real authenticated HTTP gate exists, BRP must remain loopback-only.

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
- If Lightyear behaviour appears unexplained, check `docs/features/lightyear_upstream_issue_snapshot.md` before assuming the issue is local-only or introducing a workaround. If the behaviour matches an upstream issue, reference it in the change; if not, update the snapshot with the new upstream search result.
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
- Replication and gateway tracing output is written to both the console and workspace-relative `./logs/` with a fresh timestamped file per process start; use the persisted log files for debugging service startup, transport, auth, and runtime behavior.
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
2. If this is an in-depth/project-wide decision, add or update a dedicated decision detail file under `docs/features/` using `dr-XXXX_<slug>.md`, and link it from `docs/decision_register.md`.
3. Update this `AGENTS.md` if the new rule changes contributor/agent behavior or enforcement expectations.

Do not defer this to a later PR.

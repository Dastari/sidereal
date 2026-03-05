# Decision Register

Status: Active  
Audience: engineering and design contributors

## Purpose

This register captures project-wide architectural and gameplay-policy decisions that affect multiple systems.

Use this file to:
- make decisions explicit,
- record tradeoffs and alternatives,
- prevent accidental regressions in future refactors.

When a decision needs a dedicated detail document, store it under `docs/features/` using:
- `dr-XXXX_<slug>.md`

Not every decision requires a dedicated detail doc (for example when an existing feature contract already fully covers it), but every decision must link either:
- a dedicated decision doc, or
- the existing feature contract/plan that is its source-of-truth.

## Process

For each decision:
1. Add a new entry with a stable ID (`DR-XXXX`).
2. Set `Status` (`Proposed`, `Accepted`, `Superseded`, `Deprecated`).
3. Document rationale and alternatives.
4. For in-depth decisions, create/update `docs/features/dr-XXXX_<slug>.md`.
5. If a dedicated doc is not needed, link the existing source-of-truth feature contract/plan under `docs/features/`.
6. Link impacted docs/code/tests.
7. If superseded, keep the old entry and reference the replacement.

## Entry Template

```md
## DR-XXXX: <Title>
- Status: Proposed | Accepted | Superseded | Deprecated
- Date: YYYY-MM-DD
- Owners: <names/role>
- Context:
  - <problem statement>
- Decision:
  - <what we decided>
- Alternatives considered:
  - <option A + why rejected>
  - <option B + why rejected>
- Consequences:
  - Positive:
    - <...>
  - Negative:
    - <...>
- Follow-up:
  - <required tasks/docs/tests>
- Feature doc:
  - `docs/features/dr-XXXX_<slug>.md` (preferred for in-depth decisions)
  - or an existing source-of-truth `docs/features/<feature_doc>.md`
- References:
  - <docs/code paths>
```

## Decisions

## DR-0001: Account / Character / Session Terminology
- Status: Accepted
- Date: 2026-02-24
- Owners: Core runtime team
- Context:
  - Ambiguous use of "player" caused identity confusion across auth, runtime, and persistence.
- Decision:
  - `Account` is the authenticated identity container (credentials/tokens).
  - `Character` is the durable gameplay identity, represented by a persisted player ECS entity (`player_entity_id`).
  - `Session` is the runtime transport binding between a connected client and one selected character.
  - "Player" remains informal UX language only.
- Alternatives considered:
  - Keep "player" as technical type across all layers: rejected (ambiguous in multi-character scenarios).
- Consequences:
  - Positive:
    - Clear identity boundaries.
    - Multi-character support remains first-class.
  - Negative:
    - Requires ongoing naming discipline in code/docs/tests.
- Follow-up:
  - Prefer `Character`/`Session` naming in new test and protocol docs.
- Decision doc:
  - `docs/features/dr-0001_account_character_session_model.md`
- References:
  - `docs/sidereal_design_document.md`
  - `docs/features/test_topology_and_resilience_plan.md`

## DR-0002: Explicit World Entry Lifecycle
- Status: Accepted
- Date: 2026-02-24
- Owners: Core runtime team
- Context:
  - Implicit world entry during register/login hid runtime state issues and made flow coupling brittle.
- Decision:
  - Register/login are auth-only.
  - World entry is explicit via character selection + Enter World request.
  - Gateway validates account ownership of selected `player_entity_id` before runtime bootstrap.
- Alternatives considered:
  - Auto-enter world on login/register: rejected (tight coupling, weaker observability/fail-fast behavior).
- Consequences:
  - Positive:
    - Deterministic lifecycle: Auth -> Character Select -> Enter World -> In World.
    - Better failure visibility and testability.
  - Negative:
    - Requires extra client state/UI handling.
- Follow-up:
  - Keep coverage for ownership validation and missing-entity rejection paths.
- Decision doc:
  - `docs/features/dr-0002_explicit_world_entry_flow.md`
- References:
  - `docs/sidereal_design_document.md`
  - `bins/sidereal-gateway/src/api.rs`
  - `bins/sidereal-client/src/native.rs`
  - `bins/sidereal-replication/src/bootstrap.rs`

## DR-0003: Logout Presence Policy (Open)
- Status: Proposed
- Date: 2026-02-24
- Owners: Gameplay + runtime + economy design
- Context:
  - Logout behavior impacts combat logging, economy risk, AI crew design, and persistence load.
  - Future idea: allow giving orders, then logout while AI crew continues.
- Decision:
  - Not yet finalized.
  - Candidate baseline for v1 discussion: conditional persistence policy.
- Alternatives considered:
  - Despawn on logout (safe/simple, low simulation continuity).
  - Always persist in-world while offline (high continuity, high complexity/risk).
  - Conditional persist (for example docked safe, undocked AI/offline rules).
- Consequences:
  - Positive:
    - Decision deferred intentionally with explicit tracking.
  - Negative:
    - Some systems must remain flexible until policy is accepted.
- Follow-up:
  - Accept a single logout presence policy before implementing offline AI control.
  - Define exploit and abuse constraints (combat logging, dock abuse, reconnect reclaim).
  - Add resilience tests once accepted.
- Decision doc:
  - `docs/features/dr-0003_logout_presence_policy.md`
- References:
  - `docs/features/test_topology_and_resilience_plan.md`
  - `docs/sidereal_design_document.md`

## DR-0004: Asset Catalog as Authoritative Source of Truth
- Status: Accepted
- Date: 2026-02-24
- Owners: Runtime + content pipeline
- Context:
  - Current runtime still contains hardcoded/static asset source mappings for critical assets.
  - We need a scalable registration flow that avoids manual per-asset server wiring and supports blob-backed delivery.
- Decision:
  - Adopt a generated `AssetCatalog` as the authoritative runtime master list.
  - All runtime asset resolution uses logical `asset_id` + catalog metadata (version/hash/dependencies/storage location).
  - Manual per-asset registration in runtime code is allowed only as temporary migration shims.
- Alternatives considered:
  - Keep static lists in code: rejected (not scalable, error-prone, hard to evolve).
  - Infer assets by filesystem scan at runtime: rejected (non-deterministic and weak operational control).
- Consequences:
  - Positive:
    - Deterministic, releaseable asset behavior.
    - Supports automated publish tooling and compatibility checks.
  - Negative:
    - Requires build/publish pipeline investment and schema governance.
- Follow-up:
  - Implement first-party asset tooling (`init/validate/build/publish/activate`).
  - Migrate hardcoded source lists to catalog-backed lookups.
- Decision doc:
  - `docs/features/asset_delivery_contract.md`
- References:
  - `docs/features/asset_delivery_contract.md`
  - `crates/sidereal-asset-runtime/src/lib.rs`
  - `bins/sidereal-replication/src/replication/assets.rs`

## DR-0005: Blob Storage for Runtime Asset Payloads (Not Postgres Blobs)
- Status: Accepted
- Date: 2026-02-24
- Owners: Runtime + infra
- Context:
  - We need MMO-style streamed asset payload delivery with cache reuse and patch-like updates.
  - Question raised whether large asset blobs should live in Postgres.
- Decision:
  - Runtime asset payloads (`assets.pak`/chunks) are stored in blob/object storage.
  - Postgres remains focused on gameplay ECS persistence and optional catalog metadata/release pointers.
  - Asset payloads are not stored as heavy Postgres blob rows in baseline architecture.
- Alternatives considered:
  - Store all payloads in Postgres blobs: rejected (WAL/backup/ops burden and coupling risk).
  - Keep only local disk files per server: rejected for multi-node operational consistency at scale.
- Consequences:
  - Positive:
    - Better storage semantics for immutable binary payloads.
    - Cleaner separation between world-state persistence and content delivery.
  - Negative:
    - Requires blob backend integration and release artifact management.
- Follow-up:
  - Add blob-backed publish and fetch adapters.
  - Document environment-specific backend wiring (emulator/dev/prod).
- Decision doc:
  - `docs/features/asset_delivery_contract.md`
- References:
  - `docs/features/asset_delivery_contract.md`
  - `docs/sidereal_design_document.md`

## DR-0006: Immutable Asset Versioning with Optional Alias Mapping
- Status: Accepted
- Date: 2026-02-24
- Owners: Runtime + content pipeline
- Context:
  - Asset update behavior must support deterministic cache invalidation and rollback-safe delivery.
  - Need to avoid mutating payload semantics behind unchanged IDs.
- Decision:
  - Asset versions are immutable and content-derived (hash/version based).
  - Content updates publish new immutable version IDs.
  - Optional alias mapping is allowed for stable names that point to selected immutable versions.
- Alternatives considered:
  - Mutable `asset_id` in place: rejected (cache ambiguity, rollback risk).
  - Always force versioned IDs everywhere without aliases: rejected for ergonomics in some pipelines.
- Consequences:
  - Positive:
    - Deterministic cache behavior and validation.
    - Cleaner patch/delta semantics.
  - Negative:
    - Requires alias governance and release promotion discipline.
- Follow-up:
  - Add alias schema/manifest support in tooling and catalog loader.
  - Add tests for stale-cache invalidation and alias repoint behavior.
- Decision doc:
  - `docs/features/asset_delivery_contract.md`
- References:
  - `docs/features/asset_delivery_contract.md`
  - `crates/sidereal-asset-runtime/src/lib.rs`

## DR-0007: Generic Server-Authoritative Entity Variant Framework
- Status: Proposed
- Date: 2026-02-24
- Owners: Gameplay + runtime
- Context:
  - Variant support is needed for ships and non-ship entities (missiles, stations, cargo containers, etc.).
  - Ad-hoc per-archetype variant implementations would fragment behavior and increase drift.
- Decision:
  - Implement a generic variant framework with base archetype + variant overlay model.
  - Variant selection is server-authoritative and deterministic (explicit or seeded weighted policy).
  - Selected variant identity (`VariantId`) is persisted and hydrated roundtrip.
  - Framework is entity-generic, not ship-specific.
- Alternatives considered:
  - New bundle per variant: rejected for maintenance and combinatorial growth.
  - Client-side variant selection: rejected (authority/security mismatch).
- Consequences:
  - Positive:
    - Reusable variant model across multiple entity families.
    - Deterministic behavior under replication/hydration.
  - Negative:
    - Requires shared overlay engine and validation rules.
- Follow-up:
  - Implement generic variant components/registry and spawn integration.
  - Promote to Accepted after first production use across at least 3 entity families.
- Decision doc:
  - `docs/features/dr-0007_entity_variant_framework.md`
- References:
  - `docs/features/dr-0007_entity_variant_framework.md`

## DR-0017: Dual-Lane Replication and Owner Asset Manifest
- Status: Proposed
- Date: 2026-03-05
- Owners: Replication + client runtime + gameplay UI
- Context:
  - Tactical zoom/map needs broad low-detail awareness while local in-world simulation needs high-detail local updates.
  - Owned-asset UI currently depends on local bubble presence, causing owned entities to disappear from UI when out of scope.
- Decision:
  - Introduce three explicit server-authored delivery models:
    - `LocalBubbleLane` for high-rate nearby simulation state.
    - `TacticalLane` for lower-rate wide-area reduced contact state.
    - `OwnerAssetManifestLane` for owner-only, relevance-independent asset list/state.
  - Client stores owner-manifest data in a dedicated cache resource and UI reads from this cache (not world-entity presence).
- Alternatives considered:
  - Keep owned-asset UI bound to world entities: rejected (visibility-coupled disappearing UX).
  - Expand one global relevance radius: rejected (bandwidth/scaling regressions).
  - Client-side polling side channel: rejected (authority-flow and coupling violations).
- Consequences:
  - Positive:
    - Tactical map and owned-asset UX become robust and mode-appropriate.
    - Preserves server authority while reducing unnecessary high-frequency replication.
  - Negative:
    - Adds protocol/channel complexity and client cache maintenance.
- Follow-up:
  - Define message schemas and pacing for tactical and owner-manifest lanes.
  - Add sequence/staleness telemetry and tests for lane behavior and ownership isolation.
- Decision doc:
  - `docs/features/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
- References:
  - `docs/features/visibility_replication_contract.md`
  - `docs/features/scan_intel_minimap_spatial_plan.md`
  - `docs/features/tactical_and_owner_lane_protocol_contract.md`
  - `docs/sidereal_design_document.md`

## DR-0018: Fog of War and Intel Memory Model
- Status: Proposed
- Date: 2026-03-05
- Owners: Replication + gameplay visibility + tactical UI
- Context:
  - Tactical map needs persistent exploration and stale-vs-live intel behavior.
  - Local bubble relevance cannot be the source-of-truth for long-term discovery memory.
- Decision:
  - Persist player-scoped explored coverage and intel memory on server/player data.
  - Keep live scanner visibility runtime-derived per tick.
  - Deliver fog/contact tactical products via lane payloads (snapshot+delta), with explicit live/stale state.
- Alternatives considered:
  - Infer all fog/intel client-side from live world entities: rejected (relevance-coupled and authority-weak).
  - Store global per-shard discovery only: rejected (not player-specific).
- Consequences:
  - Positive:
    - Correct MMO fog semantics with server authority.
    - UI can render unexplored/explored-stale/live states deterministically.
  - Negative:
    - Adds persisted player data and tactical lane complexity.
- Follow-up:
  - Define message schemas + sequence semantics.
  - Add tests for exploration growth, stale/live transitions, and disclosure safety.
- Decision doc:
  - `docs/features/dr-0018_fog_of_war_and_intel_memory_model.md`
- References:
  - `docs/features/visibility_replication_contract.md`
  - `docs/features/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
  - `docs/features/tactical_and_owner_lane_protocol_contract.md`
  - `docs/sidereal_design_document.md`

## DR-0008: Character Ownership Is Enforced at Every Runtime Boundary
- Status: Accepted
- Date: 2026-02-24
- Owners: Core runtime team
- Context:
  - Multi-character support requires consistent ownership enforcement from auth through replication bind.
  - Silent fallback or implicit rebind behavior creates authority and security ambiguity.
- Decision:
  - Character ownership validation is mandatory at both gateway world-entry and replication auth-bind boundaries.
  - Requests/messages with mismatched account-to-character ownership are rejected explicitly.
  - Runtime bootstrap remains idempotent per `player_entity_id` (character), not per account.
- Alternatives considered:
  - Validate only at gateway: rejected (replication boundary still vulnerable to stale/spoofed bind attempts).
  - Auto-correct/fallback to default character: rejected (breaks explicit selection and hides errors).
- Consequences:
  - Positive:
    - Stronger authority guarantees and cleaner diagnostics.
    - Correct multi-character behavior under reconnect/switch flows.
  - Negative:
    - Requires test fixtures/tokens to include valid ownership shape.
- Follow-up:
  - Keep ownership rejection-path tests in gateway and replication suites.
  - Keep auth token/test fixture generation aligned with current claims validation.
- Decision doc:
  - `docs/features/dr-0002_explicit_world_entry_flow.md`
- References:
  - `docs/features/dr-0002_explicit_world_entry_flow.md`
  - `docs/features/dr-0001_account_character_session_model.md`
  - `bins/sidereal-gateway/src/auth.rs`
  - `bins/sidereal-replication/src/replication/auth.rs`
  - `bins/sidereal-replication/src/bootstrap.rs`

## DR-0009: Register Is Fail-Closed on Starter-World Persistence
- Status: Accepted
- Date: 2026-02-24
- Owners: Gateway + persistence team
- Context:
  - Register now creates durable character identity and starter world state before world entry.
  - Allowing auth success without durable starter world persistence would create broken accounts/characters.
- Decision:
  - Registration fails if starter world persistence fails.
  - Production uses graph-backed starter-world persistence dependency; tests/in-memory paths may use noop persister.
  - No automatic runtime bootstrap is performed during register/login.
- Alternatives considered:
  - Auth success with deferred async world creation: rejected (eventual-consistency race and poor failure visibility).
  - Auto-bootstrap world entry during register/login: rejected (lifecycle coupling).
- Consequences:
  - Positive:
    - New accounts are guaranteed to have durable starter state when registration reports success.
    - Cleaner separation between identity creation and runtime world entry.
  - Negative:
    - Current implementation is non-atomic across auth DB and graph persistence, so failed persistence can still strand an account row.
- Follow-up:
  - Add compensation/transaction strategy to avoid stranded account rows on persistence failure.
  - Add explicit test coverage for partial-failure remediation.
- Decision doc:
  - `docs/features/dr-0002_explicit_world_entry_flow.md`
- References:
  - `docs/features/dr-0002_explicit_world_entry_flow.md`
  - `bins/sidereal-gateway/src/auth.rs`
  - `docs/sidereal_design_document.md`

## DR-0010: World Snapshot APIs Must Be Character-Scoped
- Status: Proposed
- Date: 2026-02-24
- Owners: Gateway + gameplay runtime team
- Context:
  - With account->many-characters, account-scoped world snapshot resolution can return the wrong character-owned ship/state.
  - Character-local state is a core model invariant.
- Decision:
  - Any future world snapshot/read APIs should resolve by selected/bound `player_entity_id`, not by account-wide ownership alone.
  - Character-local camera/control/focus/selection and controlled-entity resolution remain tied to the selected character identity.
- Alternatives considered:
  - Keep account-scoped lookup and select first matching ship: rejected (non-deterministic for multi-character accounts).
  - Keep deprecated single-character assumption in API layer: rejected (conflicts with accepted account/character/session model).
- Consequences:
  - Positive:
    - Deterministic character-specific world hydration/snapshot behavior.
    - Eliminates cross-character leakage risk in gateway world responses.
  - Negative:
    - Requires follow-up API contract and query updates when/if such endpoints are reintroduced.
- Note:
  - The gateway previously exposed `/world/me`; it was removed (unused by clients; world state and asset manifests are delivered via the replication stream). Any future world-read endpoint must follow character-scoped resolution per this decision.
- Decision doc:
  - `docs/features/dr-0001_account_character_session_model.md`
- References:
  - `docs/features/dr-0001_account_character_session_model.md`
  - `docs/features/dr-0002_explicit_world_entry_flow.md`
  - `docs/sidereal_design_document.md`

## DR-0011: Headless Client/Test Runtime Must Preserve Core Bevy Resource Invariants
- Status: Proposed
- Date: 2026-02-24
- Owners: Client + test infrastructure team
- Context:
  - Headless transport e2e runs currently exercise real runtime systems.
  - Minimal plugin setups can omit required Bevy resources and trigger panics under transform/replication systems.
- Decision:
  - Headless client runtime used by transport/integration tests must include all required core Bevy resources/plugins for systems it executes.
  - Test mode should not bypass ownership/auth bind invariants; fixtures must satisfy the same contracts as production flows.
- Alternatives considered:
  - Keep ultra-minimal headless app and ignore missing-resource panics: rejected (unstable test signal).
  - Disable substantial runtime systems in tests: rejected (reduced coverage of actual integration behavior).
- Consequences:
  - Positive:
    - More reliable e2e tests with production-like runtime invariants.
    - Fewer false failures from environment setup drift.
  - Negative:
    - Slightly heavier headless runtime startup in tests.
- Follow-up:
  - Update headless bootstrap/plugin wiring to satisfy transform resource requirements.
  - Update transport e2e token/fixture generation to satisfy current claims + ownership validation.
- Decision doc:
  - `docs/features/test_topology_and_resilience_plan.md`
- References:
  - `docs/features/test_topology_and_resilience_plan.md`
  - `docs/features/dr-0002_explicit_world_entry_flow.md`
  - `bins/sidereal-client/src/native.rs`
  - `bins/sidereal-replication/tests/transport_lightyear_e2e.rs`

## DR-0012: Visibility Pipeline Contract Uses Authorization-First Semantics with Safe Candidate Preselection
- Status: Accepted
- Date: 2026-02-24
- Owners: Replication + gameplay security team
- Context:
  - Visibility implementation discussions used both "authorization-first" and "opt-in candidate-first" wording.
  - Ambiguity risks security regressions and inconsistent implementation across replication, scan intel, and redaction.
- Decision:
  - Canonical visibility pipeline is:
    1. Authorization scope (security entitlement),
    2. Delivery/interest narrowing (performance),
    3. Payload redaction (component/field disclosure gate).
  - Spatial candidate preselection may run before full authorization as an optimization input only.
  - Candidate preselection must not be treated as authorization and must not exclude policy-required exceptions (ownership/public/faction/grants).
- Alternatives considered:
  - Delivery-first contract as primary semantics: rejected (security ambiguity and easier misuse).
  - Full-world scan only with no preselection: rejected (does not scale).
- Consequences:
  - Positive:
    - Clear security/performance separation.
    - Supports opt-in spatial performance techniques without weakening policy guarantees.
  - Negative:
    - Requires ongoing discipline to keep preselection logic fail-closed and exception-aware.
- Follow-up:
  - Keep visibility contracts and feature plans aligned to the same pipeline language.
  - Add tests ensuring preselection cannot bypass authorization exceptions.
- Decision doc:
  - `docs/features/visibility_replication_contract.md`
- References:
  - `docs/sidereal_design_document.md`
  - `docs/features/visibility_replication_contract.md`
  - `docs/features/scan_intel_minimap_spatial_plan.md`

## DR-0013: Component-Driven Action Acceptors and Control-Context Routing
- Status: Proposed
- Date: 2026-02-24
- Owners: Gameplay runtime + replication + client input
- Context:
  - Input/action flow needs to support entity-generic gameplay (ships, characters, scanners, combat systems, and future entity families) without ship-only routing assumptions.
  - Current runtime has intent actions and action queues, but server ingress remains flight-centric and control routing is not yet fully generalized.
- Decision:
  - Adopt a component-driven action acceptor model:
    - entities receive high-level actions,
    - components on that entity accept/handle specific actions,
    - multiple components may accept the same action.
  - Route actions by authoritative control context:
    - movement/actions always route to `ControlledEntityGuid` target,
    - free-roam uses self-control (`ControlledEntityGuid = player guid`).
  - Add a configurable keybind/input-binding layer on client between physical input and actions.
  - Keep server authority and authenticated routing invariants unchanged.
- Alternatives considered:
  - Keep flight-only routing and add ad-hoc side paths per feature: rejected (does not scale to multi-entity gameplay).
  - Move action interpretation to client for flexibility: rejected (authority/security conflict).
- Consequences:
  - Positive:
    - Generic action architecture across entity families.
    - Cleaner separation of input intent vs component execution logic.
    - Better extensibility for combat/scanner/utility systems.
  - Negative:
    - Requires multi-crate refactor (gameplay core, protocol, replication, client input/UI, tests).
    - Prediction policy becomes more explicit/complex across action families.
- Follow-up:
  - Implement phased migration plan for contracts, routing, movement acceptor, keybinds, and prediction policy.
  - Add control handoff and multi-acceptor determinism tests.
  - Maintain native/WASM parity through each phase.
- Decision doc:
  - `docs/features/dr-0013_action_acceptor_control_routing.md`
- References:
  - `docs/features/dr-0013_action_acceptor_control_routing.md`
  - `crates/sidereal-game/src/actions.rs`
  - `crates/sidereal-game/src/flight.rs`
  - `bins/sidereal-replication/src/replication/input.rs`
  - `bins/sidereal-client/src/native.rs`

## DR-0014: Project-Wide Migration to Server-Authoritative 2D Runtime (Avian2D + Sprites)
- Status: Proposed
- Date: 2026-02-25
- Owners: Gameplay runtime + replication + client runtime + asset pipeline
- Context:
  - Current runtime assumptions are 3D-centric (Avian3D, 3D camera flow, GLTF runtime visuals).
  - Project direction is top-down 2D gameplay with sprite-based rendering.
  - Migration must preserve existing authority, identity, persistence, and native/WASM parity contracts.
- Decision:
  - Adopt a phased whole-project migration to a 2D runtime:
    - Avian2D authoritative simulation/prediction,
    - top-down orthographic gameplay camera,
    - sprite-based visual pipeline with GLTF removed from runtime paths.
  - Use `docs/features/2d_migration_plan.md` as the execution plan and acceptance framework.
- Alternatives considered:
  - Keep Avian3D and only render sprites in pseudo-2D: rejected (retains unnecessary 3D runtime complexity).
  - Big-bang rewrite in one change: rejected (high regression risk and poor rollback safety).
  - Maintain long-term dual 2D/3D runtime support: rejected (duplication and contract drift).
- Consequences:
  - Positive:
    - Runtime architecture aligns with intended top-down 2D gameplay model.
    - Visual/content runtime complexity reduced by removing GLTF runtime dependencies.
  - Negative:
    - Migration touches multiple crates/contracts and requires staged rollout discipline.
    - Persistence/protocol compatibility must be managed carefully through transition.
- Follow-up:
  - Execute phases and quality gates in `docs/features/2d_migration_plan.md`.
  - Update impacted source-of-truth docs/contracts in the same changes that alter behavior.
  - Remove Avian3D/GLTF runtime paths only after replacement coverage is validated.
- Decision doc:
  - `docs/features/dr-0014_2d_runtime_migration.md`
- References:
  - `docs/features/dr-0014_2d_runtime_migration.md`
  - `docs/features/2d_migration_plan.md`
  - `docs/sidereal_design_document.md`
  - `docs/features/asset_delivery_contract.md`
  - `docs/features/visibility_replication_contract.md`

---

### DR-0015: Hierarchy via MountedOn, not replicated ChildOf

- Date: 2026-02-28
- Status: Accepted
- Context:
  - Bevy's `ChildOf`/`Children` relationship uses raw `Entity` references that are local to a single Bevy world.
  - When Lightyear replicates `ChildOf`, it must map server entity IDs to client entity IDs. Entity mapping order is undefined — a child can arrive before its parent is mapped, producing `Entity::PLACEHOLDER` and a panic.
  - The project already uses `MountedOn { parent_entity_id: Uuid, hardpoint_id: String }` to express module-to-parent relationships with UUID-based references that are safe across network boundaries.
- Decision:
  - `MountedOn` is the replicated/persisted source of truth for parent-child relationships.
  - Bevy `ChildOf`/`Children` hierarchy is NEVER replicated through Lightyear.
  - A shared system (`sync_mounted_hierarchy` in `sidereal-game`) reconstructs Bevy hierarchy locally on each world (server and client) from `MountedOn` + `EntityGuid` lookups.
  - The system runs in `PostUpdate` before `TransformSystems::Propagate` so `GlobalTransform` is correct for all mounted entities.
  - Hardpoint `offset_m` is applied as the child's `Transform` when a matching `Hardpoint` entity is found as a sibling under the same parent.
- Alternatives considered:
  - Replicate `ChildOf` directly: rejected (entity mapping order panics).
  - Use Lightyear `ReplicationGroup` to guarantee atomic spawn: rejected (ties all modules to ship visibility granularity and increases bandwidth per-change).
  - Manual world-position computation without Bevy hierarchy: rejected (loses Bevy transform propagation, gizmos, rendering integration).
- Consequences:
  - Positive:
    - No entity mapping panics on the client.
    - Bevy transform propagation works naturally via locally-reconstructed hierarchy.
    - `GlobalTransform` is correct for all entities, enabling future hardpoint-relative rendering and spatial queries.
  - Negative:
    - One-frame delay before hierarchy is established (entities arrive, next frame the system parents them).
    - Hardpoint offset resolution requires `Hardpoint` entities to already have `ChildOf` established (server: immediate via `with_children`; client: available after hardpoints themselves are parented).
- References:
  - `crates/sidereal-game/src/hierarchy.rs`
  - `AGENTS.md` (non-negotiable: clients never authoritatively set world transforms)

## DR-0016: Data-Driven Runtime Shader Bindings via Generic Material Types
- Status: Proposed
- Date: 2026-03-04
- Owners: Scripting + replication + client rendering + asset streaming
- Context:
  - Content scripting needs to support large numbers of shader-driven 2D visuals (sprites/polygons) without adding Rust boilerplate per shader.
  - Bevy can load/compile shader assets at runtime but cannot create/register brand-new Rust `Material2d` schemas from network/script payloads at runtime.
  - Current startup material plugin registrations are type-level wiring; this does not scale as a per-shader authoring model.
- Decision:
  - Use a small fixed set of generic runtime 2D material schemas on the client, registered at startup.
  - Drive per-entity shader selection and parameters through replicated gameplay components and script intent APIs.
  - Stream shader assets through the existing asset-delivery contract and compile/swap at runtime with deterministic fallback on failure.
- Alternatives considered:
  - Add one Rust `Material2d` type per shader: rejected (high boilerplate and poor scaling).
  - Runtime-generate new material schemas from Lua/network payloads: rejected (not compatible with Bevy material type model).
  - Client-only unsanctioned shader mutation bypassing server policy: rejected (authority/security contract violation).
- Consequences:
  - Positive:
    - Supports hundreds of shader assets without per-shader client type additions.
    - Keeps scripting flexibility while preserving authority, persistence, and replication boundaries.
    - Works with existing stream/cache invalidation model.
  - Negative:
    - Requires robust schema validation, fallback handling, and compile-thrash guardrails.
    - Some advanced shader binding patterns may need planned extensions to generic material schemas.
- Follow-up:
  - Implement phased plan in `docs/features/dynamic_runtime_shader_material_plan.md`.
  - Add tests for authorization, fallback behavior, cache invalidation, and native/WASM parity.
  - Promote to Accepted after end-to-end runtime path is implemented and validated.
- Decision doc:
  - `docs/features/dynamic_runtime_shader_material_plan.md`
- References:
  - `docs/features/scripting_support.md`
  - `docs/features/asset_delivery_contract.md`
  - `docs/sidereal_design_document.md`
  - `bins/sidereal-client/src/native/mod.rs`

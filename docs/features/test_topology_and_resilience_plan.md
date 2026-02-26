# Test Topology and Resilience Plan

Status: Planned work item (no code changes applied yet).

## 1. Purpose

This document tracks the agreed test reorganization and coverage expansion work:
- move inline tests out of production `src/*.rs` files into crate/bin `tests/` folders,
- expand coverage for asset delivery, cache management, and client non-crash behavior,
- validate player-entity camera/runtime state behavior against current architecture docs.

## 2. Architectural Baseline (From Docs)

References used:
- `AGENTS.md`
- `docs/sidereal_design_document.md`
- `docs/sidereal_implementation_checklist.md`
- `docs/component_authoring_guide.md`
- `docs/features/visibility_replication_contract.md`

Key constraints this test plan must preserve:
- character runtime state (including camera position) is persisted on character ECS entity/components,
- authority is server-side; visibility/redaction is server-side,
- component metadata (`persist`, `replicate`, `visibility`) is source-of-truth via `#[sidereal_component(...)]`,
- delivery/camera culling narrows authorization and never widens it.

Identity model updates to incorporate:
- `Account` is auth principal only.
- `Character` is the durable persisted ECS gameplay identity (character-local state + camera/control/focus/selection).
- `Session` is runtime client binding.
- "player" may remain UX wording, but new test/domain naming should prefer `Character`/`Session`.

## 3. Current Test Topology

Current `tests/` directories exist in multiple crates/bins, but inline tests still exist in production files.

Inline test modules currently in `src/*.rs`:
- `bins/sidereal-client/src/native.rs`
- `bins/sidereal-gateway/src/api.rs`
- `bins/sidereal-gateway/src/auth.rs`
- `bins/sidereal-replication/src/bootstrap.rs`
- `crates/sidereal-asset-runtime/src/lib.rs`
- `crates/sidereal-game/src/actions.rs`
- `crates/sidereal-game/src/flight.rs`
- `crates/sidereal-game/src/entities/ship/corvette.rs`
- `crates/sidereal-persistence/src/lib.rs`

## 4. Test Migration Map (Inline -> tests/)

### 4.1 `bins/sidereal-client`

Move from:
- `bins/sidereal-client/src/native.rs` (`mod tests`)

To:
- `bins/sidereal-client/tests/remote_inspect.rs`
  - `remote_endpoint_registers_when_enabled`
- `bins/sidereal-client/tests/predicted_adoption.rs`
  - `predicted_controlled_adoption_defers_until_avian_motion_available`
  - `predicted_controlled_adoption_proceeds_when_requirements_met`

### 4.2 `bins/sidereal-gateway`

Move from:
- `bins/sidereal-gateway/src/api.rs` (`mod tests`)
- `bins/sidereal-gateway/src/auth.rs` (`mod tests`)

To:
- `bins/sidereal-gateway/tests/api_assets_and_parse.rs`
  - asset path mapping and vec3 parse tests
- `bins/sidereal-gateway/tests/auth_service.rs`
  - password hash/verify, JWT encode/decode, refresh rotation, validation
  - register/login do not dispatch runtime bootstrap
  - explicit enter-world dispatch mapping
  - UDP dispatcher and replication bootstrap processor roundtrip tests

### 4.3 `bins/sidereal-replication`

Move from:
- `bins/sidereal-replication/src/bootstrap.rs` (`mod tests`)
- keep existing `bins/sidereal-replication/src/tests.rs` only if unavoidable for binary-private items, otherwise migrate too

To:
- `bins/sidereal-replication/tests/bootstrap_processor.rs`
  - idempotence and invalid mapping tests
- `bins/sidereal-replication/tests/ingest_state.rs`
  - move/extend batch ingest tests from `src/tests.rs`

### 4.4 `crates/sidereal-asset-runtime`

Move from:
- `crates/sidereal-asset-runtime/src/lib.rs` (`mod tests`)

To:
- `crates/sidereal-asset-runtime/tests/asset_dependency_expansion.rs`
  - required-asset expansion from declarative dependency maps

### 4.5 `crates/sidereal-game`

Move from:
- `crates/sidereal-game/src/actions.rs` (`mod tests`)
- `crates/sidereal-game/src/flight.rs` (`mod tests`)
- `crates/sidereal-game/src/entities/ship/corvette.rs` (`mod tests`)

To:
- `crates/sidereal-game/tests/actions_behavior.rs`
  - queue bounds and capability allowlist tests
- `crates/sidereal-game/tests/flight_behavior.rs`
  - flight intent processing, idle stabilization, angular clamp tests
- `crates/sidereal-game/tests/corvette_archetype.rs`
  - spawn overrides, deterministic spawn position, total mass

### 4.6 `crates/sidereal-persistence`

Move from:
- `crates/sidereal-persistence/src/lib.rs` (`mod tests`)

To:
- `crates/sidereal-persistence/tests/cypher_and_reflect_helpers.rs`
  - cypher literal rendering
  - AGType parse helpers
  - reflect envelope encode/decode roundtrip

## 5. Coverage Expansion Plan

### 5.1 Component/Registry/Persistence

Add/extend tests for:
- macro metadata (`persist`, `replicate`, `visibility`) correctness,
- `replicate=false` exclusion from network registration,
- persistence/hydration roundtrip for new component metadata path.

### 5.2 Visibility and Delivery

Add replication tests for:
- owner/faction/public visibility policy behavior,
- scanner-range fallback and generic entity handling,
- camera delivery narrowing in XY space only,
- authorization vs delivery separation.

### 5.3 Asset Delivery and Cache Management

Add tests for:
- manifest parsing and chunk reassembly edge cases,
- duplicate/out-of-order chunk handling,
- checksum mismatch behavior and retry/ack semantics,
- cache index load/save, stale version invalidation, corrupted index recovery.

### 5.4 Client Resilience (Non-Crash)

Add tests to ensure graceful behavior under:
- missing camera entity,
- missing controlled entity,
- missing critical assets on disk,
- malformed/partial asset stream payloads,
- bootstrap timeout/stream stall degraded-mode paths.

Assert no panic and expected fallback/dialog state transitions where applicable.

## 6. Character/Camera-Specific Test Requirements

Based on current docs and runtime model:
- character is a persisted ECS entity with runtime state components,
- camera position is persisted via player runtime state (`Transform`/`position_m` flow),
- control/selection/focus/camera state is player-entity scoped.

Required tests:
- hydration restores character runtime view state from persisted graph records,
- view updates modify the correct authenticated character entity only,
- invalid claimed player IDs are rejected server-side,
- camera persistence does not create side-table divergence.

## 7. Account/Character/Session Join-Flow Test Requirements

Add explicit tests for the proposed lifecycle:

1. Registration bootstrap:
- account creation + default character + starter corvette persisted through existing graph/template path.
- no implicit "world join" side effect.

2. Login:
- auth success does not bind replication/world session.
- character-select screen/step is required before world bind.

3. Enter World:
- explicit select/join-character request required.
- server validates account owns selected character before binding session/replication.

4. Fail-fast invariant:
- if selected character runtime entity is missing at join/bind time: reject with explicit error + log/metric.
- no replication-runtime auto-create fallback.

### 7.1 Current implementation impact notes

- Replication bootstrap validation is now ownership-based (`auth_characters` lookup) instead of enforcing `player:<account_uuid>` identity coupling.
- Bootstrap idempotency key is `player_entity_id` (not account id), which is required for multi-character accounts.
- Registration persists starter graph records directly through starter templates; runtime world bind is deferred until explicit Enter World.
- Integration tests should assert explicit rejection paths (ownership mismatch, missing hydrated player entity, invalid selected control target) without panic.

## 8. Execution Phases

1. Topology normalization:
- migrate inline tests to `tests/` folders with no behavior changes.

2. Safety net:
- add CI check blocking new inline tests in `src/*.rs` (except approved exceptions if needed).

3. Coverage expansion:
- component/registry/persistence
- visibility/delivery
- asset stream/cache
- client resilience.

4. E2E and gate pass:
- run targeted + workspace test suites and quality gates.

## 9. Acceptance Criteria

- No test modules remain inline in production `src/*.rs` for the mapped files.
- New/updated tests cover asset delivery, cache behavior, and client resilience failure modes.
- Visibility and player-camera-state tests align with current architecture contracts.
- Account/Character/Session join-flow and fail-fast invariants are covered by tests.
- Quality gates pass on changed crates; note any environment/tooling limitations explicitly.

# Legacy Cleanup Results

Date: 2026-03-05
Scope: Ordered removal sweep for legacy compatibility shims/backfills/fallbacks across IDs, actions, runtime repair paths, envelope layering, rendering fallbacks, gateway account creation, visibility bypass, and debug script bundles.

## Summary

Completed all requested phases (1 through 7) in the approved order, including test updates and quality-gate validation.

## Phase Results

### 1. Canonical IDs Only + Legacy Action Alias Removal

- Enforced bare UUID-only parsing for runtime/player entity IDs.
- Removed acceptance of prefixed ID forms (`player:...`, `ship:...`, etc.) in core parsing paths.
- Removed legacy action aliases (`ThrustForward`, `ThrustReverse`, `ThrustNeutral`, `YawLeft`, `YawRight`, `YawNeutral`) from gameplay/action routing.
- Removed legacy action aliases from Lua-generated ship `action_capabilities` payloads to keep scripting output canonical.
- Updated tests that previously depended on prefixed IDs and legacy action names.

Key files:
- `crates/sidereal-net/src/lightyear_protocol/ids.rs`
- `crates/sidereal-runtime-sync/src/lib.rs`
- `crates/sidereal-core/src/bootstrap_wire.rs`
- `crates/sidereal-game/src/actions.rs`
- `crates/sidereal-game/src/flight.rs`
- `crates/sidereal-game/src/character_movement.rs`
- `bins/sidereal-client/src/native/input.rs`
- `data/scripts/bundles/entity_registry.lua`
- `bins/sidereal-gateway/src/auth/starter_world_scripts.rs`

### 2. Remove Runtime Legacy Repair/Fallback Systems

- Removed legacy runtime “repair” systems from `SiderealGamePlugin` wiring.
- Deleted legacy ballistic range and corvette collision bootstrap repair systems.
- Removed legacy hierarchy fallback behavior; hierarchy sync now follows canonical `ParentGuid` path.
- Removed authoritative ActionState fallback path in replication input routing.

Key files:
- `crates/sidereal-game/src/lib.rs`
- `crates/sidereal-game/src/combat.rs`
- `crates/sidereal-game/src/mass.rs`
- `crates/sidereal-game/src/hierarchy.rs`
- `bins/sidereal-replication/src/replication/input.rs`

### 3. Remove Cross-Layer Legacy Envelope

- Removed legacy envelope API from persistence.
- Moved envelope definitions to core shared location.
- Updated replication/persistence test consumers to use core envelope module.

Key files:
- `crates/sidereal-persistence/src/legacy_envelope.rs` (deleted)
- `crates/sidereal-persistence/src/lib.rs`
- `crates/sidereal-core/src/net_envelope.rs` (added)
- `crates/sidereal-core/src/lib.rs`
- `bins/sidereal-replication/src/persistence_helpers.rs`

### 4. Remove Client Self-Healing/Fallback Rendering Paths

- Removed shader placeholder/self-healing cache mutation path.
- Simplified shader loading to strict streamed-cache behavior.
- Removed fullscreen fallback layer spawner path.

Key files:
- `bins/sidereal-client/src/native/shaders.rs`
- `bins/sidereal-client/src/native/mod.rs`
- `bins/sidereal-client/src/native/plugins.rs`
- `bins/sidereal-client/src/native/visuals.rs`

### 5. Gateway Atomic-Create Contract Hardening

- Made `AuthStore::create_account_atomic` mandatory.
- Removed fallback non-atomic account creation path from auth service registration flow.

Key files:
- `bins/sidereal-gateway/src/auth/store.rs`
- `bins/sidereal-gateway/src/auth/service.rs`

### 6. Visibility Bypass Confinement + Debug Script Pack Split

- Confined `SIDEREAL_VISIBILITY_BYPASS_ALL` handling to test builds.
- Removed `debug_minimal_dynamic` from production Lua bundle registry and dependent tests.

Key files:
- `bins/sidereal-replication/src/replication/visibility.rs`
- `data/scripts/bundles/bundle_registry.lua`
- `data/scripts/bundles/entity_registry.lua`
- `bins/sidereal-gateway/src/auth/starter_world_scripts.rs`

### 7. Secondary-Instance Doc/Code Drift Resolution

- Reverted implicit secondary-instance software adapter fallback; native client now keeps hardware adapter selection by default for all instances.
- `SIDEREAL_CLIENT_FORCE_SOFTWARE_ADAPTER` remains explicit opt-in for software rendering.

Key files:
- `bins/sidereal-client/src/native/platform.rs`
- `bins/sidereal-client/src/native/mod.rs`

## Test and Build Validation

Quality gates executed and passing:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace`
- `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
- `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`

Targeted tests executed and passing:

- `cargo test -p sidereal-game --tests`
- `cargo test -p sidereal-persistence --tests`
- `cargo test -p sidereal-gateway --tests`
- `cargo test -p sidereal-replication --lib`

## Notes

- This cleanup intentionally avoids introducing new features.
- Changes are focused on removing compatibility shims/backfills and enforcing canonical behavior with updated tests.

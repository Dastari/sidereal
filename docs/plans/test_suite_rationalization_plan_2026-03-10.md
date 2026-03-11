# Test Suite Rationalization Plan

Status: Proposed plan as of 2026-03-10.
Update note (2026-03-10):
- First-pass rationalization landed:
  - shared script smoke coverage now lives in `crates/sidereal-scripting/tests/`,
  - duplicated gateway/replication script smoke tests were reduced,
  - client render-layer/debug-overlay tests were merged toward behavior-oriented assertions,
  - an inline `src/*.rs` Rust test guard now exists via `scripts/check_inline_rust_tests.sh` with an explicit allowlist.

## 1. Purpose

This plan defines how to shrink and reorganize the current Rust test suite without weakening architecture-contract coverage.

Primary goals:
- keep tests that defend authority, persistence, replication, protocol, and gameplay invariants,
- remove or merge tests that duplicate the same invariant at multiple layers,
- move inline `src/*.rs` tests out of production files as a separate cleanup step after pruning decisions are made,
- add a simple guard so new inline production-file tests do not continue to accumulate by default.

This plan complements `docs/plans/test_topology_and_resilience_plan.md` and replaces its outdated topology snapshot for current planning purposes.

## 2. Current Snapshot (2026-03-10)

Workspace test inventory from source scan:
- total Rust tests: 204
- inline tests in `src/*.rs`: 122
- external tests in `tests/*.rs`: 82

Counts by workspace member:
- `bins/sidereal-replication`: 56
- `crates/sidereal-game`: 51
- `bins/sidereal-client`: 33
- `bins/sidereal-gateway`: 28
- `crates/sidereal-persistence`: 11
- `crates/sidereal-core`: 10
- `crates/sidereal-scripting`: 5
- `crates/sidereal-asset-runtime`: 4
- `crates/sidereal-net`: 3
- `crates/sidereal-shader-preview`: 3
- `crates/sidereal-component-macros`: 0
- `crates/sidereal-runtime-sync`: 0

High-density files:
- `crates/sidereal-game/tests/flight.rs`: 14
- `bins/sidereal-replication/src/tests/visibility.rs`: 11
- `bins/sidereal-gateway/tests/auth_flow.rs`: 11
- `bins/sidereal-replication/src/replication/visibility.rs`: 10
- `bins/sidereal-client/src/native/visuals.rs`: 9
- `bins/sidereal-gateway/tests/auth_helpers.rs`: 8
- `bins/sidereal-replication/src/replication/tactical.rs`: 7
- `bins/sidereal-replication/src/replication/scripting.rs`: 7
- `bins/sidereal-client/src/native/debug_overlay.rs`: 7
- `bins/sidereal-client/src/native/render_layers.rs`: 6
- `bins/sidereal-gateway/src/auth/starter_world_scripts.rs`: 6

## 3. Keep Criteria

Keep tests that protect one or more of the following:
- server-authoritative ownership and authentication flow,
- visibility authorization and delivery-scope separation,
- persistence/hydration roundtrip behavior and graph-shape invariants,
- network protocol registration and wire compatibility,
- deterministic gameplay math and motion behavior,
- generated component registry/metadata correctness,
- user-visible or protocol-visible client behavior,
- previously unstable areas where the test captures a real bug class rather than an internal implementation detail.

Default decision rule:
- if deleting the test would make an architecture contract rely on human memory, keep it,
- if deleting the test would only free an implementation to refactor internals while preserving observable behavior, merge or remove it.

## 4. Remove or Merge Criteria

Remove or merge tests that:
- assert the same invariant at both service-helper and HTTP-route layers without adding distinct failure coverage,
- duplicate script-loading smoke coverage across multiple services for the same shared content,
- lock in internal perf counters, dirty-pass counts, cache allocation counts, or world-scan counts,
- differ only by small fixture permutations that can be covered by one table-driven test,
- exist inline in production files only because that was convenient rather than because they require private-item access.

Do not remove tests solely because they are regression tests. Remove only when they are redundant, internal-detail-specific, or poorly placed.

## 5. Proposed Decisions by Workspace Area

### 5.1 Keep Mostly As-Is

These areas are protecting core contracts and should be pruned lightly, if at all:
- `bins/sidereal-replication`
- `crates/sidereal-game`
- `crates/sidereal-persistence`
- `crates/sidereal-core`
- `crates/sidereal-net`
- `crates/sidereal-asset-runtime`
- `crates/sidereal-shader-preview`

Expected action in these areas:
- keep the current behavioral coverage,
- convert dense same-shape tests to table-driven tests where it improves readability,
- migrate inline tests to `tests/` when private-item access is not required.

### 5.2 Prune Aggressively

#### `bins/sidereal-client`

Primary reduction targets:
- `src/native/render_layers.rs`
  - replace exact perf-counter assertions with 2-3 behavioral tests covering registry rebuild, rule removal, and assignment recomputation,
  - stop asserting internal scan counts or targeted-pass counts.
- `src/native/visuals.rs`
  - keep visible-behavior tests,
  - merge or remove tests that only lock in cache reuse or allocation policy unless they correspond to a known regression class.
- `src/native/debug_overlay.rs`
  - collapse lane-resolution preference tests into a table-driven suite rather than many nearly identical functions.

#### `bins/sidereal-gateway`

Primary reduction targets:
- `tests/auth_flow.rs` and `tests/auth_helpers.rs`
  - keep one canonical coverage location for "register/login does not dispatch bootstrap",
  - keep route-level happy-path and authz tests,
  - keep helper-level crypto/token/UDP tests,
  - remove route/helper duplication where the same invariant is asserted twice.
- `src/auth/starter_world_scripts.rs`
  - keep gateway-specific starter-bundle policy checks,
  - remove generic script-load smoke coverage duplicated in replication scripting tests and shared scripting tests.

#### Shared script smoke coverage

Canonical home:
- `crates/sidereal-scripting`

Service copies that should be reduced:
- `bins/sidereal-gateway/src/auth/starter_world_scripts.rs`
- `bins/sidereal-replication/src/replication/scripting.rs`

Rule:
- generic content-load success belongs in the shared scripting crate,
- service tests should remain only when they verify service-specific mapping, persistence, ownership, or override behavior.

## 6. Candidate Reduction Set

First-pass remove-or-merge candidates:
- duplicated gateway bootstrap side-effect tests,
- duplicated world-init and bundle-registry smoke tests across gateway and replication,
- client render-layer perf-counter tests,
- client visuals allocation/cache policy tests,
- repeated client debug-overlay lane-preference tests that can be table-driven.

Expected reduction target:
- remove or merge roughly 15-25 tests,
- with almost all reduction concentrated in `bins/sidereal-client` and `bins/sidereal-gateway`.

Non-goals:
- broad deletion in replication, gameplay, persistence, or protocol crates,
- removing end-to-end tests that cover cross-service contracts,
- reducing coverage for visibility, ownership binding, or persistence roundtrips.

## 7. Execution Phases

1. Classification pass
- tag every current test file as `keep`, `merge`, `remove`, or `move`.
- record duplicate invariants before deleting anything.

2. Duplicate removal pass
- remove gateway route/helper duplication,
- centralize generic script smoke coverage in `crates/sidereal-scripting`,
- collapse client render-layer and visuals implementation-detail tests.

3. Topology cleanup pass
- move surviving inline tests from `src/*.rs` into `tests/*.rs` where possible,
- keep inline tests only for genuinely private binary-internal helpers that cannot be reasonably exposed.

4. Guardrail pass
- add a lightweight CI or lint check that fails on new inline `#[test]` modules in production `src/*.rs`, with an explicit allowlist if needed.

5. Verification pass
- run targeted crate tests for touched areas,
- run workspace quality gates,
- update plan/docs snapshots if counts or file ownership changed materially during cleanup.

## 8. Validation

Minimum validation after the cleanup:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

If client code is touched, also run:

```bash
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

Run targeted test groups for:
- gateway auth,
- replication visibility/tactical,
- client render-layer and visuals behavior,
- scripting content-load and bundle-spawn behavior.

## 9. Acceptance Criteria

- redundant gateway auth tests are reduced to one canonical coverage point per invariant,
- generic script-load smoke tests exist in one shared canonical location instead of multiple service copies,
- client implementation-detail tests are replaced by behavior-focused coverage,
- surviving tests in core gameplay/replication/persistence areas are preserved,
- the number of inline production-file tests is materially reduced, with a follow-up path to reach near-zero inline tests,
- docs accurately describe the current topology and prune policy.

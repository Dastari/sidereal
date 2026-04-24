# Rust Codebase Audit Report - 2026-04-23

Status note 2026-04-23: Fresh audit run from current repository state. Existing audit reports were intentionally ignored as evidence. This pass used `docs/prompts/rust_codebase_audit_prompt.md`, `AGENTS.md`, the architecture/feature contracts, and the local `rust-skills` skill as the review lens.

## Scope

Reviewed the Rust workspace structure, shared gameplay crates, Lightyear protocol registration, replication control/input flow, client runtime presentation/motion paths, asset/bootstrap code, and representative tests. The audit focused on correctness, maintainability, authority boundaries, panic/error handling, runtime ownership, and test gaps.

## Executive Summary

The workspace has strong foundations: shared Avian motion components are registered explicitly, generated component registration is deterministic, control/input messages use UUID-like wire identities, and several high-risk data streams already have focused tests. The largest Rust risk is not idiomatic language usage; it is lifecycle complexity around dynamic controlled-entity handoff.

Important context: this is not an untouched area. The March 22 handoff work already added explicit `ControlBootstrapState` / `ControlBootstrapPhase`, pending-until-predicted client binding, server-issued `control_generation` in control acks/rejects, narrowed visibility rearm, transform repair narrowing, and Lightyear fork work for late Avian lane bootstrap. The remaining risk is that the implementation still carries containment paths while the final lifecycle target is being validated.

## Findings

### Critical - Controlled-Entity Handoff Is Partially Refactored But Still Carries Repair Paths

Evidence:

- Server role reconciliation mutates `ControlledBy`, `Replicate`, `PredictionTarget`, and `InterpolationTarget` in one large system, then forces visible clients through `lose_visibility`/`gain_visibility` on role changes (`bins/sidereal-replication/src/replication/control.rs:607`, `bins/sidereal-replication/src/replication/control.rs:621`).
- Client-side replication still has defensive prediction bootstrap and confirmed-history seeding for interpolated entities (`bins/sidereal-client/src/runtime/replication.rs:564`, `bins/sidereal-client/src/runtime/replication.rs:600`).
- Client role cleanup removes conflicting `Predicted`/`Interpolated` markers after they coexist (`bins/sidereal-client/src/runtime/replication.rs:1390`).
- Motion waits for a predicted clone instead of promoting confirmed/interpolated clones into the writer lane (`bins/sidereal-client/src/runtime/motion.rs:201`).

Impact:

This is the highest-risk path for native stabilization. Recent work moved the design in the right direction: explicit bootstrap state and control generations are present, and control no longer falls back freely to confirmed/interpolated non-anchor roots. The remaining concern is that correctness still depends on several systems converging in the right order. Under packet loss, relevance churn, or rapid A/B/A control switching, the system can still show delayed adoption, stale presentation, duplicate clones, or temporary loss of prediction if the containment paths are exercised.

Recommendation:

Continue converging on the already-started state-machine direction rather than replacing it with another architecture. The target invariants should remain: requested, server accepted with generation, role topology changed, predicted clone ready, local writer bound, old writer revoked. Add integration tests for rapid target swaps, self-control/free-roam transitions, disconnect/reconnect during control, and visibility loss/regain during handoff.

### High - Debug String Equality Is Used For Replication Target Reconciliation

Evidence:

- `maybe_set_prediction_target` and `maybe_set_interpolation_target` compare current and desired Lightyear targets by formatting `Debug` strings (`bins/sidereal-replication/src/replication/control.rs:532`, `bins/sidereal-replication/src/replication/control.rs:553`).

Impact:

This runs in the fixed replication reconciliation path and depends on non-contractual formatting. It is brittle across Lightyear upgrades, allocates strings in a hot path, and can cause unnecessary role rewrites and rearming.

Recommendation:

Replace `Debug` comparisons with a semantic target descriptor owned by Sidereal, or wrap Lightyear target construction so the desired target is cached and compared structurally before component mutation.

### High - Client Runtime Modules Are Too Large For Their Blast Radius

Evidence:

- `bins/sidereal-client/src/runtime/visuals.rs` is 3,349 lines.
- `bins/sidereal-client/src/runtime/ui.rs` is 2,608 lines.
- `bins/sidereal-client/src/runtime/replication.rs` is 1,718 lines.

Impact:

These files mix asset resolution, prediction/interpolation repair, world visuals, weapons, tactical overlays, nameplates, material updates, and tests. This increases review cost and makes it easy for rendering, networking, and simulation ownership fixes to regress each other.

Recommendation:

Split by domain and schedule ownership: prediction presentation, streamed visual attachment, projectile/tracer effects, tactical map UI, nameplates, fullscreen/backdrop, and replication adoption. Keep plugin wiring thin.

### Medium - Production Panics Are Mostly Acceptable But Should Be Narrowed

Evidence:

- Startup/config panics exist for hard-coded literals, validated tokens, embedded fonts, and critical worker startup (`bins/sidereal-replication/src/config.rs:289`, `bins/sidereal-replication/src/replication/persistence.rs:123`, `bins/sidereal-client/src/runtime/scene.rs:46`).
- Many `expect`/`unwrap` hits are in tests, but some runtime paths still assume initialized resources or validated config.

Impact:

Most panics are defensible startup failures. The audit did not find broad unchecked user-data parsing in hot runtime paths. The remaining risk is that resource initialization assumptions are hard to distinguish from recoverable runtime failures.

Recommendation:

For runtime systems after app startup, prefer graceful skip/error resources/dialogs over panics. Keep `expect` for impossible startup invariants and test code only.

### Medium - Input Handoff Accepts Stale Controlled IDs For Authoritative Targets

Evidence:

- Server input receive validates player binding and tick/rate limits (`bins/sidereal-replication/src/replication/input.rs:258`).
- Drain accepts realtime input for the authoritative target even when the message controlled ID is stale or mismatched (`bins/sidereal-replication/src/replication/input.rs:507`).

Impact:

This is a pragmatic handoff tolerance, but it weakens the input contract. If the authoritative map changes faster than client confirmation, old input may apply to the new target.

Recommendation:

Include control generation in realtime input messages or gate mismatch acceptance behind the last acknowledged generation. Tests should cover request N/N+1 races.

### Medium - Generic Entity Direction Is Not Fully Reflected In Naming

Evidence:

- Network/tactical/manifest code still falls back to `"ship"` labels/kinds (`bins/sidereal-replication/src/replication/tactical.rs:93`, `bins/sidereal-replication/src/replication/owner_manifest.rs:38`).
- Client tactical defaults also assume `"ship"` for controlled marker fallback (`bins/sidereal-client/src/runtime/ui.rs:941`).

Impact:

This does not break current gameplay, but it conflicts with the project rule to keep generic runtime systems entity-generic. It can leak ship-only defaults into later non-ship controlled entities.

Recommendation:

Move kind/icon derivation to authored metadata and make the generic fallback `"entity"`.

## Strengths

- Workspace component registration sorts generated component metadata before Lightyear registration, improving client/server determinism (`crates/sidereal-net/src/lightyear_protocol/registration.rs:191`).
- Avian motion components are registered directly with prediction/interpolation and rollback thresholds (`crates/sidereal-net/src/lightyear_protocol/registration.rs:154`).
- Shared gameplay systems use fixed time for simulation math (`crates/sidereal-game/src/flight.rs:162`, `crates/sidereal-game/src/character_movement.rs:63`).
- Dynamic mass recomputation keeps Avian `Mass` and `AngularInertia` aligned with gameplay mass (`crates/sidereal-game/src/mass.rs:64`).

## Priority Actions

1. Replace `Debug` string target comparisons in replication role reconciliation.
2. Add controlled-entity generation to realtime input or otherwise prove stale-ID acceptance is safe.
3. Finish validating the existing explicit handoff state machine with integration coverage.
4. Split the largest client runtime files along domain/schedule boundaries.
5. Remove ship-specific generic fallbacks from tactical/manifest/client UI paths.

## Validation

No cargo quality gates were run for this docs-only audit generation. The report is based on static inspection of current files and current prompts/skills.

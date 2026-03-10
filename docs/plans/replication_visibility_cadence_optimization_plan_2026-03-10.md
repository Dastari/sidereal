# Replication Visibility Cadence Optimization Plan

Status: Proposed implementation plan  
Date: 2026-03-10  
Owners: replication runtime + gameplay runtime + diagnostics

Primary references:
- `docs/features/visibility_replication_contract.md`
- `docs/plans/discovered_static_landmark_visibility_plan_2026-03-09.md`
- `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-10.md`
- `AGENTS.md`

## 1. Objective

Reduce server-side visibility tick cost and delivery burstiness without flattening Sidereal's current multi-level visibility model.

This plan explicitly preserves:

1. `Authorization -> Delivery -> Payload` ordering.
2. owner/public/faction/scanner/discovered-landmark semantics.
3. dynamic per-client delivery range and local-view mode behavior.
4. static discovered-landmark persistence and post-discovery authorization rules.
5. payload redaction/disclosure policy boundaries.

The target is implementation efficiency and cadence stability, not a visibility-policy rewrite.

## 2. Current Problem

The March 10 audit identified `bins/sidereal-replication/src/replication/visibility.rs` as a remaining high-impact indirect render-smoothness risk.

The current hot path still does too much whole-tick rebuild work:

1. scratch maps and lookup tables are cleared and rebuilt each tick,
2. replicated entities are rescanned broadly even when only a small subset moved or changed relevance inputs,
3. discovered static landmark checks still run in the same hot pass as dynamic delivery maintenance,
4. per-client visibility decisions are recalculated too monolithically,
5. replication membership mutation is driven from repeated full passes instead of stable cached memberships plus diffs.

This is acceptable for correctness, but it is not a good steady-state cadence shape.

## 3. Goals

Primary goals:

1. lower `update_network_visibility()` tick duration,
2. reduce tick-to-tick variance and burstiness,
3. preserve current visibility behavior,
4. make static/discovery maintenance cheaper than dynamic/moving-entity delivery,
5. surface enough telemetry to prove the refactor helped.

Success criteria:

1. dynamic visibility work no longer rebuilds all scratch state from zero every tick,
2. discovered static landmark maintenance is no longer treated like the same cadence class as dynamic delivery,
3. per-client visibility membership changes are diff-driven in steady state,
4. visibility timing and candidate-count telemetry show lower average and lower worst-case cost under the same load.

## 4. Non-Negotiable Rules

The following are mandatory throughout this plan:

1. Delivery must never widen authorization.
2. Spatial candidate generation remains an optimization input only.
3. Static landmark discovery remains server-authoritative and player-scoped.
4. Player runtime visibility/disclosure state remains on the player entity lane and server-owned resources, not client-owned side state.
5. Public/faction/owner/discovered-landmark behavior must remain separately distinguishable in code and diagnostics.
6. Visibility optimization must not create a hidden "ship-only" baseline path; generic entity visibility remains the model.

## 5. Optimization Strategy

Use four complementary moves:

1. persist hot-path scratch/cached state across ticks,
2. split cadence classes so static/discovery work is not paid every hot tick in the same way as dynamic delivery,
3. make entity/client invalidation more change-driven,
4. mutate replication visibility memberships from cached state plus diffs instead of repeated whole-world recomputation.

## 6. Planned Runtime Shape

### 6.1 High-frequency dynamic delivery lane

Runs every replication visibility tick.

Responsible for:

1. moving/dynamic entities,
2. currently visible nearby entities,
3. controlled/owner-critical entities,
4. fast local-bubble correctness.

This lane should consume precomputed or incrementally maintained inputs where possible:

1. effective world position,
2. effective visibility extent,
3. per-client observer anchor state,
4. per-client delivery range,
5. cached policy classification.

### 6.2 Lower-frequency static/discovery lane

Runs on a lower cadence or only when invalidated.

Responsible for:

1. static landmark discovery checks,
2. post-discovery landmark delivery maintenance,
3. landmark-specific layer/parallax delivery adjustments,
4. persistent discovery membership updates.

This lane should not force the dynamic lane to rederive the full static-landmark world set every tick.

### 6.3 Stable membership diff lane

Maintains:

1. current visible set per client,
2. current discovered-static-landmark delivered set per client where applicable,
3. enter/exit diffs,
4. telemetry for gains/losses and candidate counts.

This lane should be the only place that mutates `ReplicationState` visibility membership in steady state.

## 7. Proposed Data/Caching Additions

### 7.1 Persistent per-entity visibility inputs

Add a persistent cache resource holding the hot-path derived inputs per replicated entity.

Suggested fields:

1. entity world position,
2. entity visibility extent,
3. static/dynamic cadence class,
4. authorization policy class:
   - owner-bound,
   - public,
   - faction,
   - range-based,
   - discovered-static-landmark-capable,
5. landmark metadata needed for discovery/post-discovery checks,
6. dirty flags / generation for input refresh.

The goal is to avoid repeatedly re-deriving these values from many ECS queries each tick.

### 7.2 Persistent per-client visibility context

Maintain a per-client cache for:

1. observer anchor position,
2. delivery range,
3. visibility sources,
4. view mode,
5. player entity,
6. faction/ownership context,
7. last computed candidate counts and timing.

Refresh only when relevant source components or client view mode inputs change.

### 7.3 Persistent per-client membership state

Maintain stable sets/maps for:

1. currently visible dynamic entities,
2. currently delivered static landmarks,
3. currently forced-owner entities,
4. last sent visibility-disclosure products where applicable.

This allows:

1. enter/exit diffs,
2. amortized maintenance,
3. lower mutation pressure on `ReplicationState`.

### 7.4 Persistent spatial index reuse

Do not reconstruct broad candidate structures from scratch if the same persistent index can be updated incrementally.

Keep:

1. spatial index state alive,
2. moved-entity updates incremental,
3. static-entity registration stable,
4. low-frequency rebuild only when large invalidation occurs.

## 8. Policy/Cadence Classification

Each relevant replicated entity should fall into one or more runtime classes:

1. `OwnerForced`
2. `DynamicRangeRelevant`
3. `StaticDiscoverableLandmark`
4. `AlwaysAuthorizedConfig`
5. `FactionOrPublicPolicyOnly`

These are not user-facing semantics changes. They are execution classes used to decide:

1. how often to refresh inputs,
2. whether spatial candidate checks are needed,
3. whether landmark/discovery logic applies,
4. whether membership should be sticky or purely dynamic.

Examples:

1. environment-lighting config: `AlwaysAuthorizedConfig`
2. nearby moving ship: `DynamicRangeRelevant`
3. discovered planet: `StaticDiscoverableLandmark`
4. controlled owned root: `OwnerForced`

## 9. Phase Plan

### Phase 0: Instrumentation and Baseline

Goal:
Prove where visibility time is actually spent before changing behavior.

Work:

1. Add timers for:
   - context refresh,
   - spatial candidate generation,
   - landmark discovery maintenance,
   - authorization checks,
   - delivery checks,
   - replication membership mutation.
2. Add counters for:
   - live clients,
   - replicated entities considered,
   - candidates per client,
   - authorization passes per client,
   - delivery passes per client,
   - visible-set gains/losses,
   - discovered-landmark checks,
   - static landmark delivered count.
3. Expose this through log output and a BRP-readable resource first.

Files expected:

1. `bins/sidereal-replication/src/replication/visibility.rs`
2. replication diagnostics/resource files if split out

Acceptance:

1. there is a before/after baseline for each later phase,
2. telemetry distinguishes dynamic vs static/discovery work.

### Phase 1: Persist Scratch State Across Ticks

Goal:
Stop clearing and rebuilding all hot visibility scratch maps every tick.

Work:

1. Introduce a persistent visibility runtime cache resource.
2. Move reusable per-entity maps out of the tick-local rebuild path.
3. Move reusable per-client maps/context out of the tick-local rebuild path.
4. Refresh entries only when source data changed or when a client/entity entered/exited the tracked set.

Acceptance:

1. steady-state ticks reuse prior caches,
2. only dirty entity/client inputs are recomputed.

### Phase 2: Split Static Landmark Discovery From Dynamic Delivery

Goal:
Remove static discovery/post-discovery maintenance from the hottest per-tick path.

Work:

1. Introduce a lower-frequency or invalidation-driven static landmark pass.
2. Precompute the set of landmark-capable entities and keep it stable.
3. Maintain player discovery checks separately from dynamic local-bubble delivery checks.
4. Keep post-discovery delivery still range-narrowed, but do not recompute the full landmark-discovery world set every tick.

Acceptance:

1. dynamic visibility tick no longer does a whole-world static discovery scan every tick,
2. discovered-landmark semantics remain unchanged.

### Phase 3: Cached Per-Entity Policy and Input Classification

Goal:
Avoid repeatedly deriving the same policy and geometry inputs from ECS each tick.

Work:

1. Cache:
   - effective visibility position,
   - effective extent,
   - policy class,
   - landmark metadata,
   - ownership/faction/public classification inputs.
2. Mark entries dirty from component changes/removals rather than global rescans.
3. Recompute only the invalidated entity inputs.

Acceptance:

1. steady-state entity input derivation cost is near zero,
2. hot path consumes compact cached data.

### Phase 4: Stable Per-Client Membership Diffs

Goal:
Turn visibility membership maintenance into diff application instead of whole-set recomputation.

Work:

1. Maintain current visible memberships per client.
2. Compute enters/exits from new candidate results against cached membership.
3. Apply only membership diffs to `ReplicationState`.
4. Keep full resync capability for invalidation/reconnect/debug paths.

Acceptance:

1. `ReplicationState` mutation cost scales with changes, not full world size,
2. steady-state frames with low movement produce low membership mutation cost.

### Phase 5: Cadence Budgeting By Class

Goal:
Preserve correctness for high-value dynamic entities while reducing unnecessary work for slower/static classes.

Work:

1. Keep owner-forced and near-dynamic entities on every visibility tick.
2. Refresh static or low-priority classes on a lower cadence or amortized slice.
3. Refresh always-authorized config entities only on change.
4. Preserve immediate refresh triggers for:
   - control handoff,
   - ownership/faction changes,
   - landmark discovery updates,
   - view mode changes,
   - delivery range changes.

Acceptance:

1. no gameplay-critical near-dynamic regression,
2. static/config classes stop paying hot-tick rates.

## 10. Suggested File/Module Refactor

The current `visibility.rs` is doing too much in one place. Split along runtime concerns:

1. `visibility/cache.rs`
   - persistent cache/resources,
   - invalidation helpers
2. `visibility/context.rs`
   - per-client context derivation
3. `visibility/dynamic.rs`
   - hot dynamic candidate/auth/delivery checks
4. `visibility/landmarks.rs`
   - discovered static landmark cadence and maintenance
5. `visibility/membership.rs`
   - diff application into `ReplicationState`
6. `visibility/diagnostics.rs`
   - telemetry structs/log formatting

This should be done incrementally, not as a single rewrite.

## 11. Telemetry To Keep Long-Term

Keep these after the refactor:

1. visibility tick total ms,
2. dynamic lane ms,
3. static/discovery lane ms,
4. average and max candidates per client,
5. visible enter/exit counts per tick,
6. static landmark delivered counts,
7. cache hit/miss/dirty refresh counts,
8. forced full-resync count.

## 12. Tests

Required coverage:

1. authorization behavior is unchanged for:
   - owner,
   - public,
   - faction,
   - range-based,
   - discovered static landmarks
2. delivery narrowing still does not widen authorization,
3. discovered landmarks remain delivered correctly after discovery,
4. static discovery cadence splitting does not lose newly discovered landmarks,
5. membership diff application matches the previous effective visible-set result,
6. client view-mode changes still update delivery behavior correctly,
7. forced-owner/config entities still bypass the normal spatial narrowing when required by contract.

Performance-oriented tests where practical:

1. stable-tick cache refresh counts remain near zero,
2. no-whole-world-static-scan on steady-state dynamic-only ticks,
3. membership mutation count scales with actual changes.

## 13. Rollout Order

Recommended order:

1. instrumentation,
2. persistent caches,
3. static/discovery split,
4. cached policy/input derivation,
5. membership diff application,
6. cadence budgeting.

Do not reorder this into a single rewrite. The instrumentation and cache split should land first so later behavior can be compared safely.

## 14. Explicit Non-Goals

This plan does not:

1. simplify away the multi-level visibility model,
2. remove discovered-static-landmark behavior,
3. weaken payload redaction or authorization policy,
4. move authority to the client,
5. introduce ship-specific visibility shortcuts,
6. solve render smoothness only through client interpolation while leaving server cadence bursty.

## 15. Acceptance Checklist

This plan is complete when:

1. visibility tick telemetry proves lower average and lower worst-case cost,
2. dynamic delivery and static discovery no longer share one monolithic hot path,
3. steady-state ticks reuse caches instead of rebuilding all scratch state,
4. membership mutation is diff-driven,
5. all existing visibility policy tests still pass,
6. no documented visibility contract behavior regressed.

# Spatial Partitioning Implementation Plan

Status: Implementation plan  
Date: 2026-03-04  
Scope: Activate spatial grid candidate mode as default, make cell size configurable, prepare for galaxy-scale coordinates

## 1. Problem Statement

The replication server currently evaluates every replicated entity against every connected client every fixed tick (30 Hz) to determine visibility. With `full_scan` as the default candidate mode, this is O(clients x entities) per tick.

A player floating in empty deep space between solar systems should receive zero entity replication updates (other than their own owned entities). Today, the server still iterates all entities for that player and runs full authorization + delivery checks on each, only to reject them all. As entity counts grow (40 solar systems x 50-200 entities each = 2,000-8,000 entities, plus players and ships), this becomes the dominant per-tick cost.

The spatial grid candidate mode already exists but is not the default, is hardcoded to 300m cells, and uses `i32` cell keys that will overflow with f64 galaxy coordinates. This plan activates, hardens, and extends it.

## 2. Architecture Principle

Spatial partitioning is a **candidate preselection optimization**. It narrows the set of entities that need full visibility evaluation for each client. It does NOT replace or weaken the authorization-first security model.

The pipeline order remains:

```
1. Candidate preselection   (spatial grid — what MIGHT be visible)
2. Authorization             (owner/faction/public/scanner — what IS ALLOWED to be visible)
3. Delivery scope            (distance from observer anchor — what IS CLOSE ENOUGH to deliver)
4. Payload scope             (component/field redaction — what PARTS to send)
```

Candidate preselection can only narrow, never widen. If the grid misses an entity, the authorization bypass catches it (owned/public/faction/scanner entities bypass candidate filtering). The final `gain_visibility` / `lose_visibility` call is always the product of full authorization + delivery evaluation.

## 3. Current State

### What exists

File: `bins/sidereal-replication/src/replication/visibility.rs`

- `VisibilityCandidateMode` enum: `FullScan` (default) or `SpatialGrid`
- `VISIBILITY_CELL_SIZE_M = 300.0` (hardcoded constant)
- `cell_key(position: Vec3) -> (i32, i32)` — integer grid coordinates from f32 position
- `add_entities_in_radius(center, radius_m, entities_by_cell, out)` — walks cells in a square around center
- `build_candidate_set_for_client(...)` — in SpatialGrid mode: owned entities + entities near observer anchor (DEFAULT_VIEW_RANGE_M) + entities near each scanner source
- `should_bypass_candidate_filter(...)` — ensures owned, public, faction-matched, and scanner-range entities are never missed by the grid
- `VisibilityScratch` — per-tick scratch buffer rebuilt from scratch every tick
- `entities_by_cell: HashMap<(i32, i32), Vec<Entity>>` — the spatial grid itself (only occupied cells stored)

### What works

- The grid is correct. Occupied cells only, so empty galaxy space costs nothing.
- The bypass filter is correct. Owned/public/faction/scanner entities can never be incorrectly culled.
- Telemetry exists. `SIDEREAL_REPLICATION_SUMMARY_LOGS=1` reports candidate counts.

### What needs to change

1. **Default mode** is `full_scan` — should be `spatial_grid`
2. **Cell size** is hardcoded at 300m — too small for galaxy-scale scanner ranges (5-10km = 289-1,156 cells per query)
3. **Cell key type** is `(i32, i32)` — overflows at ±643 km from origin with 300m cells; will overflow immediately with f64 coordinates
4. **Delivery range** defaults to 300m — needs to be independently configurable and validated against cell size
5. **No solar-system preselection** — 40 distance checks per player per second would skip entire systems
6. **Position type** uses `Vec3` (f32) — will need `DVec2`/`DVec3` when f64 lands, but this is gated behind the f64 migration (separate work)
7. **No mounted-child grid coherence** — mounted children are individually gridded by their own world position, which is correct for visibility but means a parent and its children can be in different cells (expected behavior, not a bug)

## 4. Implementation Steps

### Step 1: Make cell size configurable

Replace the hardcoded constant with a runtime-configurable value.

```rust
fn cell_size_from_env() -> f32 {
    std::env::var("SIDEREAL_VISIBILITY_CELL_SIZE_M")
        .ok()
        .and_then(|raw| raw.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value >= 50.0)
        .unwrap_or(2000.0)
}
```

Add `cell_size_m: f32` to `VisibilityRuntimeConfig`. Remove the `VISIBILITY_CELL_SIZE_M` constant.

Update `cell_key` to take cell size as a parameter:

```rust
fn cell_key(position: Vec3, cell_size: f32) -> (i64, i64) {
    (
        (position.x / cell_size).floor() as i64,
        (position.y / cell_size).floor() as i64,
    )
}
```

Note: cell key type changes from `(i32, i32)` to `(i64, i64)` in the same step. At 2km cells, `i64` supports ±9.2 x 10^15 km from origin — effectively unlimited. This also prepares for the f64 coordinate migration.

Update `add_entities_in_radius` to accept `cell_size`:

```rust
fn add_entities_in_radius(
    center: Vec3,
    radius_m: f32,
    cell_size: f32,
    entities_by_cell: &HashMap<(i64, i64), Vec<Entity>>,
    out: &mut HashSet<Entity>,
) {
    let radius = radius_m.max(0.0);
    let cell_radius = (radius / cell_size).ceil() as i64;
    let (cx, cy) = cell_key(center, cell_size);
    for dx in -cell_radius..=cell_radius {
        for dy in -cell_radius..=cell_radius {
            if let Some(entities) = entities_by_cell.get(&(cx + dx, cy + dy)) {
                out.extend(entities.iter().copied());
            }
        }
    }
}
```

Update `VisibilityScratch::entities_by_cell` type to `HashMap<(i64, i64), Vec<Entity>>`.

Update all call sites to pass `runtime_cfg.cell_size_m`.

**Default: 2,000m.** With a 5km scanner range, this means 3x3 = 9 cells per query. With a 10km range, 5x5 = 25 cells. Both are negligible.

**Files changed**: `visibility.rs`

### Step 2: Switch default candidate mode to spatial_grid

Change `VisibilityCandidateMode::from_env()` default:

```rust
fn from_env() -> Self {
    match std::env::var("SIDEREAL_VISIBILITY_CANDIDATE_MODE")
        .ok()
        .unwrap_or_else(|| "spatial_grid".to_string())
        // ...
    {
        "full" | "full_scan" => Self::FullScan,
        _ => Self::SpatialGrid,
    }
}
```

**Rationale**: `spatial_grid` is already implemented and tested. The bypass filter ensures no policy exceptions are missed. `full_scan` remains available via env var for debugging/validation.

**Files changed**: `visibility.rs`

### Step 3: Validate delivery_range_m against cell_size_m

Add a startup validation log and runtime coherence check:

```rust
pub fn init_resources(app: &mut App) {
    let cell_size = cell_size_from_env();
    let delivery_range = delivery_range_m_from_env();
    if delivery_range > cell_size * 4.0 {
        warn!(
            "delivery_range_m ({:.0}) is large relative to cell_size_m ({:.0}); \
             grid queries will iterate {} cells per axis per query. \
             Consider increasing SIDEREAL_VISIBILITY_CELL_SIZE_M.",
            delivery_range,
            cell_size,
            ((delivery_range / cell_size).ceil() as i64) * 2 + 1
        );
    }
    // ...
}
```

This is advisory, not blocking. Misconfigured ratios degrade performance, not correctness.

**Files changed**: `visibility.rs`

### Step 4: Add solar-system-aware preselection

This is an **optional prefilter** layered before the grid query. It skips entities whose `SolarSystemId` references a system that is provably out of range.

#### New components required

`SolarSystemId` and `SolarSystemRadius` should already be defined (from the galaxy world structure plan). If not yet implemented, this step is deferred until those components exist.

#### Preselection logic

Add to `VisibilityScratch`:

```rust
solar_system_positions: HashMap<Uuid, Vec3>,
solar_system_radii: HashMap<Uuid, f32>,
entity_solar_system: HashMap<Entity, Uuid>,
reachable_systems_by_client: HashMap<Entity, HashSet<Uuid>>,
```

During scratch construction:

```rust
// Query all solar system entities for their position and radius
for (guid, position, radius) in &solar_system_query {
    scratch.solar_system_positions.insert(guid.0, position.0.extend(0.0));
    scratch.solar_system_radii.insert(guid.0, radius.0 as f32);
}

// For each replicated entity with a SolarSystemId, record its system
for (entity, solar_system_id) in &entities_with_system {
    if let Some(system_id) = solar_system_id {
        scratch.entity_solar_system.insert(entity, system_id.0);
    }
}
```

During per-client candidate construction, compute reachable systems:

```rust
fn compute_reachable_systems(
    observer_anchor: Option<Vec3>,
    scanner_sources: &[(Vec3, f32)],
    solar_system_positions: &HashMap<Uuid, Vec3>,
    solar_system_radii: &HashMap<Uuid, f32>,
) -> HashSet<Uuid> {
    let mut reachable = HashSet::new();
    let max_reach = scanner_sources
        .iter()
        .map(|(_, range)| *range)
        .fold(0.0_f32, f32::max)
        .max(DEFAULT_VIEW_RANGE_M);

    for (system_id, &system_center) in solar_system_positions {
        let system_radius = solar_system_radii
            .get(system_id)
            .copied()
            .unwrap_or(0.0);

        let close_enough = observer_anchor.is_some_and(|anchor| {
            (system_center - anchor).length() <= system_radius + max_reach
        }) || scanner_sources.iter().any(|(pos, range)| {
            (system_center - *pos).length() <= system_radius + range
        });

        if close_enough {
            reachable.insert(*system_id);
        }
    }
    reachable
}
```

During candidate filtering, skip entities in unreachable systems:

```rust
// In the SpatialGrid candidate builder, after grid query:
// Filter out entities in unreachable solar systems
if let Some(reachable) = scratch.reachable_systems_by_client.get(client_entity) {
    candidates.retain(|entity| {
        match scratch.entity_solar_system.get(entity) {
            Some(system_id) => reachable.contains(system_id),
            None => true, // entities without a system are always candidates
        }
    });
}
```

**Important**: This filter is applied AFTER the grid query and BEFORE authorization. Entities without a `SolarSystemId` (deep-space entities, player entities, etc.) always pass. Owned/public/faction entities still bypass via `should_bypass_candidate_filter`.

With 40 solar systems, this is 40 distance checks per client per tick. At 30 Hz with 10 clients, that is 12,000 distance checks/sec — negligible.

**Files changed**: `visibility.rs`

**Dependency**: `SolarSystemId` and `SolarSystemRadius` components must exist as defined in `galaxy_world_structure.md`. If these components are not yet implemented, skip this step and revisit after the galaxy structure work.

### Step 5: Add telemetry for spatial grid performance

Extend the existing telemetry logging to include grid-specific metrics:

```rust
if summary_logging_enabled() {
    // existing metrics...
    let occupied_cells = scratch.entities_by_cell.len();
    let max_entities_per_cell = scratch
        .entities_by_cell
        .values()
        .map(Vec::len)
        .max()
        .unwrap_or(0);
    let systems_checked = scratch.solar_system_positions.len();
    let avg_reachable_systems = if clients_count > 0 {
        scratch
            .reachable_systems_by_client
            .values()
            .map(HashSet::len)
            .sum::<usize>() as f64
            / clients_count as f64
    } else {
        0.0
    };
    info!(
        "spatial grid stats: cell_size_m={:.0} occupied_cells={} max_per_cell={} \
         systems_checked={} avg_reachable_systems={:.1}",
        runtime_cfg.cell_size_m,
        occupied_cells,
        max_entities_per_cell,
        systems_checked,
        avg_reachable_systems
    );
}
```

**Files changed**: `visibility.rs`

### Step 6: Update visibility_replication_contract.md

Document the new defaults and configuration:

- Default candidate mode is now `spatial_grid`
- Cell size is configurable via `SIDEREAL_VISIBILITY_CELL_SIZE_M` (default 2,000m)
- Cell key type is `(i64, i64)`, supporting galaxy-scale coordinates
- Solar-system preselection is active when `SolarSystemId` / `SolarSystemRadius` components are present
- `full_scan` remains available for debugging via `SIDEREAL_VISIBILITY_CANDIDATE_MODE=full_scan`

**Files changed**: `docs/features/visibility_replication_contract.md`

## 5. What Does NOT Change

The following are explicitly out of scope for this plan:

- **Authorization logic** — owner/faction/public/scanner policies are unchanged.
- **Delivery scope** — distance-from-observer check is unchanged.
- **Payload scope / component redaction** — unchanged.
- **`gain_visibility` / `lose_visibility` calls** — the final visibility decision mechanism is unchanged.
- **Mount-root resolution** — unchanged. Children inherit owner/faction/public from their mount root.
- **Observer anchor tracking** — unchanged. Player entity position remains the delivery center.
- **Scanner range computation** — unchanged. `compute_controlled_entity_scanner_ranges` aggregates module contributions.
- **Candidate bypass** — `should_bypass_candidate_filter` is unchanged. Owned, public, faction-matched, and scanner-range entities always bypass candidate filtering.
- **Tick frequency** — visibility still runs every FixedUpdate tick (30 Hz). No throttling or skip-frame logic.
- **f64 migration** — cell key type moves to `i64` now (trivial, forward-compatible), but position types remain `Vec3`/f32 until the Avian f64 migration is done separately.

## 6. Configuration Reference

| Env Var | Default | Description |
|---|---|---|
| `SIDEREAL_VISIBILITY_CANDIDATE_MODE` | `spatial_grid` | Candidate selection strategy. `spatial_grid` (default) or `full_scan` |
| `SIDEREAL_VISIBILITY_CELL_SIZE_M` | `2000.0` | Grid cell edge length in meters. Minimum 50m. |
| `SIDEREAL_VISIBILITY_DELIVERY_RANGE_M` | `300.0` | Maximum distance from observer anchor for entity delivery. Owner-authorized entities bypass this. |
| `SIDEREAL_REPLICATION_SUMMARY_LOGS` | `0` | Enable visibility telemetry logging (`1` = enabled) |
| `SIDEREAL_DEBUG_VIS_ENTITY_GUID` | (none) | Track a specific entity GUID through visibility evaluation |

### Cell size tuning guidance

| Cell Size | Scanner 5 km | Scanner 10 km | Notes |
|---|---|---|---|
| 300 m | 17x17 = 289 cells | 34x34 = 1,156 cells | Legacy default; too many cells per query |
| 1,000 m | 5x5 = 25 cells | 10x10 = 100 cells | Good balance |
| **2,000 m** | **3x3 = 9 cells** | **5x5 = 25 cells** | **Recommended default** |
| 5,000 m | 1x1 = 1 cell | 2x2 = 4 cells | Very fast but coarse candidate set |

Rule of thumb: `cell_size >= scanner_range / 3` keeps queries under 49 cells (7x7).

## 7. Correctness Invariants

These must hold at all times and are testable:

1. **No false negatives from the grid**: any entity that passes `authorize_visibility` + `passes_delivery_scope` must also be in the candidate set or pass `should_bypass_candidate_filter`. If the grid drops it, the bypass catches it.

2. **Owner entities always visible**: an entity whose `OwnerId` matches the player is always visible regardless of grid membership, cell distance, or solar system reachability. This is enforced by `should_bypass_candidate_filter` returning `true` for owned entities.

3. **Public entities always visible**: entities with `PublicVisibility` bypass the candidate filter. Solar system entities, galaxy-level metadata, etc. are always replicated to all clients.

4. **Faction entities visible to faction members**: entities with `FactionVisibility` + matching `FactionId` bypass the candidate filter.

5. **Empty space = zero candidates**: a player in deep space with no nearby entities, no owned entities in range, and no public/faction entities gets an empty candidate set. The only entities they receive are their own (owned bypass) and any public/faction entities (bypass).

6. **Grid does not affect authorization**: the grid is a narrowing prefilter only. Removing the grid (switching to `full_scan`) must produce identical `gain_visibility`/`lose_visibility` outcomes.

## 8. Testing Plan

### Unit tests (in `visibility.rs` or a companion test module)

1. **`cell_key` correctness**: verify cell assignment for positions at cell boundaries, negative coordinates, large coordinates (> i32 range), and zero.

2. **`add_entities_in_radius` correctness**: verify entities at exact boundary of radius are included, entities outside are excluded, and the square-walk covers the correct cell range.

3. **Candidate set construction**: with known entity positions and a known observer position + scanner sources, verify the candidate set contains exactly the expected entities.

4. **Bypass filter**: verify owned, public, faction-matched, and scanner-range entities pass the bypass even when not in the candidate set.

5. **Solar system preselection**: verify a system center at (10000, 0) with radius 5000 is reachable from observer at (14500, 0) with scanner range 1000 (distance to boundary = 14500 - 15000 = -500, within range), and not reachable from observer at (16500, 0) with scanner range 500 (distance to boundary = 16500 - 15000 = 1500, > 500).

### Integration tests

6. **Full-scan vs spatial-grid equivalence**: run the visibility system with both modes on the same entity/client configuration and verify identical visibility outcomes.

7. **Empty space produces zero updates**: spawn a player with no nearby entities, run visibility, verify no entities gain visibility except owned.

8. **Control swap preserves visibility**: swap a player's controlled entity. Verify the observer anchor moves and visibility updates correctly on the next tick.

9. **Performance regression**: with 100 entities and 10 clients, measure visibility tick time with both modes. Spatial grid should be measurably faster.

## 9. Implementation Order

| Order | Step | Risk | Dependencies |
|---|---|---|---|
| 1 | Make cell size configurable + `i64` cell keys | Low | None |
| 2 | Switch default to `spatial_grid` | Low | Step 1 |
| 3 | Add cell size / delivery range validation | Low | Step 1 |
| 4 | Add telemetry for grid stats | Low | Step 1 |
| 5 | Update `visibility_replication_contract.md` | Low | Steps 1-4 |
| 6 | Solar system preselection | Medium | `SolarSystemId`, `SolarSystemRadius` components |

Steps 1-5 can be done in a single change. Step 6 depends on the galaxy world structure components and can be done independently.

## 10. Scripting Integration

The spatial partition is not only used by the replication visibility pipeline. Script runtime queries (`ctx.world:query_nearby`, `ctx.world:query_in_system`) must also read from the same spatial index. This section documents how the partition interacts with the scripting system and the invariants that must hold.

### 10.1 Script World Snapshot and Partition Index

The current script runtime maintains a `ScriptWorldSnapshot` — a flat `HashMap<String, ScriptEntitySnapshot>` rebuilt every FixedUpdate tick. This snapshot has no spatial index. When `ctx.world:query_nearby` is implemented (scripting Phase D), iterating the full snapshot is O(N) per query, which is the same full-scan problem the visibility pipeline has.

**Requirement**: The `ScriptWorldSnapshot` must include a spatial index derived from the same grid used by the visibility system. This ensures scripts and replication use consistent spatial data and avoids duplicating partition construction.

**Implementation approach**: Add an `entities_by_cell: HashMap<(i64, i64), Vec<String>>` field to `ScriptWorldSnapshot`. During `refresh_script_world_snapshot`, populate it alongside the existing entity map using the same `cell_key(position, cell_size)` function (shared from the visibility module or extracted to a common utility). `query_nearby` then walks cells in the same square pattern as `add_entities_in_radius`.

Alternatively, the visibility partition (`VisibilityScratch::entities_by_cell`) could be exposed as a shared read-only resource that scripts query against. This avoids duplicating the grid but requires the partition to be built before scripts run (see section 10.3).

### 10.2 Query Budget Guardrails

Script spatial queries must enforce hard limits to prevent unbounded iteration from consuming the simulation budget.

| Guardrail | Default | Env Override |
|---|---|---|
| Max query radius | 50,000 m | `SIDEREAL_SCRIPT_MAX_QUERY_RADIUS_M` |
| Max results per query | 100 | per-query `limit` parameter (default 100) |
| Max spatial queries per handler invocation | 10 | `SIDEREAL_SCRIPT_MAX_QUERIES_PER_HANDLER` |
| Max total results per handler invocation | 500 | `SIDEREAL_SCRIPT_MAX_RESULTS_PER_HANDLER` |

Behavior when limits are exceeded:

- **Radius clamped**: if a script passes `radius > max_query_radius`, the radius is silently clamped. No error.
- **Result limit enforced**: results are truncated at the `limit` value. Nearest-first ordering so truncation drops the most distant entities.
- **Per-handler query count exceeded**: additional `query_nearby`/`query_in_system` calls return empty arrays and log a warning. The handler continues executing.
- **Per-handler total result count exceeded**: additional queries return empty arrays. Same warning behavior.

These limits are checked in the Rust closure that implements `ctx.world:query_nearby`, not in Lua. Scripts cannot bypass them.

At 2,000m cell size with 50km max radius, a query walks at most 25x25 = 625 cells. With 10,000 entities across ~200 occupied cells, the average cell has ~50 entities. A worst-case query touching 625 cells and filtering 50,000 entities is bounded by the result limit and instruction budget.

### 10.3 Schedule Slot: Snapshot Boundary

The script system and visibility system must operate on consistent spatial data. The current schedule ordering is:

```
FixedUpdate:
  1. refresh_script_world_snapshot          ← script snapshot built (before physics)
  2. run_script_intervals                   ← scripts execute, read snapshot, emit intents
  3. apply_script_intents                   ← intents applied to FlightComputer/ScriptState
     ... .before(PhysicsSystems::Prepare) ...
  4. PhysicsSystems::Prepare
  5. PhysicsSystems::StepSimulation
  6. PhysicsSystems::Writeback              ← physics positions finalized
  7. sync_controlled_entity_transforms      ← Bevy transforms updated from physics
  8. sync_player_anchor_to_controlled_entity
  9. update_client_observer_anchor_positions
  10. compute_controlled_entity_scanner_ranges
  11. update_network_visibility             ← visibility decisions made
```

**Current state**: Scripts read from a snapshot taken BEFORE physics. This means scripts see positions from the previous tick's physics writeback. This is acceptable because:

- Scripts run at 1-10 Hz, not every tick. One-tick position staleness is 33ms at 30 Hz — negligible at script update rates.
- Intent application is pre-physics, so flight computer adjustments affect the current tick's physics step.
- Visibility decisions use post-physics positions, so any entity moved by a script intent is evaluated at its new position for replication delivery.

**Invariant**: Script snapshot must be built AFTER the previous tick's physics writeback has propagated, and BEFORE the current tick's intent application. The current ordering satisfies this (the snapshot system runs at the start of FixedUpdate, reading positions written by the prior tick's writeback).

**Document this ordering explicitly**: The script snapshot sees the world state as of the previous tick's physics writeback. Script intents are applied before the current tick's physics prepare. Visibility evaluates after the current tick's physics writeback. This means a script's intent affects the current tick's physics, and the visibility system sees the resulting position — there is no one-tick visibility lag for script-driven motion.

When the spatial partition is shared between scripts and visibility, partition construction must occur at a point where both consumers can read it:

- **Option A (recommended)**: Build the partition once in `refresh_script_world_snapshot` (step 1). Scripts use it directly. The visibility system reuses the same partition data rather than rebuilding it from scratch in `update_network_visibility`. This eliminates duplicate grid construction per tick.
- **Option B**: Build the partition twice — once before scripts, once after physics writeback. This gives visibility the most current positions but doubles grid construction cost. Only justified if one-tick position staleness for visibility is unacceptable (it isn't, given 33ms at 30 Hz).

Option A is recommended. The visibility system should read from a shared partition resource rather than rebuilding `entities_by_cell` in its own scratch buffer. The partition is built once per tick, early in FixedUpdate, and both scripts and visibility consume it.

### 10.4 Intent Authority and Partition Updates

Scripts never mutate transforms or positions directly. The flow is:

```
Script reads snapshot → Script emits intent → Rust validates intent
→ Rust applies to FlightComputer/ScriptState → Physics processes forces
→ Physics writes back positions → Partition reflects new positions next tick
```

The partition observes motion indirectly through physics writeback. Scripts cannot cause an entity to appear in a different cell within the same tick — the cell change occurs on the next tick when the snapshot is rebuilt.

This is correct and intentional. It prevents scripts from influencing their own spatial query results within the same handler invocation (no feedback loops).

### 10.5 Script-Controllable Entity Policy

The existing `is_script_controllable` function enforces:

```rust
fn is_script_controllable(owner_id: Option<&OwnerId>, script_state: Option<&ScriptState>) -> bool {
    if script_state.is_none() { return false; }
    owner_id.is_some_and(|owner| PlayerEntityId::parse(owner.0.as_str()).is_none())
}
```

An entity is script-controllable if and only if:
1. It has a `ScriptState` component (opt-in to script control), AND
2. Its `OwnerId` does not parse as a player entity ID (it's NPC-owned, not player-owned).

This policy is enforced at intent application time, not at query time. Scripts can query and read any entity (read is safe), but can only emit intents for script-controllable entities.

**No changes needed for spatial partitioning.** The partition is a read-only spatial index. Script controllability is orthogonal to spatial querying.

### 10.6 Partition Events for Scripting (Future)

When the partition tracks entity cell membership across ticks, it can emit events:

| Event | Payload | Source |
|---|---|---|
| `entered_cell` | `{ entity_id, cell_x, cell_y, prev_cell_x, prev_cell_y }` | Cell membership change detected during snapshot rebuild |
| `left_cell` | `{ entity_id, cell_x, cell_y }` | Entity removed from cell (despawned or moved) |
| `entered_system` | `{ entity_id, solar_system_id }` | Entity's nearest system changed (1 Hz check) |
| `left_system` | `{ entity_id, solar_system_id }` | Entity exited system radius |

**Throttle policy**: These events are high-frequency by nature (every moving entity changes cells regularly). They must be throttled before reaching the script event bridge:

- `entered_cell` / `left_cell`: Not exposed to scripts directly. Cell membership is an implementation detail. Instead, expose derived events (`entered_system`, `left_system`, `approach_body`) that are meaningful to content authors.
- `entered_system` / `left_system`: Throttled to 1 Hz per entity (already matches the solar system context check rate). Deduplicated: if an entity oscillates across a boundary within one second, only one event pair is emitted.
- Future spatial events (`approach_body`, `deep_space_enter`): defined with explicit cooldown per entity per event type (e.g., 5s cooldown for `approach_body` to prevent spam near a planet).

**Implementation**: The partition rebuild system tracks previous cell assignments in a persistent `HashMap<Entity, (i64, i64)>`. On rebuild, it compares current vs previous and emits change records into a `SpatialEventBuffer` resource. The event bridge reads this buffer and dispatches to registered Lua handlers after throttle filtering.

This is future work gated behind the event bridge (scripting Phase C). Document the event contract now so the partition implementation reserves the cell-tracking data structure.

### 10.7 Persistence and Hydration Invariants

On server restart / hydration:

1. Entities are hydrated from graph persistence into the ECS world.
2. `EntityGuid` and `Position` components are present on hydrated entities.
3. The first `refresh_script_world_snapshot` tick builds the snapshot and spatial index from hydrated entities.
4. The first `run_script_intervals` tick can query the spatial index.

**Invariant**: The partition must be populated before any script interval callback fires. The current system ordering guarantees this (`refresh_script_world_snapshot` is chained before `run_script_intervals`).

**Hydration edge case**: During the first tick after hydration, `refresh_script_world_snapshot` runs and builds the full spatial index from all hydrated entities. If a script callback is due on the first tick (e.g., `next_run_s = 0.0`), it will see the complete hydrated world. This is correct.

**Guard**: If entity hydration occurs asynchronously (e.g., streamed from DB over multiple frames), the snapshot will initially be incomplete. Script interval callbacks should not fire until hydration is complete. The existing `PersistenceSchemaInitState` guard in `hydrate_simulation_entities` ensures all entities are present before the simulation enters steady state. Verify that script interval scheduling does not start ticking until after hydration completes.

### 10.8 API Contract Alignment: Spatial Filters

The script API defines filter options for `ctx.world:query_nearby` (scripting_support.md section 8.2):

| Filter | Type | Description |
|---|---|---|
| `has` | `string` or `{string, ...}` | Required component kind(s) |
| `has_any` | `{string, ...}` | At least one of these component kinds |
| `not_has` | `string` or `{string, ...}` | Must not have these component kind(s) |
| `faction` | `string` | Must have matching `FactionId` |
| `not_faction` | `string` | Must not have this `FactionId` |
| `labels` | `{string, ...}` | Must have all of these entity labels |
| `limit` | `number` | Max results (default: 100) |

The partition itself stores only `(cell_key → entity list)`. Filters are applied after cell lookup, during result construction. The partition does not need to index by component kind, faction, or label — these are post-filter operations on the candidate set returned by cell walking.

**Alignment requirements**:

1. **`ScriptEntitySnapshot` must store enough data for filter evaluation.** Currently it only has `guid`, `position`, and `script_state`. To support `has`, `faction`, `labels`, it must be extended with:
   - `component_kinds: HashSet<String>` — set of component kind strings present on the entity (populated during snapshot from the component registry).
   - `faction_id: Option<String>` — from `FactionId` component.
   - `labels: Vec<String>` — entity labels (if the label system is implemented; otherwise defer).

2. **Filter evaluation order**: cell lookup → distance check (reject entities outside radius) → component filter → faction filter → label filter → result limit. This ensures distance-based culling happens before expensive component checks.

3. **`query_in_system` uses `SolarSystemId` index, not spatial cells.** This query finds all entities with a given `SolarSystemId`, optionally filtered. The snapshot should include a `entities_by_system: HashMap<Uuid, Vec<String>>` index for O(1) system lookup. Filters are applied post-lookup, same as `query_nearby`.

4. **Both queries share the same filter implementation.** Extract a common `apply_entity_filter(entity, filter) -> bool` function used by both `query_nearby` and `query_in_system`.

---

## 11. References

- `bins/sidereal-replication/src/replication/visibility.rs` — current implementation
- `bins/sidereal-replication/src/replication/runtime_state.rs` — observer anchor sync, scanner range computation
- `bins/sidereal-replication/src/plugins.rs` — system ordering
- `docs/features/visibility_replication_contract.md` — authorization-first visibility contract
- `docs/features/galaxy_world_structure.md` — galaxy structure, coordinate precision, cell size recommendations
- `docs/features/lightyear_integration_analysis.md` — Lightyear visibility/rooms evaluation

# AGE Persistence Batching Plan

Status: Draft plan
Date: 2026-03-09

Update note (2026-03-09):
- Replication persistence now uses a wall-clock timer instead of fixed-tick counting.
- Default flush cadence is now `30` seconds via `SIDEREAL_PERSIST_INTERVAL_S`.
- The transactional AGE write path has been partially batched in this branch:
  - entity upserts,
  - stale component cleanup,
  - component upserts,
  - `HAS_COMPONENT` edge upserts,
  - `HAS_CHILD` maintenance,
  - `HAS_HARDPOINT` upserts,
  - `MOUNTED_ON` upserts.
- This reduces statement count per flush materially, but it is still an intermediate step rather than the final scale pass. The current implementation still embeds row payloads into Cypher literals rather than using typed SQL parameters, and removal batching outside the main transactional write path remains future work.

## 1. Problem Statement

Sidereal's current graph persistence path is functionally correct, but it is too statement-heavy to scale.

Observed symptom:

- With a small world of roughly `100` asteroids and `2` ships, the replication server can emit on the order of tens of thousands of AGE/Postgres statements across a short period when persistence debug logging is enabled.
- Example user observation from live logs on 2026-03-09:
  - prepared/executed statement IDs reached roughly `s24818` within about `10` seconds
  - this was for a world that is still very small by MMO standards

Important clarification:

- The persistence scheduler is still throttled to roughly one flush every `10` seconds by default.
- The scale problem is not "persisting the whole world at 100 Hz".
- The scale problem is that a single flush fans out into a very large number of individual Cypher statements.

## 2. Current Implementation

Current scheduling and worker flow:

- Flush cadence is controlled by `SimulationPersistenceTimer` in `bins/sidereal-replication/src/replication/persistence.rs`.
- Default interval is `300` ticks, with the code comment stating `300 ticks @ 30Hz = 10 seconds`.
- Current flush entrypoint:
  - `flush_simulation_state_persistence()`
  - file: `bins/sidereal-replication/src/replication/persistence.rs`
- Current worker path:
  - `persistence_worker_loop()`
  - file: `bins/sidereal-replication/src/replication/persistence.rs`

Current graph write path:

- Non-transactional path:
  - `GraphPersistence::persist_graph_records()`
  - file: `crates/sidereal-persistence/src/lib.rs`
- Transactional path added during this session:
  - `GraphPersistence::persist_graph_records_transactional()`
  - file: `crates/sidereal-persistence/src/lib.rs`
- Transaction helper used by the transactional path:
  - `persist_graph_records_in_transaction()`
  - file: `crates/sidereal-persistence/src/lib.rs`

### 2.1 Why The Current Path Explodes In Query Count

For each `GraphEntityRecord`, the current code performs:

1. `MERGE` entity node
2. delete stale component nodes not present in the incoming snapshot
3. for each component:
   - `MERGE` component node
   - `MERGE` `HAS_COMPONENT` edge
4. relationship maintenance:
   - `HAS_CHILD` cleanup and upsert
   - `HAS_HARDPOINT` upsert when applicable
   - `MOUNTED_ON` upsert when applicable

This means query volume scales roughly with:

`entities + components + entity-component edges + structural edges`

That is the real reason a small world already looks noisy.

## 3. Current Code References

Current per-record/per-component graph write loop:

- `crates/sidereal-persistence/src/lib.rs:199`
- `crates/sidereal-persistence/src/lib.rs:250`
- `crates/sidereal-persistence/src/lib.rs:270`

Transactional helper with the same write shape:

- `crates/sidereal-persistence/src/lib.rs:989`
- `crates/sidereal-persistence/src/lib.rs:1047`
- `crates/sidereal-persistence/src/lib.rs:1075`

Flush scheduling and dirty/fingerprint filtering:

- `bins/sidereal-replication/src/replication/persistence.rs:262`
- `bins/sidereal-replication/src/replication/persistence.rs:343`
- `bins/sidereal-replication/src/replication/persistence.rs:448`

## 4. What Changed In This Session

On 2026-03-09, the worker was switched to the transactional persistence path.

That improves:

- atomicity
- retry semantics
- protection against partially-applied snapshots

That does **not** materially reduce:

- number of AGE statements
- number of prepare/query log lines
- overall write amplification

This plan is specifically about solving the remaining scale problem.

## 5. Constraints

Any batching redesign must preserve the following:

- `EntityGuid` remains the durable identity boundary.
- Persistence shape remains `GraphEntityRecord` / `GraphComponentRecord`.
- Parent-child and mount relationships remain deterministic and single-parent where required.
- Nil GUIDs and explicitly runtime-only entities remain excluded.
- Ballistic projectiles remain runtime-only and non-durable.
- Persistence retries must remain safe and idempotent.
- Existing validation such as runtime GUID uniqueness must remain in place.

## 6. Design Goal

Reduce a full persistence flush from "thousands of statements" to "a small handful of statements per batch", while keeping the current graph model.

Target shape:

1. one statement for entity upserts
2. one statement for stale component-edge cleanup
3. one statement for component upserts
4. one statement for `HAS_COMPONENT` edge upserts
5. one statement for `HAS_CHILD` maintenance
6. one statement for `HAS_HARDPOINT` upserts
7. one statement for `MOUNTED_ON` upserts
8. one statement for removals

The exact count may vary, but the key requirement is:

- **constant-ish statements per batch**
- not linear statements per component

## 7. Proposed Architecture

### 7.1 Keep Rust Serialization, Change Cypher Write Shape

Do **not** change the Rust-side graph record generation shape yet.

Keep:

- `GraphEntityRecord`
- `GraphComponentRecord`
- existing component serialization helpers
- dirty/fingerprint filtering

Change only the write layer:

- flatten the batch into parameter payloads
- send those payloads through parameterized AGE Cypher using `UNWIND`

### 7.2 Introduce Batch Payload Types

Add internal write payload structs in `crates/sidereal-persistence` such as:

```rust
#[derive(Debug, serde::Serialize)]
struct EntityUpsertRow {
    entity_id: String,
    last_tick: i64,
    entity_labels: Vec<String>,
    properties: serde_json::Value,
}

#[derive(Debug, serde::Serialize)]
struct ComponentUpsertRow {
    entity_id: String,
    component_id: String,
    component_kind: String,
    last_tick: i64,
    properties: serde_json::Value,
}

#[derive(Debug, serde::Serialize)]
struct EntityComponentEdgeRow {
    entity_id: String,
    component_id: String,
}

#[derive(Debug, serde::Serialize)]
struct ChildEdgeRow {
    parent_entity_id: String,
    child_entity_id: String,
}

#[derive(Debug, serde::Serialize)]
struct HardpointEdgeRow {
    owner_entity_id: String,
    hardpoint_entity_id: String,
}

#[derive(Debug, serde::Serialize)]
struct MountedOnEdgeRow {
    module_entity_id: String,
    mount_entity_id: String,
}
```

These rows are not a persistence-domain redesign. They are only a batch transport shape for AGE.

### 7.3 Proposed Write Pipeline

For each batch:

1. Build `entity_rows`
2. Build `component_rows`
3. Build `entity_component_rows`
4. Build `child_rows`
5. Build `hardpoint_rows`
6. Build `mounted_on_rows`
7. Build `incoming_component_ids_by_entity`

Then execute a fixed set of SQL/Cypher statements.

## 8. Example Cypher Shapes

The exact AGE syntax may require iteration and local validation, but the intended direction is:

### 8.1 Entity Upsert

```sql
SELECT *
FROM ag_catalog.cypher('sidereal', $$
  UNWIND $rows AS row
  MERGE (e:Entity {entity_id: row.entity_id})
  SET e.last_tick = row.last_tick,
      e.entity_labels = row.entity_labels,
      e += row.properties
$$) AS (v agtype);
```

Rust-side parameter:

```rust
let rows_json = serde_json::to_value(&entity_rows)?;
tx.query(sql, &[&rows_json])?;
```

### 8.2 Component Upsert

```sql
SELECT *
FROM ag_catalog.cypher('sidereal', $$
  UNWIND $rows AS row
  MERGE (c:Component {component_id: row.component_id})
  SET c.component_kind = row.component_kind,
      c.last_tick = row.last_tick,
      c += row.properties
$$) AS (v agtype);
```

### 8.3 HAS_COMPONENT Edge Upsert

```sql
SELECT *
FROM ag_catalog.cypher('sidereal', $$
  UNWIND $rows AS row
  MATCH (e:Entity {entity_id: row.entity_id})
  MATCH (c:Component {component_id: row.component_id})
  MERGE (e)-[:HAS_COMPONENT]->(c)
$$) AS (v agtype);
```

### 8.4 Stale Component Cleanup

This is the trickiest part because the current implementation deletes components per-entity using an incoming allowed list.

Proposed direction:

```sql
SELECT *
FROM ag_catalog.cypher('sidereal', $$
  UNWIND $rows AS row
  MATCH (e:Entity {entity_id: row.entity_id})-[:HAS_COMPONENT]->(c:Component)
  WHERE NOT c.component_id IN row.allowed_component_ids
  DETACH DELETE c
$$) AS (v agtype);
```

Payload row example:

```rust
#[derive(Debug, serde::Serialize)]
struct AllowedComponentSetRow {
    entity_id: String,
    allowed_component_ids: Vec<String>,
}
```

### 8.5 Child Relationship Maintenance

The current implementation removes stale parents first, then inserts the correct edge.

That should remain a two-step batch:

```sql
-- delete stale parent edges
UNWIND $rows AS row
MATCH (e:Entity {entity_id: row.child_entity_id})
OPTIONAL MATCH (old:Entity)-[r:HAS_CHILD]->(e)
WHERE old.entity_id <> row.parent_entity_id
DELETE r
```

```sql
-- merge intended parent edge
UNWIND $rows AS row
MATCH (p:Entity {entity_id: row.parent_entity_id})
MATCH (e:Entity {entity_id: row.child_entity_id})
MERGE (p)-[:HAS_CHILD]->(e)
```

### 8.6 Removal Batching

Current removal path is also per-entity.

Target:

```sql
SELECT *
FROM ag_catalog.cypher('sidereal', $$
  UNWIND $entity_ids AS entity_id
  MATCH (e:Entity {entity_id: entity_id})
  OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(c:Component)
  DETACH DELETE c, e
$$) AS (v agtype);
```

## 9. Implementation Phases

### Phase 1: Instrument Before Rewrite

Add worker metrics before changing the write shape:

- number of entity rows
- number of component rows
- number of edge rows
- number of removals
- flush duration
- statement count per flush
- whether flush was initial full snapshot or incremental

This should be logged at info-level only when `SIDEREAL_REPLICATION_SUMMARY_LOGS=1`.

Example target log:

```text
replication persistence batch tick=42 mode=incremental entities=7 components=64 has_component_edges=64 child_edges=6 mounted_on_edges=4 hardpoint_edges=2 removals=0 statements=7 duration_ms=18
```

### Phase 2: Batched Removal Path

Implement batched entity removal first. It is relatively isolated and gives immediate statement-count wins.

Touchpoints:

- `GraphPersistence::remove_graph_entities()`
- any helper that calls it

### Phase 3: Batched Entity and Component Upsert Path

Add a new function such as:

```rust
pub fn persist_graph_records_batched_transactional(
    &mut self,
    records: &[GraphEntityRecord],
    tick: u64,
) -> Result<()>
```

Do not replace the old path immediately. Keep both until validated.

### Phase 4: Batched Relationship Maintenance

Move:

- `HAS_CHILD`
- `HAS_HARDPOINT`
- `MOUNTED_ON`

to `UNWIND`-driven writes.

### Phase 5: Cut Worker Over To Batched Path

Switch:

- `bins/sidereal-replication/src/replication/persistence.rs`

to call the batched transactional function.

Keep the old path available behind a temporary escape hatch env var during rollout if needed.

Suggested temporary env:

```text
SIDEREAL_PERSIST_USE_LEGACY_CYPHER_PATH=1
```

This should be temporary and removed after validation.

### Phase 6: Remove Legacy Per-Component Cypher Path

Once validated:

- remove the old high-amplification worker path
- keep only the batched path

## 10. Risks And Edge Cases

### 10.1 AGE Parameter Semantics

AGE parameter passing can be awkward depending on the exact SQL wrapper.

Mitigation:

- build the first version with targeted integration tests
- validate exact `serde_json::Value` parameter shapes against the local AGE setup

### 10.2 Large Batch Size

Very large `UNWIND` payloads may become heavy in a single statement.

Mitigation:

- add chunking by row count if needed
- example chunk sizes:
  - `256`
  - `512`
  - `1024`

Chunking is still far better than one statement per component.

### 10.3 Property Merge Semantics

Current code uses generated `SET` clauses and flattened component/object properties.

Mitigation:

- preserve the existing flattening semantics in Rust before rows are sent
- do not move property-shape logic into Cypher

### 10.4 Relationship Correctness

Single-parent semantics for `HAS_CHILD` must remain correct.

Mitigation:

- preserve the current "delete stale parent edge first, then merge intended edge" model

## 11. Test Plan

### Unit Tests

In `crates/sidereal-persistence`:

- batch row builders produce the expected payloads
- nil/empty batches are ignored safely
- GUID uniqueness validation still fires
- relationship row extraction is correct

### Integration Tests

Add or extend graph persistence tests to cover:

1. initial full snapshot
2. incremental component update
3. component removal
4. parent reassignment
5. mounted module reassignment
6. batched entity removal

### Performance/Operational Validation

Before and after comparison on the same world:

- total statements per flush
- flush duration
- entities persisted
- components persisted

Success target:

- statement count drops by at least an order of magnitude for full snapshots
- no partial snapshot corruption on worker retry

## 12. Recommended Rollout Order

1. add metrics
2. implement batched removals
3. implement batched upserts alongside legacy path
4. validate on local dev world
5. switch replication worker to batched path
6. remove legacy path after bake time

## 13. Immediate Recommendation

Do not spend more time trying to "tune" the existing per-component Cypher path.

The current bottleneck is structural:

- too many AGE statements per flush
- too much prepare/query overhead
- poor scaling even for modest worlds

The correct fix is a write-shape redesign around batched `UNWIND` persistence, not more throttling.

## 14. 2026-03-09 Status Note

As of 2026-03-09:

- replication worker batches are now transactional
- this improves correctness only
- Sidereal still needs a true batched AGE persistence rewrite to become production-credible at larger world sizes

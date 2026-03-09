# DR-0019: Chunked Binary Storage for Player Fog Memory

Status: Accepted  
Date: 2026-03-06  
Owners: replication + persistence + tactical UI

## 1. Context

`PlayerExploredCells` can grow to very large cardinality over long sessions. Storing explored cells as JSON object arrays (`[{x,y}, ...]`) is storage-inefficient and expensive to rewrite frequently.

## 2. Decision

Persist explored fog memory as chunked binary payloads under the player-scoped ECS component:

1. Keep `PlayerExploredCells` as the authoritative player-entity component.
2. Replace flat `cells: Vec<VisibilityGridCell>` payloads with:
   1. `chunk_size_cells`,
   2. `chunks: Vec<PlayerExploredCellsChunk>`.
3. Each chunk stores:
   1. `chunk_x/chunk_y`,
   2. `explored_count`,
   3. `encoding` (`Bitset` or `SparseDeltaVarint`),
   4. `payload_b64` binary payload.
4. Tactical runtime updates explored memory incrementally:
   1. rasterize live scanner circles into fog cells,
   2. set chunk bits for newly explored cells,
   3. emit `explored_cells_added` from newly set bits only.
5. Tactical fog cell size is fixed to 100m and decoupled from visibility spatial grid size.

## 3. Consequences

Positive:

1. Major storage reduction for large explored regions.
2. Avoids full JSON pair-list rewrites every tactical tick.
3. Enables adaptive dense/sparse chunk encoding without protocol changes.

Negative:

1. Component schema changed; local/dev persistence must be reset.
2. Snapshot materialization still requires decoding all chunks when a full fog snapshot is emitted.

## 4. Implementation Progress

1. `PlayerExploredCells` chunked schema: complete.
2. Adaptive chunk encoding (`Bitset` / `SparseDeltaVarint`): complete.
3. Incremental live-cell apply + newly explored delta emission: complete.
4. Fog cell size decoupled to 100m: complete.
5. Tactical/docs contract updates: complete.

## 5. Future TODOs

1. Add chunk-level dirty/snapshot telemetry (including materialization timing and payload sizing) to monitor large-world behavior.
2. Optimize full fog snapshot materialization path further if profiling shows snapshot/resync cost is significant at scale.

## 6. References

1. `docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md`
2. `docs/features/tactical_and_owner_lane_protocol_contract.md`
3. `docs/features/visibility_replication_contract.md`

# DR-0018: Fog of War and Intel Memory Model

Status: Proposed  
Date: 2026-03-05  
Owners: replication + gameplay visibility + tactical UI

## 1. Context

We need MMO-scale fog/intel behavior where:

1. Exploration permanently reveals map regions.
2. Live intel is only available while scanner visibility is currently active.
3. Outside live scanner coverage, player sees only last-known entity state.
4. Tactical map UX distinguishes unexplored, explored-stale, and live regions.

## 2. Decision

Server-authoritative model:

1. Persist player-scoped `ExploredCells`.
2. Persist player-scoped `IntelMemory` (last-known entity contact snapshots).
3. Compute live scanner visibility each fixed tick.
4. Deliver tactical projection products to client:
   1. fog masks (`explored_cells`, `live_cells`),
   2. tactical contacts (`live` + `stale` with last-seen metadata).

Client does not authoritatively infer fog/intel from local world entities.

## 3. Data Model

## 3.1 Player persisted components (authoritative)

1. `PlayerExploredCells`
   1. chunked/cell-set representation of discovered coverage.
2. `PlayerIntelMemory`
   1. keyed by target entity GUID,
   2. stores last authorized/seen snapshot:
      1. `last_seen_tick`,
      2. `position/heading/velocity` (as allowed),
      3. classification/contact quality,
      4. disclosure level from grants/policy.

These may live as persisted player-entity components and are not required to be standard replicated world components.

## 3.2 Runtime derived sets

1. `LiveVisibleNow` per player (scanner/policy result for current tick).
2. `LiveCellsNow` per player (current live scanner cells).

## 4. Update Flow (Server)

Per visibility tick, for each player:

1. Compute `LiveVisibleNow` and `LiveCellsNow`.
2. Union `LiveCellsNow` into persisted `ExploredCells`.
3. For each currently live-visible entity:
   1. update/insert `IntelMemory` from authorized fields only.
4. Generate tactical output:
   1. fog data (`ExploredCells`, `LiveCellsNow`),
   2. contacts in current tactical/bubble envelope:
      1. `is_live_now=true` for live,
      2. `is_live_now=false` for stale memory.

## 5. Delivery Rules

1. Tactical/fog products are lane-delivered (snapshot + delta with sequence numbers).
2. Owner manifest lane remains separate from fog/intel lane.
3. World-entity replication relevance does not control owner-manifest or fog memory products.

## 6. Security/Redaction Rules

1. `IntelMemory` stores only what observer was authorized to see at capture time.
2. Faction scope and component-level grants still apply.
3. Scan-intel snapshot/stream grant semantics remain preserved and required.
4. Unexplored regions must not leak hidden contacts.

## 7. Tactical Visual Semantics (Client)

1. Unexplored:
   1. opaque grey fog.
2. Explored but not live:
   1. darkened map region,
   2. stale contact icons from memory.
3. Live now:
   1. normal brightness,
   2. live contacts.

## 8. Economic Representation Strategy

Use grid/cell-set unions, not geometric polygon unions:

1. append touched cells/chunks,
2. persist compressed chunked bitsets or sparse sets,
3. update incrementally.

This keeps server CPU/memory predictable and scalable.

## 9. Consequences

Positive:

1. Correct fog/intel semantics with server authority.
2. Stable tactical UX independent of local bubble entity lifecycle.
3. Clean path to advanced intel features (scan quality tiers, decoys, ECM).

Negative:

1. Additional persisted player data.
2. Added tactical lane protocol and cache logic.

## 10. Follow-up

1. Define concrete message schemas for fog/contact snapshots + deltas.
2. Add sequence/staleness telemetry and replay-safe tests.
3. Add integration tests for:
   1. exploration growth,
   2. stale vs live transitions,
   3. no unauthorized disclosure in unexplored/stale states.

## 11. References

1. `docs/features/visibility_replication_contract.md`
2. `docs/features/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
3. `docs/features/scan_intel_minimap_spatial_plan.md`
4. `docs/features/tactical_and_owner_lane_protocol_contract.md`

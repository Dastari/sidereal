# Tactical and Owner Lane Protocol Contract

Status: Active implementation contract
Last updated: 2026-04-24
Owners: replication + client runtime + UI
Scope: tactical fog/contact lanes and owner asset manifest lane schemas/runtime behavior

Primary references:
- `docs/features/visibility_replication_contract.md`
- `docs/decisions/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
- `docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md`

## 0. Implementation Status

2026-04-24 status note:

1. Implemented: tactical snapshot/delta and owner manifest snapshot/delta messages are registered in `sidereal-net` and streamed by the replication runtime.
2. Implemented: client caches apply snapshots/deltas, request tactical resnapshots on sequence mismatch, and drive tactical UI/owner manifest presentation.
3. Implemented: `PlayerExploredCells`, `VisibilitySpatialGrid`, `VisibilityDisclosure`, tactical contacts, and manifest entries are integrated with current visibility/runtime data.
4. Open work: stress/load testing, richer contact redaction, and production tuning for large player/entity counts remain incomplete.

## 1. Objective

Define concrete snapshot/delta schemas for:

1. tactical fog + contacts lane,
2. owner asset manifest lane.

These payloads are server-authoritative read models and are not direct world-entity replication.

## 2. Transport and Ordering

## 2.1 Channel recommendations

1. Tactical lane:
   1. snapshot: reliable ordered,
   2. delta: sequenced unreliable (or reliable if packet loss tolerance is low in first pass).
2. Owner manifest lane:
   1. snapshot: reliable ordered,
   2. delta: reliable ordered.

## 2.2 Sequence model (mandatory)

Every stream uses monotonic `sequence: u64`.

Rules:

1. Snapshot establishes `base_sequence`.
2. Delta must include `base_sequence` = last fully applied sequence.
3. Client drops delta when `base_sequence` does not match local state.
4. Client sends `ClientTacticalResnapshotRequestMessage` on mismatch/timeout.
5. Server validates authenticated player binding and triggers tactical resnapshot.

## 3. Tactical Fog Schemas

## 3.1 Snapshot

```rust
pub struct ServerTacticalFogSnapshotMessage {
    pub player_entity_id: String, // UUID
    pub sequence: u64,
    pub cell_size_m: f32,
    pub explored_cells: Vec<GridCell>, // full current set (or chunked full snapshot)
    pub live_cells: Vec<GridCell>,     // current live scanner cells
    pub generated_at_tick: u64,
}
```

## 3.2 Delta

```rust
pub struct ServerTacticalFogDeltaMessage {
    pub player_entity_id: String, // UUID
    pub sequence: u64,
    pub base_sequence: u64,
    pub explored_cells_added: Vec<GridCell>,
    pub live_cells_added: Vec<GridCell>,
    pub live_cells_removed: Vec<GridCell>,
    pub generated_at_tick: u64,
}
```

## 3.3 Cell type

```rust
pub struct GridCell {
    pub x: i32,
    pub y: i32,
}
```

## 4. Tactical Contact Schemas

## 4.1 Snapshot

```rust
pub struct ServerTacticalContactsSnapshotMessage {
    pub player_entity_id: String, // UUID
    pub sequence: u64,
    pub contacts: Vec<TacticalContact>,
    pub generated_at_tick: u64,
}
```

## 4.2 Delta

```rust
pub struct ServerTacticalContactsDeltaMessage {
    pub player_entity_id: String, // UUID
    pub sequence: u64,
    pub base_sequence: u64,
    pub upserts: Vec<TacticalContact>,
    pub removals: Vec<String>, // entity_id UUIDs
    pub generated_at_tick: u64,
}
```

## 4.3 Contact type

```rust
pub struct TacticalContact {
    pub entity_id: String, // UUID
    pub kind: String,
    pub map_icon_asset_id: Option<String>,
    pub faction_id: Option<String>,
    pub position_xy: [f32; 2],
    pub heading_rad: f32,
    pub velocity_xy: Option<[f32; 2]>,
    pub is_live_now: bool,
    pub last_seen_tick: u64,
    pub classification: Option<String>,
    pub contact_quality: Option<String>,
}
```

Notes:

1. `is_live_now=true` means currently scanner/live visible.
2. `is_live_now=false` means stale memory projection.
3. `map_icon_asset_id` is sourced from entity `map_icon` (`MapIcon { asset_id }`) when present.
4. Fields remain redaction-scoped by policy/grants.

## 5. Owner Asset Manifest Schemas

## 5.1 Snapshot

```rust
pub struct ServerOwnerAssetManifestSnapshotMessage {
    pub player_entity_id: String, // UUID
    pub sequence: u64,
    pub assets: Vec<OwnedAssetEntry>,
    pub generated_at_tick: u64,
}
```

## 5.2 Delta

```rust
pub struct ServerOwnerAssetManifestDeltaMessage {
    pub player_entity_id: String, // UUID
    pub sequence: u64,
    pub base_sequence: u64,
    pub upserts: Vec<OwnedAssetEntry>,
    pub removals: Vec<String>, // entity_id UUIDs
    pub generated_at_tick: u64,
}
```

## 5.3 Entry type

```rust
pub struct OwnedAssetEntry {
    pub entity_id: String, // UUID
    pub display_name: String,
    pub kind: String,
    pub status: String, // active/docked/destroyed/unknown/in_transit
    pub controlled_by_owner: bool,
    pub last_known_position_xy: Option<[f32; 2]>,
    pub health_ratio: Option<f32>,
    pub fuel_ratio: Option<f32>,
    pub updated_at_tick: u64,
}
```

Admin/server-tool spawned owned entities use the same owner manifest path. No special client-side spawn lane exists; owner manifest upserts are the canonical UI update signal.

## 6. Client Cache Contracts

Client keeps independent caches:

1. `TacticalFogCache { sequence, cell_size_m, explored_cells, live_cells }`
2. `TacticalContactsCache { sequence, contacts_by_entity_id }`
3. `OwnedAssetManifestCache { sequence, assets_by_entity_id }`

Rules:

1. Apply snapshot atomically.
2. Apply delta only when `base_sequence == cache.sequence`.
3. On mismatch, request/await resnapshot and skip subsequent deltas.

## 7. Security and Redaction

1. All messages are server-authored from authenticated session/player binding.
2. Tactical and owner lanes must pass the same authorization + payload-redaction rules.
3. Unexplored fog areas must not include contacts unless explicitly authorized by policy.

## 8. Initial Cadence Targets

Suggested initial rates:

1. Tactical fog delta: 2-5 Hz.
2. Tactical contacts delta: 2-10 Hz (mode/zoom dependent).
3. Owner manifest delta: 1-2 Hz + immediate push on major status transitions.

## 9. Test Requirements

1. Sequence mismatch/resnapshot behavior.
2. Live-to-stale contact transition correctness.
3. Exploration growth monotonicity.
4. Owner manifest stability outside local bubble.
5. No unauthorized field leakage in tactical/manifest payloads.

## 10. Iteration 1 Runtime Notes

Current server implementation (first tactical lane cut):

1. Sends `ServerTacticalFogSnapshotMessage` and `ServerTacticalContactsSnapshotMessage` on `TacticalChannel`.
2. Snapshot cadence is low-frequency (~2 Hz).
3. Server now emits `ServerTacticalFogDeltaMessage` and `ServerTacticalContactsDeltaMessage` when content changes (`base_sequence` = last sent stream sequence).
4. Client emits `ClientTacticalResnapshotRequestMessage` on delta base mismatch or snapshot timeout (~3s), throttled (~1 Hz).
5. Server consumes validated resnapshot requests and forces immediate tactical snapshots.
6. Periodic snapshot resync is still sent (currently every ~2 seconds) as a safety net.
7. Client now applies tactical deltas with strict `base_sequence == cache.sequence`; mismatch is fail-closed and requests resnapshot.
8. `explored_cells` is now generated from persisted player memory (`player_explored_cells`) unioned with current live scanner cells each tactical update.
9. Live scanner cells are rasterized from scanner circles via circle-vs-cell intersection (grid representation, circle semantics).
10. Persisted explored memory is player-entity scoped and survives hydration/restart through graph persistence.
11. Tactical fog memory/live rasterization now uses a dedicated fine grid size of 100m cells (independent of visibility/relevance spatial grid cell size).
12. Persisted `player_explored_cells` is now chunked binary storage (`chunks[]` with adaptive bitset/sparse encoding) rather than flat JSON `[{x,y}]` cell lists.

Future TODOs:

1. Add chunk-level dirty/snapshot telemetry and payload-size metrics for fog updates.
2. Profile and optimize full fog snapshot materialization path if large-player-history snapshots become expensive.

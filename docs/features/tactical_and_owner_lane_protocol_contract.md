# Tactical and Owner Lane Protocol Contract

Status: Active implementation contract
Last updated: 2026-04-27
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
4. Current limitation: static landmark discovery does not yet have a dedicated notification/delta message; clients see discovered landmarks through normal world replication/map-icon visibility once delivery scope allows it.
5. Open work: stress/load testing, richer contact redaction, landmark discovery notifications/map reveal deltas, and production tuning for large player/entity counts remain incomplete.

2026-04-24 update:

1. Owner manifest clients now silently ignore stale deltas whose `sequence <= cache.sequence` before treating base-sequence differences as gaps. This prevents reliable but already-superseded deltas from producing repeated warning logs after a newer snapshot has advanced the local cache.

2026-04-24 update:

1. DR-0035 makes f64 authoritative world coordinates the target for tactical contacts, owner manifest position summaries, discovery metadata, and world-space notification payloads.
2. Tactical/contact/manifest protocol fields that carry absolute world position or velocity should use f64 arrays. UI/render consumers cast to f32 only after subtracting the active camera/map origin or otherwise projecting to local display coordinates.
3. Dashboard consumers use TypeScript `number` / JSON numbers for these f64 payloads.

2026-04-27 update:

1. Visibility System V2 proposes signal-only tactical contacts for high-signal entities that are not fully visible through replication visibility.
2. Signal-only contacts are redacted unknown contacts with relative strength and stable approximate position; they do not grant full entity replication or unauthorized component fields.
3. The V2 protocol extension adds optional `signal_strength` and `position_accuracy_m` fields to `TacticalContact`.

2026-04-27 update:

1. The tactical sensor ring introduces scanner-tier interpretation for contact presentation, documented in `docs/features/tactical_sensor_ring_design_contract.md`.
2. Current client ring implementation consumes `TacticalContact` data as disclosed by the existing tactical lane; server-side scanner-tier redaction remains open follow-up.
3. Target server behavior: tactical contact output must be gated by the currently controlled non-player-anchor entity's effective `ScannerComponent`; free roam/player-anchor control must not be used as a scanner source.

2026-04-27 update:

1. Audit follow-up plan added at `docs/plans/server_authoritative_tactical_scanner_and_contact_index_plan_2026-04-27.md`.
2. H3 remains open until server tactical streaming resolves an effective scanner source from `PlayerControlledEntityMap` and applies scanner-tier redaction before emitting fog/contact products.
3. M1 remains open until tactical streaming consumes cached authoring data plus a spatial contact index instead of scanning all replicated entities per authenticated client interval.

2026-04-27 update:

1. Tactical fog/contact streaming now resolves a server-side effective scanner source from `PlayerControlledEntityMap` before producing scanner-derived products.
2. Free roam/player-anchor control, missing controlled entities, and controlled entities without a usable `ScannerComponent` now emit empty live scanner cells and no scanner-derived tactical contacts while preserving already explored fog memory.
3. Full visible tactical contacts are additionally range-gated by the resolved scanner source; signal-only unknown contacts use the same source and preserve relative strength/quality redaction.
4. H3 remains partially open until scanner-tier redaction is centralized and tested. M1 remains open because the stream still uses the current per-client replicated-entity scan pending the tactical contact index phase.

2026-04-28 update:

1. Exact visible tactical contacts that also carry `SignalSignature` now include scanner-relative `signal_strength` and `contact_quality` when the player's effective scanner source can detect that signal.
2. This does not widen entity replication or expose private component payloads; it only lets client tactical instruments render directional signal-strength presentation for already-disclosed contacts such as stars and planets.

2026-04-27 update:

1. M2 protocol audit follow-up: Lightyear message registration is now direction-scoped by semantic owner. Client-authored messages are `ClientToServer`; server-authored snapshots, deltas, notifications, control responses, session responses, asset catalog version notices, and combat presentation notices are `ServerToClient`.
2. Channel registration remains bidirectional only where a channel intentionally carries both client requests and server responses/events (`ControlChannel`, `InputChannel`, `TacticalSnapshotChannel`, and `NotificationChannel`). `TacticalDeltaChannel` and `ManifestChannel` are server-to-client only.
3. Runtime transport repair systems must preserve those channel directions instead of adding missing senders/receivers for both sides.
4. Native impact: narrower Lightyear sender/receiver components reduce accidental wrong-side message consumption. WASM impact: shared client protocol registration is affected and must compile with the normal `sidereal-client` WASM target.
5. The replication protocol version is `7` for the directional registration contract.

2026-04-27 update:

1. `TacticalContact` now includes optional public `size_m` and `mass_kg` metadata when disclosed by the tactical lane.
2. The native client tactical map uses live planet/star/black-hole contacts from those fields to drive shader-side gravity-well grid warping; planet tactical icons render at 8x the standard tactical marker scale.
3. WASM impact: the behavior is shared client runtime/shader logic and must compile with the normal `sidereal-client` WASM target; no browser-only transport or asset-loading behavior changed.
4. The replication protocol version is `6` for the tactical contact size/mass field addition.

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
    pub position_xy: [f64; 2],
    pub size_m: Option<[f32; 3]>,
    pub mass_kg: Option<f32>,
    pub heading_rad: f64,
    pub velocity_xy: Option<[f64; 2]>,
    pub is_live_now: bool,
    pub last_seen_tick: u64,
    pub classification: Option<String>,
    pub contact_quality: Option<String>,
    pub signal_strength: Option<f32>,
    pub position_accuracy_m: Option<f32>,
}
```

Notes:

1. `is_live_now=true` means currently scanner/live visible.
2. `is_live_now=false` means stale memory projection.
3. `map_icon_asset_id` is sourced from entity `map_icon` (`MapIcon { asset_id }`) when present.
4. `signal_strength` and `position_accuracy_m` are set only for Visibility V2 signal-derived contacts.
5. Fields remain redaction-scoped by policy/grants.
6. For scanner-tier tactical products, `contact_quality` describes the scanner-disclosed quality band and `classification` may carry only server-authorized relationship values such as `friendly`, `hostile`, `neutral`, or `unknown`.
7. The client must not derive relationship/classification from replicated private ECS data for tactical ring presentation.
8. `size_m` and `mass_kg` are optional public tactical presentation hints; clients may use them for non-authoritative visual scale/gravity-well effects only.

### 4.3.1 V2 Signal Contact Rules

1. Signal-only unknown contacts use `kind = "unknown"`, `classification = Some("unknown")`, and `map_icon_asset_id = Some("map_icon_unknown_contact_svg")`.
2. Signal-only unknown contacts carry `signal_strength` plus `position_accuracy_m`.
3. Signal-only contacts must not include unauthorized faction, velocity, exact classification, or component-derived details.
4. The replication protocol version is `5` for the `TacticalContact` signal field addition and `6` for the size/mass presentation field addition.

## 4.4 Landmark Discovery Notification Direction

Planned landmark discovery notifications should be server-authored from the same authenticated player binding and discovery state described in `docs/features/visibility_replication_contract.md`.

Requirements for the first implementation:

1. Emit only after the backend inserts a new landmark UUID into `DiscoveredStaticLandmarks`.
2. Include stable UUID identity, display name when disclosed, landmark kind, map icon asset ID when disclosed, position, and discovery tick/time.
3. Preserve idempotence: reconnects or resnapshots may restate discovered state, but the client notification queue must not toast the same initial discovery repeatedly.
4. Keep notification payload redacted by the same visibility/disclosure policy used for world and tactical lanes.
5. Treat the client sonar/toast as presentation only; it must not authoritatively mutate discovery state.

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
    pub last_known_position_xy: Option<[f64; 2]>,
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
2. Ignore stale deltas when `sequence <= cache.sequence`.
3. Apply delta only when `base_sequence == cache.sequence`.
4. On a forward sequence gap, request/await resnapshot and skip subsequent deltas.

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

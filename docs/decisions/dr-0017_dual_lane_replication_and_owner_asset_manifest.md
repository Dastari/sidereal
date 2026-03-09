# DR-0017: Dual-Lane Replication and Owner Asset Manifest

Status: Proposed  
Date: 2026-03-05  
Owners: replication + client runtime + gameplay UI

## 1. Context

Current relevance is dominated by in-world local-bubble visibility. This causes UX gaps:

1. Tactical zoom/map mode needs broad, lower-detail world awareness.
2. Owned-asset UI (for example "Owned ships") disappears when owned entities leave the local bubble because the UI currently reads live world entities.

We need a robust, explicit model for:

1. high-rate local simulation data,
2. lower-rate tactical map data,
3. owner-scoped asset list/state that is not tied to local-bubble relevance.

## 2. Decision

Adopt two replication data lanes plus one owner-manifest lane:

1. `LocalBubbleLane` (high-rate, nearby, simulation-rich)
2. `TacticalLane` (lower-rate, wider-range, reduced payload)
3. `OwnerAssetManifestLane` (owner-only account/player scope, relevance-independent)

The client keeps an explicit local cache resource for owner assets populated from lane 3. UI panels must read this resource, not inferred world-entity presence.

## 3. Why this model

1. Preserves simulation authority and relevance costs for in-world entities.
2. Supports zoomed-out tactical map without forcing full-fidelity replication across huge ranges.
3. Decouples ownership/progression UI from camera/bubble relevance.
4. Aligns with existing `LocalPlayerViewState` and view-mode signaling.

## 4. Data Contracts

## 4.1 Local bubble payload (existing behavior baseline)

Use current authoritative replicated components for entities inside local delivery scope.

Properties:

1. High frequency.
2. Full transform/motion/gameplay state needed for nearby simulation and visuals.

## 4.2 Tactical payload (new reduced model)

Introduce reduced tactical records for map rendering and strategic awareness.

Suggested fields:

1. `entity_id` (UUID string)
2. `kind` (ship/station/etc)
3. `faction_id` / relationship class
4. `position_xy`
5. `heading_rad`
6. `velocity_xy` (optional, lower precision allowed)
7. `icon_state` (alive/damaged/hidden/contact quality)
8. `last_update_tick`

Properties:

1. Lower send rate than local bubble.
2. Independent range profile tuned for tactical zoom.
3. Never used as authoritative physics input; render/intel only.

## 4.3 Owner asset manifest payload (new owner-only model)

This is the answer to "should this be a client resource?": yes, but fed by authoritative server channel.

Server sends owner-scoped manifest snapshot + deltas:

1. `owner_player_entity_id`
2. `assets[]` where each item contains:
   1. `entity_id`
   2. `display_name`
   3. `kind`
   4. `last_known_position_xy` (nullable)
   5. `status` (active/docked/destroyed/unknown/in_transit)
   6. `controlled_by_owner` (bool)
   7. lightweight telemetry (health/fuel summary optional)
3. `sequence` for monotonic ordering and delta application safety

Properties:

1. Owner-only visibility policy.
2. Not gated by local bubble or tactical relevance.
3. Lower update cadence (for example 1-2 Hz) with immediate push on important transitions.

## 5. Client Model

Add a client resource cache:

1. `OwnedAssetManifestCache`
   1. `by_entity_id: HashMap<String, OwnedAssetEntry>`
   2. `last_sequence: u64`
   3. `last_snapshot_at`

Rules:

1. UI panels (Owned ships/assets) read from this cache only.
2. In-world selection/control still uses authoritative runtime entity mappings.
3. If manifest item has no live world entity in bubble, UI still shows it with manifest status.

## 6. View Mode and Lane Selection

`ClientLocalViewModeMessage` remains the mode switch signal and may be extended with tactical parameters.

Server behavior:

1. In `Tactical` mode: prioritize local bubble lane.
2. In `Map` mode: keep local bubble active for immediate vicinity, add/expand tactical lane delivery.
3. Apply hysteresis around zoom/mode transitions to avoid thrash.

## 7. Security and Authority

1. All lanes are server-authored.
2. Owner manifest is filtered by authenticated session binding (same ownership checks as control/input).
3. Client cache is read model only, never authoritative for gameplay mutation.

## 8. Implementation Plan

1. Protocol:
   1. add tactical message types (`ServerTacticalContactsSnapshotMessage`, `ServerTacticalContactsDeltaMessage`)
   2. add owner manifest message types (`ServerOwnerAssetManifestSnapshotMessage`, `ServerOwnerAssetManifestDeltaMessage`)
2. Replication runtime:
   1. build tactical contact extraction path
   2. build owner manifest extraction path
   3. send on dedicated channels with explicit pacing
3. Client runtime:
   1. add `OwnedAssetManifestCache` resource
   2. ingest snapshot/delta messages with sequence checks
   3. migrate owned-asset UI to cache-backed data source
4. Dashboard/observability:
   1. expose lane rates and payload counts
   2. track manifest staleness and sequence gaps

## 9. Acceptance Criteria

1. Owned-asset UI remains populated when camera/player is outside local bubble.
2. Tactical map can display remote contacts without full local-bubble simulation payload.
3. Control/selection semantics stay authoritative and unchanged.
4. No owner-data leakage across sessions.
5. Native and WASM consume same protocol model.

## 10. Alternatives Considered

1. Keep using world entities for owned-asset UI:
   1. rejected; tied to visibility, causes disappearing data.
2. Client-side database polling:
   1. rejected; breaks authority flow and increases coupling.
3. Single giant relevance radius:
   1. rejected; poor bandwidth scaling and defeats lane purpose.

## 11. References

1. `docs/features/visibility_replication_contract.md`
2. `docs/plans/scan_intel_minimap_spatial_plan.md`
3. `docs/sidereal_design_document.md`
4. `docs/features/tactical_and_owner_lane_protocol_contract.md`

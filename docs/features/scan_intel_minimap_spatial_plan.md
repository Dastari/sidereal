# Scan Intel, Minimap Streams, and Spatial Indexing Plan

## Goal
Define a server-authoritative visibility architecture that prevents data leakage while scaling to many entities.

This extends the phase-1 permission policy in `docs/visibility_and_data_permissions.md`.

## 1. Scan Intel Grants

### Problem
Players need to physically observe ships at a distance, but should not receive private internals (cargo manifests, hidden subsystem details) unless gameplay actions grant that knowledge.

### Core Model
Use explicit temporary grants stored server-side:
- `grant_id` (UUID)
- `observer_player_entity_id`
- `target_entity_id`
- `field_scope` (which data fields become visible)
- `granted_at_ms`
- `expires_at_ms`
- `source` (`active_scan`, `dock_access`, `boarding`, `allied_share`, etc.)

### Field Scopes (starting point)
- `physical_public`: transform, velocity, orientation, render/body identifiers
- `combat_profile`: shield state, hull state, hardpoint occupancy summary
- `cargo_summary`: aggregate cargo mass / class summary only
- `cargo_manifest`: full itemized cargo details
- `systems_detail`: module-level internals

### Enforcement Rule
When building `StateFrame`, gateway computes:
1. authorization via ownership/scanner rules,
2. applicable active grants for `(observer, target)`,
3. final redaction mask.

If no grant allows a field, it is omitted from payload.

### Revocation
Grant revocation occurs automatically by expiry or explicit invalidation events.
Expired grants must immediately revert target data to redacted output.

## 2. Minimap / Multi-Stream Model

### Problem
A single focus stream is not enough for fleet/macro awareness.
Need broad situational awareness without leaking sensitive detail or saturating bandwidth.

### Proposed Streams
1. `focus_stream` (high rate, low radius)
- Current controlled-entity local gameplay data.
- Includes precise physics state for nearby entities.

2. `strategic_stream` (low rate, wider radius)
- Minimap contacts and coarse kinematics.
- Lower precision, lower frequency.
- No private internals by default.

3. `intel_stream` (event-driven)
- Scan-grant results / intel updates.
- Contains only fields allowed by active grants.

### Message Direction
All streams remain server-authoritative and permission-filtered.
Client never requests raw hidden data directly.

## 3. Spatial Indexing for Visibility Queries

### Why
Visibility checks become too expensive when every client iterates all entities each tick.

### Phase-1 Implementation (now in gateway)
- Added a 2D spatial hash in `sidereal-web-gateway` world state.
- Added `GATEWAY_SPATIAL_CELL_M` (default `250`).
- Visibility candidate selection now pulls from spatial cells around:
  - focus radius,
  - each owned scanner radius.
- Ownership/attachment descendants are indexed separately and always included.

### Current Complexity
- Prior approach: roughly `O(total_entities)` per client frame (+ scan loops).
- Current approach: `O(entities_in_nearby_cells + owned_descendants)` per frame.

### Tradeoffs
- Uniform grids are simple and fast for roughly even distributions.
- Very dense hotspots can still overload one cell.
- Cell size tuning matters: too small => many cells per query, too large => too many entities per cell.

## 4. Next Spatial Steps
1. Add visibility-query metrics:
- candidates per frame
- included entities per frame
- query time budget per client

2. Adaptive cell strategy:
- optionally use two-level grid or quadtree in high-density regions

3. Precomputed scanner coverage cache:
- avoid recomputing owned scanner unions per frame when owned entities are static

## 5. Security Invariants
- Unauthorized fields are never serialized onto any network stream.
- Redaction occurs server-side before transport encoding.
- Packet capture by clients must not reveal hidden internals.

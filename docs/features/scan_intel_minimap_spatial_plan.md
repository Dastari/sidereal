# Scan Intel, Minimap Streams, and Spatial Indexing Plan

## Goal
Define a server-authoritative visibility architecture that prevents data leakage while scaling to many entities.

This extends the active contracts in:
- `docs/sidereal_design_document.md` (Section 7)
- `docs/features/visibility_replication_contract.md`

## Runtime Context (Canonical)

Control/observer chain:

1. `camera <- player entity <- controlled entity (optional)`

Implications for visibility and intel:

1. Observer anchor identity is player entity state (server-side runtime/persisted player transform context).
2. Controlled entity may drive player position when control is active, but does not replace player as observer identity.
3. No-controlled mode keeps player free-roam movement authoritative for observer anchor updates.

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
When building outbound replication payloads, server computes:
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
- Local gameplay data around observer/player context.
- Includes precise physics state for nearby entities allowed by authorization + delivery policy.

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

### Runtime Baseline (current)
- Replication currently performs per-client visibility updates in fixed tick.
- Visibility evaluation still includes full-world iteration behavior in hot path and needs migration to spatial opt-in candidate selection for scale.

### Target Runtime Implementation
- Add a 2D spatial index in `sidereal-replication` runtime state.
- Add tunable cell-size/runtime params under replication env config.
- Build visibility candidate sets from spatial cells around:
  - observer camera/delivery radius,
  - owned scanner-source radii,
  - plus explicit ownership/attachment exception paths.

### Current Complexity
- Current behavior: roughly `O(clients * entities)` per fixed visibility update pass, plus scanner/ownership exception handling.
- Target behavior: `O(clients * (entities_in_nearby_cells + ownership/grant exceptions))` once spatial candidate preselection is implemented.

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
- Snapshot disclosure does not imply stream disclosure.
- Grant expiry/revocation immediately restores redaction defaults.

## 6. Pipeline Clarification

Canonical visibility pipeline:
1. Authorization (security entitlement).
2. Delivery narrowing / interest management (performance).
3. Payload redaction (component/field disclosure control).

Implementation optimization:
- A spatial opt-in candidate preselection stage may run before full authorization evaluation.
- It must not be treated as authorization by itself and must not exclude policy-required exceptions (ownership/public/faction/grants).

## 7. Current Status and Gaps

Implemented:

1. Authorization-first visibility contract is documented and enforced as source-of-truth policy.
2. Per-client visibility updates use server authoritative `gain_visibility/lose_visibility` flow.
3. Player-observer anchor model is documented across design and visibility contracts.

Not yet implemented:

1. Spatial index candidate preselection in replication runtime.
2. Runtime scan grant store/evaluator integrated into outbound payload redaction flow.
3. Full component-visibility metadata enforcement for outbound redaction.

Exit criteria for this plan:

1. Spatial candidate preselection replaces full-world per-client visibility scans.
2. Snapshot-vs-stream grant semantics are enforced by runtime tests.
3. Minimap/strategic stream delivery uses shared authorization and redaction policy without data leakage.

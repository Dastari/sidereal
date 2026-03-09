# Visibility and Replication Contract

Status: Active source-of-truth (current runtime-aligned)  
Date: 2026-03-05

Primary references:
- `docs/sidereal_design_document.md`
- `AGENTS.md`
- `docs/decisions/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
- `docs/plans/scan_intel_minimap_spatial_plan.md`

## 1. Goal

Keep visibility and replication server-authoritative, scalable, and disclosure-safe:

1. Server decides what each client can know.
2. Delivery culling narrows authorized data only.
3. Component/field disclosure is policy-driven.
4. Tactical/fog/intel memory behavior is explicit and lane-based.

## 2. Canonical Stage Order (Mandatory)

All visibility-sensitive changes must preserve:

1. Authorization scope (security entitlement):
   1. ownership/public/faction policy,
   2. scanner/fog/intel grant policy.
2. Delivery scope (performance narrowing):
   1. local bubble/tactical lane range and mode.
3. Payload scope (redaction):
   1. component/field masking before serialization.

Delivery must never widen authorization.

Spatial candidate generation is optimization input only, not authorization.

## 3. Runtime Baseline (Implemented)

Current implementation baseline:

1. Per-client visibility updates are server-driven in replication fixed tick.
2. Candidate preselection uses spatial grid by default (`SIDEREAL_VISIBILITY_CANDIDATE_MODE=spatial_grid`).
3. Full policy checks run after candidates; policy exceptions (owner/public/faction/scanner) are fail-closed safe.
4. World position checks use `GlobalTransform` semantics for range/visibility behavior.
5. Observer anchor identity is player entity (`camera <- player <- controlled(optional)`).
6. Current runtime uses generic `VisibilityRangeM` / `VisibilityRangeBuffM` with no implicit `ShipTag` baseline.
7. `VisibilitySpatialGrid` and `VisibilityDisclosure` are mirrored onto player entity for owner debug/inspection.
8. Delivery range is dynamic per client view and reflected in runtime visibility telemetry.
9. Fullscreen authored config entities are treated as non-spatial overlays: legacy `FullscreenLayer` entities and fullscreen-phase `RuntimeRenderLayerDefinition` entities bypass delivery-range/visibility-range candidate culling and remain replicated while connected.
10. Background authoring settings such as `SpaceBackgroundShaderSettings` and `StarfieldShaderSettings` are durable world configuration and remain persistable so hydration recreates the full authored config entity rather than only the layer-definition shell.

2026-03-09 update:
- The native client renders fullscreen background passes directly from those authored fullscreen entities again. Client-local fullscreen renderable copies were removed because they could diverge from the authored source during zoom/hydration transitions and expose the black fallback layer.

## 4. Multi-Lane Contract (Current + Approved Direction)

Lane model:

1. `LocalBubbleLane`:
   1. high-rate nearby simulation state,
   2. authoritative world entities.
2. `TacticalLane`:
   1. lower-rate reduced contact/fog payload for zoomed-out map.
3. `OwnerAssetManifestLane`:
   1. owner-only asset list/state,
   2. independent of local bubble relevance.

Normative rules:

1. Owned-asset UI must not depend on local-bubble world-entity presence.
2. Owner manifest is server-authored and client-cached as read model.
3. Tactical lane never upgrades authorization; it is a reduced delivery/product lane.

## 5. Fog of War and Intel Memory Contract

Fog/intel behavior:

1. Players start with unexplored space (`0` explored coverage).
2. Exploration permanently grows discovered map coverage (`ExploredCells`).
3. Live intel is only from current visibility/live visibility.
4. Outside live visibility, only server-stored last-known intel may be shown (stale memory).
5. Tactical explored-memory persistence uses chunked binary component payloads (adaptive dense/sparse chunk encoding), not flat JSON coordinate lists.
6. Tactical fog memory cell size is 100m and independent from visibility relevance spatial grid cell size.

Authoritative placement:

1. Intel memory is server-authoritative and persisted on player-scoped data (player entity components).
2. Raw intel-memory components are not required to be standard replicated world components.
3. Client receives tactical projection products (snapshot/delta lane payloads), not unrestricted raw memory.

## 6. Disclosure Policy (Pending Scope Preserved)

Still required and explicitly preserved:

1. Faction-based visibility scopes.
2. Component/field-level visibility/redaction policy (for example inventory detail requiring scan-intel grant).
3. Snapshot-vs-stream grant semantics for scan intel.

Status:

1. These are active contract requirements.
2. Some parts remain implementation-in-progress and must not be removed from docs.

## 7. Edit Checklist (Mandatory)

For any PR touching visibility, tactical delivery, fog/intel memory, or redaction:

1. Verify stage order remains `Authorization -> Delivery -> Payload`.
2. Verify tactical/owner lanes do not bypass authorization.
3. Verify unauthorized component/field data is never serialized.
4. Verify player observer-anchor identity rules remain consistent.
5. Verify fog/intel memory semantics:
   1. unexplored vs explored,
   2. live vs stale intel.
6. Add/update tests for changed behavior.
7. Update:
   1. this contract,
   2. any related DR under `docs/features/`,
   3. `docs/decision_register.md` links when decisions change.

## 8. Visibility Range Naming Direction (Accepted)

Canonical generic visibility-range terminology is now:

1. `VisibilityRangeM`
   - effective resolved visibility/disclosure range read by the hot visibility path
2. `VisibilityRangeBuffM`
   - generic contributing modifier to visibility range

Normative direction:

1. Visibility systems should converge on `VisibilityRangeM` / `VisibilityRangeBuffM`.
2. Genre-specific names such as `scanner` may remain in Lua/content authoring, but not as the engine-owned built-in runtime concept.
3. `ShipTag` must not imply hidden baseline visibility range.
4. Root entities may carry both effective `VisibilityRangeM` and local `VisibilityRangeBuffM`.
5. Aggregation should compute root effective range from generic contributors before hot visibility checks run.

Implementation note:

1. Runtime code now uses `VisibilityRangeM` / `VisibilityRangeBuffM`.
2. `VisibilityDisclosure` now carries `visibility_sources`, not `scanner_sources`.
3. Genre-specific `scanner_*` wording may still exist in content/action names, but not as the engine-owned runtime component names.
4. See:
   - `docs/decisions/dr-0028_generic_visibility_range_components.md`
   - `docs/plans/generic_visibility_range_migration_plan.md`

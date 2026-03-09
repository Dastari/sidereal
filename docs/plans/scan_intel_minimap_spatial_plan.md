# Scan Intel, Tactical Contacts, and Spatial Visibility Notes

Status: Active supplemental plan (aligned to current runtime)  
Date: 2026-03-05

Primary contract docs:
- `docs/features/visibility_replication_contract.md`
- `docs/decisions/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
- `docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md`

## 1. Scope

This document focuses on:

1. scan-intel grant semantics,
2. tactical contact products,
3. spatial query scaling notes.

It does not replace canonical visibility contract wording.

## 2. Scan Intel Grants (Preserved Requirement)

Problem:

1. Entity visibility should not imply full private component disclosure.

Model:

1. Server-side grants scoped by `(observer, target, field_scope, expiry)`.
2. Support both:
   1. snapshot grants (one-time disclosure),
   2. stream grants (continuous while active).

Field-scope examples:

1. `physical_public`
2. `combat_profile`
3. `cargo_summary`
4. `cargo_manifest`
5. `systems_detail`

Rule:

1. Unauthorized fields/components are never serialized.

## 3. Tactical Contact Product

Tactical lane should carry reduced contacts, not full simulation payload:

1. `entity_id`
2. `kind`
3. `position_xy`
4. `heading_rad`
5. `velocity_xy` (optional)
6. `is_live_now`
7. `last_seen_tick`
8. relationship/classification info

Live vs stale behavior is defined by DR-0018.

## 4. Spatial Scaling Notes

Current baseline:

1. Spatial grid candidate preselection is implemented in visibility runtime.
2. Candidate mode + cell size are runtime configurable.
3. Full policy checks still execute after candidate stage.

Practical guidance:

1. Tune `cell_size_m` to entity density.
2. Monitor candidate/query telemetry.
3. Consider adaptive structures only when profiling shows grid saturation hotspots.

## 5. Security Invariants

1. Authorization remains server-authoritative.
2. Delivery lanes narrow data only.
3. Tactical and owner-manifest lanes must not bypass redaction policy.
4. Unexplored regions must not leak hidden contacts.

## 6. Remaining Work

1. Complete grant runtime evaluator for component/field disclosure.
2. Integrate faction + grant scopes through tactical/stale intel products.
3. Add tests for:
   1. snapshot-vs-stream grant behavior,
   2. stale intel redaction correctness,
   3. no leakage from unexplored regions.

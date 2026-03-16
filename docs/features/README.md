# Feature Docs

Status: Active index
Date: 2026-03-09

This folder is for active feature-scoped documentation, not for decision records, audit reports, or general implementation plans.

Update note (2026-03-09):
- Decision detail docs were moved to `docs/decisions/`.
- Plan-style docs were moved to `docs/plans/`.
- This folder now focuses on active contracts, active feature references, and feature-scoped implementation notes/specs.

Update note (2026-03-13):
- The core systems catalog now lives at `docs/core_systems_catalog_v1.md` as a cross-cutting docs-root reference rather than a feature-folder document.

Update note (2026-03-16):
- Added `resources_and_crafting_contract.md` as the active feature specification for server-authoritative resource extraction, refining, crafting, and downstream manufacturing content.

## What Belongs Here

1. Active implementation contracts:
   - `asset_delivery_contract.md`
   - `background_world_simulation_contract.md`
   - `visibility_replication_contract.md`
   - `tactical_and_owner_lane_protocol_contract.md`
2. Active feature references:
   - `asset-packs.md`
   - `asteroid_field_system.md`
   - `brp_debugging_workflow.md`
   - `galaxy_world_structure.md`
   - `procedural_asteroids.md`
   - `procedural_planets.md`
   - `projectile_firing_game_loop.md`
   - `prediction_runtime_tuning_and_validation.md`
   - `lightyear_upstream_issue_snapshot.md`
3. Active feature-scoped notes/specs:
   - `dashboard_route_shell_refactor_note.md`
   - `shader_editor_dashboard_implementation_spec.md`
   - `lightyear_integration_analysis.md`
   - `scripting_support.md`
   - `resources_and_crafting_contract.md`

## What Does Not Belong Here

1. Decision detail docs:
   - use `docs/decisions/dr-XXXX_<slug>.md`
2. Plans and migration roadmaps:
   - use `docs/plans/`
3. Audit outputs and generated reports:
   - use `docs/reports/`

## Related Indexes

1. Docs root index: `docs/README.md`
2. Core systems catalog: `docs/core_systems_catalog_v1.md`
3. Decision register: `docs/decision_register.md`
4. Architecture baseline: `docs/sidereal_design_document.md`
5. Implementation tracker: `docs/sidereal_implementation_checklist.md`

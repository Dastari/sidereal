# Sidereal Documentation Index

Status: Active index
Date: 2026-03-09

Use this page to find the right documentation surface before adding or editing docs.

## Start Here

1. Architecture and runtime baseline: `docs/sidereal_design_document.md`
2. Implementation tracker: `docs/sidereal_implementation_checklist.md`
3. Decision register: `docs/decision_register.md`
4. Contributor operating rules: `AGENTS.md`

## Folder Roles

Update note (2026-03-09):
- Documentation taxonomy normalized so folder role matches document role.

1. `docs/features/`
   - Active feature contracts, implementation contracts, active feature references, and feature-scoped notes.
2. `docs/decisions/`
   - Decision detail documents for decision-register entries.
3. `docs/plans/`
   - Proposed or active implementation plans, migration plans, tuning plans, and roadmap-style execution docs.
4. `docs/reports/`
   - Audit outputs, investigation reports, and dated reconciliation findings.
5. `docs/prompts/`
   - Reusable audit/report generation prompts.
6. `docs/samples/`
   - Sample/reference artifacts that are not active contracts.

## Common Entry Points

1. Rendering and shader/runtime direction:
   - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
   - `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`
   - `docs/plans/dynamic_runtime_shader_material_plan.md`
   - `docs/plans/rendering_optimization_pass_plan.md`
2. Visibility and replication:
   - `docs/features/visibility_replication_contract.md`
   - `docs/features/tactical_and_owner_lane_protocol_contract.md`
   - `docs/decisions/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
   - `docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md`
3. Asset delivery and scripting:
   - `docs/features/asset_delivery_contract.md`
   - `docs/features/scripting_support.md`
   - `docs/decisions/dr-0025_runtime_script_catalog_authority.md`
   - `docs/decisions/dr-0026_sql_script_catalog_persistence.md`
4. Historical audits and reports:
   - `docs/reports/`

## Maintenance Rules

1. New audit outputs go in `docs/reports/`.
2. New plans go in `docs/plans/`.
3. New decision detail docs go in `docs/decisions/`.
4. New active feature contracts/references go in `docs/features/`.
5. Add dated status/update notes when making substantive documentation changes.

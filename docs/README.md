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

1. Core system labels and Version 1 naming:
   - `docs/core_systems_catalog_v1.md`
2. Rendering and shader/runtime direction:
   - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
   - `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`
   - `docs/plans/dynamic_runtime_shader_material_plan.md`
   - `docs/features/genesis_planet_registry_contract.md`
3. Visibility and replication:
   - `docs/features/visibility_replication_contract.md`
   - `docs/features/tactical_and_owner_lane_protocol_contract.md`
   - `docs/decisions/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
   - `docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md`
   - `docs/plans/replication_server_tui_backend_plan_2026-03-10.md`
4. Background world simulation and economy direction:
   - `docs/features/background_world_simulation_contract.md`
   - `docs/features/resources_and_crafting_contract.md`
   - `docs/decisions/dr-0033_background_world_simulation_tiering.md`
5. Asset delivery and scripting:
   - `docs/features/asset_delivery_contract.md`
   - `docs/features/asteroid_field_system.md`
   - `docs/features/scripting_support.md`
   - `docs/decisions/dr-0025_runtime_script_catalog_authority.md`
   - `docs/decisions/dr-0026_sql_script_catalog_persistence.md`
6. Historical audits and reports:
   - `docs/reports/`

## Maintenance Rules

1. New audit outputs go in `docs/reports/`.
2. New plans go in `docs/plans/`.
3. New decision detail docs go in `docs/decisions/`.
4. New active feature contracts/references go in `docs/features/`.
5. Add dated status/update notes when making substantive documentation changes.

Update note (2026-03-13):
- Added `docs/core_systems_catalog_v1.md` as the reference index for stable "System V1" labels such as audio, rendering, tactical map, planets, fog/intel, and related core runtime systems.

Update note (2026-03-16):
- Added `docs/features/resources_and_crafting_contract.md` as the active feature specification for the shared material taxonomy, crafting model, and Bevy/content-authoring contract that will underpin future economy and manufacturing work.

Update note (2026-04-24):
- Added `docs/features/genesis_planet_registry_contract.md` as the active feature spec for Genesis planet/celestial authoring and the Lua planet registry.

Update note (2026-04-24):
- Removed older superseded March 2026 audit reports now replaced by the 2026-04-23 reports under `docs/reports/`.
- Removed implemented one-off plans whose current behavior is already captured by active feature contracts or accepted decision records:
  - generic visibility range migration is now covered by `docs/decisions/dr-0028_generic_visibility_range_components.md` and `docs/features/visibility_replication_contract.md`;
  - discovered static landmark visibility is now covered by `docs/decisions/dr-0032_discovered_static_landmark_visibility.md` and `docs/features/visibility_replication_contract.md`.
- Removed the stale March 10 Rust audit remediation plan, which depended on a missing March 10 audit report and has been superseded by current audit prompts/reports.
- Retained older plans that are still active, parked, partially implemented, or future-direction documents rather than deleting them based on age alone.

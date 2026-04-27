# Feature Docs

Status: Active index
Last updated: 2026-04-27
Owners: documentation + feature owners
Scope: active feature contracts, feature references, and feature-scoped notes/specs

This folder is for active feature-scoped documentation, not for decision records, audit reports, or general implementation plans.

Update note (2026-04-24):
- Added `genesis_planet_registry_contract.md` for the dedicated Genesis planet/celestial authoring workflow and Lua planet registry contract.

Update note (2026-04-26):
- Added `account_character_selection_layout_contract.md` for the reusable dashboard/native character-selection layout and account-scoped character lifecycle contract.

Update note (2026-04-27):
- Added `visibility_system_v2_signal_detection_contract.md` for the proposed Visibility System V2 direction covering signal-based detection, unknown tactical contacts, stable approximate positions, and zoom-safe delivery/client culling.

Update note (2026-04-24):
- Standardized the expected feature-document layout so each feature doc clearly states status, implementation coverage, open work, and validation scope.
- Feature docs are now grouped by current purpose rather than by the date they were originally written.

Update note (2026-03-09):
- Decision detail docs were moved to `docs/decisions/`.
- Plan-style docs were moved to `docs/plans/`.
- This folder now focuses on active contracts, active feature references, and feature-scoped implementation notes/specs.

Update note (2026-03-13):
- The core systems catalog now lives at `docs/core_systems_catalog_v1.md` as a cross-cutting docs-root reference rather than a feature-folder document.

Update note (2026-03-16):
- Added `resources_and_crafting_contract.md` as the active feature specification for server-authoritative resource extraction, refining, crafting, and downstream manufacturing content.

## Document Standard

Feature docs should use this shape:

1. `# Title`
2. Metadata fields:
   - `Status: ...`
   - `Last updated: YYYY-MM-DD`
   - `Owners: ...`
   - `Scope: ...`
   - `Primary references:` when applicable
3. `## 0. Implementation Status`
   - dated status notes,
   - what is implemented,
   - what remains open,
   - native/WASM impact for client/runtime features.
4. Contract/current behavior sections before proposed future work.
5. Validation/testing references near the end when the feature has enforceable behavior.

Preferred status labels:

1. `Active implementation contract`
2. `Active feature reference`
3. `Active partial implementation spec`
4. `Proposed feature contract`
5. `Deferred`
6. `Superseded`

## What Belongs Here

1. Active implementation contracts:
   - `asset_delivery_contract.md`
   - `visibility_replication_contract.md`
   - `tactical_and_owner_lane_protocol_contract.md`
   - `audio_runtime_contract.md`
   - `scripting_support.md`
   - `projectile_firing_game_loop.md`
2. Active feature references:
   - `asset-packs.md`
   - `brp_debugging_workflow.md`
   - `galaxy_world_structure.md`
   - `procedural_asteroids.md`
   - `procedural_planets.md`
   - `prediction_runtime_tuning_and_validation.md`
   - `lightyear_upstream_issue_snapshot.md`
3. Active feature-scoped notes/specs:
   - `dashboard_route_shell_refactor_note.md`
   - `account_character_selection_layout_contract.md`
   - `genesis_planet_registry_contract.md`
   - `asteroid_field_system_v2.md`
   - `shader_editor_dashboard_implementation_spec.md`
   - `lightyear_integration_analysis.md`
4. Proposed feature contracts:
   - `asteroid_field_system.md`
   - `background_world_simulation_contract.md`
   - `fly_by_wire_thrust_allocation_contract.md`
   - `resources_and_crafting_contract.md`
   - `visibility_system_v2_signal_detection_contract.md`

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

# Documentation Reconciliation Audit Report

Date: 2026-03-09
Prompt source: `docs/prompts/documentation_reconciliation_audit_prompt.md`
Scope: Repository documentation structure, top-level authority docs, feature/decision docs, implementation trackers, and historical notes.
Limitations: Static audit only. This report is based on current repository contents and targeted code/document cross-checking, not a full line-by-line runtime verification of every feature doc.

## 1. Executive Summary

The documentation set is no longer mainly suffering from "missing docs." It is suffering from taxonomy drift.

The most important problems are:

1. `docs/features/` is no longer a coherent folder. It mixes active source-of-truth contracts, accepted decision detail files, implementation plans, partial implementation specs, historical rollout notes, and one-line scratch files.
2. `docs/features/README.md` is stale and misleading. It still describes the folder as mainly planning docs and references files that do not exist.
3. `docs/decision_register.md` and `AGENTS.md` still normalize DR files under `docs/features/`, while the current prompt direction wants a cleaner `docs/decisions/` split. The docs are not aligned on the intended long-term decision taxonomy.
4. `README.md` is too thin to serve as a real project entrypoint.
5. Several historical or partially superseded docs still sit in active locations without strong "historical", "implemented reference", or "superseded" labeling.

The top-level design doc is in much better shape than the rest. The biggest cleanup need is not rewriting `docs/sidereal_design_document.md` from scratch. It is making the surrounding doc tree match the roles that top-level docs are already trying to play.

## 2. Documentation Structure Problems

### Finding S1: `docs/features/` is a mixed-content folder with no stable taxonomy
- Severity: Critical
- Files:
  - `docs/features/README.md`
  - `docs/features/asset_delivery_contract.md`
  - `docs/features/visibility_replication_contract.md`
  - `docs/decisions/dr-0001_account_character_session_model.md`
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
  - `docs/plans/visibility_opt_in_interest_management_plan.md`
  - `docs/plans/generic_visibility_range_migration_plan.md`
  - `docs/plans/shader_editor_dashboard_plan.md`
  - `docs/features/shader_editor_dashboard_implementation_spec.md`
- What is wrong:
  The folder contains at least six different document types:
  1. active implementation contracts,
  2. accepted DR detail docs,
  3. proposed plans,
  4. partially implemented specs,
  5. implemented reference notes,
  6. historical rollout notes.
  That makes discovery, ownership, and update discipline weak.
- What should happen:
  split
- Short rationale:
  Contributors cannot tell from the folder alone which docs are authoritative, which are planning-only, and which are historical.

### Finding S2: `docs/features/README.md` no longer describes reality
- Severity: High
- Files:
  - `docs/features/README.md`
- What is wrong:
  It describes the folder as "Features Planning Docs" and says these are planning references except for a couple of contracts. That is false now. It also lists missing files such as:
  - `bevy_features.md`
  - `universe_building_plan.md`
  - `quickstart_first_solar_system.md`
- What should happen:
  rewrite
- Short rationale:
  The folder index is currently one of the most misleading docs in the repository.

### Finding S3: The repo has no explicit home for historical docs, but multiple docs already behave as history/reference notes
- Severity: Medium
- Files:
  - `docs/plans/visibility_opt_in_interest_management_plan.md`
  - `docs/plans/generic_visibility_range_migration_plan.md`
  - `docs/lightyear_handoff_debug_summary.md`
  - `docs/reports/*.md`
  - `docs/sidereal_implementation_checklist.md`
- What is wrong:
  Several docs are clearly historical or postmortem/reference material, but they live beside active contracts or active top-level docs. The checklist even still references `docs/archive/`, which does not exist.
- What should happen:
  move
- Short rationale:
  History should be preserved, but it should not live in the same surface area as active operational contracts without strong separation.

## 3. Stale / Incorrect Docs

### Finding T1: `docs/features/README.md` contains broken file references
- Severity: High
- Files:
  - `docs/features/README.md`
- What is wrong:
  It references files that do not exist:
  - `bevy_features.md`
  - `universe_building_plan.md`
  - `quickstart_first_solar_system.md`
- What should happen:
  rewrite
- Short rationale:
  A folder index with broken references is actively harmful.

### Finding T2: `docs/decision_register.md` contains broken references to a missing 2D migration plan and decision doc
- Severity: High
- Files:
  - `docs/decision_register.md`
- What is wrong:
  DR-0014 references:
  - `docs/features/2d_migration_plan.md`
  - `docs/decisions/dr-0014_2d_runtime_migration.md`
  Neither file exists.
- What should happen:
  rewrite
- Short rationale:
  Broken decision references make it unclear whether the decision is stale, missing its detail doc, or superseded by later work.

### Finding T3: `docs/sidereal_implementation_checklist.md` still describes older runtime gaps and old folder conventions
- Severity: High
- Files:
  - `docs/sidereal_implementation_checklist.md`
  - `bins/sidereal-client/src/platform/wasm.rs`
- What is wrong:
  The checklist still contains items such as:
  - "Remove temporary WASM scaffold-only runtime"
  - "Keep old audits historical only under `docs/archive/`"
  The current WASM entry now boots the shared windowed client app shell (`bins/sidereal-client/src/platform/wasm.rs`), and the repo now uses `docs/reports/`, not `docs/archive/`.
- What should happen:
  rewrite
- Short rationale:
  The checklist is still useful, but some entries now describe work that has already moved or changed shape.

### Finding T4: Historical reports in `docs/reports/` still point at the old prompt path
- Severity: Low
- Files:
  - `docs/reports/rust_codebase_audit_report_2026-03-07.md`
  - `docs/reports/rust_codebase_audit_report_2026-03-08.md`
  - `docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-09.md`
- What is wrong:
  Their `Prompt source` lines still reference `docs/audits/...`, which is no longer the active prompt location.
- What should happen:
  keep
- Short rationale:
  These are historical reports. Do not rewrite them casually, but the docs tree should clearly treat them as historical artifacts.

### Finding T5: `docs/lightyear_handoff_debug_summary.md` reads like an active debugging handoff, not a classified historical note
- Severity: Medium
- Files:
  - `docs/lightyear_handoff_debug_summary.md`
- What is wrong:
  It documents an unresolved debugging state that predates later Lightyear/frame interpolation changes and reads as an active operational note.
- What should happen:
  move
- Short rationale:
  This should either be archived as a dated handoff note or replaced by a short summary linked from a historical section.

## 4. Duplicate / Merge Candidates

### Finding D1: The shader-workbench docs overlap heavily and should be rationalized
- Severity: High
- Files:
  - `docs/plans/shader_editor_dashboard_plan.md`
  - `docs/features/shader_editor_dashboard_implementation_spec.md`
  - `docs/plans/shader_editor_wgsl_linting_and_diagnostics_plan.md`
  - `docs/features/dashboard_route_shell_refactor_note.md`
- What is wrong:
  These docs cover overlapping dashboard/shader authoring concerns with different statuses:
  plan, implementation spec, linting plan, and refactor note.
- What should happen:
  merge
- Short rationale:
  This area should likely become:
  1. one active implementation/spec doc,
  2. one optional future tooling roadmap,
  3. one archived historical note if needed.

### Finding D2: Visibility docs are more disciplined than most areas, but still spread across contract + supplemental plan + historical rollout notes
- Severity: Medium
- Files:
  - `docs/features/visibility_replication_contract.md`
  - `docs/plans/scan_intel_minimap_spatial_plan.md`
  - `docs/plans/visibility_opt_in_interest_management_plan.md`
  - `docs/features/tactical_and_owner_lane_protocol_contract.md`
- What is wrong:
  The canonical visibility contract exists, but some policy, scaling, and rollout content is split across documents with mixed statuses.
- What should happen:
  split
- Short rationale:
  The contract should stay authoritative; supplemental and historical docs should be clearly labeled and grouped around it.

### Finding D3: Scripting documentation mixes active contract and implementation plan in one giant document
- Severity: High
- Files:
  - `docs/features/scripting_support.md`
- What is wrong:
  The title and status line explicitly say "Active contract and implementation plan". That is exactly the mixed-role pattern this audit is supposed to remove.
- What should happen:
  split
- Short rationale:
  This should become:
  1. an active scripting contract/runtime doc,
  2. one or more separate future implementation/tooling plans.

## 5. Decision Register Audit

### Finding R1: The decision register and the desired decision taxonomy are not aligned
- Severity: High
- Files:
  - `docs/decision_register.md`
  - `AGENTS.md`
  - `docs/prompts/documentation_reconciliation_audit_prompt.md`
- What is wrong:
  `docs/decision_register.md` and `AGENTS.md` both normalize DR detail docs under `docs/features/`, while the current prompt direction says the audit should evaluate moving DR files under `docs/decisions/`.
- What should happen:
  rewrite
- Short rationale:
  The repo needs one decision taxonomy. Right now it has two competing ones.

### Finding R2: Several entries in `docs/decision_register.md` still rely on old status vocabulary or missing follow-through docs
- Severity: Medium
- Files:
  - `docs/decision_register.md`
- What is wrong:
  The register uses `Proposed | Accepted | Superseded | Deprecated`, but the current audit prompt expects a richer status model. More importantly, some entries reference missing docs or plans.
- What should happen:
  relabel status
- Short rationale:
  The issue is not just vocabulary polish; it is that status and linked detail docs are not uniformly reliable.

### Finding R3: Accepted DR detail docs are currently mixed into `docs/features/` beside non-decision plans
- Severity: Medium
- Files:
  - `docs/decisions/dr-0001_account_character_session_model.md`
  - `docs/decisions/dr-0025_runtime_script_catalog_authority.md`
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
  - `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`
- What is wrong:
  These are legitimate decision detail docs, but they live beside implementation plans and partial specs with no folder-level distinction.
- What should happen:
  move
- Short rationale:
  If the project wants `docs/decisions/`, these are prime candidates. If it wants to keep DRs in `docs/features/`, then the prompt, register, and AGENTS need to stop implying otherwise.

## 6. Implemented Systems Missing Good Docs

### Finding M1: `README.md` is too thin to explain what the project currently supports
- Severity: High
- Files:
  - `README.md`
- What is wrong:
  The README only gives a one-line summary plus quick-start commands. It does not explain:
  - what Sidereal currently does,
  - what a developer can test now,
  - the key workspace services,
  - the major technologies in a useful way,
  - where to go next in the docs.
- What should happen:
  rewrite
- Short rationale:
  The README is currently a build note, not a real entrypoint.

### Finding M2: There is no current docs index for reports / historical audits / active contracts
- Severity: Medium
- Files:
  - `docs/`
  - `docs/features/README.md`
- What is wrong:
  There is no current root-level documentation index that says:
  - start here,
  - active contracts live here,
  - decisions live here,
  - reports/history live here.
- What should happen:
  split
- Short rationale:
  The tree needs a small navigation layer.

## 7. Future Features / Tooling That Should Be Clearly Isolated

### Finding F1: Several plan docs are fine to keep, but they should be isolated as future/planned work rather than co-located with active contracts
- Severity: Medium
- Files:
  - `docs/plans/advanced_fly_by_wire_and_thruster_allocation_plan.md`
  - `docs/plans/robust_weapons_combat_audio_system_plan.md`
  - `docs/plans/windows_ingame_console_plan.md`
  - `docs/plans/test_topology_and_resilience_plan.md`
  - `docs/plans/shader_editor_wgsl_linting_and_diagnostics_plan.md`
- What is wrong:
  These are future/planned docs, but the current folder structure does not isolate them from active operational contracts.
- What should happen:
  move
- Short rationale:
  Future work should remain, but it should be obviously future at the folder level, not only inside the file header.

### Finding F2: Some "plan" docs are now more like implemented reference notes and should be relabeled or moved out of the future/plans bucket
- Severity: Medium
- Files:
  - `docs/plans/generic_visibility_range_migration_plan.md`
  - `docs/features/dashboard_route_shell_refactor_note.md`
  - `docs/features/projectile_firing_game_loop.md`
  - `docs/plans/thruster_plumes_afterburner_plan.md`
- What is wrong:
  These documents already describe landed or partially landed work, but their names and folder placement still imply open implementation plans.
- What should happen:
  rename
- Short rationale:
  Keep the content, but stop labeling implemented reference docs as future work.

## 8. Top-Level Docs Audit

### 8.1 `docs/sidereal_design_document.md`

Assessment:
- Keep.

Strengths:
- It is already acting like a real high-level architecture and gameplay document.
- It separates hard rules, runtime architecture, gameplay model, timing, visibility, and client/runtime conventions reasonably well.
- It links outward to more detailed docs.

Remaining issue:
- It still embeds some current-vs-target and planned-direction detail that may be better pushed into subordinate docs over time, but this is a secondary problem.

Recommended action:
- keep
- light rewrite only where surrounding docs are cleaned up and links/status wording need adjustment

### 8.2 `docs/decision_register.md`

Assessment:
- Keep, but rewrite structurally.

Strengths:
- It is already the right place for cross-cutting decisions.
- Many entries are useful and specific.

Problems:
- mixed status discipline,
- broken references,
- unresolved folder taxonomy for DR detail docs,
- some entries still point to plans that no longer exist.

Recommended action:
- rewrite

### 8.3 `AGENTS.md`

Assessment:
- Keep, but align after the docs taxonomy is settled.

Strengths:
- It is operational and enforceable.
- It already points at the major source-of-truth docs.

Problems:
- It currently codifies `docs/decisions/dr-XXXX_*.md` as the decision-detail location.
- If the repo decides to move DRs to `docs/decisions/`, `AGENTS.md` must change in the same cleanup.

Recommended action:
- keep
- update once the decision/feature split is finalized

### 8.4 `README.md`

Assessment:
- Rewrite.

Recommended contents:
1. project overview,
2. current runtime slice,
3. workspace/service map,
4. quick start,
5. where to read next,
6. non-overcommitted near-term roadmap.

## 9. Recommended Doc Taxonomy / Folder Structure

Recommended target:

- `docs/sidereal_design_document.md`
  - authoritative high-level architecture and gameplay overview
- `docs/decision_register.md`
  - authoritative decision ledger and index
- `docs/decisions/`
  - decision detail docs only
- `docs/contracts/` or keep `docs/features/` but only for active implementation/runtime contracts
  - examples:
    - visibility
    - asset delivery
    - tactical/owner lane protocol
    - scripting runtime contract
- `docs/plans/`
  - future implementation plans and deferred work
- `docs/history/` or `docs/archive/`
  - historical rollout notes, migration references, debugging handoffs, superseded plans
- `docs/reports/`
  - audit reports and generated review artifacts
- `docs/samples/`
  - sample/reference material only

If the project does not want to introduce `docs/contracts/`, then keep `docs/features/` but narrow it strictly to active contracts and move plans/history elsewhere.

## 10. Concrete Cleanup Plan

### Priority 1

1. Rewrite `docs/features/README.md` into a real folder index or remove it and replace it with a root docs index.
2. Fix broken references in `docs/decision_register.md`, especially DR-0014.
3. Rewrite `docs/sidereal_implementation_checklist.md` so it matches current runtime state and current folder conventions.
4. Rewrite `README.md` into a real project entrypoint.

### Priority 2

1. Decide the DR taxonomy:
   either keep DR detail docs in `docs/features/` and align all docs to that,
   or move them to `docs/decisions/` and update `docs/decision_register.md` + `AGENTS.md` together.
2. Separate active contracts from plans/history at the folder level.
3. Classify and move historical/reference notes such as:
   - `visibility_opt_in_interest_management_plan.md`
   - `generic_visibility_range_migration_plan.md`
   - `lightyear_handoff_debug_summary.md`

### Priority 3

1. Split `docs/features/scripting_support.md` into contract vs plan material.
2. Rationalize the shader editor/dashboard doc cluster into one primary active spec plus isolated future plans.
3. Rename implemented-reference docs that still read like future plans.

### Priority 4

1. Add a root-level docs navigation doc or expand `README.md` with a "Docs Map" section.
2. Standardize status vocabulary across docs:
   - active contract
   - accepted decision
   - implementation plan
   - partial implementation spec
   - historical reference
   - superseded
3. Add a lightweight maintenance rule: every doc in `docs/features/`, `docs/plans/`, or equivalent must declare one of those statuses in the header.

## 11. Bottom Line

The docs are not fundamentally broken because the project lacks documentation. They are drifting because too many different kinds of documentation now live in the same places with stale labels.

The right next move is structural cleanup, not mass deletion: keep the good design doc and decision ledger, make the folder taxonomy honest, fix broken references, and isolate plans/history from current contracts. That will remove most of the confusion without forcing a full-documentation rewrite all at once.

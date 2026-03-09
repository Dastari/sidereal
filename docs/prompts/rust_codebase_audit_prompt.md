# Rust Codebase Audit Prompt

Perform a complete Rust codebase audit for this repository as a senior Bevy/Avian2D/Lightyear/server-authoritative multiplayer engineer.

## Context

- This is Sidereal, a server-authoritative multiplayer game/framework being rebuilt from scratch.
- Architecture and contributor rules are documented in:
  - `docs/sidereal_design_document.md`
  - `docs/decision_register.md`
  - `docs/features/visibility_replication_contract.md`
  - `docs/features/asset_delivery_contract.md`
  - `docs/features/scripting_support.md`
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
  - `AGENTS.md`
- The long-term direction is a more generic engine/runtime where game/content specifics are increasingly authored in Lua rather than hardcoded in Rust.
- Client uses Bevy.
- Physics uses Avian2D.
- Networking/prediction uses Lightyear.
- Gateway, replication server, persistence, scripting, asset delivery, and client are all in the same workspace.

## Audit Goals

Find and document all meaningful issues, risks, dead code, unnecessary complexity, and missed opportunities across the codebase.

Audit for all of the following:

1. Any antipatterns, even if some are already covered by clippy.
2. Any unnecessary Bevy systems, resources, entities, or components.
3. Any hardcoded “space game” assumptions that should be generalized or moved into Lua/script-authored content.
4. Any areas where systems should be regrouped into clearer plugins/modules for a cleaner codebase.
5. Any areas where shims, hacks, or patch-style fixes were introduced instead of solving the underlying architectural problem.
6. Any places where coding style/function shape/API usage differs enough that the codebase looks inconsistent or authored by many unrelated people.
7. Any places where more modern/current Bevy functionality should be used.
8. Any places where the codebase can be materially simplified, reduced, or optimized, including selective use of small external crates if justified.
9. Any places where large dependencies are used for a tiny/single-use purpose that should instead be replaced by a small local utility.
10. Any places where physics or Avian2D are used incorrectly, incompletely, or against their strengths.
11. Any places where Lightyear is underused, misused, or configured in a way that could hurt:
   - client prediction
   - reconciliation
   - rollback
   - interpolation
   - server authority
   - perceived motion quality
12. Any places where systems/processes/events/queries/ticks are being spammed or run unconstrained without need.
13. Any places where systems or entity/component mutations are duplicated across multiple areas and could conflict or cause regressions.
14. Any missing hook points or extension points that should exist to better support Lua scripting/content authoring.
15. Any Bevy client rendering paths that are likely sub-optimal for runtime performance.
16. Any TODOs, FIXMEs, placeholders, “temporary” paths, partial migrations, or documented-but-unimplemented features.
17. Any areas where large amounts of code now serve little or no purpose and should be deleted.
18. Any legacy systems or compatibility layers that are still present, unused, or accidentally still running.

## Additional Required Output

After the audit findings, provide a clear operational flow description for:

1. Gateway startup and main loop
2. Replication server startup and main loop
3. Client startup and main loop
4. How data, authority, persistence, replication, asset delivery, scripting, and rendering flow between them

Also provide, at the end of the audit report, a catalog appendix covering:

1. All workspace binaries/crates/libraries and what each is responsible for.
2. All major Bevy plugins, systems, and resources in active runtime paths and what each is responsible for.
3. Be explicit about whether an item is:
   - active runtime,
   - test-only,
   - tooling-only,
   - scaffold/placeholder,
   - likely transitional/migration code.
4. For Bevy systems/resources, group by runtime/service where possible rather than dumping a flat list.

## Output Requirements

- Produce the results as a formal audit document.
- Prioritize findings by severity:
  - Critical
  - High
  - Medium
  - Low
- For each finding include:
  - title
  - severity
  - why it matters
  - exact file/path references
  - concrete recommendation
  - whether it is a cleanup, correctness, performance, architecture, or maintainability issue
- Be explicit when something is an inference rather than directly proven.
- Distinguish between:
  - must fix
  - should fix
  - optional improvement
- Call out places where docs and code diverge.
- Call out places where the current architecture is defensible and should be kept.
- Do not give shallow advice; be specific to this codebase.
- Assume the reader wants blunt, technically rigorous feedback, not reassurance.

## Suggested Structure

1. Executive Summary
2. Architecture Findings
3. Bevy/ECS Findings
4. Rendering Findings
5. Physics/Avian2D Findings
6. Networking/Lightyear Findings
7. Scripting/Lua Findings
8. Persistence/Data Flow Findings
9. Redundancy/Dead Code Findings
10. Startup/Main Loop Flow Maps
11. Prioritized Remediation Plan
12. Workspace / Runtime Catalog Appendix

## Deliverable

Write the final report to:

- `docs/reports/rust_codebase_audit_report_YYYY-MM-DD.md`

Do not write the completed report anywhere else. The final report belongs in `docs/reports/`.

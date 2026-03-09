# Documentation Reconciliation Audit Prompt

Perform a complete documentation audit and reconciliation pass for this repository.

You are auditing the docs as if the current codebase is the source of truth for all implemented systems, except for clearly unimplemented future gameplay systems and future tooling/editor workflows.

## Core Assumption

- Apart from future gameplay systems and future tools/editor work, the project is now broadly in the state we want.
- Therefore, any documentation that disagrees with the implemented code is likely stale, misleading, duplicated, or wrong and should be corrected, merged, or removed.
- The goal is to make the docs reflect reality cleanly and rigorously.

## Repository-Specific Source-of-Truth Expectations

### `docs/sidereal_design_document.md`
This should become and remain the authoritative top-level project document.

It should:
1. state the aim of the game/project,
2. state the major technologies used,
3. describe the high-level architecture and supported runtime loops,
4. describe the current intended game loops,
5. describe future gameplay directions and future tooling directions at a high level,
6. link outward to detailed implementation/feature documents,
7. avoid deep implementation detail itself.

It should function as the top-level orientation and authoritative overview document, not as a low-level implementation dump.

### `docs/decision_register.md`
This should be the authoritative decision register.

Decision record files should preferably live under `docs/decisions/` rather than `docs/features/`. The audit should explicitly evaluate and likely recommend migrating DR files to `docs/decisions/dr-xxxx_*.md` so decision records and feature/runtime contracts are clearly separated.

It should:
1. contain decision records we are still evaluating, have completed, have rejected, or have superseded,
2. never silently lose history,
3. clearly mark every decision with an explicit status such as:
   - proposed
   - in progress
   - accepted
   - completed
   - rejected
   - superseded
   - diverged / replaced
4. link to any relevant implementation or feature docs,
5. not act as a second feature-spec directory.

Do not delete historical decisions, but ensure each one is clearly labeled and linked to the current reality.

### `AGENTS.md`
This must stay current and operational.

It should:
1. link to all relevant source-of-truth docs,
2. define the rules agents must follow,
3. reflect current architectural reality,
4. not point agents at stale or superseded docs without context,
5. be updated whenever documentation authority or contributor rules change.

### `README.md`
This should be the concise public/project entrypoint.

It should include:
1. a brief overview of the game/project,
2. what the game currently supports / what a user or developer can do now,
3. a high-level list of supported current features,
4. an intentionally non-overcommitted roadmap / future direction section,
5. quick clone/build/run instructions,
6. the major technologies used.

It should not try to duplicate the full design document.

## Primary References

- `docs/sidereal_design_document.md`
- `docs/decision_register.md`
- `docs/features/`
- `README.md`
- `AGENTS.md`

## Hard Exclusions

The audit may read `docs/prompts/`, `docs/reports/`, and `docs/samples/` for context if needed, but it must never modify, rewrite, move, delete, or create files in:

- `docs/prompts/`
- `docs/reports/`
- `docs/samples/`

Those folders are out of scope for documentation reconciliation edits.

## Audit Objectives

Review the entire documentation set and identify all places where documentation should be:
1. cleaned up,
2. merged,
3. reconciled with current implementation,
4. split more clearly between:
   - implemented behavior,
   - decisions/invariants,
   - future features,
   - future tooling,
   - historical notes.

## Look For All of the Following

1. Any docs that are stale, wrong, or contradicted by the current codebase.
2. Any implementation plans that have effectively already been implemented and should no longer exist as “plans”.
3. Any “future” sections that actually describe current behavior and should be moved into implemented contracts/docs.
4. Any docs that mix:
   - decisions
   - feature contracts
   - implementation plans
   - future ideas
   in a confusing way.
5. Any places where “feature docs” are actually decision docs and should be moved to the decision register / DR files.
6. Any DR/decision files that are no longer real decisions, are superseded, or are duplicated elsewhere.
7. Any DR files currently under `docs/features/` that should be moved to `docs/decisions/`.
8. Any decisions that are implemented in code but not clearly documented.
9. Any implemented systems that are insufficiently documented.
10. Any duplicate docs covering the same area with conflicting or overlapping guidance.
11. Any “temporary migration notes” or transitional wording that should now be removed.
12. Any docs that still describe deprecated architecture or legacy system paths.
13. Any places where documentation should explicitly mark:
   - implemented
   - partial
   - future
   - obsolete
14. Any areas where the documentation structure itself should be reorganized for clarity.
15. Any places where “features” and “decisions” are currently muddled together and should be separated.
16. Any documents that should be deleted outright.
17. Any missing top-level index/navigation links that would make the docs easier to maintain.
18. Any places where `docs/sidereal_design_document.md`, `docs/decision_register.md`, `AGENTS.md`, and `README.md` are not serving the right role and should be rewritten accordingly.

## Required Outputs

Produce a formal documentation audit report that includes:

1. Executive Summary
2. Documentation Structure Problems
3. Stale / Incorrect Docs
4. Duplicate / Merge Candidates
5. Decision Register Audit
6. Implemented Systems Missing Good Docs
7. Future Features / Tooling That Should Be Clearly Isolated
8. Top-Level Docs Audit (`sidereal_design_document.md`, `decision_register.md`, `AGENTS.md`, `README.md`)
9. Recommended Doc Taxonomy / Folder Structure
10. Concrete Cleanup Plan

For each finding include:
- title
- severity:
  - Critical
  - High
  - Medium
  - Low
- exact file references
- what is wrong
- what should happen:
  - keep
  - rewrite
  - split
  - merge
  - move
  - rename
  - delete
  - mark as future only
  - convert into DR
  - convert out of DR
  - relabel status
- short rationale

## Important Guidance

- Treat current code behavior as authoritative unless a doc is clearly intended to describe an unimplemented future feature/tool.
- Be strict. If a plan doc is now mostly implemented, say it should be rewritten into a reality/contract doc.
- If two docs overlap, recommend a single source of truth.
- If a DR is no longer a real decision, say so.
- If a feature is implemented but poorly documented, call that out explicitly.
- If a future gameplay/tooling idea should remain, require it to be clearly marked as future and not mixed into current runtime contracts.
- Do not delete decision history, but do require statuses and links to reflect reality.
- Ensure the final recommendations keep `AGENTS.md` operational and aligned with the docs hierarchy.

## Recommended Target Documentation Model

The audit should evaluate and, if appropriate, recommend a structure like:

- `docs/sidereal_design_document.md`
  - authoritative high-level project overview and architecture map
- `docs/decision_register.md`
  - authoritative index/status ledger for decisions
- `docs/decisions/dr-xxxx_*.md`
  - decision records only
- `docs/features/`
  - current implemented feature/runtime contracts
- clearly isolated future docs/sections
  - unimplemented future gameplay systems
  - future editor/tooling work
- `README.md`
  - concise project overview and run instructions
- `AGENTS.md`
  - contributor/agent operational rules and doc links

## Final Requirement

End with a prioritized step-by-step documentation cleanup sequence that can be executed in order with minimal confusion and minimal churn.

## Deliverable

Write the final report to:

- `docs/reports/documentation_reconciliation_audit_report_YYYY-MM-DD.md`

Do not write the completed report anywhere else. The final report belongs in `docs/reports/`.

# Nameplate System Redesign Plan

Status: Proposed redesign plan  
Date: 2026-03-14  
Owners: client UI/HUD + visibility/replication + gameplay disclosure

Primary references:
- `AGENTS.md`
- `docs/features/visibility_replication_contract.md`
- `docs/plans/bevy_2d_rendering_optimization_completion_plan_2026-03-12.md`
- `docs/reports/native_runtime_system_ownership_audit_2026-03-09.md`

Update note (2026-03-14):
- This document is the active redesign plan for the in-world nameplate system.
- The current implementation has shown both lifecycle leaks and poor scaling because it spawns and reconciles large Bevy UI subtrees per target.
- The replacement must be client-visibility-scoped, disclosure-aware, and robust under duplicate-winner churn, partial replication, and target data loss.
- This is intentionally written as a feature-plan scaffold with explicit placeholders so later implementation details can be added without replacing the core contract.

## 1. Purpose

Replace the current nameplate implementation with a bounded, disclosure-aware client presentation system that:

1. never leaks entities during steady-state churn,
2. scales with visible targets rather than historical target churn,
3. respects server-authoritative visibility and disclosure limits,
4. supports partial target information such as unknown health or jammed scan data,
5. remains compatible with the current duplicate-winner and native/WASM shared client architecture.

This is not only a performance fix. It is a presentation contract redesign.

## 2. Problem Statement

The current nameplate system has several structural flaws:

1. It spawns a full Bevy UI subtree per target.
2. It reconciles by spawn/despawn against a dynamic target set instead of owning a bounded presentation pool.
3. It assumes full target health disclosure when `HealthPool` is present instead of modeling what the client is actually allowed to know.
4. It couples target selection, disclosure assumptions, UI lifecycle, and screen projection too tightly.
5. It is too fragile under duplicate predicted/confirmed entity churn and state-scoped teardown.

The recent idle-entity-growth issue is a symptom of this architecture, not the only bug.

## 3. Non-Negotiable Constraints

The redesign must preserve these project rules:

1. Server authority remains one-way: the client may only present data it already received.
2. Nameplates are a client presentation product, not an authoritative gameplay system.
3. Nameplate eligibility must derive from current client-visible winner entities, not from hidden or stale server-only state.
4. The system must not infer restricted gameplay data from absence/presence patterns in unrelated components.
5. Native and WASM clients should continue sharing the same nameplate logic unless a platform boundary clearly requires otherwise.
6. The design must remain compatible with Avian2D transforms and the current `PostUpdate` projection ordering.
7. Visibility-sensitive behavior must remain consistent with `Authorization -> Delivery -> Payload`.

## 4. Scope

This redesign covers:

1. in-world target nameplate selection,
2. disclosure-aware nameplate content composition,
3. screen-space/world-space projection and visibility culling,
4. bounded lifecycle/pooling of nameplate presentation entries,
5. instrumentation and regression tests,
6. migration away from the current per-target subtree model.

This redesign does not yet define:

1. final artistic styling,
2. tactical-map marker redesign,
3. faction-specific cosmetic variants,
4. advanced text/icon grammar beyond the minimum disclosure contract.

## 5. Design Goals

### 5.1 Functional Goals

1. Show a stable nameplate for each currently eligible winner entity.
2. Allow per-field disclosure such as:
   - display name known,
   - affiliation known,
   - health unknown,
   - target class unknown,
   - target only detectable as a generic contact.
3. Hide or degrade fields cleanly when scan/jam/disclosure changes at runtime.
4. Recover correctly when the winning duplicate entity changes.
5. Cleanly remove presentation when the target leaves client visibility or disclosure no longer permits a plate.

### 5.2 Performance Goals

1. Zero unbounded ECS growth during idle or churn.
2. Stable-frame work should be mostly updates to existing entries, not spawn/despawn churn.
3. Nameplate cost should scale with active visible targets, not total historical targets.
4. The design should support explicit budgets and graceful degradation when target counts are high.

### 5.3 Debuggability Goals

1. It must be easy to tell why a plate exists or does not exist.
2. It must be easy to tell which disclosure level a plate is using.
3. Debug counters should distinguish:
   - target-set size,
   - active pooled entries,
   - hidden-by-disclosure,
   - hidden-by-viewport,
   - hidden-by-budget,
   - content updates,
   - layout/projection updates.

## 6. Target Architecture

The new design should be split into four layers.

### 6.1 Layer A: Eligibility

Responsibility:

1. determine which current client-side winner entities are allowed to produce a plate at all,
2. determine the coarse disclosure tier for each candidate.

Inputs:

1. duplicate-winner resolution,
2. current client-visible entities,
3. replicated disclosure/intel components,
4. local HUD enable state,
5. optional player settings and visible-count budgets.

Output:

1. `NameplateTargetSet` resource or equivalent derived cache containing only canonical eligible targets.

### 6.2 Layer B: Disclosure Model

Responsibility:

1. convert visible replicated target data into a disclosure-safe view model,
2. explicitly represent unknown/withheld fields instead of assuming full data.

Output:

1. `NameplateViewModel` per target.

The view model should be the only input used by the rendering/presentation layer.

### 6.3 Layer C: Presentation Registry and Pool

Responsibility:

1. own a bounded set of plate entries,
2. map `target_entity -> plate_entry`,
3. update existing entries in place,
4. reclaim entries immediately when targets leave the set.

This layer must be the sole owner of nameplate lifecycle.

### 6.4 Layer D: Layout and Rendering

Responsibility:

1. project targets after final transforms and camera state settle,
2. update screen position, visibility, and content deltas,
3. render the current active entries.

This layer should not decide disclosure or target eligibility.

## 7. Disclosure-Aware Data Model

The redesign must stop treating “component replicated” as the same thing as “field should be displayed.”

### 7.1 Required Concept

Introduce a client-side nameplate disclosure model with explicit tiers.

Example tiers:

1. `None`
   - no plate should render.
2. `ContactOnly`
   - generic contact marker only, no label text, no health.
3. `BasicIdentity`
   - display a generic or resolved name/class/faction marker, no health.
4. `CombatSummary`
   - display identity plus coarse health state or bar if permitted.
5. `Full`
   - display all currently supported plate fields.

Final naming can change later, but the concept should remain explicit.

### 7.2 Unknown vs Hidden

The view model must distinguish:

1. field unknown because client never received it,
2. field hidden because disclosure policy forbids it,
3. field intentionally suppressed because of jamming/spoofing,
4. field stale and no longer considered live.

The UI should not collapse all of these into the same branch unless content design chooses to.

### 7.3 Health and Jam-Aware Contract

The nameplate system must support cases where:

1. the target itself is visible but health is not disclosed,
2. health was disclosed previously but is now withheld,
3. a jammer or stealth effect degrades the plate from detailed to generic,
4. only a contact-class marker should remain visible.

The health bar therefore must be optional and disclosure-driven, not structurally assumed.

## 8. Recommended ECS Shape

The exact type names may change, but the architecture should converge toward:

1. `NameplateTargetSet`
   - derived per-frame or per-change cache of eligible canonical targets.
2. `NameplateViewModelCache`
   - per-target disclosure-safe content snapshot.
3. `NameplateRegistry`
   - owns active plate entries and free-list/pool state.
4. `NameplateEntry`
   - lightweight runtime record tying target entity to the presentation entities or draw handles.
5. `NameplateBudgetSettings`
   - max visible plates, distance thresholds, degradation policy.

The new design should avoid storing the target set only in the ECS graph as spawned plate entities.

## 9. Presentation Strategy

### 9.1 Preferred Direction

Move away from a large Bevy UI subtree per target.

Recommended direction:

1. one persistent root overlay container,
2. a bounded pool of reusable plate entries,
3. minimal per-entry node count,
4. prefer simpler renderables for bars/backplates where practical,
5. keep text/icons optional and lightweight.

### 9.2 Acceptable Implementation Shapes

The implementation may use one of these approaches:

1. pooled Bevy UI entries under one root,
2. a hybrid model:
   - text in UI,
   - bars/backplates in screen-space sprites/meshes,
3. a custom batched overlay/material path for large visible counts.

The first implementation should choose the smallest-risk path, but the plan should not lock us into the current subtree-heavy design.

### 9.3 Lifecycle Rules

1. Nameplate entries must be reused, not recreated every churn event.
2. Entry removal must be deterministic and immediate.
3. State-scoped teardown should be a coarse safety net, not the normal lifecycle mechanism.
4. Pool ownership must be centralized in one system/resource.

## 10. System Pipeline

The replacement should separate systems by responsibility.

Recommended runtime order:

1. `collect_nameplate_targets_system`
   - derive eligible canonical target set from winner entities and client-visible state.
2. `build_nameplate_view_models_system`
   - convert target data into disclosure-safe plate models.
3. `reconcile_nameplate_pool_system`
   - diff target set against pool and acquire/release entries.
4. `project_nameplate_layout_system`
   - in `PostUpdate`, after final transforms and camera propagation.
5. `update_nameplate_visuals_system`
   - apply content/layout deltas only to active entries.
6. `emit_nameplate_metrics_system`
   - snapshot counters for debug overlay and BRP inspection.

The pool reconciliation system should be the only place that allocates or frees presentation entries during steady-state runtime.

## 11. Visibility and Duplicate-Winner Integration

The nameplate system must integrate with the current client entity model explicitly.

### 11.1 Canonical Target Source

Nameplates must attach to the same canonical winner selection used by client visuals.

Rules:

1. only the current winner for a duplicated GUID may produce a plate,
2. loser duplicates must never each create their own plate,
3. winner swaps should update an existing plate entry in place where possible.

### 11.2 Visibility Scope

Plates should only be considered for entities the client is currently authorized and delivered to know about.

The design must not create plates from:

1. stale cached entities no longer in visibility,
2. owner-debug-only information that ordinary presentation should not use,
3. hidden duplicates,
4. inferred entities from tactical-only or unrelated systems unless explicitly approved later.

## 12. Content Contract

The exact visual design can evolve, but the runtime contract should assume the following fields are independently optional:

1. target label or display name,
2. target class/icon,
3. faction/friendly-hostile indicator,
4. health bar or coarse health state,
5. special-state indicators:
   - jammed,
   - unknown,
   - stale,
   - scanning required.

This should be represented in the view model as optional/disclosed fields, not inferred from entity-name presence alone.

## 13. Lua and ABI Boundary

Nameplates are primarily an engine-owned client presentation system.

### 13.1 Should Remain Rust/ABI-Owned

1. target eligibility,
2. disclosure gating,
3. duplicate-winner integration,
4. pooling/lifecycle,
5. projection/layout pipeline,
6. performance budgets and fallback behavior.

### 13.2 May Be Lua-Authored Later

1. style/profile selection,
2. icon or plate-skin profile IDs,
3. faction-specific presentation variants,
4. thresholds for content wording such as coarse health descriptors.

Lua should not decide whether a client is authorized to see health or identity details.

## 14. Instrumentation Requirements

The redesign should keep and improve the current counters.

Minimum counters:

1. eligible targets,
2. active pooled entries,
3. pool capacity,
4. entries acquired this frame,
5. entries released this frame,
6. disclosure tier counts,
7. health-capable plates vs health-hidden plates,
8. layout/projection ms,
9. content update ms,
10. budget-capped targets.

The debug overlay should stop implying that “spawned/despawned” is the normal healthy steady-state path once pooling is in place.

## 15. Tests and Validation

The redesign should include targeted tests for:

1. only one plate per canonical winner entity,
2. winner swap rebinds an existing entry instead of growing entity count,
3. losing visibility releases the entry,
4. health-hidden targets do not render a health bar,
5. health-disclosed targets do render the correct bar state,
6. jammed or degraded disclosure transitions update in place,
7. disabled nameplates do not continue doing unnecessary work,
8. idle frames do not grow ECS entity count,
9. projection still runs in `PostUpdate`,
10. pool capacity and budget fallback work deterministically.

Validation should include both:

1. targeted Rust tests,
2. BRP/entity-count smoke validation during a live client session.

## 16. Migration Plan

### Phase 0: Contract and Instrumentation Prep

1. define the new disclosure-aware view model,
2. define the new registry/pool resources,
3. add counters needed to compare old and new behavior,
4. document any new disclosure component requirements if needed.

### Phase 1: Parallel Skeleton

1. add the new systems and resources behind a guarded runtime switch,
2. build target-set and view-model derivation without changing visuals yet,
3. validate target counts and disclosure tiers against live BRP/debug data.

### Phase 2: Pooled Presentation Path

1. implement the pooled entry model,
2. move projection/layout updates to the new entries,
3. preserve current user-facing behavior where disclosure permits it.

### Phase 3: Remove Legacy Path

1. delete the current per-target subtree spawning path,
2. remove stale metrics and components tied only to the old implementation,
3. update docs and tests to the new steady-state contract.

### Phase 4: Optimization and Feature Follow-Up

1. tune budgets and degradation policies,
2. add richer disclosure states,
3. consider hybrid or batched rendering if visible target counts warrant it.

## 17. Open Design Questions

These should remain open sections to be filled in later rather than guessed now:

1. Should the first implementation use pooled Bevy UI nodes or a hybrid UI/material path?
2. What exact component or projection product should represent nameplate disclosure on the client?
3. Should disclosure derive directly from replicated gameplay components, from a dedicated contact/intel presentation component, or from both?
4. How should stale intel be represented visually, if at all, for future tactical integration?
5. What visible-target budget should be enforced by default?
6. Should friendly-owned entities get richer default disclosure than non-owned contacts on the same client?

## 18. Success Criteria

The redesign is successful when:

1. idle client sessions no longer show unbounded nameplate-related ECS growth,
2. target churn does not increase plate entity count over time,
3. nameplates remain correct under duplicate-winner changes,
4. plate content degrades correctly when disclosure is partial or lost,
5. nameplate frame cost is measurably lower or at least predictably bounded,
6. the implementation is easier to reason about than the current spawn/despawn subtree model.

## 19. Immediate Next Step

The next implementation document should refine this plan into an execution checklist covering:

1. exact runtime types,
2. chosen presentation path,
3. migration flag strategy,
4. test matrix,
5. debug overlay counter changes,
6. any visibility/disclosure contract updates required by the chosen data source.

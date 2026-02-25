# Visibility Opt-In Interest Management Plan

Status: Proposed execution plan  
Date: 2026-02-24  
Owners: replication + client runtime

Primary references:
- `docs/sidereal_design_document.md` (Section 7)
- `docs/features/visibility_replication_contract.md`
- `docs/sidereal_implementation_checklist.md` (Section 5)
- `docs/features/scan_intel_minimap_spatial_plan.md`
- `AGENTS.md`

## 1. Terminology (Canonical)

Use these terms consistently in code/docs/tests:

1. `Authorization` (security): policy decision about what a player is entitled to know at all.
2. `Interest management` / `delivery culling` (performance): selects a subset of authorized entities for efficient transport/rendering.
3. `Field-level redaction` (security): component/field masking on outbound payloads.
4. `Snapshot grant`: one-time intel disclosure event (no ongoing stream rights unless separately authorized).
5. `Stream grant`: temporary continuing disclosure right with expiry/revocation.

## 2. Desired Model (Target)

1. Input target fallback:
- If no controlled ship exists, movement intent applies to the player/character entity.
- Camera follows player entity by default in this mode.

2. Controlled ship mode:
- If a ship is controlled, movement intent targets the ship.
- Player entity follows/anchors with controlled runtime state; camera still follows player entity (chain: `camera <- player <- controlled`).

3. Interest management (performance gate):
- Start from character observer position and gather nearby candidates (baseline 300m) via spatial query.

4. Authorization (security gate):
- Apply ownership/fog-of-war/public/faction/scan-grant policy.
- Authorization union includes scanner coverage from all player-owned scanner sources.

5. Payload redaction (security gate):
- Entity visibility does not imply entitlement to all components/fields.
- Apply component/field claim policy and grant scopes before serialization.

## 3. Current Implementation Status

## 3.1 Implemented

- Per-client visibility is server-driven with `ReplicationState::gain_visibility/lose_visibility`.
- Baseline 300m camera bubble exists (`DEFAULT_VIEW_RANGE_M = 300.0`).
- Ownership/public/faction checks exist in visibility gate.
- Scanner-source union over owned roots exists in visibility evaluation.
- Player camera position is persisted on player entity and replicated via `ClientViewUpdateMessage` updates.
- No-controlled fallback routing to player movement is implemented in client/server action flow.
- Shared gameplay system maintains player-follow-controlled behavior when `ControlledEntityGuid` is active.
- Default camera follow anchor is player entity.

## 3.2 Partially Implemented

- Separation of authorization vs delivery is documented but pipeline remains full-world scan (`O(clients * entities)`), not spatial opt-in candidate generation.
- Scan-intel concept and field scopes are documented, but runtime grant machinery is not complete.

## 3.3 Missing / Not Yet Aligned

- True opt-in candidate query (spatial index first) is not implemented; current path iterates all replicated entities per client.
- Strict component visibility metadata enforcement (`#[sidereal_component(... visibility = [...])]`) is documented as a known gap.
- One-time snapshot grant semantics vs continuous grant semantics are documented, but runtime grant enforcement remains incomplete.

## 4. Documentation Conflict Audit

## 4.1 Conflicts to Resolve

1. Stage ordering ambiguity (resolved in source-of-truth docs; keep enforced):
- Canonical statement is `Authorization -> Delivery -> Payload`.
- Candidate preselection is optimization input only, never an authorization substitute.
- Remaining work is implementation alignment and tests, not policy wording.

2. Outdated architecture references in scan/intel plan:
- `docs/features/scan_intel_minimap_spatial_plan.md` references `sidereal-web-gateway` and `StateFrame` flow, which does not match current replication-centric runtime architecture.
- It references `docs/visibility_and_data_permissions.md`, which is not present in this repo.

3. Observer wording drift:
- Source-of-truth docs now use player-entity observer anchor wording.
- Remaining references in secondary plans should be normalized during Phase A doc sweep.

## 4.2 Non-Conflicting Source-of-Truth Items

- Security principle that delivery must never widen authorization remains valid.
- Public/faction exception model remains valid.
- Redaction-before-serialization contract remains valid.

## 5. Implementation Plan

## Phase A: Contract Alignment (Gate)

- [ ] Resolve and document canonical stage order with explicit rationale.
- [ ] Update `docs/sidereal_design_document.md` and `docs/features/visibility_replication_contract.md` to match final decision.
- [ ] Refresh `docs/features/scan_intel_minimap_spatial_plan.md` to current replication architecture and remove stale gateway references.
- [ ] Add decision-register entry if ordering/security semantics change materially.

Exit criteria:
- No conflicting visibility stage semantics across source-of-truth docs.

## Phase B: Input/Control Fallback Semantics

- [x] Add player-entity movement intent path when no controlled ship is bound.
- [x] Keep controlled-ship intent path unchanged when ship is bound.
- [x] Ensure camera anchoring behavior is deterministic in both modes.

Exit criteria:
- WASD always has a single authoritative target (player or controlled ship), never both and never neither in active mode.

## Phase C: Opt-In Interest Management

- [ ] Introduce spatial index resource in replication runtime (grid/hash first iteration).
- [ ] Build candidate set from observer position + delivery radius (+ optional edge buffer).
- [ ] Restrict visibility evaluation to candidate set plus always-included ownership/attachment roots.

Exit criteria:
- Visibility pass no longer full-scans all replicated entities for each client.

## Phase D: Authorization Engine

- [ ] Implement explicit authorization evaluation module:
  - ownership,
  - public/faction rules,
  - scanner union over all owned scanner entities,
  - scan grants.
- [ ] Preserve server-authoritative fail-closed behavior.

Exit criteria:
- Authorization decisions are explicit, testable, and independent of delivery-rate concerns.

## Phase E: Snapshot Grants + Stream Grants

- [ ] Implement one-time snapshot scan grant behavior (`snapshot_only` semantics).
- [ ] Implement temporary stream grants with expiry/revocation.
- [ ] Ensure snapshot grant does not imply continuous updates.

Exit criteria:
- Snapshot vs stream rights are distinct and covered by tests.

## Phase F: Component Claim Redaction

- [ ] Enforce component visibility metadata as outbound gate.
- [ ] Add field-scope redaction masks for grant scopes (`physical_public`, `combat_profile`, `cargo_summary`, `cargo_manifest`, `systems_detail`).
- [ ] Ensure unauthorized components/fields are never serialized.

Exit criteria:
- Entity presence and component disclosure are independently controlled.

## Phase G: Performance + Quality Gates

- [ ] Add metrics:
  - `candidates_per_frame`,
  - `included_per_frame`,
  - `query_time_per_client`.
- [ ] Add integration tests for:
  - authorization vs delivery separation,
  - ownership/public/faction exceptions,
  - scan snapshot one-time behavior,
  - component redaction correctness.
- [ ] Run required build checks for native + wasm + windows.

Exit criteria:
- Pipeline is measurable, test-covered, and conforms to contracts.

## 6. Bevy/Lightyear System Guidance (Supported Approach)

Use currently adopted primitives:

- Bevy:
  - `FixedUpdate` for deterministic visibility/interest evaluation.
  - ECS `Resource` caches/scratch storage for zero/low allocation hot paths.
  - Query filters (`With`/`Without`) and explicit system ordering for stage separation.

- Lightyear:
  - per-client entity visibility via `ReplicationState::gain_visibility/lose_visibility`.
  - authenticated client identity binding via existing session mapping.
  - keep replication transport concerns separate from authorization logic.

Avoid:
- client-side trust for visibility policy,
- frame-rate dependent authority logic,
- full-world per-client scans at scale.

## 7. Deliverable Sequence

1. Docs alignment PR (Phase A).
2. Input/control fallback PR (Phase B).
3. Spatial opt-in candidate pipeline PR (Phase C).
4. Authorization module PR (Phase D).
5. Scan grant semantics PR (Phase E).
6. Component redaction enforcement PR (Phase F).
7. Metrics and hardening PR (Phase G).

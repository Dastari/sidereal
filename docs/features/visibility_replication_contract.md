# Visibility and Replication Contract

Status: Active implementation guide for visibility-related changes.

Primary architecture references:
- `docs/sidereal_design_document.md` (Section 7)
- `docs/sidereal_implementation_checklist.md` (Section 5)
- `AGENTS.md` non-negotiable rules

## 1. Goal

Keep server-authoritative visibility correct, generic, and low-boilerplate:
- entity visibility is server-decided,
- component visibility/redaction is policy-driven,
- new components do not require ad-hoc visibility plumbing.

## 2. Required Visibility Stages

All visibility-sensitive changes must preserve this order:

1. Authorization scope:
- ownership + faction/public policy + scanner/fog-of-war.

2. Delivery scope:
- stream/camera/range narrowing of already-authorized data.

3. Payload scope:
- component/field redaction before serialization.

Delivery must never widen authorization.

Performance note:
- Systems may build an early candidate set (for example spatial nearby-cell opt-in query) before full authorization.
- That candidate step is an optimization input, not authorization.
- Final outbound entity/component decisions must still be evaluated against full server policy and remain a strict narrowing of authorization.

## 3. Current Runtime Baseline

Implemented now:
- per-client entity visibility updates in replication fixed tick,
- owner/public/faction visibility allowances,
- scanner range fallback with default floor,
- mounted/child positional fallback for visibility checks.
- observer anchor from player runtime camera state (`Transform` on player entity), with scanner-source union over owned entities.
- control semantics align to `camera <- player <- controlled(optional)`; observer/player anchor is the delivery-center source-of-truth.

Known gap:
- `#[sidereal_component(..., visibility = [...])]` metadata is recorded but not yet used as a strict per-component outbound redaction gate.

## 3.1 Observer and Control Semantics (Normative)

1. Observer anchor for delivery narrowing is player entity transform/state.
1.1 Delivery narrowing must use player observer anchor only; scanner source positions must not be used as delivery-center fallback.
2. Controlled entity state may influence player position (player follows controlled when control active), but does not replace player as observer anchor identity.
3. Free-roam control is represented as `ControlledEntityGuid = player guid` (self-control); player movement remains authoritative for observer anchor updates in that mode.
4. Camera behavior is a client presentation concern; delivery authorization remains server-authoritative and player-entity scoped.

## 3.2 Snapshot vs Stream Disclosure (Normative)

1. Entity authorization and delivery do not automatically imply full component disclosure.
2. Component/field disclosure must support:
- continuous stream entitlement (ongoing updates while grant/policy active),
- one-time snapshot entitlement (single disclosure event without future update rights).
3. Snapshot entitlement must not silently upgrade to stream entitlement.
4. Expiry/revocation must immediately restore redaction policy.

## 4. Required Direction

When touching visibility or replication payload behavior:

1. Keep entity-level visibility as coarse gate.
2. Add/keep component-level visibility as fine gate using component metadata.
3. Never rely on client-side filtering for sensitive data.
4. Keep generic entity semantics (no ship-only assumptions in shared visibility code).

## 5. Component Metadata Policy

For custom gameplay components in `crates/sidereal-game/src/components/`:
- use `#[sidereal_component(kind = \"...\", persist = bool, replicate = bool, visibility = [...])]`,
- `replicate = false` means no network replication registration,
- `visibility` must be treated as server policy input for outbound data scope.

## 6. External Components (Bevy/Avian)

- Runtime/prediction components (for example Avian motion state) may be replicated as transport/runtime requirements.
- They are not automatically durable gameplay schema.
- Durable gameplay state must live in sidereal gameplay components and pass persistence/hydration roundtrip coverage.

## 7. Edit Checklist (Mandatory for Visibility Changes)

For any PR touching visibility, replication delivery, or data redaction:

1. Confirm ownership/faction/public rules remain server-enforced.
2. Confirm scanner/range behavior remains server-enforced.
3. Confirm unauthorized fields/components are never serialized.
4. Confirm camera/delivery culling only narrows, never widens, authorization.
5. Confirm player observer-anchor semantics remain consistent (`camera <- player <- controlled(optional)`).
6. Confirm snapshot-vs-stream disclosure behavior is explicit for changed components/fields.
7. Add/update tests for changed behavior (unit + integration when cross-service).
8. Update:
- `docs/sidereal_design_document.md` if contract changed,
- this file (`docs/features/visibility_replication_contract.md`) if implementation policy changed,
- `AGENTS.md` if contributor enforcement rules changed.

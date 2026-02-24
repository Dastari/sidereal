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

## 3. Current Runtime Baseline

Implemented now:
- per-client entity visibility updates in replication fixed tick,
- owner/public/faction visibility allowances,
- scanner range fallback with default floor,
- mounted/child positional fallback for visibility checks.

Known gap:
- `#[sidereal_component(..., visibility = [...])]` metadata is recorded but not yet used as a strict per-component outbound redaction gate.

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
5. Add/update tests for changed behavior (unit + integration when cross-service).
6. Update:
- `docs/sidereal_design_document.md` if contract changed,
- this file (`docs/features/visibility_replication_contract.md`) if implementation policy changed,
- `AGENTS.md` if contributor enforcement rules changed.

# Explicit World Entry Flow

Status: Accepted decision detail (`DR-0002`)  
Date: 2026-02-24

## Purpose

Define the authoritative lifecycle from authentication to in-world runtime binding.

## Lifecycle

1. Register:
   - create account,
   - create default character ownership row,
   - persist starter character + starter corvette graph records,
   - return auth tokens,
   - do not auto-enter world.
2. Login:
   - validate credentials,
   - return auth tokens,
   - do not auto-enter world.
3. Character Select:
   - client requests account-owned characters.
4. Enter World:
   - client submits selected `player_entity_id`,
   - gateway validates account ownership,
   - bootstrap dispatch occurs.
5. Runtime bind:
   - replication validates identity/ownership and binds session to selected character.
6. World loading gate:
   - client remains in `WorldLoading` after `/world/enter` acceptance,
   - bootstrap-required asset fetch must not begin until replication emits session-ready bind acknowledgment for the selected `player_entity_id`,
   - transition to `InWorld` only after session-ready, required asset bootstrap, and replicated selected-player presence all complete.

## Enforcement Rules

- Register/login are auth-only flows.
- Runtime world bootstrap is Enter-World-only.
- Ownership mismatches must fail closed (reject + log), not auto-heal.
- Bootstrap idempotency is per `player_entity_id`.
- Enter-World reconnects must still ensure runtime presence/bind for selected character even when bootstrap persistence marker already exists.
- Enter/reconnect bootstrap may hydrate missing runtime entities from persisted graph records for the selected character, but must not synthesize a brand-new ship identity at runtime.
- Minimum runtime control protocol is:
  - `ClientControlRequestMessage { player_entity_id, controlled_entity_id, request_seq }`
  - `ServerControlAckMessage { player_entity_id, request_seq, control_generation, controlled_entity_id }`
  - `ServerControlRejectMessage { player_entity_id, request_seq, control_generation, reason, authoritative_controlled_entity_id }`
- Control routing is server-authoritative and validated by ownership; client clears pending control only on explicit ack/reject for matching `request_seq`.
- `control_generation` is the authoritative lease generation and is part of the bootstrap contract for predicted handoff/reconnect.

## Failure Behavior

- Missing/invalid selected character:
  - reject request,
  - preserve server integrity,
  - no crash/no panic/no silent fallback entity creation.
- Replication auth misconfiguration (for example missing/invalid `GATEWAY_JWT_SECRET` on replication):
  - deny session explicitly,
  - keep client out of world,
  - do not silently leave client hanging in `WorldLoading`.
- Any terminal replication auth rejection after the requesting peer is identified:
  - must emit `ServerSessionDeniedMessage`,
  - must not rely on the client-side session-ready watchdog to surface auth/account/player mismatch failures.
- Missing controlled entity:
  - valid state (`controlled = None`),
  - client remains functional in free-camera mode.

## Test Requirements

- Register creates durable starter world records.
- Login/register do not dispatch runtime bootstrap.
- Enter World dispatches only when ownership is valid.
- Identity mismatch paths are explicitly rejected.

## References

- `docs/decision_register.md` (`DR-0002`)
- `docs/plans/test_topology_and_resilience_plan.md`
- `docs/sidereal_design_document.md`

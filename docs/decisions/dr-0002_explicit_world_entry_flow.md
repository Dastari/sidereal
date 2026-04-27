# Explicit World Entry Flow

Status: Accepted decision detail (`DR-0002`)  
Date: 2026-02-24

Update note 2026-04-26:

- `DR-0036` supersedes the earlier registration detail where account registration created the default character and starter corvette.
- Target lifecycle after the `DR-0036` migration: registration creates account/auth state only; explicit character creation creates character ownership and starter-world graph records.
- World entry remains explicit and character-scoped.

## Purpose

Define the authoritative lifecycle from authentication to in-world runtime binding.

## Lifecycle

1. Register:
   - create account,
   - target after `DR-0036`: do not create a default character,
   - target after `DR-0036`: do not persist starter character/starter ship graph records,
   - return auth tokens,
   - do not auto-enter world.
2. Login:
   - validate credentials,
   - return auth tokens,
   - do not auto-enter world.
3. Character Select:
   - client requests account-owned characters.
4. Character Create:
   - client or dashboard submits a display name,
   - gateway creates the account-owned character row,
   - gateway persists starter character + starter ship graph records,
   - gateway returns the active character summary.
5. Enter World:
   - client submits selected `player_entity_id`,
   - gateway validates account ownership,
   - gateway mints a character-scoped world token,
   - bootstrap dispatch occurs.
6. Runtime bind:
   - replication validates identity/ownership and binds session to selected character.
7. World loading gate:
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

2026-04-24 update: `ClientRealtimeInputMessage` must carry the observed `control_generation`, and the replication server must reject stale-generation realtime input. This binds input intent to the same authoritative control lease as handoff/bootstrap and prevents delayed pre-handoff packets from applying to a new target.

## Failure Behavior

- Missing/invalid selected character:
  - reject request,
  - preserve server integrity,
  - no crash/no panic/no silent fallback entity creation.
- Replication auth misconfiguration (current shared-secret flow, or target asymmetric public-key/JWKS flow after `DR-0036`):
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

- Target after `DR-0036`: register creates account/auth state only and no character.
- Target after `DR-0036`: explicit character creation creates durable starter world records.
- Login/register do not dispatch runtime bootstrap.
- Enter World dispatches only when ownership is valid.
- Identity mismatch paths are explicitly rejected.

## References

- `docs/decision_register.md` (`DR-0002`)
- `docs/decision_register.md` (`DR-0036`)
- `docs/sidereal_design_document.md`
- `docs/plans/gateway_dashboard_auth_character_flow_plan_2026-04-26.md`

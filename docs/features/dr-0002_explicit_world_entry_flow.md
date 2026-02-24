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

## Enforcement Rules

- Register/login are auth-only flows.
- Runtime world bootstrap is Enter-World-only.
- Ownership mismatches must fail closed (reject + log), not auto-heal.
- Bootstrap idempotency is per `player_entity_id`.

## Failure Behavior

- Missing/invalid selected character:
  - reject request,
  - preserve server integrity,
  - no crash/no panic/no silent fallback entity creation.
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
- `docs/features/test_topology_and_resilience_plan.md`
- `docs/sidereal_design_document.md`

# Account Character Session Model

Status: Accepted decision detail (`DR-0001`)  
Date: 2026-02-24

## Purpose

Define unambiguous identity terminology and boundaries across auth, runtime, and persistence.

## Terms

- `Account`: authenticated identity container (credentials, tokens, auth lifecycle).
- `Character`: durable gameplay identity represented by a persisted player ECS entity (`player_entity_id`).
- `Session`: active runtime binding between a client transport identity and one selected character.
- `Player`: UX language only; avoid as primary technical boundary term.

## Required Invariants

- An account may own multiple characters.
- Character-local gameplay state persists on the character/player entity, not on account rows.
- Runtime control/auth binding is account-authenticated but character-scoped.
- No raw Bevy `Entity` IDs cross service boundaries.

## Data Ownership

- Account-scope examples:
  - credential hash,
  - refresh token lifecycle.
- Character-scope examples:
  - controlled/selected/focused entity guid,
  - camera/view position (`Transform` / persisted `position_m`),
  - progression/quest/score/local character state.

## Naming Guidance

- Prefer `character`/`session` naming in new protocols/resources/tests where technically accurate.
- Keep compatibility naming where required by existing APIs, but do not expand ambiguous naming.

## Impacted Areas

- Gateway auth/character ownership checks.
- Replication session binding and input identity validation.
- Persistence schema for player entities and related ownership records.

## References

- `docs/decision_register.md` (`DR-0001`)
- `docs/sidereal_design_document.md`

# DR-0033: Background World Simulation Tiering

Status: Proposed  
Date: 2026-03-11  
Owners: gameplay / replication / persistence / scripting

## Context

Sidereal needs a world that feels economically and socially alive beyond connected-player visibility bubbles. The current architecture already defines:

1. authoritative server-side visibility and persistence,
2. galaxy-scale travel and solar system structure,
3. script-driven economy/AI/mission direction,
4. future LOD/culling tiers as a target.

What is still missing is an explicit decision for how offscreen actors continue to exist and affect the world without full runtime simulation everywhere.

## Decision

Adopt a tiered background world simulation model:

1. `FullRuntime` for player-relevant authoritative ECS entities.
2. `AbstractSimulation` for offscreen actors represented by route/job progress and other high-level state rather than continuous physics.
3. `DerivedPressure` for runtime-derived lane heat, piracy pressure, security pressure, and similar systemic signals.

Use two actor classes within that model:

1. lightweight background `Quanta` actors for economy pressure and traffic,
2. persistent `HeroActor` NPCs for named/faction/story-relevant actors.

Use dynamic lanes and probability-volume-style pressure derived from recent traffic rather than relying only on hand-authored static routes.

## Consequences

Positive:

1. The economy, factions, missions, and encounters can be driven by one shared world model.
2. Offscreen areas do not require full simulation cost.
3. The design preserves player encounterability and world continuity.

Negative:

1. Promotion/demotion and persistence boundaries become a new architecture surface that must be explicitly tested.
2. Abstract conflict resolution and cargo synthesis rules need careful tuning to avoid feeling arbitrary.

## Follow-up

1. Use `docs/features/background_world_simulation_contract.md` as the active feature contract.
2. Keep V1 scoped to node inventories, budgets, transport, lane pressure, piracy/security response, and mission generation.
3. Resolve open policy questions around resource depletion/regrowth and exact abstract conflict formulas before implementation hardens.

## Decision Doc

1. `docs/features/background_world_simulation_contract.md`

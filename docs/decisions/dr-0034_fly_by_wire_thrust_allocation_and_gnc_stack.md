# DR-0034: Fly-By-Wire Thrust Allocation and GNC Stack

Status: Proposed  
Date: 2026-03-13  
Owners: gameplay / replication / client prediction / AI / scripting

## Context

Sidereal's current flight runtime still aggregates engine capability into one idealized hull-level force/torque budget. That blocks:

1. real directional actuator coverage,
2. per-engine saturation and degraded behavior,
3. explicit shared player/AI/autopilot control layers,
4. clean engine plume presentation driven by actual actuator state.

The project also needs a stable contract for what belongs in Lua authoring versus Rust-owned flight/physics ABI.

## Decision

Adopt a fly-by-wire guidance, control, and thrust-allocation stack with these boundaries:

1. pilot, AI, and scripts request desired motion rather than direct engine power,
2. flight control converts desired motion into a desired wrench,
3. thrust allocation maps desired wrench to explicit engine actuators,
4. actuation applies net force/torque through Avian-compatible physics,
5. plume and engine presentation read explicit engine actuator/effect components and command/state data,
6. Lua may author validated actuator/profile/effect-reference data and emit high-level motion/navigation intents, but Rust owns allocator math, physics application, and render ABI.

## Consequences

Positive:

1. Sidereal gets one shared control stack for players, AI, autopilot, and damaged-engine behavior.
2. Propulsion behavior becomes consistent with mounted modules, hardpoints, fuel, and mass distribution.
3. Plume rendering can be driven by explicit engine state instead of indirect hull-level guesses.

Negative:

1. Current `FlightComputer` and `Engine` schemas will need restructuring.
2. Prediction/reconciliation and scripting intent paths must be updated together when implementation begins.
3. The initial allocator must be chosen carefully so V1 remains performant and deterministic.

## Follow-up

1. Use `docs/features/fly_by_wire_thrust_allocation_contract.md` as the active feature contract.
2. Treat `docs/plans/advanced_fly_by_wire_and_thruster_allocation_plan.md` as precursor planning material rather than the authoritative architecture contract.
3. Update scripting, plume, and prediction docs in the same implementation slices as the control-stack migration.

## Decision Doc

1. `docs/features/fly_by_wire_thrust_allocation_contract.md`

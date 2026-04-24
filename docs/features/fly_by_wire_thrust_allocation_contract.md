# Fly-By-Wire Thrust Allocation Contract

Status: Proposed feature contract
Last updated: 2026-04-24
Owners: gameplay simulation + client runtime + scripting
Scope: future replacement flight-control stack for actuator/thrust allocation

## 0. Implementation Status

2026-04-24 status note:

1. Not implemented yet as a fly-by-wire actuator allocation runtime.
2. Current flight still uses the existing fixed-step force/torque path around `ActionQueue`, `FlightComputer`, mounted engine budget aggregation, and Avian force application.
3. Existing `ThrusterPlumeShaderSettings`/shader work is presentation plumbing, not the authoritative allocation system described here.
4. Native/WASM impact: future allocation math must stay in shared gameplay/runtime code; platform differences belong only at input, visual, and audio boundaries.

Update note (2026-03-13):
- This contract defines the target replacement for the current `ActionQueue -> FlightComputer(throttle/yaw) -> aggregated engine budget -> hull net force/torque` flight path in `crates/sidereal-game/src/flight.rs`.
- The target model is server-authoritative fly-by-wire: the pilot or AI requests desired motion, the flight computer computes a desired wrench, the thrust allocator assigns per-engine commands, and the actuator layer applies Avian-compatible forces and torque.
- Engine plume visuals are part of the same architecture. The client should drive plume presentation from explicit engine actuator/effect components and command/state data, not from label scans or implied engine naming.
- Native impact: this is a shared gameplay/control-stack replacement, not a native-only workaround. WASM impact: no architecture split is intended; the same shared control/allocation logic must remain usable by the browser client.

## 1. Purpose

Sidereal needs a physically grounded modular propulsion model that supports:

1. ships with multiple engines and directional thrusters,
2. degraded behavior when thrusters are missing, damaged, starved, or poorly placed,
3. shared player and AI flight handling,
4. future navigation/autopilot/avoidance layers above the same control stack,
5. engine-specific plume presentation that follows actual actuator command/state rather than hull-level throttle guesses.

This contract is the source of truth for the target control stack. Execution planning should derive from this document rather than treating the older prototype plan as authoritative.

## 2. Current Implementation Being Replaced

Current authoritative flight in `crates/sidereal-game/src/flight.rs` works like this:

1. `ActionQueue` stores discrete actions (`Forward`, `Backward`, `Left`, `Right`, `Brake`, `AfterburnerOn`, `AfterburnerOff`).
2. `process_flight_actions` converts those actions into mutable `FlightComputer` fields (`throttle`, `yaw_input`, `brake_active`).
3. `apply_engine_thrust` does not command individual actuators. Instead it:
   - aggregates all mounted engine capability by parent UUID,
   - derives forward/reverse/torque thrust budgets,
   - gates those budgets by fuel,
   - computes one net hull force and one net hull torque,
   - applies those values through Avian's `Forces` helper.
4. Client plume visuals infer drive state from hull-level `FlightComputer` state and engine presence.

That model is acceptable for a single idealized forward engine, but it does not preserve actuator directionality, per-thruster saturation, mount-position torque contribution, or degraded authority envelopes.

## 3. Design Summary

The required control stack is:

1. intent/guidance chooses desired motion,
2. flight control converts desired motion into desired wrench,
3. thrust allocation maps desired wrench onto real actuators,
4. actuation applies force and torque through Avian,
5. presentation reads actuator command/state for plumes and other engine effects.

The player does not command engines directly.

The flight computer does not care about engine names.

The engine allocator does not perform pathfinding or gameplay decision-making.

## 4. Non-Negotiable Rules

1. Authority remains one-way: `client input or AI intent -> authoritative sim -> replication`.
2. Control and allocation math runs only in fixed-step simulation. Frame delta must not drive authoritative propulsion behavior.
3. Clients must not directly set authoritative transforms, velocities, or engine commands.
4. The control stack must remain compatible with Avian2D by producing forces and torque, not direct transform/velocity writes.
5. Gameplay actuator capabilities and flight profiles live in `crates/sidereal-game` components with normal persistence/replication coverage.
6. Runtime-only solver scratch state may stay non-persisted, but durable control mode/profile/authoring state must be componentized.
7. Player and AI share the same lower flight-control, thrust-allocation, and actuation systems.
8. Engine plume visuals must be driven from explicit engine/effect components, not string-label discovery or owner-only gameplay markers on the winning visual lane.

## 5. Layered Architecture

### 5.1 Intent Layer

This is the source of pilot, AI, or script motion requests.

Examples:

1. player local stick/keyboard input,
2. script/autopilot "move to point",
3. AI orbit/strafe/flee/follow behaviors.

Intent should produce control-friendly setpoints such as:

1. `DesiredLocalVelocity`,
2. `DesiredWorldVelocity`,
3. `DesiredAngularVelocity`,
4. `FlightAssistMode`,
5. optional higher-level `NavGoal` or autopilot command components.

### 5.2 Guidance Layer

Guidance is above fly-by-wire.

It answers:

1. where should the ship go,
2. what velocity should it try to have,
3. what angular state should it try to hold.

Examples:

1. waypoint following,
2. docking approach,
3. obstacle avoidance,
4. intercept/orbit/formation behavior.

Guidance writes desired motion setpoints. It does not choose engines.

### 5.3 Flight Control Layer

Flight control converts desired motion into a desired wrench:

1. linear force in ship/body or world space,
2. angular torque about Z for the current 2D runtime.

This is the fly-by-wire "brain." It compares target motion with current Avian kinematics and produces the requested wrench subject to profile/mode limits.

### 5.4 Thrust Allocation Layer

Thrust allocation takes the desired wrench and determines which actuators can contribute.

For each engine or thruster:

1. transform its local thrust axis to ship/body/world space,
2. compute the force it can contribute,
3. compute torque contribution from mount offset,
4. solve for bounded actuator commands.

This layer owns saturation and degraded coverage behavior.

### 5.5 Actuation Layer

Actuation turns allocator output into actual Avian-compatible force and torque.

This layer also applies spool/response behavior, fuel gating, and damage/availability clipping.

### 5.6 Presentation Layer

Presentation reads explicit engine actuator/effect state and renders:

1. plumes,
2. afterburner variants,
3. engine glow,
4. future heat/damage/sputter effects.

Presentation must not infer propulsion from hull labels or ad hoc name patterns.

## 6. Recommended Canonical ECS Shape

The exact names may change during implementation, but the architecture should converge on the following split.

### 6.1 Persisted / Replicated Gameplay Components

#### `FlightComputer`

`FlightComputer` should stop being "current raw throttle/yaw command state."

Target responsibility:

1. flight-control profile selection,
2. assist/stabilization mode,
3. high-level control flags,
4. any durable/autopilot-facing policy that belongs on the controlled ship.

It should not remain the canonical storage for direct per-tick engine demand fields.

#### `EngineActuator`

Replace or evolve the current `Engine` component into an explicit actuator capability component.

Recommended authored fields:

1. `max_forward_force_n`,
2. `max_reverse_force_n` if bidirectional,
3. `local_thrust_dir`,
4. `local_mount_pos_m`,
5. optional `local_mount_rotation`,
6. `throttle_response_up_s`,
7. `throttle_response_down_s`,
8. `min_throttle`,
9. `fuel_burn_rate_curve_id` or base burn rate,
10. optional allocator participation flags for future precision tuning,
11. optional gimbal limits for future extension.

This component is gameplay-authorable and should remain Rust-schema-owned but Lua-authored in bundles.

#### `EngineHealthState`

Recommended for degradation:

1. enabled/disabled,
2. force efficiency scalar,
3. burn-rate efficiency scalar,
4. failure mode flags.

#### `ThrusterPlumeEmitter`

Plume authoring should live on the engine entity as an explicit public presentation component.

Recommended authored fields:

1. `plume_profile_id`,
2. optional local visual offset and rotation override relative to the engine entity,
3. optional `afterburner_profile_id`,
4. optional intensity/scale multipliers.

This avoids client label scans and cleanly supports different engines having different plume families.

### 6.2 Runtime / Authoritative Control Components

Recommended runtime-only or low-frequency replicated components:

1. `DesiredLocalVelocity`,
2. `DesiredWorldVelocity` where needed,
3. `DesiredAngularVelocity`,
4. `DesiredWrench { force: Vec2, torque_z: f32 }`,
5. `EngineCommand { normalized: f32 }`,
6. `EngineState { normalized: f32 }`,
7. `ControlAuthorityEstimate { requested_wrench, achievable_wrench }`.

Replication guidance:

1. `DesiredWrench` and raw `EngineCommand` should normally stay runtime-only unless debug tooling specifically needs them.
2. `EngineState` may need a public or filtered presentation lane if remote observers must see authoritative plume intensity exactly.
3. Do not replicate full solver scratch buffers or matrices.

## 7. How This Maps Onto Current Sidereal Data

The current runtime already has the right structural ingredients:

1. mounted modules via `MountedOn`,
2. hardpoint offsets and rotation via `Hardpoint`,
3. mass parity via `TotalMassKg`, Avian `Mass`, and `AngularInertia`,
4. fixed-step authoritative simulation,
5. shared `sidereal-game` logic for client/server.

The replacement should reuse those ingredients rather than inventing parallel hierarchy or physics lanes.

Key mapping rules:

1. engine mount position and orientation should derive from the mounted module entity plus hardpoint-local transform,
2. actuator force/torque calculations should use the real mount offset relative to the controlled hull,
3. `TotalMassKg` and Avian `Mass`/`AngularInertia` remain the mass source for control calculations,
4. `AfterburnerCapability` becomes an actuator capability modifier, not a hull-global cheat multiplier.

## 8. Avian2D Integration Contract

The system must stay Avian-compatible.

### 8.1 FixedUpdate Ordering

Recommended order:

1. input and AI/script intent collection,
2. guidance update,
3. flight control update,
4. thrust allocation update,
5. engine spool/response update,
6. force/torque application,
7. Avian physics step.

### 8.2 Force Application

Per actuator:

1. compute world-space thrust force from current engine state,
2. compute torque contribution from lever arm cross force,
3. sum actuator contributions into net force and torque,
4. apply through Avian using force/torque APIs already used by the current runtime.

If Avian later exposes stable force-at-point APIs suitable for this runtime, that path is valid, but the canonical requirement is physical equivalence, not a specific API surface.

### 8.3 Motion Ownership

No system in this stack may directly set:

1. `Position`,
2. `Rotation`,
3. `LinearVelocity`,
4. `AngularVelocity`.

The only authoritative write is force/torque input into physics, plus existing motion-stabilization cleanup policies where still required.

## 9. Control Logic Requirements

### 9.1 Initial Flight Control

The first useful controller should:

1. compare current linear velocity to desired linear velocity,
2. use proportional or critically damped control to request acceleration,
3. clamp requested acceleration by profile limits,
4. convert requested acceleration to force via mass,
5. do the same for angular velocity/torque around Z.

This is enough for a viable first fly-by-wire implementation.

### 9.2 Saturation Behavior

If actuator coverage is insufficient:

1. allocator saturates valid engines,
2. ship achieves the best possible approximation,
3. no fake authority is invented,
4. `ControlAuthorityEstimate` should expose the shortfall for HUD, AI, and debugging.

This is a core requirement, not an optional polish feature.

### 9.3 Control Modes

Recommended explicit modes:

1. `Stabilized`,
2. `Precision`,
3. `Combat`,
4. future `AssistOff`.

Mode should influence controller gains, braking behavior, and allocator policy, but not bypass physics.

## 10. Allocation Logic Requirements

### 10.1 V1 Allocator

The first allocator does not need to be a full constrained optimizer, but it must be structured so we can replace it later.

Acceptable V1 options:

1. weighted projection allocator,
2. greedy fill by contribution,
3. least-squares with clamping.

Required behavior:

1. only engines aligned with the requested force/torque contribute,
2. stronger actuators carry more load,
3. command bounds are respected,
4. degraded or disabled actuators are excluded,
5. actuator response/spool is respected.

### 10.2 Future Allocator

The architecture should leave room for a more correct constrained optimizer that solves:

1. translation and torque together,
2. damaged asymmetric layouts,
3. fuel/efficiency-aware optimization,
4. minimizing unwanted residual torque or fuel waste.

Do not bake the first allocator's assumptions into the component model.

## 11. Navigation / Guidance Extension Path

This contract includes the path for future autopilot and AI, but that logic stays above fly-by-wire.

Recommended high-level stack:

1. `NavGoal`,
2. path planning / route selection,
3. local avoidance / steering,
4. guidance output,
5. flight control,
6. thrust allocation,
7. actuation.

Examples that should eventually plug into this stack:

1. `MoveTo`,
2. `Follow`,
3. `Orbit`,
4. `Dock`,
5. `AttackApproach`,
6. `Flee`.

Navigation and obstacle avoidance should write desired motion, not engine throttle.

## 12. Lua vs Rust / ABI Boundary

### 12.1 Rust-Owned ABI

Rust owns:

1. control stack scheduling and authority,
2. actuator matrix/effectiveness math,
3. allocator implementation,
4. Avian force/torque application,
5. plume/effect material ABI,
6. validation of authored actuator and plume profile data,
7. debug/telemetry surfaces.

Lua must not:

1. write `EngineCommand`,
2. write `EngineState`,
3. write desired wrench directly unless a future privileged debug surface is explicitly added,
4. bind to raw plume shader uniforms or material-family internals,
5. directly apply force/torque or mutate Avian motion state.

### 12.2 Lua-Authorable Data

Lua should be allowed to author validated gameplay/presentation data such as:

1. `EngineActuator` fields,
2. `AfterburnerCapability`,
3. `ThrusterPlumeEmitter`,
4. flight-control profile IDs or tuning profile references,
5. navigation profile IDs and autopilot policy references,
6. effect preset or plume profile IDs.

Those remain Rust-schema-owned components with normal component registry generation and validation.

### 12.3 Runtime Script Intents

Runtime scripts and AI should emit intents such as:

1. `set_desired_local_velocity`,
2. `set_desired_angular_velocity`,
3. `set_nav_goal`,
4. `set_flight_mode`,
5. `cancel_nav_goal`.

They should not emit:

1. `set_engine_throttle`,
2. `fire_left_thruster`,
3. raw shader/material payload updates.

This keeps scripts on the guidance side of the boundary rather than the actuation side.

## 13. Plume Integration Contract

The plume system should be rebuilt around explicit engine presentation state.

### 13.1 Discovery

The client should attach/update plume visuals by querying explicit components such as:

1. `EngineActuator`,
2. `ThrusterPlumeEmitter`,
3. public presentation-state data needed for remote observers.

It should not discover engines by:

1. string-label scans,
2. implied component naming,
3. owner-only gameplay markers that may not exist on the winning visual lane.

### 13.2 Drive Source

Plume intensity should derive from actual engine command/state:

1. commanded throttle for predicted/local presentation where appropriate,
2. authoritative engine state for remote observers,
3. afterburner state as an effect variant or multiplier.

Hull-level throttle is only an approximation and should not remain the canonical plume driver.

### 13.3 Authoring

Different engines must be able to choose different plume families.

The recommended model is:

1. engine entity carries `ThrusterPlumeEmitter { plume_profile_id, afterburner_profile_id, ... }`,
2. client resolves that profile into the shared runtime effect ABI,
3. plume transform follows the engine module and mount hierarchy.

## 14. Migration Strategy

### Phase 1: Contract-Safe Data Split

1. Introduce explicit actuator and plume-emitter components.
2. Split current `FlightComputer` responsibilities into durable mode/profile state vs transient runtime demand state.
3. Stop relying on label scans for plume discovery.

### Phase 2: Hybrid Control Replacement

1. Keep current player input and script intent entry points.
2. Translate those into desired motion setpoints rather than direct throttle/yaw fields.
3. Replace aggregated engine-budget math with real per-actuator command allocation.
4. Continue applying net Avian force/torque so prediction and reconciliation surfaces stay familiar.

### Phase 3: Angular + Lateral Authority

1. Extend allocator to handle strafe/side thrusters cleanly.
2. Add explicit desired angular velocity / torque setpoints.
3. Validate degraded control with missing directional coverage.

### Phase 4: Navigation / Autopilot

1. Add `NavGoal` and guidance components.
2. Route AI and script behaviors into desired motion rather than raw `FlightComputer` mutation.
3. Add docking, orbit, follow, and obstacle-avoidance producers.

### Phase 5: Hardening

1. add authoritative telemetry,
2. add failure-injection tests,
3. profile allocator cost,
4. decide whether V1 allocator is sufficient or should be replaced by a bounded solver.

## 15. Testing and Observability Requirements

Unit coverage in `crates/sidereal-game` should include:

1. wrench contribution from mount offset and axis direction,
2. allocator saturation behavior,
3. degraded actuator coverage,
4. fuel/damage gating,
5. engine spool/response,
6. deterministic fixed-step behavior for identical inputs.

Integration coverage should include:

1. player authoritative control path,
2. prediction compatibility for the controlled entity,
3. module swap / hierarchy rebuild effects,
4. remote observer plume correctness,
5. AI/script intent path using the same lower stack.

Required debug surfaces:

1. desired vs achieved wrench,
2. per-engine command/state,
3. saturation reasons,
4. directional authority envelope,
5. active flight mode/profile.

## 16. Relationship To Existing Docs

1. `docs/plans/advanced_fly_by_wire_and_thruster_allocation_plan.md` remains useful as precursor planning material, but this feature contract is now the authoritative architecture target.
2. `docs/plans/thruster_plumes_afterburner_plan.md` remains the plume-specific execution history, but future plume work must follow the explicit engine-actuator contract defined here.
3. `docs/features/scripting_support.md` remains the scripting authority contract; this document narrows the flight-control-specific Lua boundary under that larger scripting contract.

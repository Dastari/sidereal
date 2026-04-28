# IFCS Navigation and Thrust Allocation Implementation Plan

Status: Active implementation plan  
Last updated: 2026-04-28  
Owners: shared gameplay simulation + replication server + native client runtime + scripting/AI + asset authoring  
Scope: player/AI/script navigation orders, IFCS/fly-by-wire control, flight modes, cruise, partial thrust, actuator allocation, hardpoint-driven directional thrusters, prediction parity, presentation state  
Primary references:

1. `AGENTS.md`
2. `docs/sidereal_design_document.md`
3. `docs/features/fly_by_wire_thrust_allocation_contract.md`
4. `docs/decisions/dr-0034_fly_by_wire_thrust_allocation_and_gnc_stack.md`
5. `docs/features/prediction_runtime_tuning_and_validation.md`
6. `docs/features/scripting_support.md`
7. `docs/features/visibility_replication_contract.md`
8. `docs/plans/control_handoff_input_prediction_stabilization_plan_2026-04-27.md`

## 0. Implementation Status

2026-04-27 plan note:

1. This is a plan only. It does not implement the new IFCS stack.
2. Current live flight is still the legacy path:
   - `ActionQueue` -> mutable `FlightComputer.throttle/yaw_input/brake_active`,
   - aggregate mounted `Engine` budgets by parent GUID,
   - compute one hull-level force and one hull-level torque,
   - apply through Avian `Forces`.
3. Current live navigation is only a primitive `ScriptNavigationTarget` path. It maps target position to coarse throttle/yaw/brake and does not support proper arrival, route queues, cruise, tactical map orders, or capital-ship autopilot.
4. This plan keeps the current "magic engine" usable as Phase 1. That magic engine should be modeled as a temporary omnidirectional actuator with partial output, not as the final architecture.
5. Native impact: client prediction must run the same IFCS/controller/allocation math as the replication server for the active predicted entity.
6. WASM impact: IFCS, guidance, allocation, and prediction logic must remain shared target-compatible code. Browser differences belong only in input device, UI, transport, and asset loading boundaries.

2026-04-28 coordination note:

1. Runtime IFCS integration is blocked until the control-lease/control-handoff refactor lands and is validated.
2. Reason: this plan's runtime path assumes exactly one input-authorized active predicted entity. The current refactor in progress is removing the old split-brain handoff behavior, fallback input binding, and motion/reconciliation shims that would otherwise contaminate IFCS.
3. Safe parallel IFCS work before the control refactor lands:
   - Phase 0 harness and diagnostics for current speed, brake, turn, afterburner, and cruise baselines.
   - Pure math modules/tests for flight envelopes, braking distance, shortest-angle heading logic, and allocator shape.
   - Documentation refinement and component boundary review.
   - Data audit of current authored movement values without wiring new runtime behavior into live input/control.
4. Blocked until the control refactor lands:
   - wiring IFCS into live `PlayerInput`, `ActionQueue`, `FlightComputer`, `process_flight_actions`, or `apply_engine_thrust`,
   - adding navigation/control protocol messages that depend on final control-lease validation APIs,
   - touching live authority paths in `bins/sidereal-client/src/runtime/input.rs`, `control.rs`, `motion.rs`, replication control-tag/bootstrap systems, `bins/sidereal-replication/src/replication/input.rs`, or `control.rs`,
   - rewriting live `crates/sidereal-game/src/flight.rs` beyond isolated tests/harness extraction.
5. When the control refactor lands, update this plan with the actual landed lease resource/module names, tests, and removed fallback paths before starting IFCS runtime implementation.

## 1. Online References Used

These external references were used for design guidance, not as hard dependencies:

1. NASA/JPL actuator allocation summary: https://www.techbriefs.com/component/content/article/24816-npo-49675
   - Design implication: the control computer should calculate desired force/torque first; a separate allocator maps that desired wrench to individual actuators under constraints and saturation. Prioritization matters when actuators cannot satisfy every requested degree of freedom.
2. Craig Reynolds, "Steering Behaviors For Autonomous Characters": https://ics-websites.science.uu.nl/docs/vakken/mcrws/papers_new/Reynolds%20-%201999%20-%20Steering%20behaviors%20for%20autonomous%20characters.pdf
   - Design implication: keep high-level goals, steering/guidance, and low-level locomotion separate. Sidereal's equivalent is order/plan -> guidance -> IFCS/allocator -> physics.
3. Red Blob Games A* pathfinding: https://www.redblobgames.com/pathfinding/a-star/introduction.html
   - Design implication: pathfinding returns graph/path structure, not physical movement. Sidereal's route planner should produce waypoints or corridors; guidance/IFCS still decide how to fly them.
4. Bevy fixed timestep docs: https://docs.rs/bevy/latest/bevy/time/struct.Fixed.html
   - Design implication: IFCS math belongs in fixed-step simulation. Render/UI frame time must not drive authoritative flight.
5. Avian forces docs: https://docs.rs/avian2d/latest/avian2d/dynamics/rigid_body/forces/index.html
   - Design implication: the final actuation layer should continue applying forces/torques to dynamic bodies instead of writing transforms/velocities directly.
6. WPILib PID/introduction to controls: https://docs.wpilib.org/en/stable/docs/software/advanced-controls/introduction/introduction-to-pid.html
   - Design implication: tune velocity/heading controllers around critically damped or slightly underdamped response, with explicit rise/settle targets and test harnesses.
7. Starsector movement reference: https://starsector.fandom.com/wiki/Movement
   - Design implication: top-down space combat feel is usually explicit max speed, acceleration, deceleration, max turn rate, and turn acceleration per hull class, not pure Newtonian realism.
8. Cosmoteer thruster reference: https://cosmoteer.wiki.gg/wiki/Thrusters
   - Design implication: modular ship-building games make thrust, mass, ramp-up, power/fuel, and thruster orientation part of ship feel. Sidereal should preserve that without exposing raw thruster control to the pilot.

## 2. Target Player-Facing Model

The correct player-facing explanation is:

```text
The pilot does not directly set ship velocity or rotation.
The pilot gives commands to IFCS.
IFCS turns those commands into desired translation and rotation.
IFCS computes the required force and torque.
The allocator assigns available propulsion resources and thrusters by priority.
Actuators apply partial thrust over time.
Sensor feedback from actual position, velocity, rotation, and angular velocity corrects the result.
```

The same model applies to AI and scripts:

```text
Player right-click order
AI tactical decision
Lua/script route command
fleet/capital ship command
all become navigation or motion intent
```

No producer should bypass IFCS to write direct engine throttle, direct hull velocity, direct rotation, or direct transform.

## 3. Current Baseline And Gaps

### 3.1 Current live components

Current ship flight data is in:

1. `crates/sidereal-game/src/components/flight_computer.rs`
   - currently stores profile string, throttle, yaw input, brake flag, turn rate.
2. `crates/sidereal-game/src/components/engine.rs`
   - currently stores forward thrust, reverse thrust, torque thrust, fuel burn.
3. `crates/sidereal-game/src/components/afterburner_capability.rs`
   - currently stores multiplier, burn multiplier, optional max afterburner velocity.
4. `crates/sidereal-game/src/components/flight_tuning.rs`
   - currently stores max linear accel, passive/active brake accel, drag, but drag is not currently used by `compute_flight_forces`.
5. `crates/sidereal-game/src/components/max_velocity_mps.rs`
   - currently stores one normal velocity cap.
6. `crates/sidereal-game/src/components/script_navigation_target.rs`
   - runtime-only target point emitted by scripts.
7. `crates/sidereal-game/src/flight.rs`
   - current authority system and current navigation-to-flight-computer bridge.

### 3.2 Current data mismatch with desired feel

The current corvette bundle does not encode the intended movement envelope:

1. `data/scripts/bundles/ship/corvette.lua` has `max_velocity_mps = 100.0`, not `30-60`.
2. It has `max_afterburner_velocity_mps = 250.0`, not about `100`.
3. There is no explicit cruise envelope for `400-500`.
4. `active_brake_accel_mps2` equals `passive_brake_accel_mps2`, so the brake command is not a strong distinct control behavior.
5. `turn_rate_deg_s = 90.0` plus the global angular velocity clamp around `2 rad/s` limits turn feel.

### 3.3 Current architectural gaps

1. The pilot currently influences raw throttle/yaw fields, not desired motion.
2. Script navigation currently writes `ScriptNavigationTarget`, then Rust writes raw `FlightComputer` throttle/yaw/brake.
3. AI and player "move here" cannot share a durable order pipeline yet.
4. There is no `NavGoal`, `WaypointQueue`, route corridor, or arrival policy.
5. There is no explicit `FlightAssistMode` / flight envelope component.
6. There is no desired velocity or desired angular velocity component.
7. There is no desired wrench component.
8. There is no actuator command/state component.
9. Engines are aggregated as scalar budgets and do not use real hardpoint offset, thrust axis, or mount transform.
10. Engines do not have first-class partial thrust/spool state.
11. Plume presentation still depends on hull-level guesses and engine presence rather than authoritative actuator command/state.
12. The allocator cannot express "torque first", "heading first", "lateral correction second", or "do not waste main engine thrust while docking".

## 4. Non-Negotiable Invariants

1. Authority flow remains one-way: client input/order -> shard sim -> replication/distribution -> persistence.
2. Clients never authoritatively set world transforms, velocities, rotations, engine commands, or force/torque.
3. Player, AI, and scripts may emit intent only.
4. IFCS/controller/allocation math runs in shared gameplay code.
5. IFCS/controller/allocation math runs only in fixed-step simulation.
6. The actuation layer applies Avian-compatible force and torque, not direct transform writes.
7. Authoritative world coordinates remain f64.
8. Server/client prediction parity must use the same shared systems for active predicted entities.
9. Runtime systems should use generic entity/control/propulsion/actuator terminology unless behavior is truly ship-only.
10. Actuator/thruster state must remain compatible with future directional hardpoints and damage/fuel/power constraints.
11. Navigation orders and route plans are authority-sensitive data. Do not replicate them publicly unless a later feature intentionally exposes tactical intent.
12. IFCS must not introduce a second control-authority path. It must consume the finalized generic control lease from the handoff refactor.
13. If no exact active predicted lease entity exists, IFCS input/prediction integration must go inert and surface diagnostics. It must not fallback to confirmed/interpolated entities.

## 4.1 Control-Lease Dependency Gate

This plan depends on the separate control-lease/handoff cleanup. Do not start IFCS runtime integration until that work has landed and these gates are true:

1. There is one generic control lease state machine for ships, player-anchor/free-roam, and future controllables.
2. The active lease carries at least:
   - `player_entity_id`,
   - `controlled_entity_id`,
   - `control_generation`,
   - control/profile/capability metadata,
   - exact local prediction entity when active.
3. Client input authority reads only that active lease.
4. Client pending states send no active input.
5. `send_lightyear_input_messages` does not authorize input through `resolve_entity_by_guid_prefer_predicted` or any confirmed/interpolated fallback resolver.
6. `enforce_single_input_marker_owner` keeps only the exact active lease entity.
7. Accepted server handoff clears short-lived realtime input state, neutralizes old and new targets, updates the authoritative lease, updates persistence, updates replication roles, and ACKs from that state.
8. Rejected server handoff returns the authoritative lease, and the client follows it explicitly.
9. Free-roam/player-anchor is a first-class control target, not a backup or auto-selected fallback.
10. Persisted self/free-roam control is not overwritten by owned-ship discovery/bootstrap.
11. Tests cover ship -> free roam -> ship, ship A -> ship B, repeated handoff loops, disconnect/reconnect, stale generation, wrong target, and wrong player.

The IFCS runtime implementation must integrate with the landed lease API. It must not create a parallel `ActiveControlledEntity`, `CurrentShip`, or IFCS-specific authority resource.

## 5. Target Architecture

The target stack is:

```text
Order Source
  player input, tactical map click, AI, script

Order Authority
  validates actor, controlled entity, ownership/control generation, permissions

Navigation Plan
  NavGoal / WaypointQueue / RoutePlan / arrival policy

Guidance
  target waypoint selection, arrival speed, desired heading, obstacle/fleet constraints

Desired Motion
  DesiredWorldVelocity / DesiredLocalVelocity / DesiredAngularVelocity

IFCS Controller
  velocity/heading feedback -> DesiredWrench

Allocator
  DesiredWrench + actuator capabilities/state -> ActuatorCommand

Actuator Response
  partial thrust, spool, fuel/power/damage limits -> ActuatorState + allocated wrench

Actuation
  apply force/torque through Avian

Physics Feedback
  Position / Rotation / LinearVelocity / AngularVelocity -> next fixed tick

Presentation
  plumes, UI, telemetry from ActuatorState / ControlAuthorityEstimate
```

## 6. Conceptual Behavior

### 6.1 Manual combat flight

Keyboard/gamepad input produces desired motion:

1. Forward input requests desired local velocity `(0, combat_forward_cap)`.
2. Backward input requests desired local velocity `(0, -combat_reverse_cap)` or brake/reverse depending mode.
3. Strafe input requests desired local velocity `(strafe_cap, current_forward_target)` if the hull has lateral authority.
4. Left/right input requests desired angular velocity, not direct torque.
5. Brake requests desired local/world velocity near zero and high angular damping.
6. Afterburner requests a temporary forward envelope if actuator/fuel conditions allow it.
7. Cruise requests the cruise envelope with long-distance guidance and constrained turning.

### 6.2 Right-click "move here"

The client sends an order to the server:

```rust
ClientNavigationOrderMessage {
    player_entity_id: String,
    controlled_entity_id: String,
    control_generation: u64,
    request_seq: u64,
    order: NavigationOrderDto,
}

NavigationOrderDto::MoveTo {
    target_position_xy_m: [f64; 2],
    queue_mode: Replace | Append,
    flight_mode: Option<FlightMode>,
    arrival_radius_m: f32,
    desired_final_speed_mps: f32,
}
```

Server validation:

1. authenticated session must match `player_entity_id`,
2. `controlled_entity_id` must match current authoritative control lease or an allowed command target,
3. `control_generation` must be current,
4. target position must be finite f64,
5. queue length must be bounded,
6. command scope must respect future fleet/trust permissions,
7. order is applied server-side as a `NavGoal` or `WaypointQueue` on the commanded entity.

The client may show a local pending order marker, but the authoritative route marker should come from server ACK or replicated owner-only route state.

### 6.3 AI and script "move here"

AI and Lua scripts must feed the same order pipeline:

1. AI may emit `NavigationOrder::MoveTo`, `Follow`, `Orbit`, `AttackApproach`, `Flee`, or `Dock`.
2. Lua may emit `set_nav_goal`, `append_waypoint`, `set_flight_mode`, or `cancel_nav_goal`.
3. Rust validates the entity is script-controllable or AI-controllable.
4. Rust writes `NavGoal` / `WaypointQueue`.
5. Guidance and IFCS do the rest.

Scripts must not write `FlightComputer.throttle`, `FlightComputer.yaw_input`, `DesiredWrench`, or `ActuatorCommand`.

## 7. Component Model

Names may be adjusted during implementation, but preserve the boundaries.

### 7.1 Persistent or replicated gameplay components

#### `FlightComputer`

Evolve from raw throttle/yaw storage into durable IFCS policy:

```rust
pub struct FlightComputer {
    pub profile_id: String,
    pub assist_mode: FlightAssistMode,
    pub active_flight_mode: FlightMode,
    pub autopilot_enabled: bool,
}
```

Avoid keeping per-tick raw throttle/yaw fields as canonical behavior. During migration they may remain as compatibility fields but should stop being the lower-level control surface.

#### `FlightMode`

Recommended enum:

```rust
pub enum FlightMode {
    Precision,
    Combat,
    Afterburner,
    Cruise,
    Drift,
}
```

`Afterburner` may also remain a transient modifier on top of `Combat`, but the envelope must be explicit.

#### `FlightEnvelope`

Authorable per hull/profile/mode:

```rust
pub struct FlightEnvelope {
    pub mode: FlightMode,
    pub max_forward_speed_mps: f32,
    pub max_reverse_speed_mps: f32,
    pub max_lateral_speed_mps: f32,
    pub max_linear_accel_mps2: f32,
    pub max_linear_decel_mps2: f32,
    pub passive_damping_mps2: f32,
    pub max_turn_rate_rad_s: f32,
    pub max_turn_accel_rad_s2: f32,
    pub heading_kp: f32,
    pub velocity_kp: f32,
    pub lateral_damping_kp: f32,
    pub cruise_spool_up_s: f32,
    pub cruise_spool_down_s: f32,
}
```

Initial suggested corvette envelope targets:

1. Precision: `10-25 m/s`, high damping, strong braking.
2. Combat: `30-60 m/s`, responsive heading, moderate drift.
3. Afterburner: `90-120 m/s`, forward-biased, reduced turn/brake authority.
4. Cruise: `400-500 m/s`, long spool, weak turn authority, no tight combat steering.

#### `NavigationPlan`

Durable or semi-durable authoritative order state:

```rust
pub struct NavigationPlan {
    pub mode: NavigationMode,
    pub queue: Vec<Waypoint>,
    pub active_index: u16,
    pub queue_mode: NavigationQueueMode,
    pub arrival_radius_m: f32,
    pub desired_final_speed_mps: f32,
}

pub struct Waypoint {
    pub position_xy_m: DVec2,
    pub desired_speed_mps: Option<f32>,
    pub flight_mode: Option<FlightMode>,
}
```

Persistence decision:

1. Player-issued capital/fleet move orders should probably persist eventually.
2. Short tactical click-to-move orders can start runtime-only if persistence is risky.
3. AI patrol routes should persist if they are authored behavior state.
4. Make the first implementation explicit in docs: either runtime-only or persisted, not accidental.

Replication decision:

1. Owner-only by default.
2. Fleet/trust-shared later.
3. Not public by default because route intent can reveal private plans.

#### `PropulsionActuator`

Replace/evolve `Engine` into a generic actuator:

```rust
pub struct PropulsionActuator {
    pub actuator_id: String,
    pub local_force_axis: Vec2,
    pub local_mount_position_m: Vec2,
    pub max_positive_force_n: f32,
    pub max_negative_force_n: f32,
    pub throttle_response_up_s: f32,
    pub throttle_response_down_s: f32,
    pub min_command: f32,
    pub max_command: f32,
    pub allocator_priority: ActuatorPriority,
    pub fuel_burn_rate_kg_s_at_full: f32,
    pub supports_afterburner: bool,
}
```

For a unidirectional rocket thruster:

```text
min_command = 0.0
max_command = 1.0
local_force_axis = direction the thruster pushes the hull
```

For a bidirectional or magic/virtual actuator:

```text
min_command may be -1.0
max_command may be 1.0
```

### 7.2 Runtime authoritative/predicted components

#### `PilotMotionIntent`

Transient intent from keyboard/gamepad/direct control:

```rust
pub struct PilotMotionIntent {
    pub desired_local_translation: Vec2,
    pub desired_turn: f32,
    pub brake: bool,
    pub afterburner: bool,
    pub cruise: bool,
}
```

This replaces raw `FlightComputer.throttle/yaw_input` as the immediate input output.

#### `DesiredMotion`

Guidance output:

```rust
pub struct DesiredMotion {
    pub desired_world_velocity_mps: DVec2,
    pub desired_heading_rad: Option<f64>,
    pub desired_angular_velocity_rad_s: Option<f32>,
    pub flight_mode: FlightMode,
    pub brake: bool,
}
```

Manual input, navigation, AI, and script guidance all converge here.

#### `DesiredWrench`

IFCS output:

```rust
pub struct DesiredWrench {
    pub force_world_n: Vec2,
    pub torque_z_nm: f32,
    pub priority: WrenchPriority,
}
```

Normally runtime-only and not persisted.

#### `ActuatorCommand`

Allocator output:

```rust
pub struct ActuatorCommand {
    pub normalized: f32,
    pub afterburner: bool,
    pub command_generation: u64,
}
```

This is where partial thrust lives. `normalized = 0.37` means IFCS only needs 37 percent of that actuator's current available authority.

#### `ActuatorState`

Actuator response output:

```rust
pub struct ActuatorState {
    pub normalized: f32,
    pub available_force_scale: f32,
    pub fuel_limited: bool,
    pub disabled: bool,
    pub last_allocated_force_world_n: Vec2,
    pub last_allocated_torque_z_nm: f32,
}
```

This is the correct source for plume intensity and engine telemetry.

#### `ControlAuthorityEstimate`

Debug/HUD/AI helper:

```rust
pub struct ControlAuthorityEstimate {
    pub requested_force_world_n: Vec2,
    pub allocated_force_world_n: Vec2,
    pub requested_torque_z_nm: f32,
    pub allocated_torque_z_nm: f32,
    pub saturated: bool,
}
```

This lets UI, AI, and logs say "cannot satisfy requested lateral thrust" instead of hiding poor thruster layouts.

## 8. Flight Mode Design

### 8.1 Precision

Purpose:

1. docking,
2. looting/salvage,
3. formation trim,
4. close capital-ship maneuvering.

Behavior:

1. low speed cap,
2. strong damping,
3. high braking authority,
4. high heading stability,
5. low overshoot tolerance.

### 8.2 Combat

Purpose:

1. normal top-down piloting,
2. dogfighting,
3. weapon-facing maneuvers.

Behavior:

1. small ship cap around `30-60 m/s`,
2. responsive turn acceleration,
3. deliberate but controllable drift,
4. strong enough lateral damping to feel assisted,
5. no full Newtonian coast unless `Drift` mode is explicitly selected.

### 8.3 Afterburner

Purpose:

1. burst reposition,
2. evade,
3. close distance.

Behavior:

1. cap around `90-120 m/s` for the small/corvette class target stated in this discussion,
2. forward-biased,
3. consumes fuel/heat/power,
4. reduced turn/lateral authority,
5. exits when released, fuel-starved, overheated, or mode changes.

### 8.4 Cruise

Purpose:

1. non-combat travel,
2. capital ship movement,
3. tactical map "move here" over long distance.

Behavior:

1. cap around `400-500 m/s` for early target tuning,
2. has spool-up and spool-down,
3. should not be instant,
4. weak turning and lateral authority,
5. computes braking distance and exits early enough to arrive cleanly,
6. may disable weapons/shields or change scanner/signature later if design wants.

Important: cruise must not be implemented as "normal max velocity = 500". It is a distinct guidance/IFCS mode with different turn, accel, braking, and arrival behavior.

## 9. Guidance And Arrival Logic

### 9.1 Direct move-to guidance

Given:

```text
current position p
target position t
current velocity v
available decel a_decel
arrival radius r
desired final speed vf
```

Compute:

```text
to_target = t - p
distance = length(to_target)
braking_distance = max((speed^2 - vf^2) / (2 * a_decel), 0)
allowed_speed = sqrt(max(vf^2 + 2 * a_decel * max(distance - r, 0), 0))
target_speed = min(mode_speed_cap, allowed_speed)
desired_velocity = normalize(to_target) * target_speed
```

Then add:

1. cross-track correction,
2. lateral drift damping,
3. heading target,
4. waypoint advance when within arrival radius and speed condition.

### 9.2 Heading behavior

Suggested first rules:

1. Combat manual mode: heading follows player aim or current turn input.
2. Move-to mode: heading aligns with desired velocity unless an order says preserve facing.
3. Attack approach: heading may face target while velocity follows intercept/orbit.
4. Cruise: heading aligns with route direction; turn rate is limited.
5. Capital ships: avoid trying to face instantly. Use turn anticipation and larger arrival radii.

### 9.3 Route queue

Waypoint queue behavior:

1. `Replace`: clear existing queue and set the new point.
2. `Append`: add to existing queue if below max queue length.
3. `Insert`: optional later for fleet commands.
4. `Cancel`: clear queue and enter brake/hold.

Waypoint advancement should require both:

1. distance within radius,
2. speed below allowed waypoint transition speed unless the waypoint is pass-through.

### 9.4 Obstacle avoidance

First implementation:

1. no expensive global avoidance,
2. direct line plus simple steering offset around immediate hazards,
3. avoid pushing obstacle avoidance into IFCS.

Later implementation:

1. route planner uses sector/cell graph,
2. local guidance blends route direction with avoidance,
3. IFCS still only sees desired motion.

## 10. IFCS Controller

The IFCS controller converts desired motion and current feedback into desired wrench.

### 10.1 Linear controller

Inputs:

1. `DesiredMotion.desired_world_velocity_mps`,
2. current `LinearVelocity`,
3. active `FlightEnvelope`,
4. mass from `TotalMassKg` / Avian `Mass`.

First implementation:

```text
velocity_error = desired_velocity - current_velocity
desired_accel = velocity_error * velocity_kp
desired_accel = clamp_length(desired_accel, max_accel_for_mode)
desired_force = desired_accel * mass
```

For brake:

```text
desired_velocity = Vec2::ZERO
max_accel_for_mode = max_linear_decel_mps2
```

For lateral assist:

```text
local_velocity = world_to_local(current_velocity)
desired_local_velocity.x = input_or_guidance_lateral_target
if no lateral input:
    desired_local_velocity.x = 0
```

### 10.2 Angular controller

Inputs:

1. desired heading or desired angular velocity,
2. current `Rotation`,
3. current `AngularVelocity`,
4. active `FlightEnvelope`,
5. angular inertia.

First implementation:

```text
heading_error = shortest_angle(desired_heading - current_heading)
target_omega = clamp(heading_error * heading_kp, -max_turn_rate, max_turn_rate)
omega_error = target_omega - current_omega
desired_alpha = clamp(omega_error * angular_kp, -max_turn_accel, max_turn_accel)
desired_torque = desired_alpha * angular_inertia
```

This should replace the current one-step "hit target angular velocity this tick" behavior, which can feel either sluggish or harsh depending torque saturation.

### 10.3 Tuning principle

Tune with step-response tests:

1. time to reach 90 percent of target speed,
2. overshoot percentage,
3. time to settle below small velocity/heading error,
4. turn response for frigate/corvette/capital hulls,
5. braking distance at combat, afterburner, and cruise speeds.

Default goal:

1. manual combat response should be critically damped or slightly underdamped,
2. capital movement should be slower but predictable, not mushy,
3. cruise should feel powerful but committed.

## 11. Allocation Model

### 11.1 Temporary magic actuator

The current engine behaves like a magic engine that can output force through the center of mass and torque independently. Keep this for Phase 1 as a virtual actuator model:

```rust
pub enum ActuatorModel {
    OmnidirectionalMagic {
        max_force_n: f32,
        max_reverse_force_n: f32,
        max_lateral_force_n: f32,
        max_torque_nm: f32,
    },
    DirectionalThruster,
}
```

The magic actuator must still support partial output:

1. IFCS requests a force/torque.
2. The allocator clamps that requested wrench to magic actuator limits.
3. It produces normalized command values below 1.0 when full power is not needed.
4. It drives `ActuatorState` and plumes from the normalized command.

This gives good player/AI/autopilot behavior before directional thruster math lands.

### 11.2 Directional actuator math

For each directional actuator:

```text
axis_world = rotate(ship_rotation, local_force_axis)
mount_world = rotate(ship_rotation, local_mount_position_m)
force_i = axis_world * max_force_i * command_i
torque_i = cross_z(mount_world, force_i)
```

Column contribution:

```text
B_i = [ force_i.x, force_i.y, torque_i.z ]
```

The allocator tries to find actuator command vector `u` such that:

```text
B * u ~= desired_wrench
u_min <= u <= u_max
rate_limit(previous_u, u)
```

### 11.3 V1 allocator

Use a deterministic bounded allocator, not a random or allocation-heavy solver.

Acceptable V1 approach:

1. Build a stable ordered list of actuators by entity GUID or actuator id.
2. Compute each actuator's contribution vector for command = 1.
3. Run priority passes:
   - pass 1: satisfy torque or heading-critical axis,
   - pass 2: satisfy forward/longitudinal force,
   - pass 3: satisfy lateral force/drift correction,
   - pass 4: minimize residual by filling remaining useful actuators.
4. Clamp each command to bounds.
5. Apply response-rate limits in the actuator response system, not directly in the solver.
6. Record residual as `ControlAuthorityEstimate`.

Later allocator:

1. projected least squares,
2. active-set bounded least squares,
3. sequential quadratic programming only if needed and profiled,
4. cache matrix scratch buffers by entity to avoid allocations.

### 11.4 Priority policy by mode

Combat:

1. heading/torque high priority,
2. forward force high priority,
3. lateral drift correction medium priority,
4. fuel efficiency low priority.

Precision:

1. position hold and lateral correction high priority,
2. torque high priority,
3. speed low priority.

Afterburner:

1. forward acceleration high priority,
2. heading stability medium priority,
3. lateral correction low priority.

Cruise:

1. route-aligned acceleration high priority,
2. heading stability high priority,
3. lateral correction medium priority,
4. aggressive turns disallowed.

Damaged/degraded:

1. keep torque/heading stable if selected policy demands it,
2. expose force shortfall,
3. do not fake missing authority.

## 12. Actuator Partial Thrust And Spool

Every actuator should have command and state.

```text
command: what IFCS/allocator wants now
state: what the actuator is actually producing after response/fuel/damage limits
```

Response:

```text
if command > state:
    state += dt / throttle_response_up_s
else:
    state -= dt / throttle_response_down_s
state = clamp(state, min_command, max_command)
```

Fuel/power/damage:

1. server applies fuel burn based on actuator state and afterburner state,
2. client prediction reads replicated fuel availability but does not burn durable fuel,
3. power starvation or damage reduces `available_force_scale`,
4. allocator sees current availability next tick.

This produces believable partial thrust and visible ramping without letting the player command engine percentages directly.

## 13. Network And Authority Changes

### 13.1 New order channel

Add a reliable client-to-server order path for navigation commands, separate from realtime input ticks.

Do not implement this live protocol until the control-lease refactor has landed. Navigation order validation must call the same centralized lease authority used by realtime input and control handoff.

Suggested messages in `sidereal-net`:

1. `ClientNavigationOrderMessage`
2. `ServerNavigationOrderAckMessage`
3. `ServerNavigationOrderRejectMessage`

Validation must mirror control request security:

1. authenticated session binding,
2. canonical player id,
3. current controlled entity id or explicit fleet-command permission,
4. current control generation for directly controlled orders,
5. finite f64 coordinates,
6. bounded waypoint count,
7. bounded message size and rate.

### 13.2 Prediction

Manual input remains realtime and prediction-friendly:

```text
PlayerInput actions -> PilotMotionIntent -> DesiredMotion -> IFCS -> allocator -> Avian
```

This chain is valid only after the client has an active input-authorized lease. During `PendingServerAck`, `PendingPredicted`, rejected/correcting states, or missing predicted entity states, IFCS must not attach local input authority or emit active prediction intent.

Navigation orders are reliable authoritative commands:

1. client sends order,
2. server ACKs,
3. owner receives owner-only replicated `NavigationPlan`,
4. active predicted client may apply a plan only after server ACK and only through the active lease.

Do not let unacknowledged map clicks mutate authoritative local prediction state silently.

### 13.3 Visibility and privacy

Navigation plans and target positions are private tactical intent by default:

1. owner-only replication,
2. future fleet/trust sharing,
3. never public just because an observer sees the ship,
4. observers infer intent only from visible motion, not from route data.

## 14. Scheduling

Target fixed-step order:

```text
FixedPreUpdate:
  1. client prediction input bridge reads the active control lease and writes PlayerInput ActionState only for the lease entity
  2. authoritative/server input drain validates against the server lease and writes ActionQueue or PilotMotionIntent

FixedUpdate before SiderealSimulationSet::SimulateGameplay:
  3. process pilot input/actions into PilotMotionIntent
  4. apply server-authoritative navigation orders to NavigationPlan
  5. advance NavigationPlan / guidance into DesiredMotion
  6. IFCS converts DesiredMotion into DesiredWrench
  7. allocator converts DesiredWrench into ActuatorCommand
  8. actuator response/fuel/damage converts ActuatorCommand into ActuatorState
  9. actuation applies net force/torque through Avian Forces

Avian physics:
  10. integrates forces/torques

FixedPostUpdate:
  11. post-physics telemetry, visibility, tactical lanes, persistence dirty marking
```

Keep entrypoints thin. Put the systems in shared `crates/sidereal-game` modules, then wire plugins from server/client.

## 15. Player Input And UI Changes

### 15.1 Direct controls

Refactor input mapping:

1. `Forward` means request local forward speed in current mode.
2. `Backward` means reverse target or brake based on mode/profile.
3. `Brake` means desired velocity zero with high decel.
4. `Left`/`Right` means desired turn or heading adjustment.
5. Afterburner toggles/request modifies the envelope.
6. Cruise key toggles/request enters cruise mode if allowed.

### 15.2 Right-click world command

Native client:

1. right-click in world projects cursor to world f64 coordinate,
2. if controlling a commandable entity, build `MoveTo` order,
3. hold modifier to append to route,
4. send reliable navigation order message,
5. show pending marker until ACK/reject,
6. push persistent dialog error for rejected critical orders.

### 15.3 Tactical map command

Tactical map:

1. convert map coordinate to world f64 coordinate without f32 truncation,
2. use same `MoveTo` order message,
3. support append/replace,
4. display owner-only route/waypoint overlay,
5. avoid showing route for unknown/redacted contacts.

### 15.4 HUD/diagnostics

Add debug overlay fields:

1. flight mode,
2. desired speed,
3. actual speed,
4. desired angular velocity,
5. actual angular velocity,
6. requested wrench,
7. allocated wrench,
8. saturation flag,
9. active waypoint,
10. cruise spool state,
11. actuator count and top saturated actuator.

## 16. AI And Scripting Changes

### 16.1 Scripting

Change `bins/sidereal-replication/src/replication/runtime_scripting.rs`:

1. `set_navigation_target` should write `NavigationPlan`/`NavGoal`, not `ScriptNavigationTarget`.
2. `stop` should clear navigation plan and request brake/hold, not directly mutate raw throttle/yaw.
3. Add script intents:
   - `set_nav_goal`,
   - `append_waypoint`,
   - `clear_nav_goal`,
   - `set_flight_mode`,
   - `set_desired_local_velocity` for trusted simple behaviors.
4. Keep scripts blocked from:
   - `ActuatorCommand`,
   - `ActuatorState`,
   - `DesiredWrench`,
   - Avian `Position`/`Rotation`/velocity writes.

### 16.2 AI

AI should produce the same orders:

1. patrol: queue waypoints,
2. intercept: set moving target guidance later,
3. flee: set destination away from threat,
4. orbit: set orbit guidance,
5. attack approach: set desired range and facing policy.

Do not give AI a separate flight-control path.

## 17. Asset And Authoring Changes

### 17.1 Lua bundles

Update `data/scripts/bundles/ship/corvette.lua` and `rocinante.lua`:

1. add flight envelope/profile authoring,
2. lower combat cap to intended range,
3. set afterburner cap around intended range,
4. add cruise cap/spool,
5. replace or mirror `engine` with `propulsion_actuator`,
6. add local thrust axis and mount position,
7. add actuator response timing,
8. add plume emitter profile id.

During migration, keep generated graph compatibility only if the project decides to keep old databases. AGENTS currently says early-development schema discipline is strict, so prefer one canonical schema and reset dev DBs rather than adding legacy shims.

### 17.2 Component registry

New persistable/replicated components need:

1. one primary component per file under `crates/sidereal-game/src/components/`,
2. `#[sidereal_component(...)]` annotations,
3. reflect + serde,
4. registry/generation updates,
5. graph persistence/hydration tests.

### 17.3 Dashboard/editor

Dashboard component editors will need schema support for:

1. flight envelopes,
2. navigation plans if persisted/editable,
3. propulsion actuator vectors,
4. plume emitter profile ids,
5. actuator health/state if debug-visible.

## 18. Presentation Changes

Thruster/plume visuals should read actuator presentation state:

1. local predicted entity may use predicted `ActuatorState`,
2. remote observers use replicated presentation state,
3. engine label scans should be removed,
4. plume direction follows engine/hardpoint transform,
5. plume length/intensity follows actual partial thrust,
6. afterburner effect follows actual state, not raw key press.

If `ActuatorState` is sensitive, split it:

1. owner-only detailed state,
2. public/observer `PropulsionEffectState` with safe plume intensity and variant only.

## 19. Implementation Phases

### Phase -1: Control lease dependency

This phase is owned by the control-handoff refactor, not the IFCS implementation.

Required before IFCS runtime wiring:

1. one client active/pending control lease resource/module,
2. one server canonical control lease authority,
3. no fallback input binding to confirmed/interpolated entities,
4. no emergency motion/reconciliation writers used as behavior fixers,
5. no server Lightyear native input runtime as the authoritative input path,
6. active input marker/writer ownership driven only by the active lease,
7. handoff/disconnect cleanup clears old input immediately,
8. the tests listed in `docs/plans/control_handoff_input_prediction_stabilization_plan_2026-04-27.md` pass.

IFCS implementers should re-read the landed code and update this document with exact API names before Phase 1 begins.

### Phase 0: Harness and diagnostics

This phase is safe to do in parallel with the control refactor if it stays out of live control/input wiring.

1. Add tests that measure current corvette speed/turn/brake response.
2. Add a small shared simulation step-response harness for:
   - forward speed step,
   - brake from 60 m/s,
   - afterburner from 60 to 100 m/s,
   - cruise from 0 to 500 m/s then arrival stop,
   - 90-degree and 180-degree turns.
3. Log current values so tuning changes are measurable.
4. Keep any new test-only helpers isolated from live runtime scheduling.
5. Do not change `send_lightyear_input_messages`, server input validation, control handoff, or live flight authority in this phase.

Done when:

1. current feel has numeric baselines,
2. future changes can prove response improvements.

### Phase 1: Flight envelopes with current magic engine

Blocked until Phase -1 is complete.

1. Add `FlightMode` and `FlightEnvelope`/profile components.
2. Add `PilotMotionIntent` and `DesiredMotion`.
3. Convert manual input actions into desired motion only through the active control lease.
4. Keep current aggregate engine force application initially.
5. Replace single max velocity use with active envelope cap.
6. Make brake, afterburner, and cruise distinct modes.

Done when:

1. combat cap can be `30-60 m/s`,
2. afterburner cap can be around `100 m/s`,
3. cruise cap can be `400-500 m/s`,
4. mode-specific accel/decel/turn tuning works with current magic engine.

### Phase 2: Navigation orders and right-click move

Blocked until Phase -1 is complete and the server lease validation API is centralized.

1. Add reliable navigation order protocol messages.
2. Add server validation and ACK/reject through the central lease authority.
3. Add `NavigationPlan` / `WaypointQueue`.
4. Route player right-click and tactical map orders through the server.
5. Add owner-only route replication or ACK-based local route display.
6. Update scripting `set_navigation_target` to write the new plan.

Done when:

1. player can right-click a point and ship moves there,
2. tactical map can issue same order,
3. AI/script can queue waypoints through same path,
4. no producer writes raw throttle/yaw for navigation.

### Phase 3: IFCS feedback controller

1. Implement desired-motion -> desired-wrench controller.
2. Use velocity feedback and heading/angular feedback.
3. Add arrival/braking-distance guidance.
4. Add lateral damping/assist.
5. Add step-response tests.
6. Keep output applied through the current magic engine clamp.

Done when:

1. move-to orders arrive without wild overshoot,
2. capital ships begin braking early,
3. manual combat flight feels assisted rather than raw throttle/yaw.

### Phase 4: Actuator command/state and partial thrust

1. Add `ActuatorCommand`.
2. Add `ActuatorState`.
3. Model the current magic engine as virtual actuator(s).
4. Apply partial command values instead of full thrust whenever IFCS asks for less.
5. Drive plumes from actuator state.
6. Keep server fuel burn authoritative and client prediction non-burning.

Done when:

1. engine state can show 0-100 percent output,
2. IFCS can hold speed with partial thrust,
3. visual plume intensity matches partial thrust.

### Phase 5: Directional hardpoint actuator allocator

1. Add `PropulsionActuator`.
2. Author axis and mount offset in Lua.
3. Build stable actuator contribution matrices per controlled root.
4. Implement deterministic bounded V1 allocator.
5. Compute torque from mount offset cross force.
6. Expose `ControlAuthorityEstimate`.
7. Add tests for symmetric, asymmetric, missing, and damaged actuator layouts.

Done when:

1. directional thrusters can translate/rotate based on placement,
2. poor layouts visibly underperform,
3. IFCS still receives the same desired wrench abstraction,
4. no fake force is invented when actuators cannot produce it.

### Phase 6: Cruise and long-range route planning

1. Add cruise spool state.
2. Add cruise entry/exit rules.
3. Add route/sector path placeholders.
4. For local space, use direct waypoint guidance.
5. For larger regions, add graph pathfinding over sector/cell nodes.
6. Use A*/Dijkstra depending on movement costs and route scope.

Done when:

1. cruise feels distinct from combat,
2. move-to over long distance uses cruise when requested,
3. arrival exits cruise early enough to stop or enter combat/precision cleanly.

### Phase 7: AI/fleet command integration

1. Route patrol AI to `NavigationPlan`.
2. Route pirate patrol script to waypoint queue.
3. Add follow/orbit/flee guidance variants.
4. Add fleet/trust permission model only when needed.
5. Keep all commands server-authoritative.

Done when:

1. player, AI, and scripts share one command/guidance path,
2. no duplicate movement stacks exist.

### Phase 8: Migration cleanup

1. Remove raw throttle/yaw as canonical flight state.
2. Remove or deprecate `ScriptNavigationTarget`.
3. Remove hull-level plume guesses.
4. Update docs and decision register.
5. Reset dev databases if component schema changes require it.

Done when:

1. IFCS stack is the only flight authority path,
2. old aggregate engine code is gone or only exists as a virtual actuator implementation.

## 20. Tests

### 20.1 Shared gameplay unit tests

Add tests under `crates/sidereal-game/tests/`:

1. desired motion clamps to active envelope,
2. brake distance calculation is correct,
3. arrival speed drops as target nears,
4. heading controller picks shortest angular error,
5. IFCS outputs finite wrench for all finite inputs,
6. magic actuator clamps requested wrench,
7. partial thrust command is below 1.0 when full thrust is unnecessary,
8. directional actuator contribution includes force and torque,
9. actuator response ramps up/down deterministically,
10. allocator saturates but records residual shortfall,
11. damaged/disabled actuator is excluded,
12. fuel-limited actuator reduces availability,
13. symmetric layout produces no unwanted torque for straight translation,
14. off-center layout requires counter-thrust or produces documented residual torque.

### 20.2 Server tests

Add tests under replication/gateway crates:

1. navigation order rejects spoofed player id,
2. navigation order rejects stale control generation,
3. navigation order rejects target not owned/controlled,
4. navigation order rejects non-finite coordinates,
5. navigation order applies replace/append bounded queue,
6. script intent writes navigation plan, not raw flight computer demand,
7. AI patrol queue advances through waypoints.

### 20.3 Client tests

Add tests where practical:

1. world right-click projects to f64 world coordinate,
2. tactical map click uses same order DTO,
3. pending route marker clears on reject,
4. route overlay uses owner-only plan data,
5. control handoff clears pending navigation UI for old controlled target.

### 20.4 Integration tests

Native validation scenarios:

1. manual combat forward/brake/turn,
2. afterburner burst,
3. cruise move-to 1000 m and stop,
4. right-click move-to from world view,
5. right-click move-to from tactical map,
6. append three waypoints and verify order,
7. capital ship 180-degree turn and move-to,
8. AI patrol through three waypoints,
9. missing side thruster layout cannot strafe cleanly,
10. damaged engine reduces authority but does not break IFCS.

## 21. Affected Areas

### 21.1 `crates/sidereal-game`

Expected changes:

1. new components,
2. flight module split,
3. shared IFCS plugin/system sets,
4. allocator code,
5. tests,
6. component registry updates,
7. persistence/hydration mapping.

Suggested module split:

```text
crates/sidereal-game/src/flight/
  mod.rs
  actions.rs
  envelopes.rs
  guidance.rs
  ifcs.rs
  allocation.rs
  actuation.rs
  diagnostics.rs
```

### 21.2 `sidereal-net`

Expected changes:

1. navigation order messages,
2. ACK/reject messages,
3. DTOs for waypoints/order modes,
4. protocol registration.

Do not add live navigation/control protocol messages until the control lease module names and validation API have landed.

### 21.3 `bins/sidereal-replication`

Expected changes:

1. validate navigation order messages,
2. apply authoritative navigation plans,
3. update scripting intent application,
4. AI route integration,
5. owner-only route replication if needed,
6. persistence dirty marking for durable nav plans.

Until Phase -1 lands, avoid changes to live replication control/input authority paths except test-only diagnostics coordinated with the handoff refactor.

### 21.4 `bins/sidereal-client`

Expected changes:

1. input mapping to desired intent,
2. right-click world order,
3. tactical map order,
4. route/waypoint UI,
5. debug overlay,
6. plume rendering from actuator state,
7. prediction plugin wiring.

Until Phase -1 lands, avoid live changes to input/control/motion ownership. IFCS must integrate through the finalized active lease instead of `LocalPlayerViewState`, GUID fallback resolution, or an IFCS-specific selected entity resource.

### 21.5 `data/scripts`

Expected changes:

1. ship bundle flight envelopes,
2. propulsion actuator authoring,
3. plume emitter authoring,
4. AI scripts emit nav goals/waypoints,
5. starter player bundle defaults.

### 21.6 Dashboard

Expected changes:

1. component editors for new components,
2. route/order debug panels later,
3. validation schemas if admin spawn/update endpoints accept new payloads.

### 21.7 Docs

Update in same implementation slices:

1. `docs/features/fly_by_wire_thrust_allocation_contract.md`,
2. `docs/features/scripting_support.md`,
3. `docs/features/prediction_runtime_tuning_and_validation.md`,
4. `docs/sidereal_design_document.md` if protocol/runtime defaults change,
5. `docs/decision_register.md` only if a new in-depth decision supersedes DR-0034.

## 22. Performance Considerations

1. Avoid per-tick heap churn in allocator.
2. Cache actuator lists and contribution scratch buffers by controlled root.
3. Rebuild actuator topology only when mount/hardpoint/actuator/health/mass layout changes.
4. Keep V1 allocator deterministic and simple.
5. Use stable ordering by GUID/id so prediction and server allocation match.
6. Do not use HashMap iteration order for solver order.
7. Expose allocator timing metrics before adding a heavier solver.

## 23. Risks And Mitigations

### Oscillation

Risk: IFCS overcorrects and ships wobble.

Mitigation:

1. step-response tests,
2. critically damped tuning targets,
3. response-rate limits,
4. residual/authority telemetry.

### Prediction divergence

Risk: client/server choose different actuator order or different clamping.

Mitigation:

1. shared code,
2. fixed-step time,
3. stable sorted actuator order,
4. identical owner-only actuator data for predicted entity,
5. parity tests.

### Control lease coupling

Risk: IFCS starts before control handoff is stable and accidentally builds on fallback input authority or duplicate motion-writer paths.

Mitigation:

1. block runtime integration until Phase -1 is complete,
2. permit only pure math, harness, docs, and data-audit work in parallel,
3. require IFCS systems to read the finalized active lease,
4. test that missing predicted lease state makes IFCS inert rather than falling back.

### Security/data leakage

Risk: route plans reveal player intent to observers.

Mitigation:

1. owner-only route state,
2. no public nav-plan replication by default,
3. validate order source and target.

### Scope creep

Risk: trying to implement full pathfinding, fleet AI, docking, and perfect allocator at once.

Mitigation:

1. Phase 1 keeps magic actuator,
2. Phase 2 implements direct move-to,
3. Phase 5 adds real directional actuators,
4. advanced pathfinding/fleet behavior waits until direct move-to is stable.

## 24. Quality Gates

Run targeted tests for touched crates. Before marking implementation complete, run:

```bash
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings
CARGO_INCREMENTAL=0 cargo check --workspace
```

Because client behavior will be touched, also run:

```bash
CARGO_INCREMENTAL=0 cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
CARGO_INCREMENTAL=0 cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

If target toolchains or cross-linker are missing locally, record that and keep CI responsible for the gate.

## 25. Definition Of Done

This implementation is done when:

1. Manual combat control uses IFCS desired-motion semantics, not raw throttle/yaw authority.
2. Right-click world move and tactical map move both create authenticated server navigation orders.
3. AI and scripts use the same navigation plan/guidance path as player orders.
4. Ships arrive at a target using braking-distance guidance without overshoot loops in normal cases.
5. Combat, afterburner, and cruise are distinct mode envelopes.
6. Current magic engine supports partial output as a virtual actuator.
7. Directional hardpoint actuators can be introduced without changing player/AI/script command semantics.
8. Actuator command/state drives plume presentation.
9. Server and active client prediction run the same IFCS/allocation logic.
10. Tests prove mode envelopes, arrival, partial thrust, actuator saturation, and order validation.
11. IFCS does not introduce or preserve any fallback input-authority path outside the finalized control lease.
12. Missing predicted lease state produces inert behavior and diagnostics, not fallback control.

## 26. First Implementation Slice Recommendation

Before the control-lease refactor lands, do only:

1. Phase 0 harness/diagnostics,
2. pure math tests for envelopes/braking/heading/allocator shape,
3. data audits of current flight values,
4. documentation refinement.

After the control-lease refactor lands and this document is updated with the landed API names, do this first runtime slice:

1. Add flight modes/envelopes.
2. Add `PilotMotionIntent` and `DesiredMotion`.
3. Convert manual input to desired motion through the active control lease.
4. Keep magic engine but make it an explicit virtual actuator with partial output.
5. Tune corvette combat/afterburner/cruise envelopes to the target ranges.
6. Add step-response tests.

Do not start with full directional actuator allocation. The player-facing feel and the order/guidance boundary need to be stable first. Once those are stable, directional hardpoints become an internal allocator upgrade rather than another rewrite of input, AI, scripting, and navigation.

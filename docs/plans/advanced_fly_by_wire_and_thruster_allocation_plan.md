# Advanced Fly-By-Wire and Thruster Allocation Plan

Status: Proposed implementation plan  
Date: 2026-03-03  
Owners: gameplay runtime + replication simulation + AI runtime + client prediction

Update note (2026-03-13):
- `docs/features/fly_by_wire_thrust_allocation_contract.md` is now the authoritative architecture contract for this work.
- This plan remains useful as precursor execution material, but implementation slices should be derived from the feature contract so Lua boundaries, Avian integration, and plume ownership stay consistent.

Primary references:
- `docs/sidereal_design_document.md`
- `docs/decisions/dr-0013_action_acceptor_control_routing.md`
- `docs/plans/thruster_plumes_afterburner_plan.md`
- `docs/component_authoring_guide.md`

## 1. Goal

Replace the current "magic" center-of-mass engine behavior with a robust fly-by-wire control system that:

1. Accepts pilot or AI intent as desired motion.
2. Determines which mounted thrusters/engines can satisfy that intent.
3. Allocates bounded thrust commands per actuator (location, direction, power, duration).
4. Applies force at mount locations (or equivalent force+torque) in fixed-step simulation.
5. Supports future strafe/lateral thrusters and damaged/partial-actuator states.

The same control stack must serve both players and AI.

## 2. Non-Negotiable Rules

1. Authority remains server-side: `input/AI intent -> authoritative sim -> replication`.
2. Client prediction uses the same shared logic but never becomes authoritative.
3. Persistent/replicated actuator state lives in gameplay components (not ad-hoc side tables).
4. Deterministic fixed-step only for control and allocation math.
5. No direct velocity setting; control outputs forces/torques only.

## 3. Why This Change Is Needed

Current behavior aggregates thrust as if a single ideal actuator exists at center of mass. This blocks:

1. Accurate module/hardpoint-based propulsion behavior.
2. Real strafe and asymmetric damaged-flight behavior.
3. High-fidelity AI/autopilot that reasons about actual actuator limits.
4. Consistent interaction with mass shifts and module swaps.

## 4. Comparative Research Summary (Game Patterns)

Observed patterns worth adopting:

1. Star Citizen IFCS model: per-thruster force at mount location, IFCS allocates to satisfy pilot command and compensates when COM shifts/damage occurs.
   - Reference: RSI engineering post and IFCS documentation.
2. Kerbal-style RCS practice: translation and rotation authority can be separated per thruster/axis, and balancing around COM matters.
3. Space Engineers dampener pattern: automatic stabilization quality depends on directional thruster coverage; degraded coverage should degrade behavior, not fake capability.

Design takeaway for Sidereal:

1. Keep high-level pilot/AI command interface simple.
2. Make low-level actuator availability explicit and physically grounded.
3. Degrade gracefully when directional control authority is missing.

## 5. Target Control Architecture

Three layers, shared between player and AI.

### 5.1 Guidance Layer (intent -> desired motion)

Input:

1. Player actions (`Forward`, `Backward`, `Left`, `Right`, `Brake`, `AfterburnerOn/Off`).
2. AI/autopilot goals (hold heading, intercept, dock, orbit, evade).

Output:

1. `DesiredMotionCommand` in body space:
   - desired linear acceleration vector
   - desired angular acceleration/turn rate
   - mode flags (`stabilize`, `brake`, `precision`, `afterburner_allowed`)

### 5.2 Control Layer (desired motion -> desired wrench)

Convert motion target into desired body wrench:

1. `w_des = [Fx, Fy, TauZ]` for current 2D runtime.
2. Future extension path: `[Fx, Fy, Fz, Tx, Ty, Tz]` when 3D is introduced.

Controller behavior:

1. Velocity/heading feedback (PID-like or critically damped controller).
2. Optional feed-forward from planned maneuver.
3. Hard safety bounds from flight profile and current mode.

### 5.3 Allocation Layer (desired wrench -> per-thruster command)

Given `N` actuators, build effectiveness matrix `B`:

1. Each actuator contributes force in its local thrust axis transformed to body/world.
2. Torque contribution from mount offset `r` is `tau = r x F`.

Solve constrained optimization each fixed tick:

1. Minimize wrench error and command effort:
   - `min ||W(Bu - w_des)||^2 + lambda*||u||^2 + penalties`
2. Subject to constraints:
   - `0 <= u_i <= u_i_max`
   - slew/ramp limits
   - fuel/heat/damage/availability gates
   - directional limits (fixed thruster vs gimbaled)

Output:

1. `ThrusterCommandSet { actuator_id -> normalized_command }`.

## 6. Data Model Additions (Proposed)

Add/extend gameplay components under `crates/sidereal-game/src/components/`.

1. `ThrusterActuator`
   - `max_thrust_n`
   - `response_time_s`
   - `min_command`, `max_command`
   - `thrust_axis_local`
   - `gimbal_limit_deg` (optional)
   - `fuel_curve_id` / `efficiency` hooks
2. `ThrusterMount`
   - mount position and rotation in parent local space (can reuse `Hardpoint` data if sufficient)
3. `ThrusterHealthState`
   - efficiency scalar, disabled flag, failure mode
4. `FlightControlProfile`
   - tuning gains and mode policy (combat, cruise, docking, precision)
5. `DesiredMotionCommand` (runtime, optionally replicated for debug)
6. `ThrusterCommandSet` (runtime authoritative output; replicate only if needed for effects/debug)

Notes:

1. Keep durable state persistable where needed.
2. Keep high-frequency transient solver internals runtime-only unless debugging requires replication.

## 7. Force Application Contract

Authoritative simulation must apply actuator outputs as hardpoint-based physics.

Per fixed tick:

1. Compute actuator world force vector from command and mount orientation.
2. Apply at mount position if API supports force-at-point.
3. If force-at-point API is unavailable, compute equivalent net force + torque from `sum(F)` and `sum(r x F)` and apply both.

This preserves physically meaningful yaw/translation coupling from asymmetric layouts.

## 8. Player and AI Shared Interface

Unify control producers behind a single request interface:

1. `ControlIntentSource` (player, AI, script).
2. Each source emits `DesiredMotionCommand` or higher-level setpoints consumed by same guidance/control pipeline.
3. Priority/ownership rules remain server authoritative (existing control routing contract).

AI benefits:

1. AI no longer cheats by setting velocity directly.
2. AI can reason about actuator availability and degraded states.
3. Tactical behaviors (strafe orbit, reverse burn, drift correction) emerge from same allocator.

## 9. Mode Behavior

Define explicit FBW modes:

1. `Stabilized` (default): damp unwanted velocity/rotation when no command.
2. `Precision`: reduced gains + lower max command for docking.
3. `Combat`: aggressive response, higher allowed transients.
4. `Drift/Assist-Off` (future): minimal damping, allocator still enforces physical limits.

## 10. Multi-Engine and Module Swap Rules

1. Total authority derives from currently mounted and healthy actuators.
2. Adding/removing engines updates allocator matrix at runtime.
3. Afterburner and other capability multipliers affect only participating actuators.
4. Missing directional coverage reduces achievable wrench; controller must saturate and report limited authority.

## 11. Failure/Degraded Handling

1. Thruster offline: remove column from allocator and continue.
2. Efficiency loss: scale actuator bounds.
3. Fuel starvation: command may solve but execute as zero for affected thrusters; allocator should learn available budget each tick.
4. COM shift from cargo/module changes: recompute mount offsets/effectiveness matrix.

## 12. Implementation Phases

### Phase A: Foundations

1. Add actuator/mount/profile components.
2. Build matrix/effectiveness utilities and deterministic math tests.
3. Add runtime debug telemetry for commanded vs achieved wrench.

### Phase B: Hybrid Integration

1. Keep existing flight interface but route through new allocator backend.
2. Support current forward/reverse/yaw with hardpoint-aware allocation.
3. Validate afterburner compatibility.

### Phase C: Strafe and Advanced Axes

1. Introduce lateral thrust commands and side thruster modules.
2. Add control profile tuning for strafing and precision movement.
3. Ensure prediction/reconciliation stability.

### Phase D: AI Autopilot Integration

1. Add AI intent producer that emits same motion commands.
2. Add behaviors: hold velocity, intercept, orbit strafe, docking approach.
3. Add degraded-actuator fallback behaviors.

### Phase E: Hardening

1. Stress and soak tests with many entities.
2. Failure-injection tests (missing thrusters, COM shifts, fuel outages).
3. Performance optimization (cache matrices, sparse updates).

## 13. Testing Plan

Unit tests (`crates/sidereal-game`):

1. Allocator selects valid thrusters for requested wrench direction.
2. Saturation behavior when authority is insufficient.
3. Deterministic results across identical inputs.
4. COM shift and mount offset torque math correctness.
5. Damage/fuel gating removes/limits actuators correctly.

Integration tests (`bins/sidereal-replication`):

1. Authenticated player input routes through allocator and produces expected movement.
2. Multi-engine + side-thruster ship performs strafe without direct velocity writes.
3. Module swap/hardpoint change updates authority without restart.
4. No client authority leakage.

Client/prediction checks:

1. Predicted controlled entity uses shared allocator path.
2. Reconciliation remains stable under high command churn.

## 14. Instrumentation and Debugging

Add debug outputs for:

1. Desired wrench vs achieved wrench.
2. Per-actuator command and saturation reason.
3. Available directional authority envelope.
4. Mode and profile currently active.

These are required for tuning and AI behavior validation.

## 15. Open Decisions

1. Allocation solver choice for v1: weighted pseudo-inverse vs bounded QP.
2. Whether to expose per-axis actuator participation toggles (KSP-style) in developer tooling.
3. Which AI behaviors ship first after allocator foundation.
4. Whether assist-off mode is included in same milestone or deferred.

## 16. External References

1. Star Citizen flight model engineering notes (IFCS + thruster-at-mount model):
   - https://robertsspaceindustries.com/en/comm-link/engineering/13951-Flight-Model
2. Star Citizen IFCS summary:
   - https://starcitizen.tools/Intelligent_Flight_Control_System
3. Kerbal RCS control/balancing patterns:
   - https://kerbalspaceprogram.fandom.com/wiki/RCS
4. Space Engineers inertial dampener directional-coverage behavior:
   - https://spaceengineers.fandom.com/wiki/Inertial_Dampers
5. Spacecraft control allocation background (engineering reference):
   - https://www.nasa.gov/smallsat-institute/sst-soa/guidance-navigation-and-control/

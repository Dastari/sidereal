//! Flight Control System
//!
//! Implements the action routing chain:
//! EntityAction → FlightComputer → Engine → fuel check → Forces.apply_force()
//!
//! Architecture:
//! 1. FlightComputer component on parent entity translates actions to control state (throttle, yaw)
//! 2. Engine modules mounted on parent read control state
//! 3. Engines check fuel availability via FuelTank
//! 4. If fuel available: compute force vector, apply via Avian's Forces query helper, drain fuel
//! 5. Avian's physics integrator handles the rest

use avian2d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    ActionQueue, Engine, EntityAction, EntityGuid, FlightComputer, FlightTuning, FuelTank,
    MaxVelocityMps, MountedOn, SizeM, TotalMassKg,
};

const PASSIVE_ANGULAR_DAMP_RATE: f32 = 4.0;
const ACTIVE_ANGULAR_DAMP_RATE: f32 = 10.0;
const MAX_ANGULAR_VELOCITY_RAD_S: f32 = 2.0;
const IDLE_LINEAR_SPEED_EPSILON_MPS: f32 = 3.0;
const IDLE_ANGULAR_SPEED_EPSILON_RAD_S: f32 = 0.08;
const ACTIVE_BRAKE_STOP_EPSILON_MPS: f32 = 5.0;
type BodyForceQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static EntityGuid,
        &'static Transform,
        Option<&'static TotalMassKg>,
        &'static FlightTuning,
        &'static MaxVelocityMps,
        Option<&'static SizeM>,
        Forces,
    ),
>;
type BodyKinematicsQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static EntityGuid,
        &'static LinearVelocity,
        &'static AngularVelocity,
    ),
>;

#[doc(hidden)]
pub fn compute_brake_decel_accel_mps2(
    speed_mps: f32,
    dt_s: f32,
    passive_brake_accel_mps2: f32,
    active_brake_accel_mps2: f32,
    engine_limited_accel_mps2: f32,
    brake_active: bool,
) -> f32 {
    if speed_mps <= 0.0 || dt_s <= 0.0 {
        return 0.0;
    }
    let mut target_accel_mps2 = passive_brake_accel_mps2.max(0.0);
    if brake_active {
        target_accel_mps2 = active_brake_accel_mps2
            .min(engine_limited_accel_mps2.max(0.0))
            .max(passive_brake_accel_mps2.max(0.0));
    }
    let no_overshoot_accel_mps2 = speed_mps / dt_s;
    target_accel_mps2.min(no_overshoot_accel_mps2)
}

pub fn is_brake_active(computer: &FlightComputer) -> bool {
    computer.brake_active
}

pub fn apply_flight_action_to_computer(
    computer: &mut FlightComputer,
    action: EntityAction,
) -> bool {
    match action {
        EntityAction::Forward | EntityAction::ThrustForward => {
            computer.throttle = 1.0;
            computer.brake_active = false;
        }
        EntityAction::Backward | EntityAction::ThrustReverse => {
            computer.throttle = -0.7;
            computer.brake_active = false;
        }
        EntityAction::LongitudinalNeutral | EntityAction::ThrustNeutral => {
            computer.throttle = 0.0;
            computer.brake_active = false;
        }
        EntityAction::Brake => {
            computer.throttle = 0.0;
            computer.brake_active = true;
            computer.yaw_input = 0.0;
        }
        EntityAction::Left | EntityAction::YawLeft => {
            computer.yaw_input = 1.0;
            computer.brake_active = false;
        }
        EntityAction::Right | EntityAction::YawRight => {
            computer.yaw_input = -1.0;
            computer.brake_active = false;
        }
        EntityAction::LateralNeutral | EntityAction::YawNeutral => computer.yaw_input = 0.0,
        _ => return false,
    }
    true
}

/// System that processes actions and updates FlightComputer state
pub fn process_flight_actions(
    mut query: Query<(&mut ActionQueue, &mut FlightComputer), Without<MountedOn>>,
) {
    for (mut queue, mut computer) in &mut query {
        if queue.pending.is_empty() {
            continue;
        }

        for action in queue.drain() {
            let _ = apply_flight_action_to_computer(&mut computer, action);
        }
    }
}

/// System that applies engine thrust based on FlightComputer state
/// Uses Avian's Forces query helper for proper force integration
pub fn apply_engine_thrust(
    time: Res<Time>,
    // Root hull entities with flight computers (by GUID)
    computers: Query<(&EntityGuid, &FlightComputer), Without<MountedOn>>,
    // Parent entities that can receive forces (Avian Forces query helper)
    mut body_queries: ParamSet<(BodyForceQuery<'_, '_>, BodyKinematicsQuery<'_, '_>)>,
    // Engine modules
    mut engines: Query<(&MountedOn, &Engine, &mut FuelTank)>,
) {
    let dt = time.delta_secs();

    // Build control state by root parent GUID from hull flight-computer only.
    let mut control_by_parent = HashMap::<Uuid, (f32, f32, f32, bool)>::new();
    for (guid, computer) in &computers {
        let brake_active = is_brake_active(computer);
        control_by_parent.insert(
            guid.0,
            (
                computer.throttle,
                computer.yaw_input,
                computer.turn_rate_deg_s,
                brake_active,
            ),
        );
    }

    // Aggregate engine thrust/torque budgets by parent GUID.
    let mut forward_thrust_budget_by_parent = HashMap::<Uuid, f32>::new();
    let mut reverse_thrust_budget_by_parent = HashMap::<Uuid, f32>::new();
    let mut torque_thrust_budget_by_parent = HashMap::<Uuid, f32>::new();
    let mut fuel_exhausted_count = HashMap::<Uuid, usize>::new();

    for (mounted_on, engine, mut fuel_tank) in &mut engines {
        let Some((throttle, yaw_input, _, brake_active)) =
            control_by_parent.get(&mounted_on.parent_entity_id)
        else {
            continue;
        };

        if fuel_tank.fuel_kg <= 0.0 {
            *fuel_exhausted_count
                .entry(mounted_on.parent_entity_id)
                .or_insert(0) += 1;
            continue;
        }

        let throttle_demand = throttle.abs().clamp(0.0, 1.0);
        let brake_demand = if *brake_active { 1.0 } else { 0.0 };
        let yaw_demand = yaw_input.abs().clamp(0.0, 1.0);
        let demand = throttle_demand.max(brake_demand).max(yaw_demand);
        let requested_burn_kg = engine.burn_rate_kg_s * demand * dt;

        if requested_burn_kg > 0.0 {
            let actual_burn_kg = requested_burn_kg.min(fuel_tank.fuel_kg);
            let thrust_scale = actual_burn_kg / requested_burn_kg;
            fuel_tank.fuel_kg -= actual_burn_kg;

            forward_thrust_budget_by_parent
                .entry(mounted_on.parent_entity_id)
                .and_modify(|v| *v += engine.thrust.abs() * thrust_scale)
                .or_insert(engine.thrust.abs() * thrust_scale);
            reverse_thrust_budget_by_parent
                .entry(mounted_on.parent_entity_id)
                .and_modify(|v| *v += engine.reverse_thrust.abs() * thrust_scale)
                .or_insert(engine.reverse_thrust.abs() * thrust_scale);
            torque_thrust_budget_by_parent
                .entry(mounted_on.parent_entity_id)
                .and_modify(|v| *v += engine.torque_thrust.abs() * thrust_scale)
                .or_insert(engine.torque_thrust.abs() * thrust_scale);
        }
    }

    let mut kinematics_by_guid = HashMap::<Uuid, (Vec2, f32)>::new();
    for (guid, linear_velocity, angular_velocity) in &body_queries.p1() {
        kinematics_by_guid.insert(guid.0, (linear_velocity.0, angular_velocity.0));
    }

    // Apply aggregated forces to parent bodies using Avian's Forces helper
    for (guid, transform, total_mass, flight_tuning, max_velocity, size_m, mut forces) in
        &mut body_queries.p0()
    {
        let mass_kg = total_mass.map(|mass| mass.0.max(1.0)).unwrap_or(1.0);
        let planar_moi_kg_m2 = planar_moment_of_inertia_z_kg_m2(mass_kg, size_m.copied());
        let control = control_by_parent.get(&guid.0).copied();

        if let Some((throttle, yaw_input, turn_rate_deg_s, brake_active)) = control {
            let (velocity, angular_velocity) = kinematics_by_guid
                .get(&guid.0)
                .copied()
                .unwrap_or((Vec2::ZERO, 0.0));

            let forward_available_thrust = forward_thrust_budget_by_parent
                .get(&guid.0)
                .copied()
                .unwrap_or(0.0);
            let reverse_available_thrust = reverse_thrust_budget_by_parent
                .get(&guid.0)
                .copied()
                .unwrap_or(0.0);
            let available_torque_thrust = torque_thrust_budget_by_parent
                .get(&guid.0)
                .copied()
                .unwrap_or(0.0);

            let (force, torque) = compute_flight_forces(
                (throttle, yaw_input, turn_rate_deg_s, brake_active),
                velocity,
                angular_velocity,
                transform.rotation,
                mass_kg,
                planar_moi_kg_m2,
                flight_tuning,
                max_velocity.0.max(1.0),
                forward_available_thrust,
                reverse_available_thrust,
                available_torque_thrust,
                dt,
            );

            forces.apply_force(force);
            forces.apply_torque(torque);
        }

        // Log if throttle was applied but no thrust budget was available (fuel exhausted path).
        if let Some((throttle, _, _, brake_active)) = control_by_parent.get(&guid.0)
            && !*brake_active
            && *throttle != 0.0
            && forward_thrust_budget_by_parent
                .get(&guid.0)
                .copied()
                .unwrap_or(0.0)
                <= 0.0
        {
            let exhausted = fuel_exhausted_count.get(&guid.0).copied().unwrap_or(0);
            if exhausted > 0 {
                debug!(
                    entity_guid = %guid.0,
                    exhausted_engines = exhausted,
                    "throttle applied but all engines out of fuel"
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn compute_flight_forces(
    control: (f32, f32, f32, bool), // throttle, yaw_input, turn_rate_deg_s, brake_active
    velocity: Vec2,
    angular_velocity: f32,
    rotation: Quat,
    mass_kg: f32,
    planar_moi_kg_m2: f32,
    flight_tuning: &FlightTuning,
    max_linear_speed_mps: f32,
    forward_available_thrust: f32,
    reverse_available_thrust: f32,
    available_torque_thrust: f32,
    dt: f32,
) -> (Vec2, f32) {
    if !velocity.is_finite()
        || !angular_velocity.is_finite()
        || !rotation.is_finite()
        || !mass_kg.is_finite()
        || !planar_moi_kg_m2.is_finite()
        || !dt.is_finite()
    {
        return (Vec2::ZERO, 0.0);
    }
    let (throttle, yaw_input, turn_rate_deg_s, brake_active) = control;
    let max_linear_accel_mps2 = flight_tuning.max_linear_accel_mps2.max(0.1);
    let passive_brake_accel_mps2 = flight_tuning.passive_brake_accel_mps2.max(0.1);
    let active_brake_accel_mps2 = flight_tuning
        .active_brake_accel_mps2
        .max(passive_brake_accel_mps2);

    let planar_velocity = velocity;
    let speed = planar_velocity.length();
    let forward_axis_world = {
        let axis = rotation * Vec3::Y;
        let axis = Vec2::new(axis.x, axis.y);
        let len_sq = axis.length_squared();
        if len_sq > 1e-6 {
            axis / len_sq.sqrt()
        } else {
            Vec2::Y
        }
    };

    let mut applied_force = Vec2::ZERO;
    let mut applied_torque = 0.0_f32;

    if !brake_active && throttle != 0.0 {
        let directional_thrust = if throttle > 0.0 {
            forward_available_thrust
        } else {
            reverse_available_thrust
        };
        let engine_accel_cap = if directional_thrust > 0.0 {
            directional_thrust / mass_kg
        } else {
            0.0
        };
        let accel_target = max_linear_accel_mps2 * throttle.abs();
        let accel_cap = accel_target.min(engine_accel_cap.max(0.0));

        let current_forward_speed = planar_velocity.dot(forward_axis_world);
        let target_forward_speed = max_linear_speed_mps * throttle.abs() * throttle.signum();
        let speed_delta = target_forward_speed - current_forward_speed;

        // Use standard acceleration approach rather than immediate target velocity matching if below target speed
        if dt > 0.0 && accel_cap > 0.0 && speed_delta.abs() > 0.01 {
            let max_speed_step = accel_cap * dt;
            let applied_step = speed_delta.clamp(-max_speed_step, max_speed_step);
            let actual_accel = applied_step / dt;
            let required_force = forward_axis_world * (actual_accel * mass_kg);
            applied_force += required_force;
        }

        // Hard speed governor to prevent runaway values.
        if speed > max_linear_speed_mps {
            let overspeed = speed - max_linear_speed_mps;
            let governor_accel = (overspeed / dt.max(1e-6)).min(max_linear_accel_mps2 * 2.0);
            let governor_force = -(planar_velocity / speed) * governor_accel * mass_kg;
            applied_force += governor_force;
        }
    } else if brake_active && speed > 0.01
        || !brake_active && speed > IDLE_LINEAR_SPEED_EPSILON_MPS && throttle == 0.0
    {
        let engine_limited_accel = if reverse_available_thrust > 0.0 {
            reverse_available_thrust / mass_kg
        } else {
            0.0
        };
        let decel_accel = compute_brake_decel_accel_mps2(
            speed,
            dt,
            passive_brake_accel_mps2,
            active_brake_accel_mps2,
            engine_limited_accel,
            brake_active,
        );
        let braking_force = -(planar_velocity / speed) * decel_accel * mass_kg;
        applied_force += braking_force;
    }

    if yaw_input != 0.0 {
        let target_angular_velocity_z = yaw_input * turn_rate_deg_s.to_radians();
        let current_angular_velocity_z = angular_velocity;
        let required_angular_accel_z =
            (target_angular_velocity_z - current_angular_velocity_z) / dt.max(1e-6);
        let commanded_torque_z = required_angular_accel_z * planar_moi_kg_m2;
        let capped_torque_z =
            commanded_torque_z.clamp(-available_torque_thrust, available_torque_thrust);
        applied_torque += capped_torque_z;
    } else {
        let angular_z = angular_velocity;
        if angular_z.abs() > 0.001 {
            let rate = if brake_active {
                ACTIVE_ANGULAR_DAMP_RATE
            } else {
                PASSIVE_ANGULAR_DAMP_RATE
            };
            let damp_torque = -angular_z * rate * planar_moi_kg_m2;
            applied_torque += damp_torque;
        }
    }

    (
        sanitize_finite_vec2(applied_force),
        sanitize_finite_scalar(applied_torque),
    )
}

fn planar_moment_of_inertia_z_kg_m2(mass_kg: f32, size_m: Option<SizeM>) -> f32 {
    let Some(size) = size_m else {
        return mass_kg.max(1.0);
    };
    let length = size.length.max(0.1);
    let width = size.width.max(0.1);
    ((mass_kg * (length * length + width * width)) / 12.0).max(1.0)
}

/// Computes Avian-compatible 2D angular inertia from gameplay SizeM and mass.
pub fn angular_inertia_from_size(mass_kg: f32, size: &SizeM) -> AngularInertia {
    let m = mass_kg.max(1.0);
    let l = size.length.max(0.1);
    let w = size.width.max(0.1);
    let iz = (m * (l * l + w * w)) / 12.0;
    AngularInertia(iz.max(1.0))
}

fn sanitize_finite_vec2(value: Vec2) -> Vec2 {
    if value.is_finite() { value } else { Vec2::ZERO }
}

fn sanitize_finite_scalar(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// Clamp angular velocity around Z to prevent excessive blur-spin.
pub fn clamp_angular_velocity(
    mut bodies: Query<(&mut AngularVelocity, Option<&MountedOn>), With<FlightComputer>>,
) {
    for (mut angular_velocity, mounted_on) in &mut bodies {
        if mounted_on.is_some() {
            continue;
        }
        angular_velocity.0 =
            sanitize_planar_angular_velocity(angular_velocity.0, MAX_ANGULAR_VELOCITY_RAD_S);
    }
}

pub fn sanitize_planar_angular_velocity(angular_velocity: f32, max_abs_z_rad_s: f32) -> f32 {
    angular_velocity.clamp(-max_abs_z_rad_s.abs(), max_abs_z_rad_s.abs())
}

/// Clamp tiny residual drift/spin while controls are neutral.
pub fn stabilize_idle_motion(
    mut bodies: Query<(
        &FlightComputer,
        &mut LinearVelocity,
        &mut AngularVelocity,
        Option<&MountedOn>,
    )>,
) {
    for (computer, mut linear_velocity, mut angular_velocity, mounted_on) in &mut bodies {
        if mounted_on.is_some() {
            continue;
        }
        let brake_active = computer.brake_active;
        let neutral_throttle = computer.throttle.abs() <= f32::EPSILON;
        let neutral_yaw = computer.yaw_input.abs() <= f32::EPSILON;
        let planar_speed = Vec2::new(linear_velocity.0.x, linear_velocity.0.y).length();

        if brake_active {
            if planar_speed <= ACTIVE_BRAKE_STOP_EPSILON_MPS {
                linear_velocity.0.x = 0.0;
                linear_velocity.0.y = 0.0;
            }
            if angular_velocity.0.abs() <= IDLE_ANGULAR_SPEED_EPSILON_RAD_S {
                angular_velocity.0 = 0.0;
            }
            continue;
        }
        if !neutral_throttle || !neutral_yaw {
            continue;
        }

        if planar_speed <= IDLE_LINEAR_SPEED_EPSILON_MPS {
            linear_velocity.0.x = 0.0;
            linear_velocity.0.y = 0.0;
        }

        if angular_velocity.0.abs() <= IDLE_ANGULAR_SPEED_EPSILON_RAD_S {
            angular_velocity.0 = 0.0;
        }
    }
}

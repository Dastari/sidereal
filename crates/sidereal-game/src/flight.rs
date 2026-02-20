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

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::actions::{ActionQueue, EntityAction};
use crate::generated::components::{
    Engine, EntityGuid, FlightComputer, FuelTank, MountedOn, TotalMassKg,
};

const BRAKE_SENTINEL_THROTTLE: f32 = 2.0;
const MAX_LINEAR_SPEED_MPS: f32 = 600.0;
const TIME_TO_MAX_SPEED_S: f32 = 10.0;
const MAX_LINEAR_ACCEL_MPS2: f32 = MAX_LINEAR_SPEED_MPS / TIME_TO_MAX_SPEED_S;
const PASSIVE_LINEAR_BRAKE_ACCEL_MPS2: f32 = 1.5;
const ACTIVE_LINEAR_BRAKE_ACCEL_MPS2: f32 = 8.0;
const PASSIVE_ANGULAR_DAMP_GAIN: f32 = 4_500.0;
const ACTIVE_ANGULAR_DAMP_GAIN: f32 = 9_000.0;

/// System that processes actions and updates FlightComputer state
pub fn process_flight_actions(
    mut query: Query<(&mut ActionQueue, &mut FlightComputer, Option<&MountedOn>)>,
) {
    for (mut queue, mut computer, mounted_on) in &mut query {
        if queue.pending.is_empty() {
            continue;
        }

        for action in queue.drain() {
            match action {
                EntityAction::ThrustForward => computer.throttle = 1.0,
                EntityAction::ThrustReverse => computer.throttle = -0.7, // Reverse is typically weaker
                EntityAction::ThrustNeutral => computer.throttle = 0.0,
                EntityAction::Brake => {
                    computer.throttle = BRAKE_SENTINEL_THROTTLE;
                    computer.yaw_input = 0.0;
                }
                EntityAction::YawLeft => computer.yaw_input = 1.0,
                EntityAction::YawRight => computer.yaw_input = -1.0,
                EntityAction::YawNeutral => computer.yaw_input = 0.0,
                _ => {
                    // Flight computer doesn't handle this action
                    if mounted_on.is_some() {
                        debug!(action = ?action, "FlightComputer module ignoring non-flight action");
                    }
                }
            }
        }
    }
}

/// System that applies engine thrust based on FlightComputer state
/// Uses Avian's Forces query helper for proper force integration
pub fn apply_engine_thrust(
    time: Res<Time>,
    // Parent entities with flight computers (by GUID)
    computers: Query<(&EntityGuid, &FlightComputer, Option<&MountedOn>)>,
    // Parent entities that can receive forces (Avian Forces query helper)
    mut body_queries: ParamSet<(
        Query<(&EntityGuid, &Transform, Option<&TotalMassKg>, Forces)>,
        Query<(&EntityGuid, &LinearVelocity, &AngularVelocity)>,
    )>,
    // Engine modules
    mut engines: Query<(&MountedOn, &Engine, &mut FuelTank)>,
) {
    let dt = time.delta_secs();

    // Build map of control state by parent entity GUID
    let mut control_by_parent = HashMap::<Uuid, (f32, f32, f32, bool)>::new();
    for (guid, computer, mounted_on) in &computers {
        let parent_guid = if let Some(mount) = mounted_on {
            // FlightComputer is a module, use parent GUID
            mount.parent_entity_id
        } else {
            // FlightComputer is built-in to the entity
            guid.0
        };
        let brake_active = computer.throttle >= BRAKE_SENTINEL_THROTTLE;

        control_by_parent.entry(parent_guid).or_insert((
            computer.throttle,
            computer.yaw_input,
            computer.turn_rate_deg_s,
            brake_active,
        ));
    }

    // Aggregate engine thrust budget by parent GUID
    let mut thrust_budget_by_parent = HashMap::<Uuid, f32>::new();
    let mut brake_thrust_budget_by_parent = HashMap::<Uuid, f32>::new();
    let mut fuel_exhausted_count = HashMap::<Uuid, usize>::new();

    for (mounted_on, engine, mut fuel_tank) in &mut engines {
        let Some((throttle, _, _, brake_active)) = control_by_parent.get(&mounted_on.parent_entity_id) else {
            // No flight computer on this parent, engine idle
            continue;
        };

        // Check fuel availability
        if fuel_tank.fuel_kg <= 0.0 {
            *fuel_exhausted_count
                .entry(mounted_on.parent_entity_id)
                .or_insert(0) += 1;
            continue;
        }

        if *brake_active {
            // Active braking uses available engine thrust budget opposite current velocity.
            let requested_burn_kg = engine.burn_rate_kg_s * dt;
            let actual_burn_kg = requested_burn_kg.min(fuel_tank.fuel_kg);
            let thrust_scale = if requested_burn_kg > 0.0 {
                actual_burn_kg / requested_burn_kg
            } else {
                1.0
            };
            fuel_tank.fuel_kg -= actual_burn_kg;
            brake_thrust_budget_by_parent
                .entry(mounted_on.parent_entity_id)
                .and_modify(|v| *v += engine.thrust_n.abs() * thrust_scale)
                .or_insert(engine.thrust_n.abs() * thrust_scale);
            continue;
        }

        if *throttle == 0.0 {
            continue;
        }

        // Compute fuel burn
        let requested_burn_kg = engine.burn_rate_kg_s * throttle.abs() * dt;
        let actual_burn_kg = requested_burn_kg.min(fuel_tank.fuel_kg);
        let thrust_scale = if requested_burn_kg > 0.0 {
            actual_burn_kg / requested_burn_kg
        } else {
            1.0
        };

        fuel_tank.fuel_kg -= actual_burn_kg;

        thrust_budget_by_parent
            .entry(mounted_on.parent_entity_id)
            .and_modify(|v| *v += engine.thrust_n.abs() * thrust_scale)
            .or_insert(engine.thrust_n.abs() * thrust_scale);
    }

    let mut kinematics_by_guid = HashMap::<Uuid, (Vec3, Vec3)>::new();
    for (guid, linear_velocity, angular_velocity) in &body_queries.p1() {
        kinematics_by_guid.insert(guid.0, (linear_velocity.0, angular_velocity.0));
    }

    // Apply aggregated forces to parent bodies using Avian's Forces helper
    for (guid, transform, total_mass, mut forces) in &mut body_queries.p0() {
        let mass_kg = total_mass.map(|mass| mass.0.max(1.0)).unwrap_or(15_000.0);
        let control = control_by_parent.get(&guid.0).copied();

        if let Some((throttle, yaw_input, turn_rate_deg_s, brake_active)) = control {
            let (velocity, angular_velocity) = kinematics_by_guid
                .get(&guid.0)
                .copied()
                .unwrap_or((Vec3::ZERO, Vec3::ZERO));
            let speed = velocity.length();
            let forward_axis_world = {
                let axis = transform.rotation * Vec3::Y;
                let len_sq = axis.length_squared();
                if len_sq > 1e-6 {
                    axis / len_sq.sqrt()
                } else {
                    Vec3::Y
                }
            };

            if !brake_active && throttle != 0.0 {
                let available_thrust = thrust_budget_by_parent.get(&guid.0).copied().unwrap_or(0.0);
                let engine_accel_cap = if available_thrust > 0.0 {
                    available_thrust / mass_kg
                } else {
                    0.0
                };
                let accel_target = MAX_LINEAR_ACCEL_MPS2 * throttle.abs();
                let accel_cap = accel_target.min(engine_accel_cap.max(0.0));

                let current_forward_speed = velocity.dot(forward_axis_world);
                let target_forward_speed =
                    MAX_LINEAR_SPEED_MPS * throttle.abs() * throttle.signum();
                let speed_delta = target_forward_speed - current_forward_speed;
                if dt > 0.0 && accel_cap > 0.0 {
                    let max_speed_step = accel_cap * dt;
                    let applied_step = speed_delta.clamp(-max_speed_step, max_speed_step);
                    let required_accel = applied_step / dt;
                    let required_force = forward_axis_world * (required_accel * mass_kg);
                    forces.apply_force(required_force);
                }

                // Hard speed governor to prevent runaway values.
                if speed > MAX_LINEAR_SPEED_MPS {
                    let overspeed = speed - MAX_LINEAR_SPEED_MPS;
                    let governor_accel = (overspeed / dt.max(1e-6)).min(MAX_LINEAR_ACCEL_MPS2 * 2.0);
                    let governor_force = -(velocity / speed) * governor_accel * mass_kg;
                    forces.apply_force(governor_force);
                }
            } else {
                if speed > 0.01 {
                    let mut target_accel = PASSIVE_LINEAR_BRAKE_ACCEL_MPS2;
                    if brake_active {
                        let available_thrust = brake_thrust_budget_by_parent
                            .get(&guid.0)
                            .copied()
                            .unwrap_or(0.0);
                        let engine_limited_accel = if available_thrust > 0.0 {
                            available_thrust / mass_kg
                        } else {
                            0.0
                        };
                        target_accel = ACTIVE_LINEAR_BRAKE_ACCEL_MPS2.min(engine_limited_accel.max(target_accel));
                    }
                    let no_overshoot_accel = if dt > 0.0 { speed / dt } else { target_accel };
                    let decel_accel = target_accel.min(no_overshoot_accel);
                    let braking_force = -(velocity / speed) * decel_accel * mass_kg;
                    forces.apply_force(braking_force);
                }
            }

            if yaw_input != 0.0 {
                let yaw_rate_rad_s = yaw_input * turn_rate_deg_s.to_radians();
                // TODO: Proper torque calculation based on inertia tensor
                let torque = Vec3::new(0.0, 0.0, yaw_rate_rad_s * 4000.0);
                forces.apply_torque(torque);
            } else {
                let angular_z = angular_velocity.z;
                if angular_z.abs() > 0.001 {
                    let gain = if brake_active {
                        ACTIVE_ANGULAR_DAMP_GAIN
                    } else {
                        PASSIVE_ANGULAR_DAMP_GAIN
                    };
                    forces.apply_torque(Vec3::new(0.0, 0.0, -angular_z * gain));
                }
            }
        }

        // Log if throttle was applied but no thrust budget was available (fuel exhausted path).
        if let Some((throttle, _, _, brake_active)) = control_by_parent.get(&guid.0)
            && !*brake_active
            && *throttle != 0.0
            && thrust_budget_by_parent
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

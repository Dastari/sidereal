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
    Engine, EntityGuid, FlightComputer, FlightTuning, FuelTank, MountedOn, TotalMassKg,
};

const BRAKE_SENTINEL_THROTTLE: f32 = 2.0;
const DEFAULT_MAX_LINEAR_SPEED_MPS: f32 = 600.0;
const DEFAULT_TIME_TO_MAX_SPEED_S: f32 = 10.0;
const DEFAULT_MAX_LINEAR_ACCEL_MPS2: f32 =
    DEFAULT_MAX_LINEAR_SPEED_MPS / DEFAULT_TIME_TO_MAX_SPEED_S;
const DEFAULT_PASSIVE_LINEAR_BRAKE_ACCEL_MPS2: f32 = 3.0;
const DEFAULT_ACTIVE_LINEAR_BRAKE_ACCEL_MPS2: f32 = 12.0;
const PASSIVE_ANGULAR_DAMP_GAIN: f32 = 6_000.0;
const ACTIVE_ANGULAR_DAMP_GAIN: f32 = 12_000.0;
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
        Option<&'static FlightTuning>,
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

fn compute_brake_decel_accel_mps2(
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
    mut body_queries: ParamSet<(BodyForceQuery<'_, '_>, BodyKinematicsQuery<'_, '_>)>,
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
        let Some((throttle, _, _, brake_active)) =
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

        let requested_burn_kg = if *brake_active {
            engine.burn_rate_kg_s * dt
        } else if *throttle != 0.0 {
            engine.burn_rate_kg_s * throttle.abs() * dt
        } else {
            0.0
        };

        if requested_burn_kg > 0.0 {
            let actual_burn_kg = requested_burn_kg.min(fuel_tank.fuel_kg);
            let thrust_scale = actual_burn_kg / requested_burn_kg;
            fuel_tank.fuel_kg -= actual_burn_kg;

            let force_mag = engine.thrust_n.abs() * thrust_scale;
            if *brake_active {
                brake_thrust_budget_by_parent
                    .entry(mounted_on.parent_entity_id)
                    .and_modify(|v| *v += force_mag)
                    .or_insert(force_mag);
            } else {
                thrust_budget_by_parent
                    .entry(mounted_on.parent_entity_id)
                    .and_modify(|v| *v += force_mag)
                    .or_insert(force_mag);
            }
        }
    }

    let mut kinematics_by_guid = HashMap::<Uuid, (Vec3, Vec3)>::new();
    for (guid, linear_velocity, angular_velocity) in &body_queries.p1() {
        kinematics_by_guid.insert(guid.0, (linear_velocity.0, angular_velocity.0));
    }

    // Apply aggregated forces to parent bodies using Avian's Forces helper
    for (guid, transform, total_mass, flight_tuning, mut forces) in &mut body_queries.p0() {
        let mass_kg = total_mass.map(|mass| mass.0.max(1.0)).unwrap_or(15_000.0);
        let control = control_by_parent.get(&guid.0).copied();

        if let Some((throttle, yaw_input, turn_rate_deg_s, brake_active)) = control {
            let (velocity, angular_velocity) = kinematics_by_guid
                .get(&guid.0)
                .copied()
                .unwrap_or((Vec3::ZERO, Vec3::ZERO));
            
            let available_thrust = thrust_budget_by_parent.get(&guid.0).copied().unwrap_or(0.0);
            let brake_available_thrust = brake_thrust_budget_by_parent.get(&guid.0).copied().unwrap_or(0.0);

            let (force, torque) = compute_flight_forces(
                (throttle, yaw_input, turn_rate_deg_s, brake_active),
                velocity,
                angular_velocity,
                transform.rotation,
                mass_kg,
                flight_tuning,
                available_thrust,
                brake_available_thrust,
                dt,
            );

            forces.apply_force(force);
            forces.apply_torque(torque);
        }

        // Log if throttle was applied but no thrust budget was available (fuel exhausted path).
        if let Some((throttle, _, _, brake_active)) = control_by_parent.get(&guid.0)
            && !*brake_active
            && *throttle != 0.0
            && thrust_budget_by_parent.get(&guid.0).copied().unwrap_or(0.0) <= 0.0
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
    velocity: Vec3,
    angular_velocity: Vec3,
    rotation: Quat,
    mass_kg: f32,
    flight_tuning: Option<&FlightTuning>,
    available_thrust: f32,
    brake_available_thrust: f32,
    dt: f32,
) -> (Vec3, Vec3) {
    let (throttle, yaw_input, turn_rate_deg_s, brake_active) = control;

    let max_linear_speed_mps = flight_tuning
        .map(|tuning| tuning.max_linear_speed_mps.max(1.0))
        .unwrap_or(DEFAULT_MAX_LINEAR_SPEED_MPS);
    let max_linear_accel_mps2 = flight_tuning
        .map(|tuning| tuning.max_linear_accel_mps2.max(0.1))
        .unwrap_or(DEFAULT_MAX_LINEAR_ACCEL_MPS2);
    let passive_brake_accel_mps2 = flight_tuning
        .map(|tuning| tuning.passive_brake_accel_mps2.max(0.1))
        .unwrap_or(DEFAULT_PASSIVE_LINEAR_BRAKE_ACCEL_MPS2);
    let active_brake_accel_mps2 = flight_tuning
        .map(|tuning| tuning.active_brake_accel_mps2.max(passive_brake_accel_mps2))
        .unwrap_or(DEFAULT_ACTIVE_LINEAR_BRAKE_ACCEL_MPS2);

    let planar_velocity = Vec3::new(velocity.x, velocity.y, 0.0);
    let speed = planar_velocity.length();
    let forward_axis_world = {
        let axis = rotation * Vec3::Y;
        let axis = Vec3::new(axis.x, axis.y, 0.0);
        let len_sq = axis.length_squared();
        if len_sq > 1e-6 {
            axis / len_sq.sqrt()
        } else {
            Vec3::Y
        }
    };

    let mut applied_force = Vec3::ZERO;
    let mut applied_torque = Vec3::ZERO;

    if !brake_active && throttle != 0.0 {
        let engine_accel_cap = if available_thrust > 0.0 {
            available_thrust / mass_kg
        } else {
            0.0
        };
        let accel_target = max_linear_accel_mps2 * throttle.abs();
        let accel_cap = accel_target.min(engine_accel_cap.max(0.0));

        let current_forward_speed = planar_velocity.dot(forward_axis_world);
        let target_forward_speed =
            max_linear_speed_mps * throttle.abs() * throttle.signum();
        let speed_delta = target_forward_speed - current_forward_speed;
        
        // Use standard acceleration approach rather than immediate target velocity matching if below target speed
        if dt > 0.0 && accel_cap > 0.0 {
            if speed_delta.abs() > 0.01 {
                let max_speed_step = accel_cap * dt;
                let applied_step = speed_delta.clamp(-max_speed_step, max_speed_step);
                let actual_accel = applied_step / dt;
                let required_force = forward_axis_world * (actual_accel * mass_kg);
                applied_force += required_force;
            }
        }

        // Hard speed governor to prevent runaway values.
        if speed > max_linear_speed_mps {
            let overspeed = speed - max_linear_speed_mps;
            let governor_accel =
                (overspeed / dt.max(1e-6)).min(max_linear_accel_mps2 * 2.0);
            let governor_force = -(planar_velocity / speed) * governor_accel * mass_kg;
            applied_force += governor_force;
        }
    } else if brake_active && speed > 0.01 || !brake_active && speed > IDLE_LINEAR_SPEED_EPSILON_MPS && throttle == 0.0 {
        let engine_limited_accel = if brake_available_thrust > 0.0 {
            brake_available_thrust / mass_kg
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
        let yaw_rate_rad_s = yaw_input * turn_rate_deg_s.to_radians();
        // TODO: Proper torque calculation based on inertia tensor
        let torque = Vec3::new(0.0, 0.0, yaw_rate_rad_s * 4000.0);
        applied_torque += torque;
    } else {
        let angular_z = angular_velocity.z;
        if angular_z.abs() > 0.001 {
            let gain = if brake_active {
                ACTIVE_ANGULAR_DAMP_GAIN
            } else {
                PASSIVE_ANGULAR_DAMP_GAIN
            };
            applied_torque += Vec3::new(0.0, 0.0, -angular_z * gain);
        }
    }

    (applied_force, applied_torque)
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

pub fn sanitize_planar_angular_velocity(angular_velocity: Vec3, max_abs_z_rad_s: f32) -> Vec3 {
    Vec3::new(
        0.0,
        0.0,
        angular_velocity
            .z
            .clamp(-max_abs_z_rad_s.abs(), max_abs_z_rad_s.abs()),
    )
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
        let brake_active = computer.throttle >= BRAKE_SENTINEL_THROTTLE;
        let neutral_throttle = computer.throttle.abs() <= f32::EPSILON;
        let neutral_yaw = computer.yaw_input.abs() <= f32::EPSILON;
        let planar_speed = Vec2::new(linear_velocity.0.x, linear_velocity.0.y).length();

        if brake_active {
            if planar_speed <= ACTIVE_BRAKE_STOP_EPSILON_MPS {
                linear_velocity.0.x = 0.0;
                linear_velocity.0.y = 0.0;
            }
            if angular_velocity.0.z.abs() <= IDLE_ANGULAR_SPEED_EPSILON_RAD_S {
                angular_velocity.0.z = 0.0;
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

        if angular_velocity.0.z.abs() <= IDLE_ANGULAR_SPEED_EPSILON_RAD_S {
            angular_velocity.0.z = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_flight_actions_only_updates_flight_intents() {
        let mut app = App::new();
        app.add_systems(Update, process_flight_actions);
        let entity = app
            .world_mut()
            .spawn((
                ActionQueue {
                    pending: vec![
                        EntityAction::ThrustForward,
                        EntityAction::YawRight,
                        EntityAction::FirePrimary,
                    ],
                },
                FlightComputer {
                    profile: "basic_fly_by_wire".to_string(),
                    throttle: 0.0,
                    yaw_input: 0.0,
                    turn_rate_deg_s: 45.0,
                },
            ))
            .id();
        app.update();

        let queue = app.world().entity(entity).get::<ActionQueue>().unwrap();
        let computer = app.world().entity(entity).get::<FlightComputer>().unwrap();
        assert!(queue.pending.is_empty());
        assert!((computer.throttle - 1.0).abs() < f32::EPSILON);
        assert!((computer.yaw_input + 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn process_flight_actions_handles_brake_as_intent_only() {
        let mut app = App::new();
        app.add_systems(Update, process_flight_actions);
        let entity = app
            .world_mut()
            .spawn((
                ActionQueue {
                    pending: vec![EntityAction::Brake],
                },
                FlightComputer {
                    profile: "basic_fly_by_wire".to_string(),
                    throttle: 0.4,
                    yaw_input: 0.6,
                    turn_rate_deg_s: 45.0,
                },
            ))
            .id();
        app.update();

        let computer = app.world().entity(entity).get::<FlightComputer>().unwrap();
        assert!(computer.throttle >= BRAKE_SENTINEL_THROTTLE);
        assert!(computer.yaw_input.abs() < f32::EPSILON);
    }

    #[test]
    fn process_flight_actions_ignores_non_flight_actions() {
        let mut app = App::new();
        app.add_systems(Update, process_flight_actions);
        let entity = app
            .world_mut()
            .spawn((
                ActionQueue {
                    pending: vec![EntityAction::FirePrimary],
                },
                FlightComputer {
                    profile: "basic_fly_by_wire".to_string(),
                    throttle: 0.25,
                    yaw_input: -0.5,
                    turn_rate_deg_s: 45.0,
                },
            ))
            .id();
        app.update();

        let computer = app.world().entity(entity).get::<FlightComputer>().unwrap();
        assert!((computer.throttle - 0.25).abs() < f32::EPSILON);
        assert!((computer.yaw_input + 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn stabilize_idle_motion_zeros_small_residual_velocity_and_spin() {
        let mut app = App::new();
        app.add_systems(Update, stabilize_idle_motion);
        let entity = app
            .world_mut()
            .spawn((
                FlightComputer {
                    profile: "basic_fly_by_wire".to_string(),
                    throttle: 0.0,
                    yaw_input: 0.0,
                    turn_rate_deg_s: 45.0,
                },
                LinearVelocity(Vec3::new(0.02, -0.03, 0.0)),
                AngularVelocity(Vec3::new(0.0, 0.0, 0.01)),
            ))
            .id();
        app.update();

        let linear_velocity = app.world().entity(entity).get::<LinearVelocity>().unwrap();
        let angular_velocity = app.world().entity(entity).get::<AngularVelocity>().unwrap();
        assert_eq!(linear_velocity.0.x, 0.0);
        assert_eq!(linear_velocity.0.y, 0.0);
        assert_eq!(angular_velocity.0.z, 0.0);
    }

    #[test]
    fn stabilize_idle_motion_preserves_active_control_state() {
        let mut app = App::new();
        app.add_systems(Update, stabilize_idle_motion);
        let entity = app
            .world_mut()
            .spawn((
                FlightComputer {
                    profile: "basic_fly_by_wire".to_string(),
                    throttle: 1.0,
                    yaw_input: 0.0,
                    turn_rate_deg_s: 45.0,
                },
                LinearVelocity(Vec3::new(0.02, -0.03, 0.0)),
                AngularVelocity(Vec3::new(0.0, 0.0, 0.01)),
            ))
            .id();
        app.update();

        let linear_velocity = app.world().entity(entity).get::<LinearVelocity>().unwrap();
        let angular_velocity = app.world().entity(entity).get::<AngularVelocity>().unwrap();
        assert!((linear_velocity.0.x - 0.02).abs() < 1e-6);
        assert!((linear_velocity.0.y + 0.03).abs() < 1e-6);
        assert!((angular_velocity.0.z - 0.01).abs() < 1e-6);
    }

    #[test]
    fn stabilize_idle_motion_honors_brake_stop_window() {
        let mut app = App::new();
        app.add_systems(Update, stabilize_idle_motion);
        let entity = app
            .world_mut()
            .spawn((
                FlightComputer {
                    profile: "basic_fly_by_wire".to_string(),
                    throttle: BRAKE_SENTINEL_THROTTLE,
                    yaw_input: 0.0,
                    turn_rate_deg_s: 45.0,
                },
                LinearVelocity(Vec3::new(3.0, -1.0, 0.0)),
                AngularVelocity(Vec3::new(0.0, 0.0, 0.01)),
            ))
            .id();
        app.update();

        let linear_velocity = app.world().entity(entity).get::<LinearVelocity>().unwrap();
        let angular_velocity = app.world().entity(entity).get::<AngularVelocity>().unwrap();
        assert_eq!(linear_velocity.0.x, 0.0);
        assert_eq!(linear_velocity.0.y, 0.0);
        assert_eq!(angular_velocity.0.z, 0.0);
    }

    #[test]
    fn brake_decel_never_overshoots_speed_to_negative() {
        let speed_mps = 4.0;
        let dt_s = 0.5;
        let decel = compute_brake_decel_accel_mps2(speed_mps, dt_s, 1.5, 8.0, 100.0, true);
        let next_speed = (speed_mps - decel * dt_s).max(0.0);
        assert!(next_speed <= speed_mps);
        assert!(next_speed >= 0.0);
    }

    #[test]
    fn active_brake_honors_passive_floor_when_engine_budget_is_low() {
        let decel = compute_brake_decel_accel_mps2(20.0, 1.0 / 30.0, 1.5, 8.0, 0.2, true);
        assert!(decel >= 1.5);
    }

    #[test]
    fn neutral_brake_uses_passive_decel() {
        let decel = compute_brake_decel_accel_mps2(20.0, 1.0 / 30.0, 1.5, 8.0, 100.0, false);
        assert!((decel - 1.5).abs() < 1e-6);
    }

    #[test]
    fn sanitize_planar_angular_velocity_clamps_and_zeros_non_planar_axes() {
        let sanitized = sanitize_planar_angular_velocity(Vec3::new(1.2, -2.8, 7.5), 2.0);
        assert_eq!(sanitized.x, 0.0);
        assert_eq!(sanitized.y, 0.0);
        assert_eq!(sanitized.z, 2.0);
    }

    #[test]
    fn clamp_angular_velocity_skips_mounted_modules() {
        let mut app = App::new();
        app.add_systems(Update, clamp_angular_velocity);

        let parent = uuid::Uuid::new_v4();
        let hull = app
            .world_mut()
            .spawn((
                FlightComputer {
                    profile: "basic_fly_by_wire".to_string(),
                    throttle: 0.0,
                    yaw_input: 0.0,
                    turn_rate_deg_s: 45.0,
                },
                AngularVelocity(Vec3::new(0.3, -0.4, 6.0)),
            ))
            .id();
        let module = app
            .world_mut()
            .spawn((
                FlightComputer {
                    profile: "basic_fly_by_wire".to_string(),
                    throttle: 0.0,
                    yaw_input: 0.0,
                    turn_rate_deg_s: 45.0,
                },
                MountedOn {
                    parent_entity_id: parent,
                    hardpoint_id: "test".to_string(),
                },
                AngularVelocity(Vec3::new(0.7, 0.8, 9.0)),
            ))
            .id();

        app.update();

        let hull_angular = app.world().entity(hull).get::<AngularVelocity>().unwrap();
        assert_eq!(hull_angular.0.x, 0.0);
        assert_eq!(hull_angular.0.y, 0.0);
        assert_eq!(hull_angular.0.z, 2.0);

        let module_angular = app.world().entity(module).get::<AngularVelocity>().unwrap();
        assert!((module_angular.0.x - 0.7).abs() < 1e-6);
        assert!((module_angular.0.y - 0.8).abs() < 1e-6);
        assert!((module_angular.0.z - 9.0).abs() < 1e-6);
    }
}

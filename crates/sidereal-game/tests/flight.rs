use avian2d::prelude::{AngularVelocity, LinearVelocity};
use bevy::prelude::*;
use sidereal_game::{ActionQueue, EntityAction, FlightComputer, MountedOn};
use sidereal_game::{
    clamp_angular_velocity, compute_brake_decel_accel_mps2, process_flight_actions,
    sanitize_planar_angular_velocity, stabilize_idle_motion,
};
use uuid::Uuid;

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
                brake_active: false,
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
                brake_active: false,
                turn_rate_deg_s: 45.0,
            },
        ))
        .id();
    app.update();

    let computer = app.world().entity(entity).get::<FlightComputer>().unwrap();
    assert!(computer.brake_active);
    assert!(computer.throttle.abs() < f32::EPSILON);
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
                brake_active: false,
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
                brake_active: false,
                turn_rate_deg_s: 45.0,
            },
            LinearVelocity(bevy::prelude::Vec2::new(0.02, -0.03)),
            AngularVelocity(0.01),
        ))
        .id();
    app.update();

    let linear_velocity = app.world().entity(entity).get::<LinearVelocity>().unwrap();
    let angular_velocity = app.world().entity(entity).get::<AngularVelocity>().unwrap();
    assert_eq!(linear_velocity.0.x, 0.0);
    assert_eq!(linear_velocity.0.y, 0.0);
    assert_eq!(angular_velocity.0, 0.0);
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
                brake_active: false,
                turn_rate_deg_s: 45.0,
            },
            LinearVelocity(bevy::prelude::Vec2::new(0.02, -0.03)),
            AngularVelocity(0.01),
        ))
        .id();
    app.update();

    let linear_velocity = app.world().entity(entity).get::<LinearVelocity>().unwrap();
    let angular_velocity = app.world().entity(entity).get::<AngularVelocity>().unwrap();
    assert!((linear_velocity.0.x - 0.02).abs() < 1e-6);
    assert!((linear_velocity.0.y + 0.03).abs() < 1e-6);
    assert!((angular_velocity.0 - 0.01).abs() < 1e-6);
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
                throttle: 0.0,
                yaw_input: 0.0,
                brake_active: true,
                turn_rate_deg_s: 45.0,
            },
            LinearVelocity(bevy::prelude::Vec2::new(3.0, -1.0)),
            AngularVelocity(0.01),
        ))
        .id();
    app.update();

    let linear_velocity = app.world().entity(entity).get::<LinearVelocity>().unwrap();
    let angular_velocity = app.world().entity(entity).get::<AngularVelocity>().unwrap();
    assert_eq!(linear_velocity.0.x, 0.0);
    assert_eq!(linear_velocity.0.y, 0.0);
    assert_eq!(angular_velocity.0, 0.0);
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
    let sanitized = sanitize_planar_angular_velocity(7.5, 2.0);
    assert_eq!(sanitized, 2.0);
}

#[test]
fn clamp_angular_velocity_skips_mounted_modules() {
    let mut app = App::new();
    app.add_systems(Update, clamp_angular_velocity);

    let parent = Uuid::new_v4();
    let hull = app
        .world_mut()
        .spawn((
            FlightComputer {
                profile: "basic_fly_by_wire".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                brake_active: false,
                turn_rate_deg_s: 45.0,
            },
            AngularVelocity(6.0),
        ))
        .id();
    let _module = app
        .world_mut()
        .spawn((
            FlightComputer {
                profile: "basic_fly_by_wire".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                brake_active: false,
                turn_rate_deg_s: 45.0,
            },
            MountedOn {
                parent_entity_id: parent,
                hardpoint_id: "test".to_string(),
            },
            AngularVelocity(9.0),
        ))
        .id();

    app.update();

    let hull_angular = app.world().entity(hull).get::<AngularVelocity>().unwrap();
    assert_eq!(hull_angular.0, 2.0);

    let module_angular = app
        .world()
        .entity(_module)
        .get::<AngularVelocity>()
        .unwrap();
    assert!((module_angular.0 - 9.0).abs() < 1e-6);
}

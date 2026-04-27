use avian2d::prelude::{AngularVelocity, LinearVelocity};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use sidereal_game::flight::compute_flight_forces;
use sidereal_game::{
    ActionQueue, AfterburnerState, Engine, EntityAction, EntityGuid, FlightComputer,
    FlightControlAuthority, FlightFuelConsumptionEnabled, FuelTank, MountedOn,
    SimulationMotionWriter, apply_engine_thrust,
};
use sidereal_game::{
    clamp_angular_velocity, compute_brake_decel_accel_mps2, process_flight_actions,
    sanitize_planar_angular_velocity, stabilize_idle_motion,
};
use std::time::Duration;
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
                    EntityAction::Forward,
                    EntityAction::Right,
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
            FlightControlAuthority,
            SimulationMotionWriter,
        ))
        .id();
    app.update();

    let queue = app.world().entity(entity).get::<ActionQueue>().unwrap();
    let computer = app.world().entity(entity).get::<FlightComputer>().unwrap();
    assert_eq!(queue.pending, vec![EntityAction::FirePrimary]);
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
            FlightControlAuthority,
            SimulationMotionWriter,
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
            FlightControlAuthority,
            SimulationMotionWriter,
        ))
        .id();
    app.update();

    let computer = app.world().entity(entity).get::<FlightComputer>().unwrap();
    let queue = app.world().entity(entity).get::<ActionQueue>().unwrap();
    assert!((computer.throttle - 0.25).abs() < f32::EPSILON);
    assert!((computer.yaw_input + 0.5).abs() < f32::EPSILON);
    assert_eq!(queue.pending, vec![EntityAction::FirePrimary]);
}

#[test]
fn process_flight_actions_toggles_afterburner_state() {
    let mut app = App::new();
    app.add_systems(Update, process_flight_actions);
    let entity = app
        .world_mut()
        .spawn((
            ActionQueue {
                pending: vec![EntityAction::AfterburnerOn, EntityAction::AfterburnerOff],
            },
            FlightComputer {
                profile: "basic_fly_by_wire".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                brake_active: false,
                turn_rate_deg_s: 45.0,
            },
            AfterburnerState::default(),
            FlightControlAuthority,
            SimulationMotionWriter,
        ))
        .id();
    app.update();
    let afterburner_state = app
        .world()
        .entity(entity)
        .get::<AfterburnerState>()
        .unwrap();
    assert!(!afterburner_state.active);
}

#[test]
fn apply_engine_thrust_does_not_consume_fuel_when_disabled_for_prediction() {
    let mut app = App::new();
    app.insert_resource(Time::<Fixed>::from_hz(30.0));
    app.insert_resource(FlightFuelConsumptionEnabled(false));
    let parent_guid = Uuid::new_v4();
    let tank = app
        .world_mut()
        .spawn((
            EntityGuid(Uuid::new_v4()),
            MountedOn {
                parent_entity_id: parent_guid,
                hardpoint_id: "fuel".to_string(),
            },
            FuelTank { fuel_kg: 10.0 },
        ))
        .id();
    app.world_mut().spawn((
        EntityGuid(parent_guid),
        FlightComputer {
            profile: "basic_fly_by_wire".to_string(),
            throttle: 1.0,
            yaw_input: 0.0,
            brake_active: false,
            turn_rate_deg_s: 45.0,
        },
        FlightControlAuthority,
        SimulationMotionWriter,
    ));
    app.world_mut().spawn((
        EntityGuid(Uuid::new_v4()),
        MountedOn {
            parent_entity_id: parent_guid,
            hardpoint_id: "engine".to_string(),
        },
        Engine {
            thrust: 100.0,
            reverse_thrust: 50.0,
            torque_thrust: 25.0,
            burn_rate_kg_s: 3.0,
        },
    ));

    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(Duration::from_millis(100));
    let _ = app.world_mut().run_system_once(apply_engine_thrust);

    let fuel = app.world().entity(tank).get::<FuelTank>().unwrap();
    assert_eq!(fuel.fuel_kg, 10.0);
}

#[test]
fn apply_engine_thrust_consumes_fuel_by_default_for_authority() {
    let mut app = App::new();
    app.insert_resource(Time::<Fixed>::from_hz(30.0));
    let parent_guid = Uuid::new_v4();
    let tank = app
        .world_mut()
        .spawn((
            EntityGuid(Uuid::new_v4()),
            MountedOn {
                parent_entity_id: parent_guid,
                hardpoint_id: "fuel".to_string(),
            },
            FuelTank { fuel_kg: 10.0 },
        ))
        .id();
    app.world_mut().spawn((
        EntityGuid(parent_guid),
        FlightComputer {
            profile: "basic_fly_by_wire".to_string(),
            throttle: 1.0,
            yaw_input: 0.0,
            brake_active: false,
            turn_rate_deg_s: 45.0,
        },
        FlightControlAuthority,
        SimulationMotionWriter,
    ));
    app.world_mut().spawn((
        EntityGuid(Uuid::new_v4()),
        MountedOn {
            parent_entity_id: parent_guid,
            hardpoint_id: "engine".to_string(),
        },
        Engine {
            thrust: 100.0,
            reverse_thrust: 50.0,
            torque_thrust: 25.0,
            burn_rate_kg_s: 3.0,
        },
    ));

    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(Duration::from_millis(100));
    let _ = app.world_mut().run_system_once(apply_engine_thrust);

    let fuel = app.world().entity(tank).get::<FuelTank>().unwrap();
    assert!(fuel.fuel_kg < 10.0);
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
            FlightControlAuthority,
            SimulationMotionWriter,
            LinearVelocity(bevy::prelude::Vec2::new(0.02, -0.03).into()),
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
            FlightControlAuthority,
            SimulationMotionWriter,
            LinearVelocity(bevy::prelude::Vec2::new(0.02, -0.03).into()),
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
            FlightControlAuthority,
            SimulationMotionWriter,
            LinearVelocity(bevy::prelude::Vec2::new(3.0, -1.0).into()),
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
            FlightControlAuthority,
            SimulationMotionWriter,
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
            FlightControlAuthority,
            SimulationMotionWriter,
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

#[test]
fn forward_thrust_does_not_brake_when_overspeed_from_external_velocity() {
    let rotation = Quat::IDENTITY;
    let velocity = Vec2::Y * 300.0;
    let flight_tuning = sidereal_game::FlightTuning {
        max_linear_accel_mps2: 120.0,
        passive_brake_accel_mps2: 1.0,
        active_brake_accel_mps2: 10.0,
        drag_per_s: 0.0,
    };
    let (force, _torque) = compute_flight_forces(
        (1.0, 0.0, 90.0, false),
        velocity,
        0.0,
        rotation,
        10_000.0,
        1000.0,
        &flight_tuning,
        100.0,
        200_000.0,
        200_000.0,
        0.0,
        1.0 / 60.0,
    );
    assert!(force.dot(Vec2::Y) >= -1e-3);
}

#[test]
fn reverse_thrust_can_decelerate_overspeed_forward_motion() {
    let rotation = Quat::IDENTITY;
    let velocity = Vec2::Y * 300.0;
    let flight_tuning = sidereal_game::FlightTuning {
        max_linear_accel_mps2: 120.0,
        passive_brake_accel_mps2: 1.0,
        active_brake_accel_mps2: 10.0,
        drag_per_s: 0.0,
    };
    let (force, _torque) = compute_flight_forces(
        (-0.7, 0.0, 90.0, false),
        velocity,
        0.0,
        rotation,
        10_000.0,
        1000.0,
        &flight_tuning,
        100.0,
        200_000.0,
        200_000.0,
        0.0,
        1.0 / 60.0,
    );
    assert!(force.dot(Vec2::Y) < 0.0);
}

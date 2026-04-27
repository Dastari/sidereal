use avian2d::prelude::{
    AngularInertia, AngularVelocity, Collider, Gravity, LinearVelocity, Mass, PhysicsPlugins,
    Position, RigidBody, Rotation,
};
use bevy::prelude::*;
use sidereal_game::{
    ActionQueue, BallisticProjectileSpawnedEvent, EntityAction, EntityDestroyedEvent,
    EntityDestructionStartedEvent, EntityGuid, FlightComputer, FlightControlAuthority,
    FlightFuelConsumptionEnabled, FlightTuning, MaxVelocityMps, MountedOn, ShotFiredEvent,
    ShotHitEvent, ShotImpactResolvedEvent, SiderealGameCorePlugin, SiderealSharedSimulationPlugin,
    SimulationMotionWriter, SimulationRuntimeRole, SizeM, TotalMassKg, angular_inertia_from_size,
};
use std::time::Duration;
use uuid::Uuid;

fn init_game_messages(app: &mut App) {
    app.add_message::<ShotFiredEvent>();
    app.add_message::<ShotImpactResolvedEvent>();
    app.add_message::<ShotHitEvent>();
    app.add_message::<BallisticProjectileSpawnedEvent>();
    app.add_message::<EntityDestructionStartedEvent>();
    app.add_message::<EntityDestroyedEvent>();
}

fn init_physics(app: &mut App) {
    app.add_plugins(PhysicsPlugins::default().with_length_unit(1.0));
    app.insert_resource(Gravity(Vec2::ZERO.into()));
}

fn run_fixed_tick(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f64(1.0 / 60.0));
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(Duration::from_secs_f64(1.0 / 60.0));
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().run_schedule(FixedPostUpdate);
}

fn configure_sim_app(role: SimulationRuntimeRole) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
    app.insert_resource(FlightFuelConsumptionEnabled(false));
    init_physics(&mut app);
    app.add_plugins(SiderealGameCorePlugin);
    init_game_messages(&mut app);
    app.add_plugins(SiderealSharedSimulationPlugin { role });
    app.finish();
    app.cleanup();
    app
}

#[test]
fn client_prediction_role_does_not_grant_global_motion_authority() {
    let mut app = configure_sim_app(SimulationRuntimeRole::ClientPrediction);

    let entity = app
        .world_mut()
        .spawn((
            ActionQueue {
                pending: vec![EntityAction::Forward],
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

    run_fixed_tick(&mut app);

    let entity_ref = app.world().entity(entity);
    assert!(entity_ref.get::<FlightControlAuthority>().is_none());
    assert!(entity_ref.get::<SimulationMotionWriter>().is_none());
    assert_eq!(entity_ref.get::<FlightComputer>().unwrap().throttle, 0.0);
}

#[test]
fn server_authority_role_grants_motion_authority_and_processes_input() {
    let mut app = configure_sim_app(SimulationRuntimeRole::ServerAuthority);

    let entity = app
        .world_mut()
        .spawn((
            ActionQueue {
                pending: vec![EntityAction::Forward],
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

    run_fixed_tick(&mut app);
    run_fixed_tick(&mut app);

    let entity_ref = app.world().entity(entity);
    assert!(entity_ref.get::<FlightControlAuthority>().is_some());
    assert!(entity_ref.get::<SimulationMotionWriter>().is_some());
    assert_eq!(entity_ref.get::<FlightComputer>().unwrap().throttle, 1.0);
}

fn spawn_predicted_flight_body(app: &mut App) -> Entity {
    let parent_guid = Uuid::new_v4();
    let size = SizeM {
        length: 12.0,
        width: 8.0,
        height: 3.0,
    };
    let mass = 10_000.0;
    let entity = app
        .world_mut()
        .spawn((
            EntityGuid(parent_guid),
            ActionQueue::default(),
            FlightComputer {
                profile: "basic_fly_by_wire".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                brake_active: false,
                turn_rate_deg_s: 45.0,
            },
            FlightControlAuthority,
            SimulationMotionWriter,
            FlightTuning {
                max_linear_accel_mps2: 120.0,
                passive_brake_accel_mps2: 1.0,
                active_brake_accel_mps2: 10.0,
                drag_per_s: 0.0,
            },
            MaxVelocityMps(500.0),
            TotalMassKg(mass),
            size,
        ))
        .insert((
            RigidBody::Dynamic,
            Collider::circle(4.0),
            Mass(mass),
            angular_inertia_from_size(mass, &size),
            Position::default(),
            Rotation::default(),
            Transform::default(),
            LinearVelocity::default(),
            AngularVelocity::default(),
        ))
        .id();
    app.world_mut().spawn((
        EntityGuid(Uuid::new_v4()),
        MountedOn {
            parent_entity_id: parent_guid,
            hardpoint_id: "engine".to_string(),
        },
        sidereal_game::Engine {
            thrust: 200_000.0,
            reverse_thrust: 100_000.0,
            torque_thrust: 200_000.0,
            burn_rate_kg_s: 1.0,
        },
    ));
    app.world_mut().spawn((
        EntityGuid(Uuid::new_v4()),
        MountedOn {
            parent_entity_id: parent_guid,
            hardpoint_id: "fuel".to_string(),
        },
        sidereal_game::FuelTank { fuel_kg: 1_000.0 },
    ));
    entity
}

fn set_actions(app: &mut App, entity: Entity, actions: &[EntityAction]) {
    app.world_mut()
        .entity_mut(entity)
        .get_mut::<ActionQueue>()
        .unwrap()
        .pending = actions.to_vec();
}

#[derive(Debug, Clone, Copy)]
struct MotionExpectation {
    min_distance_m: f64,
    min_rotation_rad: f64,
}

fn assert_motion_parity_for_actions(
    actions: &[EntityAction],
    ticks: usize,
    expectation: MotionExpectation,
) {
    let mut server = configure_sim_app(SimulationRuntimeRole::ServerAuthority);
    let mut client = configure_sim_app(SimulationRuntimeRole::ClientPrediction);
    let server_entity = spawn_predicted_flight_body(&mut server);
    let client_entity = spawn_predicted_flight_body(&mut client);

    for _ in 0..ticks {
        set_actions(&mut server, server_entity, actions);
        set_actions(&mut client, client_entity, actions);
        run_fixed_tick(&mut server);
        run_fixed_tick(&mut client);
    }

    let server_entity_ref = server.world().entity(server_entity);
    let client_entity_ref = client.world().entity(client_entity);
    let server_position = server_entity_ref.get::<Position>().unwrap().0;
    let client_position = client_entity_ref.get::<Position>().unwrap().0;
    let server_rotation = server_entity_ref.get::<Rotation>().unwrap().as_radians();
    let client_rotation = client_entity_ref.get::<Rotation>().unwrap().as_radians();
    let server_velocity = server_entity_ref.get::<LinearVelocity>().unwrap().0;
    let client_velocity = client_entity_ref.get::<LinearVelocity>().unwrap().0;
    let server_angular = server_entity_ref.get::<AngularVelocity>().unwrap().0;
    let client_angular = client_entity_ref.get::<AngularVelocity>().unwrap().0;

    assert!(
        server_position.length() >= expectation.min_distance_m,
        "scenario did not move far enough to exercise thrust: position={server_position:?}"
    );
    assert!(
        server_rotation.abs() >= expectation.min_rotation_rad,
        "scenario did not rotate far enough to exercise torque: rotation={server_rotation}"
    );
    assert!((server_position - client_position).length() <= 0.001);
    assert!((server_rotation - client_rotation).abs() <= 0.001);
    assert!((server_velocity - client_velocity).length() <= 0.001);
    assert!((server_angular - client_angular).abs() <= 0.001);
    assert_eq!(
        server_entity_ref.get::<Mass>().unwrap().0,
        client_entity_ref.get::<Mass>().unwrap().0
    );
    assert_eq!(
        server_entity_ref.get::<AngularInertia>().unwrap().0,
        client_entity_ref.get::<AngularInertia>().unwrap().0
    );
}

#[test]
fn client_prediction_and_server_authority_have_motion_parity_for_same_input() {
    assert_motion_parity_for_actions(
        &[EntityAction::Forward, EntityAction::Left],
        60,
        MotionExpectation {
            min_distance_m: 0.1,
            min_rotation_rad: 0.001,
        },
    );
}

#[test]
fn client_prediction_and_server_authority_have_forward_thrust_parity() {
    assert_motion_parity_for_actions(
        &[EntityAction::Forward, EntityAction::LateralNeutral],
        60,
        MotionExpectation {
            min_distance_m: 0.1,
            min_rotation_rad: 0.0,
        },
    );
}

#[test]
fn client_prediction_and_server_authority_have_left_turn_parity() {
    assert_motion_parity_for_actions(
        &[EntityAction::LongitudinalNeutral, EntityAction::Left],
        120,
        MotionExpectation {
            min_distance_m: 0.0,
            min_rotation_rad: 0.01,
        },
    );
}

#[test]
fn client_prediction_and_server_authority_have_right_turn_parity() {
    assert_motion_parity_for_actions(
        &[EntityAction::LongitudinalNeutral, EntityAction::Right],
        120,
        MotionExpectation {
            min_distance_m: 0.0,
            min_rotation_rad: 0.01,
        },
    );
}

use avian2d::prelude::{LinearVelocity, Position};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use sidereal_game::{
    ActionQueue, CharacterMovementController, ControlledEntityGuid, EntityAction, EntityGuid,
    process_character_movement_actions,
};
use std::time::Duration;
use uuid::Uuid;

#[test]
fn movement_actions_move_when_not_controlled() {
    let mut app = App::new();
    app.insert_resource(Time::<Fixed>::from_hz(30.0));
    let own_guid = Uuid::new_v4();
    let entity = app
        .world_mut()
        .spawn((
            ActionQueue {
                pending: vec![EntityAction::Forward],
            },
            EntityGuid(own_guid),
            CharacterMovementController {
                speed_mps: 30.0,
                max_accel_mps2: 120.0,
                damping_per_s: 8.0,
            },
            Transform::default(),
            Position(Vec2::ZERO),
            LinearVelocity(Vec2::ZERO),
            ControlledEntityGuid(Some(own_guid.to_string())),
        ))
        .id();

    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(Duration::from_millis(33));
    let _ = app
        .world_mut()
        .run_system_once(process_character_movement_actions);

    let velocity = app.world().entity(entity).get::<LinearVelocity>().unwrap();
    assert!(velocity.0.y > 0.0);
}

#[test]
fn movement_actions_do_not_move_when_controlled() {
    let mut app = App::new();
    app.insert_resource(Time::<Fixed>::from_hz(30.0));
    let controlled_guid = Uuid::new_v4().to_string();
    let entity = app
        .world_mut()
        .spawn((
            ActionQueue {
                pending: vec![EntityAction::Forward],
            },
            EntityGuid(Uuid::new_v4()),
            CharacterMovementController {
                speed_mps: 30.0,
                max_accel_mps2: 120.0,
                damping_per_s: 8.0,
            },
            Transform::default(),
            Position(Vec2::ZERO),
            LinearVelocity(Vec2::ZERO),
            ControlledEntityGuid(Some(controlled_guid)),
        ))
        .id();

    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(Duration::from_millis(33));
    let _ = app
        .world_mut()
        .run_system_once(process_character_movement_actions);

    let velocity = app.world().entity(entity).get::<LinearVelocity>().unwrap();
    assert_eq!(velocity.0, Vec2::ZERO);
    let queue = app.world().entity(entity).get::<ActionQueue>().unwrap();
    assert!(queue.pending.is_empty());
}

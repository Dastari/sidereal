use avian2d::prelude::{LinearVelocity, Position};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use sidereal_game::{
    ActionQueue, CharacterMovementController, ControlledEntityGuid, EntityAction, EntityGuid,
    PlayerTag, SimulationMotionWriter, process_character_movement_actions,
    sync_player_to_controlled_entity,
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
            PlayerTag,
            SimulationMotionWriter,
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
            PlayerTag,
            SimulationMotionWriter,
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

#[test]
fn player_anchor_follows_controlled_entity_when_controlling_other() {
    let mut app = App::new();
    let controlled_guid = Uuid::new_v4();
    let player_guid = Uuid::new_v4();
    let target_pos = Vec2::new(125.0, -42.0);

    app.world_mut().spawn((
        EntityGuid(controlled_guid),
        Transform::from_xyz(target_pos.x, target_pos.y, 0.0),
        Position(target_pos),
    ));

    let player = app
        .world_mut()
        .spawn((
            PlayerTag,
            EntityGuid(player_guid),
            Transform::from_xyz(0.0, 0.0, 0.0),
            Position(Vec2::ZERO),
            ControlledEntityGuid(Some(controlled_guid.to_string())),
        ))
        .id();

    let _ = app
        .world_mut()
        .run_system_once(sync_player_to_controlled_entity);

    let player_transform = app.world().entity(player).get::<Transform>().unwrap();
    let player_position = app.world().entity(player).get::<Position>().unwrap();
    assert_eq!(player_transform.translation.x, target_pos.x);
    assert_eq!(player_transform.translation.y, target_pos.y);
    assert_eq!(player_transform.translation.z, 0.0);
    assert_eq!(player_position.0, target_pos);
}

#[test]
fn player_anchor_does_not_move_when_controlled_entity_is_self() {
    let mut app = App::new();
    let player_guid = Uuid::new_v4();
    let original = Vec2::new(9.0, 13.0);
    let player = app
        .world_mut()
        .spawn((
            PlayerTag,
            EntityGuid(player_guid),
            Transform::from_xyz(original.x, original.y, 0.0),
            Position(original),
            ControlledEntityGuid(Some(player_guid.to_string())),
        ))
        .id();

    let _ = app
        .world_mut()
        .run_system_once(sync_player_to_controlled_entity);

    let player_transform = app.world().entity(player).get::<Transform>().unwrap();
    let player_position = app.world().entity(player).get::<Position>().unwrap();
    assert_eq!(player_transform.translation.x, original.x);
    assert_eq!(player_transform.translation.y, original.y);
    assert_eq!(player_position.0, original);
}

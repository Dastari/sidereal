use avian3d::prelude::Position;
use bevy::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    ActionQueue, CharacterMovementController, ControlledEntityGuid, EntityAction, EntityGuid,
    PlayerTag,
};

fn apply_character_action(longitudinal: &mut f32, lateral: &mut f32, action: EntityAction) -> bool {
    match action {
        EntityAction::Forward | EntityAction::ThrustForward => {
            *longitudinal = 1.0;
            true
        }
        EntityAction::Backward | EntityAction::ThrustReverse => {
            *longitudinal = -1.0;
            true
        }
        EntityAction::LongitudinalNeutral | EntityAction::ThrustNeutral => {
            *longitudinal = 0.0;
            true
        }
        EntityAction::Left | EntityAction::YawLeft => {
            *lateral = -1.0;
            true
        }
        EntityAction::Right | EntityAction::YawRight => {
            *lateral = 1.0;
            true
        }
        EntityAction::LateralNeutral | EntityAction::YawNeutral => {
            *lateral = 0.0;
            true
        }
        _ => false,
    }
}

/// Applies character movement intents directly to observer/player transform.
/// This system only moves entities when they are not currently controlling another target.
#[allow(clippy::type_complexity)]
pub fn process_character_movement_actions(
    time: Res<'_, Time<Fixed>>,
    mut query: Query<
        '_,
        '_,
        (
            &mut ActionQueue,
            &EntityGuid,
            &CharacterMovementController,
            &mut Transform,
            Option<&mut Position>,
            Option<&ControlledEntityGuid>,
        ),
    >,
) {
    let dt_s = time.delta_secs();
    if dt_s <= 0.0 {
        return;
    }

    for (mut queue, entity_guid, controller, mut transform, maybe_position, controlled) in
        &mut query
    {
        let own_guid = entity_guid.0.to_string();
        let controls_other_entity = controlled
            .and_then(|value| value.0.as_ref())
            .is_some_and(|guid| guid != &own_guid);
        if controls_other_entity {
            queue.clear();
            continue;
        }
        if queue.pending.is_empty() {
            continue;
        }

        let mut longitudinal = 0.0_f32;
        let mut lateral = 0.0_f32;
        for action in queue.drain() {
            let _ = apply_character_action(&mut longitudinal, &mut lateral, action);
        }

        let mut delta = Vec3::new(lateral, longitudinal, 0.0);
        if delta.length_squared() > 1.0 {
            delta = delta.normalize();
        }
        delta *= controller.speed_mps.max(0.0) * dt_s;
        transform.translation += delta;
        transform.translation.z = 0.0;

        if let Some(mut position) = maybe_position {
            position.0 = transform.translation;
        }
    }
}

/// Keeps player observer entities attached to their currently controlled entity.
/// This enforces the runtime chain: camera <- player <- controlled entity.
#[allow(clippy::type_complexity)]
pub fn sync_player_to_controlled_entity(
    mut target_position_by_guid: Local<'_, HashMap<Uuid, Vec3>>,
    mut params: ParamSet<
        '_,
        '_,
        (
            Query<
                '_,
                '_,
                (&'_ EntityGuid, &'_ Transform, Option<&'_ Position>),
                Without<PlayerTag>,
            >,
            Query<
                '_,
                '_,
                (
                    &'_ mut Transform,
                    Option<&'_ mut Position>,
                    Option<&'_ ControlledEntityGuid>,
                ),
                With<PlayerTag>,
            >,
        ),
    >,
) {
    target_position_by_guid.clear();
    for (guid, transform, maybe_position) in &params.p0() {
        let world_position = maybe_position
            .map(|position| position.0)
            .unwrap_or(transform.translation);
        target_position_by_guid.insert(guid.0, world_position);
    }

    for (mut player_transform, maybe_player_position, controlled) in &mut params.p1() {
        let Some(control_guid) = controlled.and_then(|value| value.0.as_ref()) else {
            continue;
        };
        let Ok(control_guid) = Uuid::parse_str(control_guid) else {
            continue;
        };
        let Some(target_position) = target_position_by_guid.get(&control_guid) else {
            continue;
        };

        player_transform.translation = *target_position;
        if let Some(mut player_position) = maybe_player_position {
            player_position.0 = *target_position;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
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
                CharacterMovementController { speed_mps: 30.0 },
                Transform::default(),
                Position(Vec3::ZERO),
                ControlledEntityGuid(Some(own_guid.to_string())),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .advance_by(Duration::from_millis(33));
        let _ = app
            .world_mut()
            .run_system_once(process_character_movement_actions);

        let transform = app.world().entity(entity).get::<Transform>().unwrap();
        assert!(transform.translation.y > 0.0);
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
                CharacterMovementController { speed_mps: 30.0 },
                Transform::default(),
                Position(Vec3::ZERO),
                ControlledEntityGuid(Some(controlled_guid)),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .advance_by(Duration::from_millis(33));
        let _ = app
            .world_mut()
            .run_system_once(process_character_movement_actions);

        let transform = app.world().entity(entity).get::<Transform>().unwrap();
        assert_eq!(transform.translation, Vec3::ZERO);
        let queue = app.world().entity(entity).get::<ActionQueue>().unwrap();
        assert!(queue.pending.is_empty());
    }
}

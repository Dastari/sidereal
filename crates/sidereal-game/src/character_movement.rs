use avian2d::prelude::Position;
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

        let mut delta = Vec2::new(lateral, longitudinal);
        if delta.length_squared() > 1.0 {
            delta = delta.normalize();
        }
        delta *= controller.speed_mps.max(0.0) * dt_s;
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
        transform.translation.z = 0.0;

        if let Some(mut position) = maybe_position {
            position.0 = transform.translation.truncate();
        }
    }
}

/// Keeps player observer entities attached to their currently controlled entity.
/// This enforces the runtime chain: camera <- player <- controlled entity.
#[allow(clippy::type_complexity)]
pub fn sync_player_to_controlled_entity(
    mut target_position_by_guid: Local<'_, HashMap<Uuid, Vec2>>,
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
            .unwrap_or(transform.translation.truncate());
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

        player_transform.translation.x = target_position.x;
        player_transform.translation.y = target_position.y;
        player_transform.translation.z = 0.0;
        if let Some(mut player_position) = maybe_player_position {
            player_position.0 = *target_position;
        }
    }
}

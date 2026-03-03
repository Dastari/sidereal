use avian2d::prelude::{LinearVelocity, Position};
use bevy::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    ActionQueue, CharacterMovementController, ControlledEntityGuid, EntityAction, EntityGuid,
    PlayerTag,
};

const DEFAULT_PLAYER_SPEED_MPS: f32 = 220.0;
const DEFAULT_PLAYER_MAX_ACCEL_MPS2: f32 = 880.0;
const DEFAULT_PLAYER_DAMPING_PER_S: f32 = 8.0;
const MIN_PLAYER_SPEED_MPS: f32 = 1.0;
const STOP_EPSILON_MPS: f32 = 0.05;

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
            Option<&CharacterMovementController>,
            Option<&mut LinearVelocity>,
            Option<&mut Transform>,
            Option<&mut Position>,
            Option<&ControlledEntityGuid>,
        ),
        With<PlayerTag>,
    >,
) {
    let dt_s = time.delta_secs();
    if dt_s <= 0.0 {
        return;
    }

    for (
        mut queue,
        entity_guid,
        maybe_controller,
        maybe_linear_velocity,
        maybe_transform,
        maybe_position,
        controlled,
    ) in &mut query
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
        let pending = std::mem::take(&mut queue.pending);
        for action in pending {
            let handled = apply_character_action(&mut longitudinal, &mut lateral, action);
            if !handled {
                queue.pending.push(action);
            }
        }

        let speed_mps = maybe_controller
            .map(|controller| controller.speed_mps)
            .unwrap_or(DEFAULT_PLAYER_SPEED_MPS)
            .max(MIN_PLAYER_SPEED_MPS);
        let max_accel_mps2 = maybe_controller
            .map(|controller| controller.max_accel_mps2)
            .unwrap_or(DEFAULT_PLAYER_MAX_ACCEL_MPS2)
            .max(5.0);
        let damping_per_s = maybe_controller
            .map(|controller| controller.damping_per_s)
            .unwrap_or(DEFAULT_PLAYER_DAMPING_PER_S)
            .max(0.0);

        let mut desired_dir = Vec2::new(lateral, longitudinal);
        if desired_dir.length_squared() > 1.0 {
            desired_dir = desired_dir.normalize();
        }

        let desired_velocity = desired_dir * speed_mps;
        let damping = (-damping_per_s * dt_s).exp();

        if let Some(mut linear_velocity) = maybe_linear_velocity {
            let current_velocity = linear_velocity.0;
            let max_delta_speed = max_accel_mps2 * dt_s;
            let velocity_delta = desired_velocity - current_velocity;
            let next_velocity = if velocity_delta.length_squared() > 0.0
                && velocity_delta.length() > max_delta_speed
            {
                current_velocity + velocity_delta.normalize() * max_delta_speed
            } else {
                desired_velocity
            };
            let mut damped_velocity = if desired_dir == Vec2::ZERO {
                next_velocity * damping
            } else {
                next_velocity
            };
            if damped_velocity.length() <= STOP_EPSILON_MPS {
                damped_velocity = Vec2::ZERO;
            }
            linear_velocity.0 = damped_velocity;
            continue;
        }

        // Fallback path for non-physics entities.
        if let Some(mut transform) = maybe_transform {
            let mut fallback_velocity = desired_velocity;
            if desired_dir == Vec2::ZERO {
                fallback_velocity *= damping;
            }
            if fallback_velocity.length() <= STOP_EPSILON_MPS {
                fallback_velocity = Vec2::ZERO;
            }
            transform.translation.x += fallback_velocity.x * dt_s;
            transform.translation.y += fallback_velocity.y * dt_s;
            transform.translation.z = 0.0;
            if let Some(mut position) = maybe_position {
                position.0 = transform.translation.truncate();
            }
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

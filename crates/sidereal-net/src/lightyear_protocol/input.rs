use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use sidereal_game::{ActionQueue, EntityAction};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, Reflect)]
pub struct PlayerInput {
    pub actions: Vec<EntityAction>,
}

impl MapEntities for PlayerInput {
    fn map_entities<M: EntityMapper>(&mut self, _entity_mapper: &mut M) {}
}

pub fn actions_from_axis_inputs(
    thrust: f32,
    turn: f32,
    brake: bool,
    afterburner: bool,
    fire_primary: bool,
) -> Vec<EntityAction> {
    let mut actions = Vec::new();
    if brake {
        actions.push(EntityAction::Brake);
    } else if thrust > 0.0 {
        actions.push(EntityAction::Forward);
    } else if thrust < 0.0 {
        actions.push(EntityAction::Backward);
    } else {
        actions.push(EntityAction::LongitudinalNeutral);
    }

    if turn > 0.0 {
        actions.push(EntityAction::Left);
    } else if turn < 0.0 {
        actions.push(EntityAction::Right);
    } else {
        actions.push(EntityAction::LateralNeutral);
    }

    if afterburner {
        actions.push(EntityAction::AfterburnerOn);
    } else {
        actions.push(EntityAction::AfterburnerOff);
    }
    if fire_primary {
        actions.push(EntityAction::FirePrimary);
    }

    actions
}

impl PlayerInput {
    pub fn from_axis_inputs(
        thrust: f32,
        turn: f32,
        brake: bool,
        afterburner: bool,
        fire_primary: bool,
    ) -> Self {
        Self {
            actions: actions_from_axis_inputs(thrust, turn, brake, afterburner, fire_primary),
        }
    }
}

/// Replace an action queue with one input snapshot.
///
/// Both client prediction and server authority use latest-intent semantics:
/// each fixed tick consumes the current snapshot, not an appended backlog.
pub fn replace_action_queue_from_player_input(queue: &mut ActionQueue, input: &PlayerInput) {
    replace_action_queue_from_actions(queue, &input.actions);
}

/// Replace an action queue with one action snapshot.
pub fn replace_action_queue_from_actions(queue: &mut ActionQueue, actions: &[EntityAction]) {
    queue.clear();
    queue.pending.extend_from_slice(actions);
}

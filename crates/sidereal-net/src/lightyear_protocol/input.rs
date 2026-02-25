use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use sidereal_game::EntityAction;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, Reflect)]
pub struct PlayerInput {
    pub actions: Vec<EntityAction>,
}

impl MapEntities for PlayerInput {
    fn map_entities<M: EntityMapper>(&mut self, _entity_mapper: &mut M) {}
}

pub fn actions_from_axis_inputs(thrust: f32, turn: f32, brake: bool) -> Vec<EntityAction> {
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

    actions
}

impl PlayerInput {
    pub fn from_axis_inputs(thrust: f32, turn: f32, brake: bool) -> Self {
        Self {
            actions: actions_from_axis_inputs(thrust, turn, brake),
        }
    }
}

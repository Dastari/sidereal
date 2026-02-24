use bevy::prelude::*;
use bevy::reflect::Reflect;

use crate::EntityAction;

/// Component that queues pending actions for an entity.
#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct ActionQueue {
    /// Actions to process this tick.
    pub pending: Vec<EntityAction>,
}

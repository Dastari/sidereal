use bevy::prelude::*;
use bevy::reflect::Reflect;

use crate::EntityAction;

/// Component that declares which actions an entity can handle.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct ActionCapabilities {
    /// Set of actions this entity can process.
    pub supported: Vec<EntityAction>,
}

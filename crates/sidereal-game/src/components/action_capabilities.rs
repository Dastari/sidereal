use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityAction;

#[sidereal_component_macros::sidereal_component(
    kind = "action_capabilities",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ActionCapabilities {
    /// Set of actions this entity can process.
    pub supported: Vec<EntityAction>,
}

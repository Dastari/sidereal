use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityAction;

#[sidereal_component_macros::sidereal_component(
    kind = "action_queue",
    persist = true,
    replicate = true,
    predict = true,
    visibility = [OwnerOnly]
)]
#[derive(Debug, Clone, Component, Default, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ActionQueue {
    /// Actions to process this tick.
    pub pending: Vec<EntityAction>,
}

use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(
    kind = "cost",
    persist = true,
    replicate = true,
    visibility = [OwnerOnly, Public]
)]
#[derive(
    Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Default,
)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct Cost {
    pub credits: u64,
}

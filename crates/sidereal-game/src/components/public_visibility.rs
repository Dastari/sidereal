use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "public_visibility", persist = true, replicate = true, visibility = [Public])]
#[derive(
    Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Default,
)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct PublicVisibility;

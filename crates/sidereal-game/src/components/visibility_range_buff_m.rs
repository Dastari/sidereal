use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "visibility_range_buff_m", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct VisibilityRangeBuffM {
    pub additive_m: f32,
    pub multiplier: f32,
}

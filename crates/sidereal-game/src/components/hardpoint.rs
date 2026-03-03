use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "hardpoint", persist = true, replicate = true, visibility = [Public])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct Hardpoint {
    pub hardpoint_id: String,
    pub offset_m: Vec3,
    #[serde(default = "default_hardpoint_local_rotation")]
    pub local_rotation: Quat,
}

fn default_hardpoint_local_rotation() -> Quat {
    Quat::IDENTITY
}

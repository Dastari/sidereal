use bevy::prelude::*;
use bevy::reflect::{ReflectDeserialize, ReflectSerialize};
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "collision_outline_m", persist = true, replicate = true, visibility = [Public])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct CollisionOutlineM {
    pub points: Vec<Vec2>,
}

impl CollisionOutlineM {
    pub fn is_valid(&self) -> bool {
        self.points.len() >= 3
    }
}

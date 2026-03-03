use bevy::prelude::*;
use bevy::reflect::{ReflectDeserialize, ReflectSerialize};
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[derive(Debug, Clone, Copy, Default, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Serialize, Deserialize)]
pub enum CollisionMode {
    #[default]
    None,
    Aabb,
}

#[sidereal_component_macros::sidereal_component(kind = "collision_profile", persist = true, replicate = true, visibility = [Public])]
#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct CollisionProfile {
    pub mode: CollisionMode,
}

impl CollisionProfile {
    pub fn solid_aabb() -> Self {
        Self {
            mode: CollisionMode::Aabb,
        }
    }

    pub fn is_collidable(self) -> bool {
        !matches!(self.mode, CollisionMode::None)
    }
}

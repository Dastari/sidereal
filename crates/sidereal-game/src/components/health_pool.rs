use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "health_pool", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct HealthPool {
    pub current: f32,
    pub maximum: f32,
}

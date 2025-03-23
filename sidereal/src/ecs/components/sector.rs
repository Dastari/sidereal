use bevy::prelude::*;
use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect, Default)]
#[reflect(Component)]
pub struct Sector {
    pub x: i32,
    pub y: i32,
}

impl Sector {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

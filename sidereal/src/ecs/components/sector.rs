use bevy::prelude::*;
use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

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

// Implement Hash trait for Sector
impl Hash for Sector {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
    }
}

// Implement Eq trait for Sector (as it already has PartialEq)
impl Eq for Sector {}

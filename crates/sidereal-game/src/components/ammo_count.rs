use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "ammo_count", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct AmmoCount {
    pub current: u32,
    pub capacity: u32,
}

impl AmmoCount {
    pub fn new(current: u32, capacity: u32) -> Self {
        Self { current, capacity }
    }

    pub fn can_consume(&self, amount: u32) -> bool {
        self.current >= amount
    }

    pub fn consume(&mut self, amount: u32) -> bool {
        if !self.can_consume(amount) {
            return false;
        }
        self.current -= amount;
        true
    }
}

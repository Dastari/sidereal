use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::EntityGuid;

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Serialize, Deserialize)]
pub struct InventoryEntry {
    pub item_entity_id: Uuid,
    pub quantity: u32,
    pub unit_mass_kg: f32,
}

#[sidereal_component_macros::sidereal_component(
    kind = "inventory",
    persist = true,
    replicate = true,
    visibility = [OwnerOnly]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct Inventory {
    pub entries: Vec<InventoryEntry>,
}

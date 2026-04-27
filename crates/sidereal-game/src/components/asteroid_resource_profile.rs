use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Serialize, Deserialize)]
pub struct AsteroidYieldEntry {
    pub item_id: String,
    pub weight: f32,
    pub min_units: f32,
    pub max_units: f32,
}

#[sidereal_component_macros::sidereal_component(
    kind = "asteroid_resource_profile",
    persist = true,
    replicate = false
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AsteroidResourceProfile {
    pub profile_id: String,
    pub extraction_profile_id: Option<String>,
    pub yield_table: Vec<AsteroidYieldEntry>,
    pub depletion_pool_units: f32,
}

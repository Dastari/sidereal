use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Serialize, Deserialize)]
pub struct AsteroidSizeRangeM {
    pub min_m: f32,
    pub max_m: f32,
}

#[sidereal_component_macros::sidereal_component(
    kind = "asteroid_field_population",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AsteroidFieldPopulation {
    pub target_large_count: u32,
    pub target_medium_count: u32,
    pub target_small_count: u32,
    pub large_size_range_m: AsteroidSizeRangeM,
    pub medium_size_range_m: AsteroidSizeRangeM,
    pub small_size_range_m: AsteroidSizeRangeM,
    pub sprite_profile_id: String,
    pub resource_profile_id: String,
    pub fracture_profile_id: String,
}

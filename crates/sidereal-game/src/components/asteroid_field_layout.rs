use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use super::asteroid_field::{AsteroidFieldShape, AsteroidSizeTier};

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Serialize, Deserialize)]
pub struct AsteroidFieldCluster {
    pub cluster_key: String,
    pub offset_xy_m: [f32; 2],
    pub radius_m: f32,
    pub density_weight: f32,
    pub preferred_size_tier: AsteroidSizeTier,
    pub rarity_weight: f32,
}

#[sidereal_component_macros::sidereal_component(
    kind = "asteroid_field_layout",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AsteroidFieldLayout {
    pub shape: AsteroidFieldShape,
    pub density: f32,
    pub clusters: Vec<AsteroidFieldCluster>,
    pub spawn_noise_amplitude_m: f32,
    pub spawn_noise_frequency: f32,
}

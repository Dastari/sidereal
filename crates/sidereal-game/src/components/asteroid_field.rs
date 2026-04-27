use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
#[reflect(Serialize, Deserialize)]
pub enum AsteroidSizeTier {
    Small,
    #[default]
    Medium,
    Large,
    Massive,
}

#[derive(Debug, Clone, Copy, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
#[reflect(Serialize, Deserialize)]
pub enum AsteroidFieldShape {
    Ring,
    Ellipse,
    #[default]
    ClusterPatch,
    DenseCoreHalo,
    DebrisLane,
}

#[sidereal_component_macros::sidereal_component(
    kind = "asteroid_field",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AsteroidField {
    pub field_profile_id: String,
    pub content_version: u32,
    pub layout_seed: u64,
    pub activation_radius_m: f32,
    pub field_radius_m: f32,
    pub max_active_members: u32,
    pub max_active_fragments: u32,
    pub max_fracture_depth: u8,
    pub ambient_profile_id: Option<String>,
}

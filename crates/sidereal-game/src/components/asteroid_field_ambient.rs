use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[sidereal_component_macros::sidereal_component(
    kind = "asteroid_field_ambient",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AsteroidFieldAmbient {
    pub trigger_radius_m: f32,
    pub fade_band_m: f32,
    pub background_shader_asset_id: Option<String>,
    pub foreground_shader_asset_id: Option<String>,
    pub post_process_shader_asset_id: Option<String>,
    pub max_intensity: f32,
}

use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[sidereal_component_macros::sidereal_component(kind = "procedural_sprite", persist = true, replicate = true, visibility = [Public])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ProceduralSprite {
    pub generator_id: String,
    pub resolution_px: u32,
    pub edge_noise: f32,
    pub lobe_amplitude: f32,
    pub crater_count: u32,
    pub palette_dark_rgb: [f32; 3],
    pub palette_light_rgb: [f32; 3],
}

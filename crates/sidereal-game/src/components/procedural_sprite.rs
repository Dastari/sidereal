use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
#[reflect(Serialize, Deserialize)]
pub enum ProceduralSpriteSurfaceStyle {
    #[default]
    Rocky,
    Carbonaceous,
    Metallic,
    Shard,
    GemRich,
}

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
    pub surface_style: ProceduralSpriteSurfaceStyle,
    pub pixel_step_px: u32,
    pub crack_intensity: f32,
    pub mineral_vein_intensity: f32,
    pub mineral_accent_rgb: [f32; 3],
    pub family_seed_key: Option<String>,
}

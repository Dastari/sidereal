use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct AsteroidFieldProfileDefinition {
    pub field_profile_id: String,
    pub display_name: String,
    pub shape: String,
    pub radius_m: f32,
    pub density: f32,
    pub layout_seed: u64,
    pub sprite_profile_id: String,
    pub fracture_profile_id: String,
    pub resource_profile_id: String,
    pub ambient_profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct AsteroidSpriteProfileDefinition {
    pub sprite_profile_id: String,
    pub generator_id: String,
    #[serde(default)]
    pub surface_styles: Vec<String>,
    pub pixel_step_px: u32,
    pub crack_intensity_range: [f32; 2],
    pub mineral_vein_intensity_range: [f32; 2],
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct AsteroidFractureProfileDefinition {
    pub fracture_profile_id: String,
    pub break_massive_into_large: [u8; 2],
    pub break_large_into_medium: [u8; 2],
    pub break_medium_into_small: [u8; 2],
    pub child_impulse_mps: [f32; 2],
    pub mass_retention_ratio: f32,
    pub terminal_debris_loss_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct AsteroidResourceYieldDefinition {
    pub item_id: String,
    pub weight: f32,
    pub min_units: f32,
    pub max_units: f32,
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct AsteroidResourceProfileDefinition {
    pub resource_profile_id: String,
    pub extraction_profile_id: Option<String>,
    pub depletion_pool_units: f32,
    #[serde(default)]
    pub yield_table: Vec<AsteroidResourceYieldDefinition>,
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct AsteroidAmbientProfileDefinition {
    pub ambient_profile_id: String,
    pub trigger_radius_m: f32,
    pub fade_band_m: f32,
    pub background_shader_asset_id: Option<String>,
    pub foreground_shader_asset_id: Option<String>,
    pub post_process_shader_asset_id: Option<String>,
    pub max_intensity: f32,
}

#[derive(Resource, Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
#[reflect(Resource)]
pub struct AsteroidRegistry {
    pub schema_version: u32,
    #[serde(default)]
    pub field_profiles: Vec<AsteroidFieldProfileDefinition>,
    #[serde(default)]
    pub sprite_profiles: Vec<AsteroidSpriteProfileDefinition>,
    #[serde(default)]
    pub fracture_profiles: Vec<AsteroidFractureProfileDefinition>,
    #[serde(default)]
    pub resource_profiles: Vec<AsteroidResourceProfileDefinition>,
    #[serde(default)]
    pub ambient_profiles: Vec<AsteroidAmbientProfileDefinition>,
}

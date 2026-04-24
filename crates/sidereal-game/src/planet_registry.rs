use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::PlanetBodyShaderSettings;

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct PlanetRegistryEntry {
    pub planet_id: String,
    pub script: String,
    #[serde(default)]
    pub spawn_enabled: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct PlanetSpawnDefinition {
    pub entity_id: String,
    #[serde(default = "default_planet_owner_id")]
    pub owner_id: String,
    #[serde(default = "default_planet_size_m")]
    pub size_m: f32,
    #[serde(default)]
    pub spawn_position: [f32; 2],
    #[serde(default)]
    pub spawn_rotation_rad: f32,
    #[serde(default = "default_planet_map_icon_asset_id")]
    pub map_icon_asset_id: String,
    #[serde(default = "default_planet_visual_shader_asset_id")]
    pub planet_visual_shader_asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct PlanetDefinition {
    pub planet_id: String,
    pub display_name: String,
    #[serde(default)]
    pub entity_labels: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub spawn: Option<PlanetSpawnDefinition>,
    pub shader_settings: PlanetBodyShaderSettings,
}

#[derive(Resource, Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
#[reflect(Resource)]
pub struct PlanetRegistry {
    pub schema_version: u32,
    pub entries: Vec<PlanetRegistryEntry>,
    pub definitions: Vec<PlanetDefinition>,
}

fn default_planet_owner_id() -> String {
    "world:system".to_string()
}

fn default_planet_size_m() -> f32 {
    640.0
}

fn default_planet_map_icon_asset_id() -> String {
    "map_icon_planet_svg".to_string()
}

fn default_planet_visual_shader_asset_id() -> String {
    "planet_visual_wgsl".to_string()
}

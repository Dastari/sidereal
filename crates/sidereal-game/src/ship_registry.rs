use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShipRegistryEntry {
    pub ship_id: String,
    pub bundle_id: String,
    pub script: String,
    #[serde(default)]
    pub spawn_enabled: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShipVisualDefinition {
    pub visual_asset_id: String,
    #[serde(default = "default_ship_map_icon_asset_id")]
    pub map_icon_asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipDimensionsDefinition {
    pub length_m: f32,
    #[serde(default)]
    pub width_m: Option<f32>,
    #[serde(default = "default_ship_height_m")]
    pub height_m: f32,
    #[serde(default = "default_ship_collision_mode")]
    pub collision_mode: String,
    #[serde(default = "default_true")]
    pub collision_from_texture: bool,
}

impl Default for ShipDimensionsDefinition {
    fn default() -> Self {
        Self {
            length_m: 1.0,
            width_m: None,
            height_m: default_ship_height_m(),
            collision_mode: default_ship_collision_mode(),
            collision_from_texture: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipRootDefinition {
    #[serde(default = "default_ship_base_mass_kg")]
    pub base_mass_kg: f32,
    #[serde(default)]
    pub total_mass_kg: Option<f32>,
    #[serde(default)]
    pub cargo_mass_kg: Option<f32>,
    #[serde(default)]
    pub module_mass_kg: Option<f32>,
    #[serde(default)]
    pub angular_inertia: Option<f32>,
    #[serde(default = "default_ship_max_velocity_mps")]
    pub max_velocity_mps: f32,
    pub health_pool: JsonValue,
    pub destructible: JsonValue,
    pub flight_computer: JsonValue,
    pub flight_tuning: JsonValue,
    pub visibility_range_buff_m: JsonValue,
    #[serde(default)]
    pub scanner_component: Option<JsonValue>,
    #[serde(default)]
    pub avian_linear_damping: Option<f32>,
    #[serde(default)]
    pub avian_angular_damping: Option<f32>,
}

impl Default for ShipRootDefinition {
    fn default() -> Self {
        Self {
            base_mass_kg: default_ship_base_mass_kg(),
            total_mass_kg: None,
            cargo_mass_kg: None,
            module_mass_kg: None,
            angular_inertia: None,
            max_velocity_mps: default_ship_max_velocity_mps(),
            health_pool: JsonValue::Null,
            destructible: JsonValue::Null,
            flight_computer: JsonValue::Null,
            flight_tuning: JsonValue::Null,
            visibility_range_buff_m: JsonValue::Null,
            scanner_component: None,
            avian_linear_damping: None,
            avian_angular_damping: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShipHardpointDefinition {
    pub hardpoint_id: String,
    pub display_name: String,
    pub slot_kind: String,
    pub offset_m: [f32; 3],
    #[serde(default)]
    pub local_rotation_rad: f32,
    #[serde(default)]
    pub mirror_group: Option<String>,
    #[serde(default)]
    pub compatible_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShipMountedModuleDefinition {
    pub hardpoint_id: String,
    pub module_id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub component_overrides: HashMap<String, JsonValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShipDefinition {
    pub ship_id: String,
    pub bundle_id: String,
    pub display_name: String,
    #[serde(default)]
    pub entity_labels: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub visual: ShipVisualDefinition,
    pub dimensions: ShipDimensionsDefinition,
    pub root: ShipRootDefinition,
    #[serde(default)]
    pub hardpoints: Vec<ShipHardpointDefinition>,
    #[serde(default)]
    pub mounted_modules: Vec<ShipMountedModuleDefinition>,
}

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShipRegistry {
    pub schema_version: u32,
    pub entries: Vec<ShipRegistryEntry>,
    pub definitions: Vec<ShipDefinition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShipModuleRegistryEntry {
    pub module_id: String,
    pub script: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShipModuleComponentDefinition {
    pub kind: String,
    #[serde(default)]
    pub properties: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShipModuleDefinition {
    pub module_id: String,
    pub display_name: String,
    pub category: String,
    #[serde(default)]
    pub entity_labels: Vec<String>,
    #[serde(default)]
    pub compatible_slot_kinds: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub components: Vec<ShipModuleComponentDefinition>,
}

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShipModuleRegistry {
    pub schema_version: u32,
    pub entries: Vec<ShipModuleRegistryEntry>,
    pub definitions: Vec<ShipModuleDefinition>,
}

fn default_ship_map_icon_asset_id() -> String {
    "map_icon_ship_svg".to_string()
}

fn default_ship_height_m() -> f32 {
    8.0
}

fn default_ship_collision_mode() -> String {
    "Aabb".to_string()
}

fn default_ship_base_mass_kg() -> f32 {
    15000.0
}

fn default_ship_max_velocity_mps() -> f32 {
    100.0
}

fn default_true() -> bool {
    true
}

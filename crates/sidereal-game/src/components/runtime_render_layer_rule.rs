use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

fn default_enabled() -> bool {
    true
}

#[sidereal_component_macros::sidereal_component(
    kind = "runtime_render_layer_rule",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RuntimeRenderLayerRule {
    pub rule_id: String,
    pub target_layer_id: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub labels_any: Vec<String>,
    #[serde(default)]
    pub labels_all: Vec<String>,
    #[serde(default)]
    pub archetypes_any: Vec<String>,
    #[serde(default)]
    pub components_all: Vec<String>,
    #[serde(default)]
    pub components_any: Vec<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[sidereal_component_macros::sidereal_component(
    kind = "static_landmark",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct StaticLandmark {
    pub kind: String,
    #[serde(default)]
    pub discoverable: bool,
    #[serde(default)]
    pub always_known: bool,
    #[serde(default)]
    pub discovery_radius_m: Option<f32>,
    #[serde(default = "default_use_extent_for_discovery")]
    pub use_extent_for_discovery: bool,
}

const fn default_use_extent_for_discovery() -> bool {
    true
}

impl Default for StaticLandmark {
    fn default() -> Self {
        Self {
            kind: "Landmark".to_string(),
            discoverable: true,
            always_known: false,
            discovery_radius_m: None,
            use_extent_for_discovery: true,
        }
    }
}

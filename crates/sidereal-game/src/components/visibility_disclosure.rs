use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[reflect(Serialize, Deserialize)]
pub struct VisibilityScannerSource {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub range_m: f32,
}

#[sidereal_component_macros::sidereal_component(
    kind = "visibility_disclosure",
    persist = true,
    replicate = true,
    predict = true,
    visibility = [OwnerOnly]
)]
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct VisibilityDisclosure {
    pub scanner_sources: Vec<VisibilityScannerSource>,
}

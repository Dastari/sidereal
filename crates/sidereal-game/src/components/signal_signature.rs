use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(
    kind = "signal_signature",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct SignalSignature {
    pub strength: f32,
    pub detection_radius_m: f32,
    pub use_extent_for_detection: bool,
}

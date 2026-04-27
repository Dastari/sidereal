use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[derive(
    Debug, Clone, Copy, Default, Reflect, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord,
)]
#[reflect(Serialize, Deserialize)]
pub enum ScannerContactDetailTier {
    #[default]
    Basic,
    Iff,
    Classified,
    Telemetry,
}

#[sidereal_component_macros::sidereal_component(kind = "scanner_component", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct ScannerComponent {
    pub base_range_m: f32,
    pub level: u8,
    pub detail_tier: ScannerContactDetailTier,
    pub supports_density: bool,
    pub supports_directional_awareness: bool,
    pub max_contacts: u16,
}

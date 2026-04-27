use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use super::asteroid_field::AsteroidSizeTier;

#[sidereal_component_macros::sidereal_component(
    kind = "asteroid_field_member",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AsteroidFieldMember {
    pub field_entity_id: String,
    pub cluster_key: String,
    pub member_key: String,
    pub parent_member_key: Option<String>,
    pub size_tier: AsteroidSizeTier,
    pub fracture_depth: u8,
    pub resource_profile_id: String,
    pub fracture_profile_id: String,
}

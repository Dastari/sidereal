use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use super::asteroid_field::AsteroidSizeTier;

#[derive(Debug, Clone, Copy, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
#[reflect(Serialize, Deserialize)]
pub enum AsteroidMemberStateKind {
    #[default]
    Intact,
    Activated,
    Fractured,
    Depleted,
    Harvested,
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Serialize, Deserialize)]
pub struct AsteroidMemberStateEntry {
    pub member_key: String,
    pub parent_member_key: Option<String>,
    pub state: AsteroidMemberStateKind,
    pub size_tier: AsteroidSizeTier,
    pub fracture_depth: u8,
    pub remaining_health: Option<f32>,
    pub remaining_mass_kg: Option<f32>,
    pub spawned_children: Vec<String>,
    pub resource_units_consumed: f32,
    pub last_update_tick: Option<u64>,
}

#[sidereal_component_macros::sidereal_component(
    kind = "asteroid_field_damage_state",
    persist = true,
    replicate = false
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AsteroidFieldDamageState {
    pub entries: Vec<AsteroidMemberStateEntry>,
}

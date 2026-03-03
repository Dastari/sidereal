use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::{DamageType, EntityGuid};

#[sidereal_component_macros::sidereal_component(kind = "ballistic_weapon", persist = true, replicate = true, visibility = [Public])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct BallisticWeapon {
    pub weapon_name: String,
    pub rpm: f32,
    pub damage_per_shot: f32,
    pub max_range_m: f32,
    pub spread_rad: f32,
    pub damage_type: DamageType,
}

impl BallisticWeapon {
    pub fn corvette_ballistic_gatling() -> Self {
        Self {
            weapon_name: "Ballistic Gatling".to_string(),
            rpm: 720.0,
            damage_per_shot: 12.0,
            max_range_m: 150.0,
            spread_rad: 0.0,
            damage_type: DamageType::Ballistic,
        }
    }

    pub fn cooldown_seconds(&self) -> f32 {
        let rpm = self.rpm.max(1.0);
        60.0 / rpm
    }
}

#[derive(Debug, Clone, Component, Reflect, PartialEq)]
#[reflect(Component)]
pub struct WeaponCooldownState {
    pub remaining_s: f32,
}

impl Default for WeaponCooldownState {
    fn default() -> Self {
        Self { remaining_s: 0.0 }
    }
}

use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::DamageType;

#[sidereal_component_macros::sidereal_component(kind = "ballistic_projectile", persist = false, replicate = true, visibility = [Public])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct BallisticProjectile {
    pub shooter_guid: Uuid,
    pub weapon_guid: Uuid,
    pub damage_per_hit: f32,
    pub damage_type: DamageType,
    pub remaining_lifetime_s: f32,
    pub collision_radius_m: f32,
}

impl BallisticProjectile {
    pub fn new(
        shooter_guid: Uuid,
        weapon_guid: Uuid,
        damage_per_hit: f32,
        damage_type: DamageType,
        remaining_lifetime_s: f32,
        collision_radius_m: f32,
    ) -> Self {
        Self {
            shooter_guid,
            weapon_guid,
            damage_per_hit,
            damage_type,
            remaining_lifetime_s,
            collision_radius_m,
        }
    }
}

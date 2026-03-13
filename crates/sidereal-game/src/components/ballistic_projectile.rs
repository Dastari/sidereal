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

    pub fn prespawn_hash_for_tick(&self, spawn_tick: u16) -> u64 {
        Self::compute_prespawn_hash(self.shooter_guid, self.weapon_guid, spawn_tick)
    }

    fn compute_prespawn_hash(shooter_guid: Uuid, weapon_guid: Uuid, spawn_tick: u16) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

        let mut hash = FNV_OFFSET_BASIS;
        for byte in shooter_guid.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        for byte in weapon_guid.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        for byte in spawn_tick.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::BallisticProjectile;
    use crate::DamageType;

    #[test]
    fn prespawn_hash_is_deterministic_and_disambiguates_weapons_and_ticks() {
        let shooter_guid = uuid::Uuid::new_v4();
        let weapon_a = uuid::Uuid::new_v4();
        let weapon_b = uuid::Uuid::new_v4();

        let projectile_a = BallisticProjectile::new(
            shooter_guid,
            weapon_a,
            10.0,
            DamageType::Ballistic,
            0.25,
            0.35,
        );
        let projectile_a_repeat = BallisticProjectile::new(
            shooter_guid,
            weapon_a,
            10.0,
            DamageType::Ballistic,
            0.25,
            0.35,
        );
        let projectile_b = BallisticProjectile::new(
            shooter_guid,
            weapon_b,
            10.0,
            DamageType::Ballistic,
            0.25,
            0.35,
        );

        assert_eq!(
            projectile_a.prespawn_hash_for_tick(42),
            projectile_a_repeat.prespawn_hash_for_tick(42)
        );
        assert_ne!(
            projectile_a.prespawn_hash_for_tick(42),
            projectile_b.prespawn_hash_for_tick(42)
        );
        assert_ne!(
            projectile_a.prespawn_hash_for_tick(42),
            projectile_a.prespawn_hash_for_tick(43)
        );
    }
}

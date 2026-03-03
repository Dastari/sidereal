use avian2d::prelude::{Position, Rotation, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    ActionQueue, AmmoCount, BallisticWeapon, DamageType, EntityAction, EntityGuid,
    FlightControlAuthority, Hardpoint, HealthPool, MountedOn, ParentGuid, WeaponCooldownState,
};

#[derive(Debug, Clone)]
pub struct ShotFiredEvent {
    pub shooter_guid: Uuid,
    pub weapon_entity: Entity,
    pub weapon_guid: Uuid,
    pub origin: Vec2,
    pub direction: Vec2,
    pub damage_type: DamageType,
}

#[derive(Debug, Clone)]
pub struct ShotHitEvent {
    pub shooter_guid: Uuid,
    pub target_entity: Entity,
    pub target_guid: Option<Uuid>,
    pub weapon_entity: Entity,
    pub damage: f32,
    pub damage_type: DamageType,
}

pub fn bootstrap_weapon_cooldown_state(
    mut commands: Commands<'_, '_>,
    needs_cooldown: Query<'_, '_, Entity, (With<BallisticWeapon>, Without<WeaponCooldownState>)>,
) {
    for entity in &needs_cooldown {
        commands
            .entity(entity)
            .insert(WeaponCooldownState::default());
    }
}

pub fn bootstrap_legacy_ballistic_weapon_ranges(
    mut weapons: Query<'_, '_, &'_ mut BallisticWeapon>,
) {
    for mut weapon in &mut weapons {
        if weapon.weapon_name == "Ballistic Gatling"
            && (weapon.max_range_m <= 1.0 || weapon.max_range_m > 300.0)
        {
            weapon.max_range_m = 150.0;
        }
    }
}

pub fn tick_weapon_cooldowns(
    time: Res<'_, Time<Fixed>>,
    mut cooldowns: Query<'_, '_, &'_ mut WeaponCooldownState>,
) {
    let dt_s = time.delta_secs();
    if dt_s <= 0.0 {
        return;
    }
    for mut cooldown in &mut cooldowns {
        cooldown.remaining_s = (cooldown.remaining_s - dt_s).max(0.0);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn process_weapon_fire_actions(
    mut shooter_entities: Query<
        '_,
        '_,
        (Entity, &EntityGuid, &Position, &Rotation, &mut ActionQueue),
        With<FlightControlAuthority>,
    >,
    hardpoints: Query<'_, '_, (&'_ ParentGuid, &'_ Hardpoint)>,
    mut weapons: Query<
        '_,
        '_,
        (
            &'_ MountedOn,
            &'_ BallisticWeapon,
            &'_ mut WeaponCooldownState,
            Option<&'_ mut AmmoCount>,
        ),
    >,
    mut health_pools: Query<'_, '_, &'_ mut HealthPool>,
    spatial_query: SpatialQuery<'_, '_>,
) {
    let mut hardpoint_by_mount = HashMap::<(Uuid, String), (Vec2, Quat)>::new();
    for (parent_guid, hardpoint) in &hardpoints {
        hardpoint_by_mount.insert(
            (parent_guid.0, hardpoint.hardpoint_id.clone()),
            (hardpoint.offset_m.truncate(), hardpoint.local_rotation),
        );
    }

    for (ship_entity, ship_guid, ship_position, ship_rotation, mut queue) in &mut shooter_entities {
        let mut wants_fire_primary = false;
        let pending = std::mem::take(&mut queue.pending);
        for action in pending {
            if action == EntityAction::FirePrimary {
                wants_fire_primary = true;
            } else {
                queue.pending.push(action);
            }
        }
        if !wants_fire_primary {
            continue;
        }

        let ship_quat: Quat = (*ship_rotation).into();
        for (mounted_on, weapon, mut cooldown, ammo_opt) in &mut weapons {
            if mounted_on.parent_entity_id != ship_guid.0 {
                continue;
            }
            if cooldown.remaining_s > 0.0 {
                continue;
            }

            let Some((hardpoint_offset, hardpoint_rotation)) = hardpoint_by_mount
                .get(&(mounted_on.parent_entity_id, mounted_on.hardpoint_id.clone()))
            else {
                continue;
            };

            let mut ammo_opt = ammo_opt;
            if let Some(ammo) = ammo_opt.as_ref()
                && !ammo.can_consume(1)
            {
                continue;
            }

            let muzzle_quat = ship_quat * *hardpoint_rotation;
            let local_forward = (muzzle_quat * Vec3::Y).truncate();
            if local_forward.length_squared() <= f32::EPSILON {
                continue;
            }
            let direction = local_forward.normalize();
            let origin = ship_position.0 + rotate_vec2(ship_quat, *hardpoint_offset);
            let Ok(ray_direction) = Dir2::new(direction) else {
                continue;
            };
            let filter = SpatialQueryFilter::from_excluded_entities([ship_entity]);

            if let Some(ammo) = ammo_opt.as_deref_mut() {
                let _ = ammo.consume(1);
            }

            if let Some(hit) = spatial_query.cast_ray(
                origin,
                ray_direction,
                weapon.max_range_m.max(1.0),
                true,
                &filter,
            ) && let Ok(mut health_pool) = health_pools.get_mut(hit.entity)
            {
                health_pool.current = (health_pool.current - weapon.damage_per_shot).max(0.0);
            }

            cooldown.remaining_s = weapon.cooldown_seconds();
        }
    }
}

fn rotate_vec2(rotation: Quat, input: Vec2) -> Vec2 {
    (rotation * input.extend(0.0)).truncate()
}

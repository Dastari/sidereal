use avian2d::prelude::{Position, Rotation, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    ActionQueue, AmmoCount, BallisticWeapon, DamageType, EntityAction, EntityGuid,
    FlightControlAuthority, Hardpoint, HealthPool, MountedOn, ParentGuid, SimulationMotionWriter,
    WeaponCooldownState,
};

#[derive(Debug, Clone, Message)]
pub struct ShotFiredEvent {
    pub shooter_guid: Uuid,
    pub weapon_entity: Entity,
    pub weapon_guid: Uuid,
    pub origin: Vec2,
    pub direction: Vec2,
    pub max_range_m: f32,
    pub damage_per_shot: f32,
    pub damage_type: DamageType,
}

#[derive(Debug, Clone, Message)]
pub struct ShotImpactResolvedEvent {
    pub shooter_guid: Uuid,
    pub weapon_entity: Entity,
    pub weapon_guid: Uuid,
    pub origin: Vec2,
    pub impact_pos: Vec2,
    pub max_range_m: f32,
    pub damage_per_shot: f32,
    pub damage_type: DamageType,
    pub target_entity: Option<Entity>,
    pub target_guid: Option<Uuid>,
}

#[derive(Debug, Clone, Message)]
pub struct ShotHitEvent {
    pub shooter_guid: Uuid,
    pub target_entity: Entity,
    pub target_guid: Option<Uuid>,
    pub weapon_entity: Entity,
    pub weapon_guid: Uuid,
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
        (With<FlightControlAuthority>, With<SimulationMotionWriter>),
    >,
    hardpoints: Query<'_, '_, (&'_ ParentGuid, &'_ Hardpoint)>,
    mut weapons: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            &'_ MountedOn,
            &'_ BallisticWeapon,
            &'_ mut WeaponCooldownState,
            Option<&'_ mut AmmoCount>,
        ),
    >,
    mut shot_fired_events: MessageWriter<'_, ShotFiredEvent>,
) {
    let mut hardpoint_by_mount = HashMap::<(Uuid, String), (Vec2, Quat)>::new();
    for (parent_guid, hardpoint) in &hardpoints {
        hardpoint_by_mount.insert(
            (parent_guid.0, hardpoint.hardpoint_id.clone()),
            (hardpoint.offset_m.truncate(), hardpoint.local_rotation),
        );
    }

    for (_shooter_entity, shooter_guid, shooter_position, shooter_rotation, mut queue) in
        &mut shooter_entities
    {
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

        let shooter_quat: Quat = (*shooter_rotation).into();
        for (weapon_entity, weapon_guid, mounted_on, weapon, mut cooldown, ammo_opt) in &mut weapons
        {
            if mounted_on.parent_entity_id != shooter_guid.0 {
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

            let muzzle_quat = shooter_quat * *hardpoint_rotation;
            let local_forward = (muzzle_quat * Vec3::Y).truncate();
            if local_forward.length_squared() <= f32::EPSILON {
                continue;
            }
            let direction = local_forward.normalize();
            let origin = shooter_position.0 + rotate_vec2(shooter_quat, *hardpoint_offset);
            if let Some(ammo) = ammo_opt.as_deref_mut() {
                let _ = ammo.consume(1);
            }

            shot_fired_events.write(ShotFiredEvent {
                shooter_guid: shooter_guid.0,
                weapon_entity,
                weapon_guid: weapon_guid.0,
                origin,
                direction,
                max_range_m: weapon.max_range_m.max(1.0),
                damage_per_shot: weapon.damage_per_shot,
                damage_type: weapon.damage_type,
            });

            cooldown.remaining_s = weapon.cooldown_seconds();
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn resolve_shot_impacts(
    mut fired_events: MessageReader<'_, '_, ShotFiredEvent>,
    guid_entities: Query<'_, '_, (Entity, &'_ EntityGuid)>,
    spatial_query: SpatialQuery<'_, '_>,
    mut resolved_events: MessageWriter<'_, ShotImpactResolvedEvent>,
) {
    let mut entity_by_guid = HashMap::<Uuid, Entity>::new();
    for (entity, guid) in &guid_entities {
        entity_by_guid.insert(guid.0, entity);
    }

    for fired in fired_events.read() {
        if fired.direction.length_squared() <= f32::EPSILON {
            continue;
        }
        let direction = fired.direction.normalize();
        let Ok(ray_direction) = Dir2::new(direction) else {
            continue;
        };
        let filter = entity_by_guid
            .get(&fired.shooter_guid)
            .copied()
            .map_or_else(SpatialQueryFilter::default, |excluded| {
                SpatialQueryFilter::from_excluded_entities([excluded])
            });
        let hit = spatial_query.cast_ray(
            fired.origin,
            ray_direction,
            fired.max_range_m.max(1.0),
            true,
            &filter,
        );
        let impact_pos = hit
            .map(|hit| fired.origin + ray_direction.as_vec2() * hit.distance)
            .unwrap_or_else(|| fired.origin + ray_direction.as_vec2() * fired.max_range_m.max(1.0));
        let target_entity = hit.map(|hit| hit.entity);
        let target_guid =
            target_entity.and_then(|entity| guid_entities.get(entity).ok().map(|g| g.1.0));
        resolved_events.write(ShotImpactResolvedEvent {
            shooter_guid: fired.shooter_guid,
            weapon_entity: fired.weapon_entity,
            weapon_guid: fired.weapon_guid,
            origin: fired.origin,
            impact_pos,
            max_range_m: fired.max_range_m.max(1.0),
            damage_per_shot: fired.damage_per_shot.max(0.0),
            damage_type: fired.damage_type,
            target_entity,
            target_guid,
        });
    }
}

#[allow(clippy::type_complexity)]
pub fn apply_damage_from_shot_impacts(
    mut resolved_events: MessageReader<'_, '_, ShotImpactResolvedEvent>,
    mut health_pools: Query<'_, '_, &'_ mut HealthPool>,
    mut shot_hit_events: MessageWriter<'_, ShotHitEvent>,
) {
    for resolved in resolved_events.read() {
        let Some(target_entity) = resolved.target_entity else {
            continue;
        };
        let Ok(mut health_pool) = health_pools.get_mut(target_entity) else {
            continue;
        };
        health_pool.current = (health_pool.current - resolved.damage_per_shot).max(0.0);
        shot_hit_events.write(ShotHitEvent {
            shooter_guid: resolved.shooter_guid,
            target_entity,
            target_guid: resolved.target_guid,
            weapon_entity: resolved.weapon_entity,
            weapon_guid: resolved.weapon_guid,
            damage: resolved.damage_per_shot,
            damage_type: resolved.damage_type,
        });
    }
}

fn rotate_vec2(rotation: Quat, input: Vec2) -> Vec2 {
    (rotation * input.extend(0.0)).truncate()
}

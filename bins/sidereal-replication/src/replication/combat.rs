use avian2d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::prelude::*;
use lightyear::prelude::server::RawServer;
use lightyear::prelude::{NetworkTarget, Server, ServerMultiMessageSender};
use sidereal_game::{AmmoCount, BallisticWeapon, EntityGuid, Hardpoint, MountedOn, ParentGuid};
use sidereal_net::{InputChannel, ServerWeaponFiredMessage};
use std::collections::HashMap;
use uuid::Uuid;

const TRACER_VISUAL_SPEED_MPS: f32 = 1800.0;
const TRACER_VISUAL_MIN_TTL_S: f32 = 0.01;

#[derive(Resource, Default)]
pub struct WeaponAmmoSnapshot {
    pub ammo_by_weapon_entity: HashMap<Entity, u32>,
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(WeaponAmmoSnapshot::default());
}

#[allow(clippy::type_complexity)]
pub fn broadcast_weapon_fired_messages(
    server_query: Query<'_, '_, &'_ Server, With<RawServer>>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    mut ammo_snapshot: ResMut<'_, WeaponAmmoSnapshot>,
    hardpoints: Query<'_, '_, (&'_ ParentGuid, &'_ Hardpoint)>,
    ships: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            &'_ Position,
            &'_ Rotation,
            Option<&'_ LinearVelocity>,
            Option<&'_ AngularVelocity>,
        ),
    >,
    weapons: Query<
        '_,
        '_,
        (
            Entity,
            &'_ MountedOn,
            &'_ BallisticWeapon,
            Option<&'_ AmmoCount>,
        ),
    >,
) {
    let Ok(server) = server_query.single() else {
        return;
    };
    let mut hardpoint_by_mount = HashMap::<(Uuid, String), (Vec2, Quat)>::new();
    for (parent_guid, hardpoint) in &hardpoints {
        hardpoint_by_mount.insert(
            (parent_guid.0, hardpoint.hardpoint_id.clone()),
            (hardpoint.offset_m.truncate(), hardpoint.local_rotation),
        );
    }

    let mut ship_state_by_guid = HashMap::<Uuid, (String, Vec2, Quat, Vec2, f32)>::new();
    for (ship_guid, ship_pos, ship_rot, linear, angular) in &ships {
        let ship_quat: Quat = (*ship_rot).into();
        ship_state_by_guid.insert(
            ship_guid.0,
            (
                ship_guid.0.to_string(),
                ship_pos.0,
                ship_quat,
                linear.map(|v| v.0).unwrap_or(Vec2::ZERO),
                angular.map(|v| v.0).unwrap_or(0.0),
            ),
        );
    }

    for (weapon_entity, mounted_on, weapon, ammo) in &weapons {
        let current_ammo = ammo.map(|v| v.current).unwrap_or(u32::MAX);
        let previous_ammo = ammo_snapshot
            .ammo_by_weapon_entity
            .insert(weapon_entity, current_ammo)
            .unwrap_or(current_ammo);
        let consumed_this_tick = previous_ammo.saturating_sub(current_ammo);
        if consumed_this_tick == 0 {
            continue;
        }

        let Some((shooter_entity_id, ship_pos, ship_quat, ship_linear, ship_omega)) =
            ship_state_by_guid
                .get(&mounted_on.parent_entity_id)
                .cloned()
        else {
            continue;
        };
        let Some((hardpoint_offset, hardpoint_rotation)) = hardpoint_by_mount
            .get(&(mounted_on.parent_entity_id, mounted_on.hardpoint_id.clone()))
            .cloned()
        else {
            continue;
        };
        let muzzle_quat = ship_quat * hardpoint_rotation;
        let direction = (muzzle_quat * Vec3::Y).truncate();
        if direction.length_squared() <= f32::EPSILON {
            continue;
        }
        let direction = direction.normalize();
        let muzzle_offset_world = rotate_vec2(ship_quat, hardpoint_offset);
        let tangential_velocity = Vec2::new(
            -ship_omega * muzzle_offset_world.y,
            ship_omega * muzzle_offset_world.x,
        );
        let initial_velocity =
            direction * TRACER_VISUAL_SPEED_MPS + ship_linear + tangential_velocity;
        let origin = ship_pos + muzzle_offset_world;
        let visual_ttl_s =
            (weapon.max_range_m.max(1.0) / TRACER_VISUAL_SPEED_MPS).max(TRACER_VISUAL_MIN_TTL_S);

        let message = ServerWeaponFiredMessage {
            shooter_entity_id,
            origin_xy: [origin.x, origin.y],
            velocity_xy: [initial_velocity.x, initial_velocity.y],
            ttl_s: visual_ttl_s,
        };
        let target = NetworkTarget::All;
        for _ in 0..consumed_this_tick {
            let _ =
                sender.send::<ServerWeaponFiredMessage, InputChannel>(&message, server, &target);
        }
    }
}

fn rotate_vec2(rotation: Quat, input: Vec2) -> Vec2 {
    (rotation * input.extend(0.0)).truncate()
}

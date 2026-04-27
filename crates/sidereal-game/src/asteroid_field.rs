use avian2d::prelude::{
    AngularDamping, AngularVelocity, LinearDamping, LinearVelocity, Position, RigidBody, Rotation,
};
use bevy::math::DVec2;
use bevy::prelude::*;
use uuid::Uuid;

use crate::{
    AsteroidField, AsteroidFieldDamageState, AsteroidFieldMember, AsteroidFractureProfile,
    AsteroidMemberStateEntry, AsteroidMemberStateKind, AsteroidSizeTier, CollisionAabbM,
    CollisionProfile, Destructible, DisplayName, EntityDestroyedEvent,
    EntityDestructionStartedEvent, EntityGuid, EntityLabels, HealthPool, MassKg, OwnerId,
    PendingDestruction, ProceduralSprite, SizeM, SpriteShaderAssetId, VisualAssetId,
    compute_collision_half_extents_from_procedural_sprite,
    generate_rdp_collision_outline_from_procedural_sprite,
};

#[derive(Debug, Clone, PartialEq)]
pub struct AsteroidChildPlan {
    pub child_member_key: String,
    pub size_tier: AsteroidSizeTier,
    pub fracture_depth: u8,
    pub mass_kg: f32,
    pub diameter_m: f32,
    pub health_points: f32,
    pub offset_xy_m: [f32; 2],
    pub impulse_xy_mps: [f32; 2],
    pub procedural_sprite: ProceduralSprite,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsteroidFractureParent<'a> {
    pub member_key: &'a str,
    pub size_tier: AsteroidSizeTier,
    pub fracture_depth: u8,
    pub mass_kg: f32,
    pub diameter_m: f32,
    pub health_points: f32,
    pub procedural_sprite: &'a ProceduralSprite,
}

pub fn asteroid_member_key(field_entity_id: &str, cluster_key: &str, member_index: u32) -> String {
    format!("{field_entity_id}:{cluster_key}:{member_index:04}")
}

pub fn asteroid_child_member_key(parent_member_key: &str, child_index: u32) -> String {
    format!("{parent_member_key}/c{child_index:02}")
}

pub fn asteroid_member_uuid(member_key: &str) -> Uuid {
    let a = hash_u64(member_key, 0x11);
    let b = hash_u64(member_key, 0x29);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_be_bytes());
    bytes[8..].copy_from_slice(&b.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

#[must_use]
pub fn next_smaller_tier(tier: AsteroidSizeTier) -> Option<AsteroidSizeTier> {
    match tier {
        AsteroidSizeTier::Massive => Some(AsteroidSizeTier::Large),
        AsteroidSizeTier::Large => Some(AsteroidSizeTier::Medium),
        AsteroidSizeTier::Medium => Some(AsteroidSizeTier::Small),
        AsteroidSizeTier::Small => None,
    }
}

pub fn fracture_child_count(
    profile: &AsteroidFractureProfile,
    parent_tier: AsteroidSizeTier,
    parent_member_key: &str,
) -> u8 {
    let (min, max) = match parent_tier {
        AsteroidSizeTier::Massive => (
            profile.break_massive_into_large_min,
            profile.break_massive_into_large_max,
        ),
        AsteroidSizeTier::Large => (
            profile.break_large_into_medium_min,
            profile.break_large_into_medium_max,
        ),
        AsteroidSizeTier::Medium => (
            profile.break_medium_into_small_min,
            profile.break_medium_into_small_max,
        ),
        AsteroidSizeTier::Small => return 0,
    };
    if max <= min {
        return min;
    }
    min + (hash_u64(parent_member_key, 17) % u64::from(max - min + 1)) as u8
}

pub fn fracture_child_sprite(
    parent: &ProceduralSprite,
    child_key: &str,
    child_index: u32,
) -> ProceduralSprite {
    let mut child = parent.clone();
    child.family_seed_key = Some(child_key.to_string());
    child.crater_count = parent.crater_count.saturating_sub(1).max(1);
    child.edge_noise = (parent.edge_noise * (1.08 + hash01(child_key, 3) * 0.18)).min(0.12);
    child.lobe_amplitude = (parent.lobe_amplitude * (0.86 + hash01(child_key, 5) * 0.18)).max(0.04);
    child.pixel_step_px = parent.pixel_step_px.max(1);
    child.crack_intensity =
        (parent.crack_intensity * 0.7 + 0.1 + hash01(child_key, 7) * 0.12).min(1.0);
    child.mineral_vein_intensity =
        (parent.mineral_vein_intensity * (0.75 + hash01(child_key, 11) * 0.35)).min(1.0);
    if child_index % 2 == 1 {
        child.palette_dark_rgb = mix_rgb(parent.palette_dark_rgb, parent.palette_light_rgb, 0.08);
    }
    child
}

pub fn build_fracture_child_plans(
    profile: &AsteroidFractureProfile,
    parent: &AsteroidFractureParent<'_>,
) -> Vec<AsteroidChildPlan> {
    let Some(child_tier) = next_smaller_tier(parent.size_tier) else {
        return Vec::new();
    };
    let child_count = fracture_child_count(profile, parent.size_tier, parent.member_key);
    if child_count == 0 {
        return Vec::new();
    }
    let total_child_mass = parent.mass_kg.max(0.0) * profile.mass_retention_ratio.clamp(0.0, 1.0);
    let child_depth = parent.fracture_depth.saturating_add(1);
    (0..child_count)
        .map(|index| {
            let child_key = asteroid_child_member_key(parent.member_key, u32::from(index));
            let share_jitter = 0.82 + hash01(&child_key, 13) * 0.36;
            let mass_kg = (total_child_mass / f32::from(child_count)) * share_jitter;
            let diameter_scale = match child_tier {
                AsteroidSizeTier::Large => 0.58,
                AsteroidSizeTier::Medium => 0.48,
                AsteroidSizeTier::Small => 0.36,
                AsteroidSizeTier::Massive => 0.72,
            };
            let diameter_m =
                (parent.diameter_m * diameter_scale * (0.86 + hash01(&child_key, 19) * 0.24))
                    .max(2.0);
            let angle = hash01(&child_key, 23) * std::f32::consts::TAU;
            let offset_distance = parent.diameter_m * (0.12 + hash01(&child_key, 29) * 0.18);
            let impulse = profile.child_impulse_min_mps
                + (profile.child_impulse_max_mps - profile.child_impulse_min_mps).max(0.0)
                    * hash01(&child_key, 31);
            AsteroidChildPlan {
                child_member_key: child_key.clone(),
                size_tier: child_tier,
                fracture_depth: child_depth,
                mass_kg,
                diameter_m,
                health_points: (parent.health_points * 0.35 / f32::from(child_count)).max(10.0),
                offset_xy_m: [angle.cos() * offset_distance, angle.sin() * offset_distance],
                impulse_xy_mps: [angle.cos() * impulse, angle.sin() * impulse],
                procedural_sprite: fracture_child_sprite(
                    parent.procedural_sprite,
                    &child_key,
                    u32::from(index),
                ),
            }
        })
        .collect()
}

fn hash_u64(key: &str, salt: u64) -> u64 {
    let mut hash = 0xcbf29ce484222325u64 ^ salt;
    for byte in key.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn hash01(key: &str, salt: u64) -> f32 {
    let value = hash_u64(key, salt);
    ((value >> 11) as f64 / ((1u64 << 53) as f64)) as f32
}

fn mix_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[allow(clippy::type_complexity)]
pub fn fracture_depleted_asteroid_members(
    mut commands: Commands<'_, '_>,
    mut fields: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ AsteroidField>,
            &'_ mut AsteroidFieldDamageState,
            Option<&'_ AsteroidFractureProfile>,
        ),
    >,
    members: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            &'_ AsteroidFieldMember,
            &'_ HealthPool,
            &'_ MassKg,
            &'_ SizeM,
            &'_ ProceduralSprite,
            &'_ Position,
            Option<&'_ LinearVelocity>,
            Option<&'_ Rotation>,
            Option<&'_ OwnerId>,
            Option<&'_ VisualAssetId>,
            Option<&'_ SpriteShaderAssetId>,
            Option<&'_ Destructible>,
        ),
        Without<PendingDestruction>,
    >,
    mut started_events: MessageWriter<'_, EntityDestructionStartedEvent>,
    mut destroyed_events: MessageWriter<'_, EntityDestroyedEvent>,
) {
    for (
        entity,
        entity_guid,
        member,
        health,
        mass,
        size,
        sprite,
        position,
        linear_velocity,
        rotation,
        owner,
        visual_asset,
        sprite_shader,
        destructible,
    ) in &members
    {
        if health.current > 0.0 {
            continue;
        }
        let Some((_, field, mut damage_state, Some(fracture_profile))) = fields
            .iter_mut()
            .find(|(field_guid, _, _, _)| field_guid.0.to_string() == member.field_entity_id)
        else {
            continue;
        };
        let max_depth = field.map(|value| value.max_fracture_depth).unwrap_or(2);
        if member.fracture_depth >= max_depth {
            upsert_member_damage_state(
                &mut damage_state,
                member,
                AsteroidMemberStateKind::Depleted,
                health,
                Some(mass.0),
                Vec::new(),
            );
            continue;
        }
        let parent = AsteroidFractureParent {
            member_key: member.member_key.as_str(),
            size_tier: member.size_tier,
            fracture_depth: member.fracture_depth,
            mass_kg: mass.0,
            diameter_m: size.length.max(size.width),
            health_points: health.maximum,
            procedural_sprite: sprite,
        };
        let child_plans = build_fracture_child_plans(fracture_profile, &parent);
        if child_plans.is_empty() {
            upsert_member_damage_state(
                &mut damage_state,
                member,
                AsteroidMemberStateKind::Depleted,
                health,
                Some(mass.0),
                Vec::new(),
            );
            continue;
        }

        let spawned_children = child_plans
            .iter()
            .map(|child| child.child_member_key.clone())
            .collect::<Vec<_>>();
        upsert_member_damage_state(
            &mut damage_state,
            member,
            AsteroidMemberStateKind::Fractured,
            health,
            Some(mass.0),
            spawned_children,
        );

        let destruction_profile_id = destructible
            .map(|value| value.destruction_profile_id.clone())
            .unwrap_or_else(|| "destruction.asteroid.default".to_string());
        started_events.write(EntityDestructionStartedEvent {
            entity,
            entity_guid: entity_guid.0,
            destruction_profile_id: destruction_profile_id.clone(),
            effect_origin: position.0,
            destroy_delay_s: 0.0,
        });
        destroyed_events.write(EntityDestroyedEvent {
            entity,
            entity_guid: entity_guid.0,
            destruction_profile_id,
            effect_origin: position.0,
        });

        for child in child_plans {
            spawn_fracture_child(
                &mut commands,
                member,
                &child,
                position,
                linear_velocity,
                rotation,
                owner,
                visual_asset,
                sprite_shader,
                destructible,
            );
        }
        commands.entity(entity).despawn();
    }
}

fn upsert_member_damage_state(
    damage_state: &mut AsteroidFieldDamageState,
    member: &AsteroidFieldMember,
    state: AsteroidMemberStateKind,
    health: &HealthPool,
    remaining_mass_kg: Option<f32>,
    spawned_children: Vec<String>,
) {
    let entry = AsteroidMemberStateEntry {
        member_key: member.member_key.clone(),
        parent_member_key: member.parent_member_key.clone(),
        state,
        size_tier: member.size_tier,
        fracture_depth: member.fracture_depth,
        remaining_health: Some(health.current),
        remaining_mass_kg,
        spawned_children,
        resource_units_consumed: 0.0,
        last_update_tick: None,
    };
    if let Some(existing) = damage_state
        .entries
        .iter_mut()
        .find(|existing| existing.member_key == member.member_key)
    {
        *existing = entry;
    } else {
        damage_state.entries.push(entry);
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_fracture_child(
    commands: &mut Commands<'_, '_>,
    parent_member: &AsteroidFieldMember,
    child: &AsteroidChildPlan,
    parent_position: &Position,
    parent_velocity: Option<&LinearVelocity>,
    parent_rotation: Option<&Rotation>,
    owner: Option<&OwnerId>,
    visual_asset: Option<&VisualAssetId>,
    sprite_shader: Option<&SpriteShaderAssetId>,
    destructible: Option<&Destructible>,
) {
    let child_guid = asteroid_member_uuid(&child.child_member_key);
    let child_position = Position(
        parent_position.0
            + DVec2::new(
                f64::from(child.offset_xy_m[0]),
                f64::from(child.offset_xy_m[1]),
            ),
    );
    let base_velocity = parent_velocity
        .map(|velocity| velocity.0)
        .unwrap_or(DVec2::ZERO);
    let child_velocity = LinearVelocity(
        base_velocity
            + DVec2::new(
                f64::from(child.impulse_xy_mps[0]),
                f64::from(child.impulse_xy_mps[1]),
            ),
    );
    let (half_x, half_y) = compute_collision_half_extents_from_procedural_sprite(
        &child.child_member_key,
        &child.procedural_sprite,
        child.diameter_m,
    )
    .unwrap_or((child.diameter_m * 0.5, child.diameter_m * 0.5));
    let half_z = half_x.max(half_y).mul_add(0.7, 0.0).max(0.5);
    let mut entity_commands = commands.spawn((
        EntityGuid(child_guid),
        DisplayName("Asteroid Fragment".to_string()),
        EntityLabels(vec![
            "Asteroid".to_string(),
            "FieldMember".to_string(),
            "FieldFragment".to_string(),
        ]),
        HealthPool {
            current: child.health_points,
            maximum: child.health_points,
        },
        destructible.cloned().unwrap_or_else(|| Destructible {
            destruction_profile_id: "destruction.asteroid.default".to_string(),
            destroy_delay_s: 0.18,
        }),
        MassKg(child.mass_kg),
        SizeM {
            length: child.diameter_m,
            width: child.diameter_m,
            height: child.diameter_m * 0.8,
        },
        CollisionProfile::solid_aabb(),
        CollisionAabbM {
            half_extents: Vec3::new(half_x, half_y, half_z),
        },
        visual_asset
            .cloned()
            .unwrap_or_else(|| VisualAssetId("asteroid_texture_red_png".to_string())),
        sprite_shader
            .cloned()
            .unwrap_or_else(|| SpriteShaderAssetId(Some("asteroid_wgsl".to_string()))),
        child.procedural_sprite.clone(),
        AsteroidFieldMember {
            field_entity_id: parent_member.field_entity_id.clone(),
            cluster_key: parent_member.cluster_key.clone(),
            member_key: child.child_member_key.clone(),
            parent_member_key: Some(parent_member.member_key.clone()),
            size_tier: child.size_tier,
            fracture_depth: child.fracture_depth,
            resource_profile_id: parent_member.resource_profile_id.clone(),
            fracture_profile_id: parent_member.fracture_profile_id.clone(),
        },
    ));
    entity_commands.insert((
        child_position,
        parent_rotation
            .copied()
            .unwrap_or_else(|| Rotation::from(Quat::IDENTITY)),
        child_velocity,
        AngularVelocity(0.0),
        RigidBody::Dynamic,
        LinearDamping(0.0),
        AngularDamping(0.0),
    ));
    if let Some(owner) = owner {
        entity_commands.insert(owner.clone());
    }
    if let Ok(outline) = generate_rdp_collision_outline_from_procedural_sprite(
        &child.child_member_key,
        &child.procedural_sprite,
        half_x,
        half_y,
    ) {
        entity_commands.insert(outline);
    }
}

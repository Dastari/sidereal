use avian2d::prelude::{LinearVelocity, Position};
use bevy::ecs::system::RunSystemOnce;
use bevy::math::DVec2;
use bevy::prelude::*;
use sidereal_game::{
    AsteroidField, AsteroidFieldCluster, AsteroidFieldDamageState, AsteroidFieldLayout,
    AsteroidFieldMember, AsteroidFieldPopulation, AsteroidFieldShape, AsteroidFractureParent,
    AsteroidFractureProfile, AsteroidMemberStateEntry, AsteroidMemberStateKind,
    AsteroidResourceProfile, AsteroidSizeRangeM, AsteroidSizeTier, AsteroidYieldEntry,
    Destructible, EntityGuid, HealthPool, MassKg, ProceduralSprite, ProceduralSpriteSurfaceStyle,
    SizeM, asteroid_child_member_key, asteroid_member_key, build_fracture_child_plans,
    fracture_child_count, fracture_depleted_asteroid_members, next_smaller_tier,
};
use uuid::Uuid;

fn fracture_profile() -> AsteroidFractureProfile {
    AsteroidFractureProfile {
        break_massive_into_large_min: 2,
        break_massive_into_large_max: 3,
        break_large_into_medium_min: 2,
        break_large_into_medium_max: 5,
        break_medium_into_small_min: 2,
        break_medium_into_small_max: 6,
        child_impulse_min_mps: 0.4,
        child_impulse_max_mps: 2.0,
        mass_retention_ratio: 0.82,
        terminal_debris_loss_ratio: 0.65,
    }
}

fn sprite() -> ProceduralSprite {
    ProceduralSprite {
        generator_id: "asteroid_rocky_v1".to_string(),
        resolution_px: 128,
        edge_noise: 0.03,
        lobe_amplitude: 0.12,
        crater_count: 6,
        palette_dark_rgb: [0.18, 0.16, 0.14],
        palette_light_rgb: [0.54, 0.48, 0.42],
        surface_style: ProceduralSpriteSurfaceStyle::Rocky,
        pixel_step_px: 2,
        crack_intensity: 0.35,
        mineral_vein_intensity: 0.18,
        mineral_accent_rgb: [0.72, 0.52, 0.24],
        family_seed_key: None,
    }
}

#[test]
fn asteroid_member_keys_are_deterministic() {
    assert_eq!(
        asteroid_member_key("field-a", "core", 7),
        "field-a:core:0007"
    );
    assert_eq!(
        asteroid_child_member_key("field-a:core:0007", 3),
        "field-a:core:0007/c03"
    );
}

#[test]
fn asteroid_tiers_step_down_to_terminal_small() {
    assert_eq!(
        next_smaller_tier(AsteroidSizeTier::Massive),
        Some(AsteroidSizeTier::Large)
    );
    assert_eq!(
        next_smaller_tier(AsteroidSizeTier::Large),
        Some(AsteroidSizeTier::Medium)
    );
    assert_eq!(
        next_smaller_tier(AsteroidSizeTier::Medium),
        Some(AsteroidSizeTier::Small)
    );
    assert_eq!(next_smaller_tier(AsteroidSizeTier::Small), None);
}

#[test]
fn fracture_child_generation_is_deterministic_and_mass_bounded() {
    let profile = fracture_profile();
    let parent_key = "field-a:core:0007";
    let sprite = sprite();
    let parent = AsteroidFractureParent {
        member_key: parent_key,
        size_tier: AsteroidSizeTier::Large,
        fracture_depth: 0,
        mass_kg: 10_000.0,
        diameter_m: 80.0,
        health_points: 1_000.0,
        procedural_sprite: &sprite,
    };
    let first = build_fracture_child_plans(&profile, &parent);
    let second = build_fracture_child_plans(&profile, &parent);
    assert_eq!(first, second);
    assert_eq!(
        first.len(),
        fracture_child_count(&profile, AsteroidSizeTier::Large, parent_key) as usize
    );
    assert!(
        first
            .iter()
            .all(|child| child.size_tier == AsteroidSizeTier::Medium)
    );
    assert!(first.iter().all(|child| child.fracture_depth == 1));
    let total_mass: f32 = first.iter().map(|child| child.mass_kg).sum();
    assert!(total_mass <= 10_000.0 * profile.mass_retention_ratio * 1.36);
}

#[test]
fn asteroid_field_payloads_roundtrip_json() {
    let layout = AsteroidFieldLayout {
        shape: AsteroidFieldShape::ClusterPatch,
        density: 0.7,
        clusters: vec![AsteroidFieldCluster {
            cluster_key: "core".to_string(),
            offset_xy_m: [0.0, 0.0],
            radius_m: 1_800.0,
            density_weight: 1.0,
            preferred_size_tier: AsteroidSizeTier::Medium,
            rarity_weight: 0.2,
        }],
        spawn_noise_amplitude_m: 120.0,
        spawn_noise_frequency: 0.25,
    };
    let json = serde_json::to_string(&layout).expect("serialize layout");
    let decoded: AsteroidFieldLayout = serde_json::from_str(&json).expect("deserialize layout");
    assert_eq!(decoded, layout);

    let field = AsteroidField {
        field_profile_id: "starter.belt".to_string(),
        content_version: 2,
        layout_seed: 4242,
        activation_radius_m: 3_200.0,
        field_radius_m: 2_600.0,
        max_active_members: 160,
        max_active_fragments: 96,
        max_fracture_depth: 2,
        ambient_profile_id: Some("starter.dust".to_string()),
    };
    let field_json = serde_json::to_string(&field).expect("serialize field");
    let field_decoded: AsteroidField =
        serde_json::from_str(&field_json).expect("deserialize field");
    assert_eq!(field_decoded, field);

    let population = AsteroidFieldPopulation {
        target_large_count: 8,
        target_medium_count: 64,
        target_small_count: 48,
        large_size_range_m: AsteroidSizeRangeM {
            min_m: 40.0,
            max_m: 120.0,
        },
        medium_size_range_m: AsteroidSizeRangeM {
            min_m: 12.0,
            max_m: 40.0,
        },
        small_size_range_m: AsteroidSizeRangeM {
            min_m: 3.0,
            max_m: 12.0,
        },
        sprite_profile_id: "asteroid.sprite.rocky".to_string(),
        resource_profile_id: "asteroid.resource.common_ore".to_string(),
        fracture_profile_id: "asteroid.fracture.default".to_string(),
    };
    let population_json = serde_json::to_string(&population).expect("serialize population");
    let population_decoded: AsteroidFieldPopulation =
        serde_json::from_str(&population_json).expect("deserialize population");
    assert_eq!(population_decoded, population);

    let damage = AsteroidFieldDamageState {
        entries: vec![AsteroidMemberStateEntry {
            member_key: "field-a:core:0001".to_string(),
            parent_member_key: None,
            state: AsteroidMemberStateKind::Fractured,
            size_tier: AsteroidSizeTier::Large,
            fracture_depth: 0,
            remaining_health: Some(0.0),
            remaining_mass_kg: Some(8_000.0),
            spawned_children: vec!["field-a:core:0001/c00".to_string()],
            resource_units_consumed: 3.0,
            last_update_tick: Some(44),
        }],
    };
    let damage_json = serde_json::to_string(&damage).expect("serialize damage");
    let damage_decoded: AsteroidFieldDamageState =
        serde_json::from_str(&damage_json).expect("deserialize damage");
    assert_eq!(damage_decoded, damage);

    let resources = AsteroidResourceProfile {
        profile_id: "asteroid.resource.common_ore".to_string(),
        extraction_profile_id: Some("extraction.mining_laser.basic".to_string()),
        yield_table: vec![AsteroidYieldEntry {
            item_id: "resource.iron_ore".to_string(),
            weight: 1.0,
            min_units: 4.0,
            max_units: 18.0,
        }],
        depletion_pool_units: 100.0,
    };
    let resources_json = serde_json::to_string(&resources).expect("serialize resources");
    let resources_decoded: AsteroidResourceProfile =
        serde_json::from_str(&resources_json).expect("deserialize resources");
    assert_eq!(resources_decoded, resources);
}

#[test]
fn zero_health_field_member_fractures_into_linked_children() {
    let mut app = App::new();
    app.add_message::<sidereal_game::EntityDestructionStartedEvent>();
    app.add_message::<sidereal_game::EntityDestroyedEvent>();

    let field_guid = Uuid::new_v4();
    let field_id = field_guid.to_string();
    app.world_mut().spawn((
        EntityGuid(field_guid),
        AsteroidField {
            field_profile_id: "asteroid.field.test".to_string(),
            content_version: 2,
            layout_seed: 7,
            activation_radius_m: 1_000.0,
            field_radius_m: 800.0,
            max_active_members: 32,
            max_active_fragments: 32,
            max_fracture_depth: 2,
            ambient_profile_id: None,
        },
        AsteroidFieldDamageState::default(),
        fracture_profile(),
    ));

    let parent_member_key = "field:test:0001".to_string();
    let parent = app
        .world_mut()
        .spawn((
            EntityGuid(Uuid::new_v4()),
            AsteroidFieldMember {
                field_entity_id: field_id,
                cluster_key: "test".to_string(),
                member_key: parent_member_key.clone(),
                parent_member_key: None,
                size_tier: AsteroidSizeTier::Large,
                fracture_depth: 0,
                resource_profile_id: "asteroid.resource.common_ore".to_string(),
                fracture_profile_id: "asteroid.fracture.default".to_string(),
            },
            HealthPool {
                current: 0.0,
                maximum: 500.0,
            },
            Destructible {
                destruction_profile_id: "destruction.asteroid.default".to_string(),
                destroy_delay_s: 0.18,
            },
            MassKg(9_000.0),
            SizeM {
                length: 80.0,
                width: 72.0,
                height: 48.0,
            },
            sprite(),
            Position(DVec2::new(10.0, -4.0)),
            LinearVelocity(DVec2::new(1.0, 2.0)),
        ))
        .id();

    let _ = app
        .world_mut()
        .run_system_once(fracture_depleted_asteroid_members);

    assert!(app.world().get_entity(parent).is_err());

    let mut member_query = app
        .world_mut()
        .query::<(&AsteroidFieldMember, &HealthPool, &MassKg)>();
    let children = member_query
        .iter(app.world())
        .filter(|(member, _, _)| {
            member.parent_member_key.as_deref() == Some(parent_member_key.as_str())
        })
        .map(|(member, health, mass)| (member.size_tier, health.current, mass.0))
        .collect::<Vec<_>>();
    let child_count = children.len();
    assert!(child_count > 0);
    assert!(
        children
            .iter()
            .all(
                |(size_tier, health_current, mass_kg)| *size_tier == AsteroidSizeTier::Medium
                    && *health_current > 0.0
                    && *mass_kg > 0.0
            )
    );

    let mut damage_query = app.world_mut().query::<&AsteroidFieldDamageState>();
    let damage_state = damage_query.single(app.world()).expect("damage state");
    let parent_entry = damage_state
        .entries
        .iter()
        .find(|entry| entry.member_key == parent_member_key)
        .expect("parent damage entry");
    assert_eq!(parent_entry.state, AsteroidMemberStateKind::Fractured);
    assert_eq!(parent_entry.spawned_children.len(), child_count);
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        PROJECTILE_VISUAL_Z, StreamedVisualMaterialKind, WEAPON_IMPACT_SPARK_TTL_S,
        activate_destruction_effect, attach_ballistic_projectile_visuals_system,
        attach_thruster_plume_visuals_system,
        bootstrap_local_ballistic_projectile_visual_roots_system,
        ensure_planet_body_root_visibility_system, ensure_visual_parent_spatial_components,
        planet_camera_relative_translation, planet_projected_cull_buffer_m,
        planet_visual_child_translation, receive_remote_weapon_tracer_messages_system,
        runtime_layer_screen_scale_factor, streamed_visual_needs_rebuild,
        suppress_duplicate_predicted_interpolated_visuals_system,
        sync_unadopted_ballistic_projectile_visual_roots_system,
        update_weapon_impact_sparks_system,
    };
    use crate::runtime::backdrop::RuntimeEffectMaterial;
    use crate::runtime::combat_messages::RemoteWeaponFiredRuntimeMessage;
    use crate::runtime::components::{
        BallisticProjectileVisualAttached, CanonicalPresentationEntity, ControlledEntity,
        PendingInitialVisualReady, RuntimeWorldVisualPass, RuntimeWorldVisualPassKind,
        StreamedVisualAttachmentKind, SuppressedPredictedDuplicateVisual, WeaponImpactExplosion,
        WeaponImpactExplosionPool, WeaponImpactSpark, WeaponTracerBolt, WeaponTracerPool,
        WorldEntity,
    };
    use crate::runtime::lighting::{CameraLocalLightSet, WorldLightingState};
    use crate::runtime::resources::{DuplicateVisualResolutionState, RuntimeSharedQuadMesh};
    use crate::runtime::transforms::reveal_world_entities_when_initial_transform_ready;
    use avian2d::prelude::{Position, Rotation};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use bevy::sprite_render::MeshMaterial2d;
    use lightyear::prelude::{Confirmed, Interpolated};
    use sidereal_game::{
        BallisticProjectile, DamageType, EntityGuid, EntityLabels, Hardpoint, MountedOn,
        ParentGuid, PlanetBodyShaderSettings,
    };
    use sidereal_net::ServerWeaponFiredMessage;
    use sidereal_runtime_sync::RuntimeEntityHierarchy;

    #[test]
    fn streamed_visual_rebuilds_when_material_kind_changes() {
        assert!(streamed_visual_needs_rebuild(
            Some(StreamedVisualAttachmentKind::Plain),
            StreamedVisualMaterialKind::AsteroidShader,
        ));
        assert!(streamed_visual_needs_rebuild(
            Some(StreamedVisualAttachmentKind::GenericShader),
            StreamedVisualMaterialKind::Plain,
        ));
        assert!(!streamed_visual_needs_rebuild(
            Some(StreamedVisualAttachmentKind::AsteroidShader),
            StreamedVisualMaterialKind::AsteroidShader,
        ));
    }

    #[test]
    fn thruster_plume_attaches_to_childless_engine_entity() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<RuntimeEffectMaterial>>();
        app.init_resource::<RuntimeSharedQuadMesh>();
        app.insert_resource(WorldLightingState::default());
        app.insert_resource(CameraLocalLightSet::default());
        app.add_systems(Update, attach_thruster_plume_visuals_system);

        let engine = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityLabels(vec!["Module".to_string(), "Engine".to_string()]),
            ))
            .id();

        assert!(
            app.world().entity(engine).get::<Children>().is_none(),
            "fixture should exercise first-child plume attachment"
        );

        app.update();

        let children = app
            .world()
            .entity(engine)
            .get::<Children>()
            .expect("plume attachment should create the first child");
        assert_eq!(children.len(), 1);
        let child = children[0];
        let pass = app
            .world()
            .entity(child)
            .get::<RuntimeWorldVisualPass>()
            .expect("plume child should carry a runtime visual pass tag");
        assert_eq!(pass.kind, RuntimeWorldVisualPassKind::ThrusterPlume);
    }

    #[test]
    fn planet_root_visibility_waits_for_initial_visual_ready() {
        let mut app = App::new();
        app.add_systems(Update, ensure_planet_body_root_visibility_system);

        let entity = app
            .world_mut()
            .spawn((
                WorldEntity,
                PlanetBodyShaderSettings::default(),
                Visibility::Visible,
                PendingInitialVisualReady,
            ))
            .id();

        app.update();

        let entity_ref = app.world().entity(entity);
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Hidden
        );
        assert!(
            entity_ref.contains::<PendingInitialVisualReady>(),
            "planet root should stay pending until visuals are actually ready"
        );
    }

    #[test]
    fn visual_parent_spatial_components_are_backfilled() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();

        let mut commands = app.world_mut().commands();
        let mut entity_commands = commands.entity(entity);
        ensure_visual_parent_spatial_components(&mut entity_commands);

        app.update();

        let entity_ref = app.world().entity(entity);
        assert!(entity_ref.contains::<Transform>());
        assert!(entity_ref.contains::<GlobalTransform>());
        assert!(entity_ref.contains::<Visibility>());
    }

    #[test]
    fn planet_camera_relative_translation_tracks_projected_position() {
        let offset = planet_camera_relative_translation(
            None,
            Vec2::new(100.0, 50.0),
            Vec2::new(300.0, 90.0),
        );
        assert_eq!(offset, Vec2::new(-200.0, -40.0));

        let layer = crate::runtime::components::ResolvedRuntimeRenderLayer {
            layer_id: "midground_planets".to_string(),
            definition: sidereal_game::RuntimeRenderLayerDefinition {
                layer_id: "midground_planets".to_string(),
                phase: "world".to_string(),
                material_domain: "world_polygon".to_string(),
                shader_asset_id: "planet_visual_wgsl".to_string(),
                parallax_factor: Some(0.25),
                ..Default::default()
            },
        };
        let offset = planet_camera_relative_translation(
            Some(&layer),
            Vec2::new(100.0, 50.0),
            Vec2::new(300.0, 90.0),
        );
        assert_eq!(offset, Vec2::new(-50.0, -10.0));
    }

    #[test]
    fn planet_visual_child_translation_compensates_for_off_origin_parent() {
        let parent_world_position = Vec2::new(8000.0, 0.0);
        let projected_center_world = Vec2::ZERO;
        let child_translation =
            planet_visual_child_translation(projected_center_world, parent_world_position, 0.0);

        assert_eq!(child_translation, Vec2::new(-8000.0, 0.0));
        assert_eq!(
            parent_world_position + child_translation,
            projected_center_world
        );
    }

    #[test]
    fn planet_projected_cull_buffer_expands_during_rapid_zoom_out() {
        let viewport_size = Vec2::new(1920.0, 1080.0);
        let static_buffer = planet_projected_cull_buffer_m(viewport_size, 1.0, 128.0, false);
        let zoom_out_buffer = planet_projected_cull_buffer_m(viewport_size, 1.0, 128.0, true);

        assert_eq!(static_buffer, 480.0);
        assert_eq!(zoom_out_buffer, 960.0);
        assert!(zoom_out_buffer > static_buffer);
    }

    #[test]
    fn planet_projected_cull_buffer_respects_large_body_margin() {
        let viewport_size = Vec2::new(320.0, 180.0);
        let buffer = planet_projected_cull_buffer_m(viewport_size, 1.0, 500.0, false);

        assert_eq!(buffer, 500.0);
    }

    #[test]
    fn runtime_layer_screen_scale_defaults_and_clamps() {
        let default_layer = sidereal_game::RuntimeRenderLayerDefinition::default();
        assert_eq!(runtime_layer_screen_scale_factor(&default_layer), 1.0);

        let mut authored = sidereal_game::RuntimeRenderLayerDefinition {
            screen_scale_factor: Some(1.5),
            ..Default::default()
        };
        assert_eq!(runtime_layer_screen_scale_factor(&authored), 1.5);

        authored.screen_scale_factor = Some(1000.0);
        assert_eq!(runtime_layer_screen_scale_factor(&authored), 64.0);
    }

    #[test]
    fn weapon_impact_spark_expiry_hides_instead_of_despawning() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<RuntimeEffectMaterial>();
        app.insert_resource(Time::<()>::default());
        app.add_systems(Update, update_weapon_impact_sparks_system);

        let material = {
            let mut materials = app
                .world_mut()
                .resource_mut::<Assets<RuntimeEffectMaterial>>();
            materials.add(RuntimeEffectMaterial::default())
        };

        let entity = app
            .world_mut()
            .spawn((
                WeaponImpactSpark {
                    ttl_s: 0.0,
                    max_ttl_s: WEAPON_IMPACT_SPARK_TTL_S,
                },
                Transform::default(),
                MeshMaterial2d(material),
                Visibility::Visible,
            ))
            .id();

        app.update();

        let entity_ref = app.world().entity(entity);
        assert!(entity_ref.contains::<WeaponImpactSpark>());
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Hidden
        );
    }

    #[test]
    fn destruction_effect_uses_existing_explosion_pool() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<RuntimeEffectMaterial>();
        app.insert_resource(WeaponImpactExplosionPool {
            explosions: Vec::new(),
            next_index: 0,
        });

        let material = {
            let mut materials = app
                .world_mut()
                .resource_mut::<Assets<RuntimeEffectMaterial>>();
            materials.add(RuntimeEffectMaterial::default())
        };

        let explosion = app
            .world_mut()
            .spawn((
                WeaponImpactExplosion {
                    ttl_s: 0.0,
                    max_ttl_s: 0.18,
                    base_scale: 1.2,
                    growth_scale: 4.4,
                    intensity_scale: 1.0,
                    domain_scale: 1.12,
                    screen_distortion_scale: 0.0,
                },
                Transform::default(),
                MeshMaterial2d(material),
                Visibility::Hidden,
            ))
            .id();
        app.world_mut()
            .resource_mut::<WeaponImpactExplosionPool>()
            .explosions
            .push(explosion);

        let _ = app.world_mut().run_system_once(
            |mut pool: ResMut<'_, WeaponImpactExplosionPool>,
             mut materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
             mut query: Query<
                '_,
                '_,
                (
                    &'_ mut WeaponImpactExplosion,
                    &'_ mut Transform,
                    &'_ MeshMaterial2d<RuntimeEffectMaterial>,
                    &'_ mut Visibility,
                ),
                super::WeaponImpactExplosionQueryFilter,
            >| {
                activate_destruction_effect(
                    "explosion_burst",
                    Vec2::new(12.0, -4.0),
                    &mut pool,
                    &mut query,
                    &mut materials,
                );
            },
        );

        let entity_ref = app.world().entity(explosion);
        let effect = entity_ref
            .get::<WeaponImpactExplosion>()
            .expect("explosion effect");
        let transform = entity_ref.get::<Transform>().expect("transform");
        assert_eq!(transform.translation.x, 12.0);
        assert_eq!(transform.translation.y, -4.0);
        assert!(
            effect.screen_distortion_scale > 0.0,
            "destruction effects should opt into screen-space distortion"
        );
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Visible
        );
    }

    #[test]
    fn local_ballistic_projectiles_get_immediate_visual_root_and_preserve_pose_on_attach() {
        let mut app = App::new();

        let entity = app
            .world_mut()
            .spawn((
                Position(Vec2::new(18.0, -7.5).into()),
                Rotation::from(Quat::from_rotation_z(0.35)),
                BallisticProjectile::new(
                    uuid::Uuid::new_v4(),
                    uuid::Uuid::new_v4(),
                    10.0,
                    DamageType::Ballistic,
                    0.25,
                    0.35,
                ),
            ))
            .id();

        let _ = app
            .world_mut()
            .run_system_once(bootstrap_local_ballistic_projectile_visual_roots_system);
        let _ = app
            .world_mut()
            .run_system_once(attach_ballistic_projectile_visuals_system);

        let entity_ref = app.world().entity(entity);
        let transform = entity_ref.get::<Transform>().expect("transform");
        assert_eq!(transform.translation.truncate(), Vec2::new(18.0, -7.5));
        assert!((transform.rotation.to_euler(EulerRot::XYZ).2 - 0.35).abs() < 0.001);
        assert!((transform.translation.z - PROJECTILE_VISUAL_Z).abs() < f32::EPSILON);
        assert!(
            entity_ref.contains::<BallisticProjectileVisualAttached>(),
            "local prespawned projectile should become renderable before replication adoption"
        );
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Visible
        );
    }

    #[test]
    fn unadopted_ballistic_projectile_transform_sync_preserves_visual_depth() {
        let mut app = App::new();

        let entity = app
            .world_mut()
            .spawn((
                Position(Vec2::new(4.0, 9.0).into()),
                Rotation::from(Quat::from_rotation_z(-0.6)),
                BallisticProjectile::new(
                    uuid::Uuid::new_v4(),
                    uuid::Uuid::new_v4(),
                    10.0,
                    DamageType::Ballistic,
                    0.25,
                    0.35,
                ),
                Transform::from_xyz(-100.0, 55.0, PROJECTILE_VISUAL_Z),
            ))
            .id();

        let _ = app
            .world_mut()
            .run_system_once(sync_unadopted_ballistic_projectile_visual_roots_system);

        let transform = app
            .world()
            .entity(entity)
            .get::<Transform>()
            .expect("transform");
        assert_eq!(transform.translation.truncate(), Vec2::new(4.0, 9.0));
        assert!((transform.rotation.to_euler(EulerRot::XYZ).2 + 0.6).abs() < 0.001);
        assert!(
            (transform.translation.z - PROJECTILE_VISUAL_Z).abs() < f32::EPSILON,
            "projectile root sync should not flatten the visual layer depth"
        );
    }

    #[test]
    fn observer_ballistic_projectile_uses_authoritative_spawn_pose_before_first_history_sample() {
        let mut app = App::new();
        app.init_resource::<RuntimeEntityHierarchy>();
        let projectile_guid = uuid::Uuid::new_v4();

        let entity = app
            .world_mut()
            .spawn((
                Interpolated,
                WorldEntity,
                PendingInitialVisualReady,
                EntityGuid(projectile_guid),
                Visibility::Hidden,
                Transform::default(),
                Position(Vec2::new(64.0, -22.0).into()),
                Rotation::from(Quat::from_rotation_z(1.1)),
                Confirmed(Position(Vec2::new(64.0, -22.0).into())),
                Confirmed(Rotation::from(Quat::from_rotation_z(1.1))),
                BallisticProjectile::new(
                    uuid::Uuid::new_v4(),
                    uuid::Uuid::new_v4(),
                    10.0,
                    DamageType::Ballistic,
                    0.25,
                    0.35,
                ),
            ))
            .id();

        let _ = app
            .world_mut()
            .run_system_once(reveal_world_entities_when_initial_transform_ready);
        let _ = app
            .world_mut()
            .run_system_once(attach_ballistic_projectile_visuals_system);

        let entity_ref = app.world().entity(entity);
        let transform = entity_ref.get::<Transform>().expect("transform");
        assert_eq!(transform.translation.truncate(), Vec2::new(64.0, -22.0));
        assert_ne!(
            transform.translation.truncate(),
            Vec2::ZERO,
            "observer projectile should not render at the origin before interpolation history exists"
        );
        assert!((transform.rotation.to_euler(EulerRot::XYZ).2 - 1.1).abs() < 0.001);
        assert!(
            entity_ref.contains::<BallisticProjectileVisualAttached>(),
            "observer projectile should attach the projectile tracer visual"
        );
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Visible
        );
        assert!(
            !entity_ref.contains::<PendingInitialVisualReady>(),
            "observer projectile should leave the pending-visual gate once the authoritative pose is available"
        );
    }

    #[test]
    fn local_weapon_tracer_message_uses_predicted_muzzle_origin() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<RuntimeEffectMaterial>();
        app.add_message::<RemoteWeaponFiredRuntimeMessage>();

        let shooter_guid = uuid::Uuid::new_v4();
        let weapon_guid = uuid::Uuid::new_v4();
        let controlled = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(shooter_guid),
                ControlledEntity {
                    entity_id: shooter_guid.to_string(),
                    player_entity_id: uuid::Uuid::new_v4().to_string(),
                },
                Position(Vec2::new(100.0, 50.0).into()),
                Rotation::from(Quat::IDENTITY),
            ))
            .id();

        app.world_mut().spawn((
            ParentGuid(shooter_guid),
            Hardpoint {
                hardpoint_id: "weapon_fore_center".to_string(),
                offset_m: Vec3::new(0.0, 4.0, 0.0),
                local_rotation: Quat::IDENTITY,
            },
        ));
        app.world_mut().spawn((
            WorldEntity,
            EntityGuid(weapon_guid),
            MountedOn {
                parent_entity_id: shooter_guid,
                hardpoint_id: "weapon_fore_center".to_string(),
            },
        ));

        let material = {
            let mut materials = app
                .world_mut()
                .resource_mut::<Assets<RuntimeEffectMaterial>>();
            materials.add(RuntimeEffectMaterial::default())
        };
        let bolt = app
            .world_mut()
            .spawn((
                WeaponTracerBolt {
                    excluded_entity: None,
                    velocity: Vec2::ZERO,
                    impact_xy: None,
                    ttl_s: 0.0,
                    lateral_normal: Vec2::ZERO,
                    wiggle_phase_rad: 0.0,
                    wiggle_freq_hz: 0.0,
                    wiggle_amp_mps: 0.0,
                },
                Transform::default(),
                MeshMaterial2d(material),
                Visibility::Hidden,
            ))
            .id();
        app.insert_resource(WeaponTracerPool {
            bolts: vec![bolt],
            next_index: 0,
        });
        app.world_mut()
            .write_message(RemoteWeaponFiredRuntimeMessage {
                message: ServerWeaponFiredMessage {
                    shooter_entity_id: shooter_guid.to_string(),
                    weapon_guid: weapon_guid.to_string(),
                    audio_profile_id: None,
                    cooldown_s: None,
                    origin_xy: [0.0, 0.0],
                    velocity_xy: [0.0, 650.0],
                    impact_xy: Some([100.0, 140.0]),
                    ttl_s: 0.05,
                },
            });

        let _ = app
            .world_mut()
            .run_system_once(receive_remote_weapon_tracer_messages_system);

        let entity_ref = app.world().entity(bolt);
        let transform = entity_ref.get::<Transform>().expect("transform");
        let tracer = entity_ref.get::<WeaponTracerBolt>().expect("tracer");
        assert_eq!(transform.translation.truncate(), Vec2::new(100.0, 54.0));
        assert_eq!(tracer.excluded_entity, Some(controlled));
        assert_eq!(tracer.velocity, Vec2::new(0.0, 650.0));
        assert_eq!(tracer.impact_xy, Some(Vec2::new(100.0, 140.0)));
        assert!(
            tracer.ttl_s > 0.05,
            "local predicted muzzle should recompute visual travel time to the authoritative impact"
        );
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Visible
        );
    }

    #[test]
    fn duplicate_visual_winner_swaps_without_full_world_scan_state_reset() {
        let mut app = App::new();
        app.init_resource::<DuplicateVisualResolutionState>();
        app.add_systems(
            Update,
            suppress_duplicate_predicted_interpolated_visuals_system,
        );

        let guid = uuid::Uuid::new_v4();
        let controlled = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(guid),
                ControlledEntity {
                    entity_id: "controlled".to_string(),
                    player_entity_id: "player".to_string(),
                },
                Visibility::Visible,
            ))
            .id();
        let fallback = app
            .world_mut()
            .spawn((WorldEntity, EntityGuid(guid), Visibility::Visible))
            .id();

        app.update();
        assert!(
            !app.world()
                .entity(controlled)
                .contains::<SuppressedPredictedDuplicateVisual>()
        );
        assert!(
            app.world()
                .entity(controlled)
                .contains::<CanonicalPresentationEntity>()
        );
        assert!(
            app.world()
                .entity(fallback)
                .contains::<SuppressedPredictedDuplicateVisual>()
        );
        assert!(
            !app.world()
                .entity(fallback)
                .contains::<CanonicalPresentationEntity>()
        );

        app.world_mut()
            .entity_mut(controlled)
            .remove::<ControlledEntity>();
        app.world_mut()
            .entity_mut(fallback)
            .insert(ControlledEntity {
                entity_id: "fallback".to_string(),
                player_entity_id: "player".to_string(),
            });

        app.update();

        assert!(
            app.world()
                .entity(controlled)
                .contains::<SuppressedPredictedDuplicateVisual>()
        );
        assert!(
            !app.world()
                .entity(controlled)
                .contains::<CanonicalPresentationEntity>()
        );
        assert!(
            !app.world()
                .entity(fallback)
                .contains::<SuppressedPredictedDuplicateVisual>()
        );
        assert!(
            app.world()
                .entity(fallback)
                .contains::<CanonicalPresentationEntity>()
        );
        assert_eq!(
            app.world()
                .resource::<DuplicateVisualResolutionState>()
                .winner_by_guid
                .get(&guid),
            Some(&fallback)
        );
    }

    #[test]
    fn duplicate_visual_prefers_interpolated_clone_with_authoritative_pose_before_history() {
        let mut app = App::new();
        app.init_resource::<DuplicateVisualResolutionState>();
        app.add_systems(
            Update,
            suppress_duplicate_predicted_interpolated_visuals_system,
        );

        let guid = uuid::Uuid::new_v4();
        let confirmed = app
            .world_mut()
            .spawn((WorldEntity, EntityGuid(guid), Visibility::Visible))
            .id();
        let interpolated = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(guid),
                Interpolated,
                Position(Vec2::new(25.0, -12.0).into()),
                Rotation::from(Quat::from_rotation_z(0.6)),
                Confirmed(Position(Vec2::new(25.0, -12.0).into())),
                Confirmed(Rotation::from(Quat::from_rotation_z(0.6))),
                Visibility::Visible,
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .resource::<DuplicateVisualResolutionState>()
                .winner_by_guid
                .get(&guid),
            Some(&interpolated)
        );
        assert!(
            app.world()
                .entity(confirmed)
                .contains::<SuppressedPredictedDuplicateVisual>()
        );
        assert!(
            !app.world()
                .entity(interpolated)
                .contains::<SuppressedPredictedDuplicateVisual>()
        );
        assert!(
            app.world()
                .entity(interpolated)
                .contains::<CanonicalPresentationEntity>()
        );
    }

    #[test]
    fn duplicate_visual_recomputes_when_interpolated_pose_changes_from_invalid_to_valid() {
        let mut app = App::new();
        app.init_resource::<DuplicateVisualResolutionState>();
        app.add_systems(
            Update,
            suppress_duplicate_predicted_interpolated_visuals_system,
        );

        let guid = uuid::Uuid::new_v4();
        let confirmed = app
            .world_mut()
            .spawn((WorldEntity, EntityGuid(guid), Visibility::Visible))
            .id();
        let interpolated = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(guid),
                Interpolated,
                Position(Vec2::splat(f32::NAN).into()),
                Rotation::from(Quat::from_rotation_z(0.0)),
                Visibility::Visible,
            ))
            .id();

        app.update();
        assert_eq!(
            app.world()
                .resource::<DuplicateVisualResolutionState>()
                .winner_by_guid
                .get(&guid),
            Some(&confirmed)
        );

        app.world_mut().entity_mut(interpolated).insert((
            Position(Vec2::new(48.0, -12.0).into()),
            Confirmed(Position(Vec2::new(48.0, -12.0).into())),
            Confirmed(Rotation::from(Quat::from_rotation_z(0.0))),
        ));

        app.update();

        assert_eq!(
            app.world()
                .resource::<DuplicateVisualResolutionState>()
                .winner_by_guid
                .get(&guid),
            Some(&interpolated)
        );
        assert!(
            app.world()
                .entity(interpolated)
                .contains::<CanonicalPresentationEntity>()
        );
    }
}

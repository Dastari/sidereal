#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(crate) fn adopt_native_lightyear_replicated_entities(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    tuning: Res<'_, PredictionBootstrapTuning>,
    time: Res<'_, Time>,
    mut adoption_state: ResMut<'_, DeferredPredictedAdoptionState>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    mut entity_registry: ResMut<'_, RuntimeEntityHierarchy>,
    mut remote_registry: ResMut<'_, RemoteEntityRegistry>,
    live_entities: Query<'_, '_, ()>,
    _collision_outlines: Query<'_, '_, &'_ CollisionOutlineM>,
    replicated_entities: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ PlayerTag>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ SizeM>,
            Option<&'_ VisualAssetId>,
            Option<&'_ SpriteShaderAssetId>,
            Has<lightyear::prelude::Replicated>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
        (
            With<EntityGuid>,
            Without<ReplicatedAdoptionHandled>,
            Without<WorldEntity>,
            Without<DespawnOnExit<ClientAppState>>,
        ),
    >,
    world_spatial_query: Query<'_, '_, (Option<&'_ WorldPosition>, Option<&'_ WorldRotation>)>,
    controlled_query: Query<'_, '_, Entity, With<ControlledEntity>>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    for (
        entity,
        guid,
        mounted_on,
        hardpoint,
        player_tag,
        position,
        rotation,
        linear_velocity,
        size_m,
        visual_asset_id,
        sprite_shader_asset_id,
        is_replicated,
        is_predicted,
        is_interpolated,
    ) in &replicated_entities
    {
        if !is_lightyear_replication_lane(is_replicated, is_predicted, is_interpolated) {
            continue;
        }
        let (world_position, world_rotation) =
            world_spatial_query.get(entity).unwrap_or((None, None));
        let Some(guid) = guid else {
            continue;
        };
        watchdog.replication_state_seen = true;
        let runtime_entity_id = guid.0.to_string();
        let is_root_entity = mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none();
        let is_canonical_runtime_entity =
            is_canonical_runtime_entity_lane(is_replicated, is_predicted, is_interpolated);
        let is_local_player_entity =
            ids_refer_to_same_guid(runtime_entity_id.as_str(), local_player_entity_id);
        let is_local_controlled_entity = (is_root_entity || is_local_player_entity)
            && player_view_state.controlled_entity_id.as_deref()
                == Some(runtime_entity_id.as_str());
        let is_spatial_root = is_root_entity && size_m.is_some();
        if should_defer_spatial_root_adoption(
            is_spatial_root,
            position.is_some(),
            rotation.is_some(),
            world_position.is_some(),
            world_rotation.is_some(),
        ) {
            // Avoid adopting spatial roots at (0,0) until we at least have a usable pose.
            // Stationary remote observers may legitimately bootstrap without velocity.
            continue;
        }
        if should_defer_controlled_predicted_adoption(
            is_local_controlled_entity,
            position.is_some(),
            rotation.is_some(),
            linear_velocity.is_some(),
        ) {
            let now_s = time.elapsed_secs_f64();
            let mut missing = Vec::new();
            if position.is_none() {
                missing.push("Position");
            }
            if rotation.is_none() {
                missing.push("Rotation");
            }
            if linear_velocity.is_none() {
                missing.push("LinearVelocity");
            }
            let missing_summary = missing.join(", ");
            if adoption_state.waiting_entity_id.as_deref() != Some(runtime_entity_id.as_str()) {
                adoption_state.waiting_entity_id = Some(runtime_entity_id.clone());
                adoption_state.wait_started_at_s = Some(now_s);
                adoption_state.last_warn_at_s = 0.0;
                adoption_state.dialog_shown = false;
            }
            adoption_state.last_missing_components = missing_summary.clone();
            if let Some(started_at_s) = adoption_state.wait_started_at_s {
                let wait_s = (now_s - started_at_s).max(0.0);
                if wait_s >= tuning.defer_warn_after_s
                    && now_s - adoption_state.last_warn_at_s >= tuning.defer_warn_interval_s
                {
                    bevy::log::warn!(
                        "deferring predicted controlled adoption for {} (wait {:.2}s, missing: {})",
                        runtime_entity_id,
                        wait_s,
                        missing_summary
                    );
                    adoption_state.last_warn_at_s = now_s;
                }
            }
            continue;
        }

        if is_canonical_runtime_entity
            && let Some(&existing_entity) =
                entity_registry.by_entity_id.get(runtime_entity_id.as_str())
            && existing_entity != entity
            && live_entities.get(existing_entity).is_err()
        {
            entity_registry
                .by_entity_id
                .remove(runtime_entity_id.as_str());
        }

        if adoption_state.waiting_entity_id.as_deref() == Some(runtime_entity_id.as_str()) {
            if let Some(started_at_s) = adoption_state.wait_started_at_s {
                let resolved_wait_s = (time.elapsed_secs_f64() - started_at_s).max(0.0);
                adoption_state.resolved_samples = adoption_state.resolved_samples.saturating_add(1);
                adoption_state.resolved_total_wait_s += resolved_wait_s;
                adoption_state.resolved_max_wait_s =
                    adoption_state.resolved_max_wait_s.max(resolved_wait_s);
                bevy::log::info!(
                    "predicted controlled adoption resolved for {} after {:.2}s (samples={}, max_wait_s={:.2})",
                    runtime_entity_id,
                    resolved_wait_s,
                    adoption_state.resolved_samples,
                    adoption_state.resolved_max_wait_s
                );
            }
            adoption_state.waiting_entity_id = None;
            adoption_state.wait_started_at_s = None;
            adoption_state.last_warn_at_s = 0.0;
            adoption_state.last_missing_components.clear();
            adoption_state.dialog_shown = false;
        }

        // Keep canonical runtime ID mapping pinned to the Confirmed entity (`Replicated`).
        // Predicted/Interpolated clones share EntityGuid and are resolved by GUID queries.
        if is_canonical_runtime_entity {
            register_runtime_entity(&mut entity_registry, runtime_entity_id.clone(), entity);
        }
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            Name::new(runtime_entity_id.clone()),
            ReplicatedAdoptionHandled,
            PendingInitialVisualReady,
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
            Visibility::Hidden,
        ));
        if position.is_some() && rotation.is_some() {
            entity_commands.insert(FrameInterpolate::<Transform>::default());
        } else {
            entity_commands.remove::<FrameInterpolate<Transform>>();
        }

        if player_tag.is_none() {
            if let Some(visual_asset_id) = visual_asset_id {
                entity_commands.insert(StreamedVisualAssetId(visual_asset_id.0.clone()));
            } else {
                entity_commands.remove::<(
                    StreamedVisualAssetId,
                    StreamedVisualAttached,
                    StreamedVisualAttachmentKind,
                )>();
            }
        } else {
            entity_commands.remove::<(
                StreamedVisualAssetId,
                StreamedVisualAttached,
                StreamedVisualAttachmentKind,
                StreamedSpriteShaderAssetId,
            )>();
        }
        if player_tag.is_none()
            && let Some(sprite_shader_asset_id) = sprite_shader_asset_id
            && let Some(shader_asset_id) = sprite_shader_asset_id.0.as_ref()
        {
            entity_commands.insert(StreamedSpriteShaderAssetId(shader_asset_id.clone()));
        } else {
            entity_commands.remove::<StreamedSpriteShaderAssetId>();
        }

        if is_local_controlled_entity {
            entity_commands.remove::<RemoteEntity>();
            entity_commands.insert(RemoteVisibleEntity {
                entity_id: runtime_entity_id.clone(),
            });
        } else if is_root_entity {
            entity_commands.insert((
                RemoteEntity,
                RemoteVisibleEntity {
                    entity_id: runtime_entity_id.clone(),
                },
            ));
            if is_canonical_runtime_entity {
                remote_registry
                    .by_entity_id
                    .insert(runtime_entity_id, entity);
            }
            entity_commands.remove::<ActionQueue>();
        } else if !is_local_player_entity {
            entity_commands.remove::<ActionQueue>();
        }
    }

    let now_s = time.elapsed_secs_f64();
    if adoption_state.resolved_samples > 0
        && now_s - adoption_state.last_summary_at_s >= tuning.defer_summary_interval_s
    {
        let avg_wait_s =
            adoption_state.resolved_total_wait_s / adoption_state.resolved_samples as f64;
        bevy::log::info!(
            "predicted adoption delay summary samples={} avg_wait_s={:.2} max_wait_s={:.2}",
            adoption_state.resolved_samples,
            avg_wait_s,
            adoption_state.resolved_max_wait_s
        );
        adoption_state.last_summary_at_s = now_s;
    }

    let controlled_count = controlled_query.iter().count();
    if controlled_count > 1 {
        bevy::log::warn!(
            "multiple controlled entities detected under native replication; keeping latest control target"
        );
    }
    if controlled_count > 0 {
        adoption_state.waiting_entity_id = None;
        adoption_state.wait_started_at_s = None;
        adoption_state.last_warn_at_s = 0.0;
        adoption_state.last_missing_components.clear();
        adoption_state.dialog_shown = false;
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        adopt_native_lightyear_replicated_entities, configure_prediction_manager_tuning,
        has_local_player_runtime_presence,
        is_canonical_runtime_entity_lane, is_lightyear_replication_lane,
        resolve_control_target_entity_id, should_defer_controlled_predicted_adoption,
        should_defer_spatial_root_adoption, sync_controlled_entity_tags_system,
    };
    use crate::runtime::app_state::{ClientSession, LocalPlayerViewState};
    use crate::runtime::components::{ControlledEntity, ReplicatedAdoptionHandled, WorldEntity};
    use crate::runtime::resources::{
        BootstrapWatchdogState, ControlBootstrapPhase, ControlBootstrapState,
        DeferredPredictedAdoptionState, PredictionBootstrapTuning, PredictionCorrectionTuning,
        PredictionRollbackStateTuning, RemoteEntityRegistry,
    };
    use bevy::app::Update;
    use bevy::prelude::{App, Time};
    use lightyear::prediction::prelude::{PredictionManager, RollbackMode};
    use sidereal_game::{ControlledEntityGuid, EntityGuid, PlayerTag, SimulationMotionWriter};
    use sidereal_runtime_sync::RuntimeEntityHierarchy;
    use uuid::Uuid;

    #[test]
    fn spatial_root_adoption_allows_stationary_pose_complete_remote_entities() {
        assert!(!should_defer_spatial_root_adoption(
            true, true, true, false, false
        ));
    }

    #[test]
    fn spatial_root_adoption_still_defers_when_pose_is_missing() {
        assert!(should_defer_spatial_root_adoption(
            true, false, true, false, false
        ));
        assert!(should_defer_spatial_root_adoption(
            true, true, false, false, false
        ));
    }

    #[test]
    fn controlled_predicted_adoption_still_requires_velocity() {
        assert!(should_defer_controlled_predicted_adoption(
            true, true, true, false
        ));
        assert!(!should_defer_controlled_predicted_adoption(
            false, true, true, false
        ));
    }

    #[test]
    fn prediction_manager_uses_state_rollback_and_disables_input_rollback() {
        let mut app = App::new();
        app.insert_resource(PredictionCorrectionTuning {
            max_rollback_ticks: 160,
            instant_correction: false,
            rollback_state: PredictionRollbackStateTuning::Always,
        });
        app.add_systems(Update, configure_prediction_manager_tuning);
        let entity = app.world_mut().spawn(PredictionManager::default()).id();

        app.update();

        let manager = app
            .world()
            .get::<PredictionManager>(entity)
            .expect("prediction manager");
        assert!(matches!(
            manager.rollback_policy.state,
            RollbackMode::Always
        ));
        assert!(matches!(
            manager.rollback_policy.input,
            RollbackMode::Disabled
        ));
        assert_eq!(manager.rollback_policy.max_rollback_ticks, 160);
    }

    #[test]
    fn only_confirmed_lane_is_canonical_runtime_entity() {
        assert!(is_canonical_runtime_entity_lane(true, false, false));
        assert!(!is_canonical_runtime_entity_lane(true, false, true));
        assert!(!is_canonical_runtime_entity_lane(true, true, false));
    }

    #[test]
    fn adoption_only_accepts_lightyear_replication_lanes() {
        assert!(is_lightyear_replication_lane(true, false, false));
        assert!(is_lightyear_replication_lane(false, true, false));
        assert!(is_lightyear_replication_lane(false, false, true));
        assert!(!is_lightyear_replication_lane(false, false, false));
    }

    #[test]
    fn adoption_ignores_guid_entities_without_lightyear_role() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(PredictionBootstrapTuning::from_env());
        app.insert_resource(DeferredPredictedAdoptionState::default());
        app.insert_resource(BootstrapWatchdogState::default());
        app.insert_resource(LocalPlayerViewState::default());
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.insert_resource(RemoteEntityRegistry::default());
        app.add_systems(Update, adopt_native_lightyear_replicated_entities);

        let entity = app
            .world_mut()
            .spawn((EntityGuid(
                Uuid::parse_str("5cac6889-d3bd-4d82-920b-ca883d97bb92").unwrap(),
            ),))
            .id();

        app.update();

        assert!(app.world().get::<WorldEntity>(entity).is_none());
        assert!(
            app.world()
                .get::<ReplicatedAdoptionHandled>(entity)
                .is_none()
        );
        assert!(
            !app
                .world()
                .resource::<BootstrapWatchdogState>()
                .replication_state_seen
        );
    }

    #[test]
    fn control_bootstrap_generation_prefers_authoritative_server_generation() {
        assert_eq!(super::control_bootstrap_generation(4, 9, false), 9);
        assert_eq!(super::control_bootstrap_generation(0, 3, true), 3);
    }

    #[test]
    fn control_bootstrap_generation_falls_back_to_local_increment_only_when_needed() {
        assert_eq!(super::control_bootstrap_generation(0, 0, true), 1);
        assert_eq!(super::control_bootstrap_generation(7, 0, true), 8);
        assert_eq!(super::control_bootstrap_generation(7, 0, false), 7);
    }

    #[test]
    fn controlled_tag_target_falls_back_to_raw_guid_when_registry_is_not_ready() {
        let registry = RuntimeEntityHierarchy::default();

        let resolved = resolve_control_target_entity_id(
            &registry,
            "1521601b-7e69-4700-853f-eb1eb3a41199",
            Some("ce9e421c-8b62-458a-803e-51e9ad272908"),
        );

        assert_eq!(
            resolved.as_deref(),
            Some("ce9e421c-8b62-458a-803e-51e9ad272908")
        );
    }

    #[test]
    fn world_loading_presence_accepts_guid_only_local_player_clone() {
        let registry = RuntimeEntityHierarchy::default();
        let player_guid =
            Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").expect("valid player guid");
        let player_entity_guid = EntityGuid(player_guid);

        let present = has_local_player_runtime_presence(
            &registry,
            "1521601b-7e69-4700-853f-eb1eb3a41199",
            [&player_entity_guid],
        );

        assert!(present);
    }

    #[test]
    fn world_loading_presence_rejects_missing_local_player_guid() {
        let registry = RuntimeEntityHierarchy::default();
        let other_guid =
            Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").expect("valid other guid");
        let other_entity_guid = EntityGuid(other_guid);

        let present = has_local_player_runtime_presence(
            &registry,
            "1521601b-7e69-4700-853f-eb1eb3a41199",
            [&other_entity_guid],
        );

        assert!(!present);
    }

    #[test]
    fn controlled_ship_without_predicted_clone_stays_pending_bootstrap() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()),
            ..Default::default()
        });
        app.insert_resource(DeferredPredictedAdoptionState::default());
        app.insert_resource(ControlBootstrapState::default());
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.add_systems(Update, sync_controlled_entity_tags_system);

        let entity = app
            .world_mut()
            .spawn((EntityGuid(
                Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").unwrap(),
            ),))
            .id();

        app.update();

        assert!(app.world().get::<ControlledEntity>(entity).is_none());
        assert!(app.world().get::<SimulationMotionWriter>(entity).is_none());
        assert_eq!(
            app.world().resource::<ControlBootstrapState>().phase,
            ControlBootstrapPhase::PendingPredicted {
                target_entity_id: "ce9e421c-8b62-458a-803e-51e9ad272908".to_string(),
                generation: 1,
            }
        );
    }

    #[test]
    fn controlled_ship_binds_only_when_predicted_clone_exists() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()),
            ..Default::default()
        });
        app.insert_resource(DeferredPredictedAdoptionState::default());
        app.insert_resource(ControlBootstrapState::default());
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.add_systems(Update, sync_controlled_entity_tags_system);

        let entity = app
            .world_mut()
            .spawn((
                EntityGuid(Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").unwrap()),
                lightyear::prelude::Predicted,
            ))
            .id();

        app.update();

        assert!(app.world().get::<ControlledEntity>(entity).is_some());
        assert!(app.world().get::<SimulationMotionWriter>(entity).is_some());
        assert_eq!(
            app.world().resource::<ControlBootstrapState>().phase,
            ControlBootstrapPhase::ActivePredicted {
                target_entity_id: "ce9e421c-8b62-458a-803e-51e9ad272908".to_string(),
                generation: 1,
                entity,
            }
        );
    }

    #[test]
    fn controlled_tags_ignore_stale_replicated_player_control_component() {
        let mut app = App::new();
        let player_entity_id = "1521601b-7e69-4700-853f-eb1eb3a41199";
        let ship_entity_id = "ce9e421c-8b62-458a-803e-51e9ad272908";
        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some(player_entity_id.to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some(ship_entity_id.to_string()),
            desired_controlled_entity_id: Some(ship_entity_id.to_string()),
            controlled_entity_generation: 2,
            ..Default::default()
        });
        app.insert_resource(DeferredPredictedAdoptionState::default());
        app.insert_resource(ControlBootstrapState::default());
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.add_systems(Update, sync_controlled_entity_tags_system);

        let player_entity = app
            .world_mut()
            .spawn((
                PlayerTag,
                EntityGuid(Uuid::parse_str(player_entity_id).unwrap()),
                ControlledEntityGuid(Some(player_entity_id.to_string())),
            ))
            .id();
        let ship_entity = app
            .world_mut()
            .spawn((
                EntityGuid(Uuid::parse_str(ship_entity_id).unwrap()),
                lightyear::prelude::Predicted,
            ))
            .id();

        app.update();

        let view_state = app.world().resource::<LocalPlayerViewState>();
        assert_eq!(view_state.controlled_entity_id.as_deref(), Some(ship_entity_id));
        assert_eq!(
            view_state.desired_controlled_entity_id.as_deref(),
            Some(ship_entity_id)
        );
        assert!(app.world().get::<ControlledEntity>(player_entity).is_none());
        assert!(app.world().get::<SimulationMotionWriter>(player_entity).is_none());
        assert!(app.world().get::<ControlledEntity>(ship_entity).is_some());
        assert!(
            app.world()
                .get::<SimulationMotionWriter>(ship_entity)
                .is_some()
        );
        assert_eq!(
            app.world().resource::<ControlBootstrapState>().phase,
            ControlBootstrapPhase::ActivePredicted {
                target_entity_id: ship_entity_id.to_string(),
                generation: 2,
                entity: ship_entity,
            }
        );
    }

    #[test]
    fn player_anchor_without_predicted_clone_stays_pending_bootstrap() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(DeferredPredictedAdoptionState::default());
        app.insert_resource(ControlBootstrapState::default());
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.add_systems(Update, sync_controlled_entity_tags_system);

        let entity = app
            .world_mut()
            .spawn((
                PlayerTag,
                EntityGuid(Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").unwrap()),
            ))
            .id();

        app.update();

        assert!(app.world().get::<ControlledEntity>(entity).is_none());
        assert!(app.world().get::<SimulationMotionWriter>(entity).is_none());
        assert_eq!(
            app.world().resource::<ControlBootstrapState>().phase,
            ControlBootstrapPhase::PendingPredicted {
                target_entity_id: "1521601b-7e69-4700-853f-eb1eb3a41199".to_string(),
                generation: 1,
            }
        );
    }

    #[test]
    fn stale_player_anchor_writer_is_removed_until_predicted_clone_exists() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(DeferredPredictedAdoptionState::default());
        app.insert_resource(ControlBootstrapState::default());
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.add_systems(Update, sync_controlled_entity_tags_system);

        let entity = app
            .world_mut()
            .spawn((
                PlayerTag,
                EntityGuid(Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").unwrap()),
                ControlledEntity {
                    entity_id: "1521601b-7e69-4700-853f-eb1eb3a41199".to_string(),
                    player_entity_id: "1521601b-7e69-4700-853f-eb1eb3a41199".to_string(),
                },
                SimulationMotionWriter,
            ))
            .id();

        app.update();

        assert!(app.world().get::<ControlledEntity>(entity).is_none());
        assert!(app.world().get::<SimulationMotionWriter>(entity).is_none());
        assert_eq!(
            app.world()
                .resource::<DeferredPredictedAdoptionState>()
                .missing_predicted_control_entity_id
                .as_deref(),
            Some("1521601b-7e69-4700-853f-eb1eb3a41199")
        );
    }

    #[test]
    fn player_anchor_binds_only_when_predicted_clone_exists() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(DeferredPredictedAdoptionState::default());
        app.insert_resource(ControlBootstrapState::default());
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.add_systems(Update, sync_controlled_entity_tags_system);

        let entity = app
            .world_mut()
            .spawn((
                PlayerTag,
                EntityGuid(Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").unwrap()),
                lightyear::prelude::Predicted,
            ))
            .id();

        app.update();

        assert!(app.world().get::<ControlledEntity>(entity).is_some());
        assert!(app.world().get::<SimulationMotionWriter>(entity).is_some());
        assert_eq!(
            app.world().resource::<ControlBootstrapState>().phase,
            ControlBootstrapPhase::ActivePredicted {
                target_entity_id: "1521601b-7e69-4700-853f-eb1eb3a41199".to_string(),
                generation: 1,
                entity,
            }
        );
    }
}

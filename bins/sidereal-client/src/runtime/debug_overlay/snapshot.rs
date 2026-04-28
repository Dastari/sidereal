#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn collect_debug_overlay_snapshot_system(
    debug_overlay: Res<'_, DebugOverlayState>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    control_bootstrap_state: Res<'_, ControlBootstrapState>,
    timeline: Res<'_, LocalTimeline>,
    real_time: Res<'_, Time<Real>>,
    prediction_tuning: Res<'_, PredictionCorrectionTuning>,
    runtime_stall_diagnostics: Res<'_, RuntimeStallDiagnostics>,
    prediction_recovery_state: Res<'_, NativePredictionRecoveryState>,
    mut snapshot: ResMut<'_, DebugOverlaySnapshot>,
    entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            &'_ GlobalTransform,
            (
                Option<&'_ SizeM>,
                Option<&'_ CollisionAabbM>,
                Option<&'_ CollisionOutlineM>,
                Option<&'_ LinearVelocity>,
                Option<&'_ AngularVelocity>,
            ),
            (
                Option<&'_ MountedOn>,
                Option<&'_ Hardpoint>,
                Option<&'_ ParentGuid>,
                Option<&'_ PlayerTag>,
                Option<&'_ ControlledEntity>,
                Option<&'_ Visibility>,
                Option<&'_ DisplayName>,
                Option<&'_ EntityLabels>,
                Has<Engine>,
                Option<&'_ BallisticWeapon>,
            ),
            (
                Has<lightyear::prelude::Replicated>,
                Has<lightyear::prelude::Interpolated>,
                Has<lightyear::prelude::Predicted>,
                Has<SuppressedPredictedDuplicateVisual>,
                Has<PlanetBodyShaderSettings>,
                Option<&'_ Position>,
                Option<&'_ Rotation>,
                Option<&'_ WorldPosition>,
                Option<&'_ WorldRotation>,
                Option<&'_ ConfirmedHistory<Position>>,
                Option<&'_ ConfirmedHistory<Rotation>>,
                Option<&'_ lightyear::prelude::Confirmed<Position>>,
                Option<&'_ lightyear::prelude::Confirmed<Rotation>>,
                Option<&'_ ConfirmedTick>,
            ),
        ),
        With<WorldEntity>,
    >,
    hardpoints: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            &'_ Hardpoint,
            Option<&'_ ParentGuid>,
            Option<&'_ MountedOn>,
            Option<&'_ Visibility>,
            Option<&'_ DisplayName>,
            Option<&'_ EntityLabels>,
            Has<lightyear::prelude::Replicated>,
            Has<lightyear::prelude::Interpolated>,
            Has<lightyear::prelude::Predicted>,
        ),
    >,
    stats_inputs: DebugOverlayStatsInputs<'_, '_>,
) {
    snapshot.frame_index = snapshot.frame_index.saturating_add(1);
    snapshot.entities.clear();
    snapshot.controlled_lane = None;
    snapshot.stats = DebugOverlayStats::default();
    snapshot.text_rows.clear();

    if !debug_overlay.enabled {
        return;
    }

    snapshot.stats.window_focused = runtime_stall_diagnostics.window_focused;
    snapshot.stats.focus_transitions = runtime_stall_diagnostics.focus_transitions;
    snapshot.stats.last_focus_change_age_s = if runtime_stall_diagnostics.focus_initialized {
        (real_time.elapsed_secs_f64() - runtime_stall_diagnostics.last_focus_change_at_s).max(0.0)
    } else {
        0.0
    };
    snapshot.stats.observed_unfocused_duration_s =
        runtime_stall_diagnostics.observed_unfocused_duration_s;
    snapshot.stats.observed_unfocused_frames = runtime_stall_diagnostics.observed_unfocused_frames;
    snapshot.stats.prediction_recovery_phase = prediction_recovery_state
        .phase
        .label(real_time.elapsed_secs_f64());
    snapshot.stats.prediction_recovery_suppressing_input = prediction_recovery_state
        .is_suppressing_input(real_time.elapsed_secs_f64())
        || prediction_recovery_state.pending_neutral_send;
    snapshot.stats.prediction_recovery_last_unfocused_s =
        prediction_recovery_state.last_unfocused_duration_s;
    snapshot.stats.prediction_recovery_transitions = prediction_recovery_state.transition_count;
    snapshot.stats.prediction_recovery_neutral_sends = prediction_recovery_state.neutral_send_count;
    snapshot.stats.last_update_delta_ms = runtime_stall_diagnostics.last_update_delta_ms;
    snapshot.stats.max_update_delta_ms = runtime_stall_diagnostics.max_update_delta_ms;
    snapshot.stats.last_stall_gap_ms = runtime_stall_diagnostics.last_stall_gap_ms;
    snapshot.stats.last_stall_gap_estimated_ticks =
        runtime_stall_diagnostics.last_stall_gap_estimated_ticks;
    snapshot.stats.max_stall_gap_ms = runtime_stall_diagnostics.max_stall_gap_ms;
    snapshot.stats.max_stall_gap_estimated_ticks =
        runtime_stall_diagnostics.max_stall_gap_estimated_ticks;
    snapshot.stats.fixed_runs_last_frame = runtime_stall_diagnostics.fixed_runs_last_frame;
    snapshot.stats.fixed_runs_max_frame = runtime_stall_diagnostics.fixed_runs_max_frame;
    snapshot.stats.fixed_overstep_ms = runtime_stall_diagnostics.fixed_overstep_ms;
    snapshot.stats.control_bootstrap_phase = match &control_bootstrap_state.phase {
        ControlBootstrapPhase::Idle => "Idle".to_string(),
        ControlBootstrapPhase::PendingPredicted {
            target_entity_id,
            generation,
        } => format!("Pending {target_entity_id} g{generation}"),
        ControlBootstrapPhase::ActivePredicted {
            target_entity_id,
            generation,
            ..
        } => format!("Predicted {target_entity_id} g{generation}"),
    };
    snapshot.stats.rollback_budget_ticks = prediction_tuning.max_rollback_ticks;
    snapshot.stats.rollback_budget_ms =
        f64::from(prediction_tuning.max_rollback_ticks) * 1000.0 / f64::from(SIM_TICK_HZ);
    snapshot.stats.local_timeline_tick = Some(u32::from(timeline.tick().0));

    snapshot.stats.mesh_asset_count = stats_inputs.mesh_assets.iter().count();
    snapshot.stats.active_camera_count = stats_inputs.cameras.iter().count();
    snapshot.stats.generic_sprite_material_count =
        stats_inputs.generic_sprite_materials.iter().count();
    snapshot.stats.asteroid_material_count = stats_inputs.asteroid_materials.iter().count();
    snapshot.stats.planet_material_count = stats_inputs.planet_materials.iter().count();
    snapshot.stats.effect_material_count = stats_inputs.effect_materials.iter().count();
    snapshot.stats.streamed_visual_child_count =
        stats_inputs.streamed_visual_children.iter().count();
    snapshot.stats.planet_pass_count = stats_inputs
        .visual_passes
        .iter()
        .filter(|pass| pass.family == RuntimeWorldVisualFamily::Planet)
        .count();
    snapshot.stats.tracer_pool_size = stats_inputs.tracer_pool.bolts.len();
    snapshot.stats.active_tracers = stats_inputs
        .tracer_entities
        .iter()
        .filter(|visibility| **visibility != Visibility::Hidden)
        .count();
    snapshot.stats.spark_pool_size = stats_inputs.spark_pool.sparks.len();
    snapshot.stats.active_sparks = stats_inputs
        .spark_entities
        .iter()
        .filter(|visibility| **visibility != Visibility::Hidden)
        .count();
    snapshot.stats.bootstrap_ready_bytes = stats_inputs.asset_manager.bootstrap_ready_bytes;
    snapshot.stats.bootstrap_total_bytes = stats_inputs.asset_manager.bootstrap_total_bytes;
    snapshot.stats.runtime_dependency_candidate_count = stats_inputs
        .runtime_asset_dependency_state
        .candidate_asset_ids
        .len();
    snapshot.stats.runtime_dependency_graph_rebuilds = stats_inputs
        .runtime_asset_dependency_state
        .dependency_graph_rebuilds;
    snapshot.stats.runtime_dependency_scan_runs = stats_inputs
        .runtime_asset_dependency_state
        .dependency_scan_runs;
    snapshot.stats.runtime_in_flight_fetch_count = stats_inputs
        .runtime_asset_fetch_state
        .as_ref()
        .in_flight_asset_ids_len();
    snapshot.stats.runtime_pending_fetch_count =
        stats_inputs.runtime_asset_perf.pending_fetch_count;
    snapshot.stats.runtime_pending_persist_count =
        stats_inputs.runtime_asset_perf.pending_persist_count;
    snapshot.stats.runtime_asset_fetch_poll_last_ms =
        stats_inputs.runtime_asset_perf.fetch_poll_last_ms;
    snapshot.stats.runtime_asset_fetch_poll_max_ms =
        stats_inputs.runtime_asset_perf.fetch_poll_max_ms;
    snapshot.stats.runtime_asset_persist_task_last_ms =
        stats_inputs.runtime_asset_perf.persist_task_last_ms;
    snapshot.stats.runtime_asset_persist_task_max_ms =
        stats_inputs.runtime_asset_perf.persist_task_max_ms;
    snapshot.stats.runtime_asset_save_index_last_ms =
        stats_inputs.runtime_asset_perf.save_index_last_ms;
    snapshot.stats.runtime_asset_save_index_max_ms =
        stats_inputs.runtime_asset_perf.save_index_max_ms;
    snapshot.stats.render_layer_registry_rebuilds =
        stats_inputs.render_layer_perf.registry_rebuilds;
    snapshot.stats.render_layer_assignment_recomputes =
        stats_inputs.render_layer_perf.assignment_recomputes;
    snapshot.stats.render_layer_assignment_skips = stats_inputs.render_layer_perf.assignment_skips;
    snapshot.stats.duplicate_winner_swaps = stats_inputs.duplicate_resolution.winner_swap_count;
    snapshot.stats.tactical_contacts_last = stats_inputs.hud_perf.tactical_contacts_last;
    snapshot.stats.tactical_markers_last = stats_inputs.hud_perf.tactical_markers_last;
    snapshot.stats.tactical_marker_spawns_last = stats_inputs.hud_perf.tactical_marker_spawns_last;
    snapshot.stats.tactical_marker_updates_last =
        stats_inputs.hud_perf.tactical_marker_updates_last;
    snapshot.stats.tactical_marker_despawns_last =
        stats_inputs.hud_perf.tactical_marker_despawns_last;
    snapshot.stats.tactical_overlay_last_ms = stats_inputs.hud_perf.tactical_overlay_last_ms;
    snapshot.stats.tactical_overlay_max_ms = stats_inputs.hud_perf.tactical_overlay_max_ms;
    snapshot.stats.nameplate_targets_last = stats_inputs.hud_perf.nameplate_targets_last;
    snapshot.stats.nameplate_visible_last = stats_inputs.hud_perf.nameplate_visible_last;
    snapshot.stats.nameplate_hidden_last = stats_inputs.hud_perf.nameplate_hidden_last;
    snapshot.stats.nameplate_health_updates_last =
        stats_inputs.hud_perf.nameplate_health_updates_last;
    snapshot.stats.nameplate_entity_data_last = stats_inputs.hud_perf.nameplate_entity_data_last;
    snapshot.stats.nameplate_sync_last_ms = stats_inputs.hud_perf.nameplate_sync_last_ms;
    snapshot.stats.nameplate_sync_max_ms = stats_inputs.hud_perf.nameplate_sync_max_ms;
    snapshot.stats.nameplate_position_last_ms = stats_inputs.hud_perf.nameplate_position_last_ms;
    snapshot.stats.nameplate_position_max_ms = stats_inputs.hud_perf.nameplate_position_max_ms;
    snapshot.stats.nameplate_camera_candidates_last =
        stats_inputs.hud_perf.nameplate_camera_candidates_last;
    snapshot.stats.nameplate_camera_active_last =
        stats_inputs.hud_perf.nameplate_camera_active_last;
    snapshot.stats.nameplate_missing_target_last =
        stats_inputs.hud_perf.nameplate_missing_target_last;
    snapshot.stats.nameplate_projection_failures_last =
        stats_inputs.hud_perf.nameplate_projection_failures_last;
    snapshot.stats.nameplate_viewport_culled_last =
        stats_inputs.hud_perf.nameplate_viewport_culled_last;

    let logical_control_guid = player_view_state
        .controlled_entity_id
        .as_deref()
        .and_then(parse_guid_from_entity_id)
        .or_else(|| {
            session
                .player_entity_id
                .as_deref()
                .and_then(parse_guid_from_entity_id)
        });

    let mut root_candidates_by_guid = HashMap::<uuid::Uuid, Vec<RootDebugCandidate>>::new();
    let mut auxiliary_entities = Vec::new();
    let mut anomaly_messages = Vec::<String>::new();

    for (
        entity,
        guid,
        global_transform,
        (size_m, collision_aabb, collision_outline, linear_velocity, angular_velocity),
        (
            mounted_on,
            hardpoint,
            parent_guid,
            player_tag,
            controlled_marker,
            visibility,
            display_name,
            entity_labels,
            has_engine,
            ballistic_weapon,
        ),
        (
            is_replicated,
            is_interpolated,
            is_predicted,
            is_suppressed_duplicate,
            is_planet,
            position,
            rotation,
            world_position,
            world_rotation,
            position_history,
            rotation_history,
            confirmed_position,
            confirmed_rotation,
            confirmed_tick,
        ),
    ) in &entities
    {
        if player_tag.is_some() {
            continue;
        }
        if is_suppressed_duplicate {
            continue;
        }
        if !debug_overlay_candidate_visible(visibility) {
            continue;
        }
        if hardpoint.is_some() {
            continue;
        }

        let is_local_controlled = logical_control_guid
            .map(|control_guid| {
                mounted_on.is_none() && hardpoint.is_none() && guid.0 == control_guid
            })
            .unwrap_or_else(|| {
                controlled_marker.is_some_and(|controlled| {
                    session
                        .player_entity_id
                        .as_deref()
                        .is_some_and(|player_id| controlled.player_entity_id == player_id)
                })
            });
        let collision = build_collision_shape(
            size_m,
            collision_aabb,
            collision_outline,
            hardpoint.is_some(),
        );
        let world = global_transform.compute_transform();
        let label = if is_planet {
            display_name
                .map(|name| name.0.trim().to_ascii_uppercase())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "PLANET".to_string())
        } else {
            debug_overlay_entity_label(
                display_name,
                entity_labels,
                hardpoint,
                mounted_on,
                has_engine,
                ballistic_weapon,
            )
        };
        let overlay_entity = DebugOverlayEntity {
            entity,
            guid: guid.0,
            label,
            lane: DebugEntityLane::Auxiliary,
            position_xy: world.translation.truncate(),
            rotation_rad: world.rotation.to_euler(EulerRot::XYZ).2,
            velocity_xy: linear_velocity
                .map(|value| value.0.as_vec2())
                .unwrap_or(Vec2::ZERO),
            angular_velocity_rps: angular_velocity
                .map(|value| value.0 as f32)
                .unwrap_or_default(),
            collision,
            is_controlled: is_local_controlled,
            is_component: mounted_on.is_some()
                || hardpoint.is_some()
                || has_engine
                || ballistic_weapon.is_some(),
        };

        if is_predicted && is_interpolated {
            anomaly_messages.push(format!(
                "entity {} ({}) has both Predicted and Interpolated markers",
                entity, guid.0
            ));
        }
        if is_local_controlled && !is_predicted && (is_interpolated || is_replicated) {
            anomaly_messages.push(format!(
                "controlled guid {} resolved without a Predicted root",
                guid.0
            ));
        }
        if !is_local_controlled && is_predicted {
            anomaly_messages.push(format!("remote guid {} resolved as Predicted", guid.0));
        }

        let interpolated_ready = interpolated_presentation_ready(
            position,
            rotation,
            world_position,
            world_rotation,
            confirmed_position,
            confirmed_rotation,
            position_history,
            rotation_history,
        );

        if let Some(parent_root_guid) = mounted_on
            .map(|mounted_on| mounted_on.parent_entity_id)
            .or_else(|| parent_guid.map(|parent_guid| parent_guid.0))
        {
            auxiliary_entities.push(AuxiliaryDebugCandidate {
                guid: guid.0,
                parent_root_guid,
                overlay_entity,
                is_replicated,
                is_interpolated,
                is_predicted,
                interpolated_ready,
            });
            continue;
        }

        root_candidates_by_guid
            .entry(guid.0)
            .or_default()
            .push(RootDebugCandidate {
                overlay_entity,
                is_replicated,
                is_interpolated,
                is_predicted,
                interpolated_ready,
                has_confirmed_wrappers: confirmed_position.is_some()
                    && confirmed_rotation.is_some(),
                confirmed_pose: confirmed_position.zip(confirmed_rotation).map(
                    |(position, rotation)| ConfirmedGhostPose {
                        position_xy: position.0.0.as_vec2(),
                        rotation_rad: rotation.0.as_radians() as f32,
                    },
                ),
                confirmed_tick: confirmed_tick.map(|tick| tick.tick.0),
            });
    }

    snapshot.stats.duplicate_guid_groups = root_candidates_by_guid
        .values()
        .filter(|candidates| candidates.len() > 1)
        .count();

    let mut resolved_root_lanes = HashMap::<uuid::Uuid, DebugEntityLane>::new();
    let mut resolved_root_poses = HashMap::<uuid::Uuid, (Vec2, f32)>::new();

    for (guid, candidates) in root_candidates_by_guid {
        if candidates.len() > 1 {
            let predicted_count = candidates
                .iter()
                .filter(|candidate| candidate.is_predicted)
                .count();
            let interpolated_count = candidates
                .iter()
                .filter(|candidate| candidate.is_interpolated)
                .count();
            let confirmed_count = candidates
                .iter()
                .filter(|candidate| {
                    candidate.is_replicated && !candidate.is_predicted && !candidate.is_interpolated
                })
                .count();
            if predicted_count > 1 || interpolated_count > 1 || confirmed_count > 1 {
                anomaly_messages.push(format!(
                    "guid {} has duplicate lane winners p={} i={} c={}",
                    guid, predicted_count, interpolated_count, confirmed_count
                ));
            }
        }

        let resolved = resolve_root_candidates(&candidates);
        if let Some(primary) = resolved.primary {
            let has_confirmed_ghost = resolved.confirmed_ghost.is_some();
            resolved_root_lanes.insert(guid, resolved.primary_lane);
            resolved_root_poses.insert(
                guid,
                (
                    primary.overlay_entity.position_xy,
                    primary.overlay_entity.rotation_rad,
                ),
            );
            if primary.overlay_entity.is_controlled {
                snapshot.controlled_lane = Some(DebugControlledLane {
                    guid,
                    primary_lane: resolved.primary_lane,
                    has_confirmed_ghost,
                });
                if let Some(confirmed_tick) = primary.confirmed_tick {
                    let confirmed_tick = u32::from(confirmed_tick);
                    snapshot.stats.controlled_confirmed_tick = Some(confirmed_tick);
                    snapshot.stats.controlled_tick_gap = snapshot
                        .stats
                        .local_timeline_tick
                        .map(|local_tick| local_tick.saturating_sub(confirmed_tick));
                }
            }
            push_snapshot_entity(
                &mut snapshot,
                &primary.overlay_entity,
                resolved.primary_lane,
            );
        }
        if let Some(confirmed_ghost) = resolved.confirmed_ghost {
            push_snapshot_entity(
                &mut snapshot,
                &confirmed_ghost.overlay_entity,
                DebugEntityLane::ConfirmedGhost,
            );
        }
    }

    for (
        entity,
        guid,
        hardpoint,
        parent_guid,
        mounted_on,
        visibility,
        display_name,
        entity_labels,
        is_replicated,
        is_interpolated,
        is_predicted,
    ) in &hardpoints
    {
        if !debug_overlay_candidate_visible(visibility) {
            continue;
        }
        let Some(parent_root_guid) = parent_guid
            .map(|parent_guid| parent_guid.0)
            .or_else(|| mounted_on.map(|mounted_on| mounted_on.parent_entity_id))
        else {
            continue;
        };
        let Some((parent_position_xy, parent_rotation_rad)) =
            resolved_root_poses.get(&parent_root_guid).copied()
        else {
            continue;
        };

        let parent_rotation = Quat::from_rotation_z(parent_rotation_rad);
        let hardpoint_world_offset = parent_rotation * hardpoint_debug_offset_m(hardpoint);
        let local_rotation_rad = hardpoint.local_rotation.to_euler(EulerRot::XYZ).2;
        let overlay_entity = DebugOverlayEntity {
            entity,
            guid: guid.0,
            label: debug_overlay_entity_label(
                display_name,
                entity_labels,
                Some(hardpoint),
                mounted_on,
                false,
                None,
            ),
            lane: DebugEntityLane::Auxiliary,
            position_xy: parent_position_xy + hardpoint_world_offset.truncate(),
            rotation_rad: parent_rotation_rad + local_rotation_rad,
            velocity_xy: Vec2::ZERO,
            angular_velocity_rps: 0.0,
            collision: DebugCollisionShape::HardpointMarker,
            is_controlled: false,
            is_component: true,
        };

        auxiliary_entities.push(AuxiliaryDebugCandidate {
            guid: guid.0,
            parent_root_guid,
            overlay_entity,
            is_replicated,
            is_interpolated,
            is_predicted,
            interpolated_ready: true,
        });
    }

    let mut auxiliary_candidates_by_guid =
        HashMap::<uuid::Uuid, Vec<AuxiliaryDebugCandidate>>::new();
    for candidate in auxiliary_entities {
        if resolved_root_lanes.contains_key(&candidate.parent_root_guid) {
            auxiliary_candidates_by_guid
                .entry(candidate.guid)
                .or_default()
                .push(candidate);
        }
    }

    for candidates in auxiliary_candidates_by_guid.into_values() {
        let Some(entity) = resolve_auxiliary_candidate(&candidates, &resolved_root_lanes) else {
            continue;
        };
        push_snapshot_entity(
            &mut snapshot,
            &entity.overlay_entity,
            DebugEntityLane::Auxiliary,
        );
    }

    snapshot.stats.anomaly_count = anomaly_messages.len();
    snapshot.text_rows =
        build_debug_text_rows(&snapshot.stats, snapshot.controlled_lane, &anomaly_messages);
}


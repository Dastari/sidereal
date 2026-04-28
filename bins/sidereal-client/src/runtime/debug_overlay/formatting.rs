fn build_debug_text_rows(
    stats: &DebugOverlayStats,
    controlled_lane: Option<DebugControlledLane>,
    anomaly_messages: &[String],
) -> Vec<DebugTextRow> {
    let stall_severity =
        if stats.last_stall_gap_estimated_ticks > u32::from(stats.rollback_budget_ticks) {
            DebugSeverity::Error
        } else if stats.last_stall_gap_estimated_ticks
            > (u32::from(stats.rollback_budget_ticks) / 2).max(1)
        {
            DebugSeverity::Warn
        } else {
            DebugSeverity::Normal
        };
    let mut rows = vec![
        DebugTextRow {
            label: "Window Focus".to_string(),
            value: format!(
                "{} @ {:>4.1}s",
                if stats.window_focused { "on" } else { "off" },
                stats.last_focus_change_age_s
            ),
            severity: if stats.window_focused {
                DebugSeverity::Normal
            } else {
                DebugSeverity::Warn
            },
        },
        DebugTextRow {
            label: "Upd/Fix/Ov".to_string(),
            value: format!(
                "{:>4.1}ms {:>2}/{:>2} {:>4.1}ms",
                stats.last_update_delta_ms,
                stats.fixed_runs_last_frame,
                stats.fixed_runs_max_frame,
                stats.fixed_overstep_ms
            ),
            severity: if stats.fixed_runs_last_frame > 1 || stats.fixed_overstep_ms > 16.7 {
                DebugSeverity::Warn
            } else {
                DebugSeverity::Normal
            },
        },
        DebugTextRow {
            label: "Stall Gap".to_string(),
            value: format!(
                "{:>5.0}ms ~{:>3}t ({:>5.0}/{:>3})",
                stats.last_stall_gap_ms,
                stats.last_stall_gap_estimated_ticks,
                stats.max_stall_gap_ms,
                stats.max_stall_gap_estimated_ticks
            ),
            severity: stall_severity,
        },
        DebugTextRow {
            label: "Rollback Win".to_string(),
            value: format!(
                "{:>3}t {:>4.0}ms",
                stats.rollback_budget_ticks, stats.rollback_budget_ms
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Ctrl TickGap".to_string(),
            value: match (
                stats.local_timeline_tick,
                stats.controlled_confirmed_tick,
                stats.controlled_tick_gap,
            ) {
                (Some(local_tick), Some(confirmed_tick), Some(gap)) => {
                    format!("l{:>5} c{:>5} d{:>4}", local_tick, confirmed_tick, gap)
                }
                _ => "n/a".to_string(),
            },
            severity: match stats.controlled_tick_gap {
                Some(gap) if gap > u32::from(stats.rollback_budget_ticks) => DebugSeverity::Error,
                Some(gap) if gap > (u32::from(stats.rollback_budget_ticks) / 2).max(1) => {
                    DebugSeverity::Warn
                }
                _ => DebugSeverity::Normal,
            },
        },
        DebugTextRow {
            label: "Unfocus Obs".to_string(),
            value: format!(
                "{:>4.1}s {:>4}f {:>3}x",
                stats.observed_unfocused_duration_s,
                stats.observed_unfocused_frames,
                stats.focus_transitions
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Pred Recover".to_string(),
            value: stats.prediction_recovery_phase.clone(),
            severity: if stats.prediction_recovery_suppressing_input {
                DebugSeverity::Warn
            } else {
                DebugSeverity::Normal
            },
        },
        DebugTextRow {
            label: "Recover Input".to_string(),
            value: format!(
                "sup={} last={:>4.1}s n={} t={}",
                if stats.prediction_recovery_suppressing_input {
                    "yes"
                } else {
                    " no"
                },
                stats.prediction_recovery_last_unfocused_s,
                stats.prediction_recovery_neutral_sends,
                stats.prediction_recovery_transitions
            ),
            severity: if stats.prediction_recovery_suppressing_input {
                DebugSeverity::Warn
            } else {
                DebugSeverity::Normal
            },
        },
        DebugTextRow {
            label: "Predicted".to_string(),
            value: format!("{:>4}", stats.predicted_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Confirmed".to_string(),
            value: format!("{:>4}", stats.confirmed_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Interpolated".to_string(),
            value: format!("{:>4}", stats.interpolated_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Duplicate GUIDs".to_string(),
            value: format!("{:>4}", stats.duplicate_guid_groups),
            severity: if stats.duplicate_guid_groups > 0 {
                DebugSeverity::Warn
            } else {
                DebugSeverity::Normal
            },
        },
        DebugTextRow {
            label: "Winner Swaps".to_string(),
            value: format!("{:>4}", stats.duplicate_winner_swaps),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Anomalies".to_string(),
            value: format!("{:>4}", stats.anomaly_count),
            severity: if stats.anomaly_count > 0 {
                DebugSeverity::Warn
            } else {
                DebugSeverity::Normal
            },
        },
        DebugTextRow {
            label: "Cameras".to_string(),
            value: format!("{:>4}", stats.active_camera_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Mesh Assets".to_string(),
            value: format!("{:>4}", stats.mesh_asset_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Gen Sprite Mats".to_string(),
            value: format!("{:>4}", stats.generic_sprite_material_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Asteroid Mats".to_string(),
            value: format!("{:>4}", stats.asteroid_material_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Planet Mats".to_string(),
            value: format!("{:>4}", stats.planet_material_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Effect Mats".to_string(),
            value: format!("{:>4}", stats.effect_material_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Visual Children".to_string(),
            value: format!("{:>4}", stats.streamed_visual_child_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Planet Passes".to_string(),
            value: format!("{:>4}", stats.planet_pass_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Tracer Pool".to_string(),
            value: format!("{:>3}/{:>3}", stats.active_tracers, stats.tracer_pool_size),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Spark Pool".to_string(),
            value: format!("{:>3}/{:>3}", stats.active_sparks, stats.spark_pool_size),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Layer Rebuilds".to_string(),
            value: format!("{:>4}", stats.render_layer_registry_rebuilds),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Layer Recompute".to_string(),
            value: format!("{:>4}", stats.render_layer_assignment_recomputes),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Layer Skips".to_string(),
            value: format!("{:>4}", stats.render_layer_assignment_skips),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Bootstrap".to_string(),
            value: format!(
                "{:>6}/{:>6}",
                stats.bootstrap_ready_bytes, stats.bootstrap_total_bytes
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Asset Candidates".to_string(),
            value: format!("{:>4}", stats.runtime_dependency_candidate_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Asset Rebuilds".to_string(),
            value: format!("{:>4}", stats.runtime_dependency_graph_rebuilds),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Asset Scans".to_string(),
            value: format!("{:>4}", stats.runtime_dependency_scan_runs),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Fetch InFlight".to_string(),
            value: format!("{:>4}", stats.runtime_in_flight_fetch_count),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Fetch/Persist".to_string(),
            value: format!(
                "{:>3}/{:>3}",
                stats.runtime_pending_fetch_count, stats.runtime_pending_persist_count
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Asset Poll ms".to_string(),
            value: format!(
                "{:>4.1}/{:>4.1}",
                stats.runtime_asset_fetch_poll_last_ms, stats.runtime_asset_fetch_poll_max_ms
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Persist ms".to_string(),
            value: format!(
                "{:>4.1}/{:>4.1}",
                stats.runtime_asset_persist_task_last_ms, stats.runtime_asset_persist_task_max_ms
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "SaveIdx ms".to_string(),
            value: format!(
                "{:>4.1}/{:>4.1}",
                stats.runtime_asset_save_index_last_ms, stats.runtime_asset_save_index_max_ms
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Tact Mk/Cont".to_string(),
            value: format!(
                "{:>3}/{:>3}",
                stats.tactical_markers_last, stats.tactical_contacts_last
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Tact Delta".to_string(),
            value: format!(
                "+{:>2} ~{:>2} -{:>2}",
                stats.tactical_marker_spawns_last,
                stats.tactical_marker_updates_last,
                stats.tactical_marker_despawns_last
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Tact ms".to_string(),
            value: format!(
                "{:>4.1}/{:>4.1}",
                stats.tactical_overlay_last_ms, stats.tactical_overlay_max_ms
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Plates".to_string(),
            value: format!(
                "{:>3}/{:>3}",
                stats.nameplate_visible_last, stats.nameplate_targets_last
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Plate Hidden".to_string(),
            value: format!("{:>4}", stats.nameplate_hidden_last),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "HP Updates".to_string(),
            value: format!("{:>4}", stats.nameplate_health_updates_last),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Plate Sync ms".to_string(),
            value: format!(
                "{:>4.1}/{:>4.1}",
                stats.nameplate_sync_last_ms, stats.nameplate_sync_max_ms
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Plate Pos ms".to_string(),
            value: format!(
                "{:>4.1}/{:>4.1}",
                stats.nameplate_position_last_ms, stats.nameplate_position_max_ms
            ),
            severity: DebugSeverity::Normal,
        },
        DebugTextRow {
            label: "Plate Cam".to_string(),
            value: format!(
                "{:>2}/{:>2}",
                stats.nameplate_camera_active_last, stats.nameplate_camera_candidates_last
            ),
            severity: if stats.nameplate_camera_active_last == 1 {
                DebugSeverity::Normal
            } else {
                DebugSeverity::Warn
            },
        },
        DebugTextRow {
            label: "Plate Miss/Proj".to_string(),
            value: format!(
                "{:>2}/{:>2}/{:>2}",
                stats.nameplate_missing_target_last,
                stats.nameplate_projection_failures_last,
                stats.nameplate_viewport_culled_last
            ),
            severity: DebugSeverity::Normal,
        },
    ];
    if let Some(controlled_lane) = controlled_lane {
        rows.push(DebugTextRow {
            label: "Control Lane".to_string(),
            value: format!("{:?}", controlled_lane.primary_lane),
            severity: if controlled_lane.primary_lane == DebugEntityLane::Predicted {
                DebugSeverity::Normal
            } else {
                DebugSeverity::Warn
            },
        });
        rows.push(DebugTextRow {
            label: "Ctrl Bootstrap".to_string(),
            value: stats.control_bootstrap_phase.clone(),
            severity: if stats.control_bootstrap_phase.starts_with("Predicted")
                || stats.control_bootstrap_phase.starts_with("Anchor")
            {
                DebugSeverity::Normal
            } else {
                DebugSeverity::Warn
            },
        });
        rows.push(DebugTextRow {
            label: "Control GUID".to_string(),
            value: controlled_lane.guid.to_string(),
            severity: DebugSeverity::Normal,
        });
        rows.push(DebugTextRow {
            label: "Confirmed Ghost".to_string(),
            value: if controlled_lane.has_confirmed_ghost {
                "yes".to_string()
            } else {
                "no".to_string()
            },
            severity: if controlled_lane.has_confirmed_ghost {
                DebugSeverity::Normal
            } else {
                DebugSeverity::Warn
            },
        });
    }
    if let Some(message) = anomaly_messages.first() {
        rows.push(DebugTextRow {
            label: "Alert".to_string(),
            value: message.clone(),
            severity: DebugSeverity::Error,
        });
    }
    rows
}

fn overlay_world_position(position_xy: Vec2, lane: DebugEntityLane) -> Vec3 {
    let z_step = match lane {
        DebugEntityLane::Predicted => PREDICTED_OVERLAY_Z_STEP,
        DebugEntityLane::Interpolated => INTERPOLATED_OVERLAY_Z_STEP,
        DebugEntityLane::Confirmed => REPLICATED_OVERLAY_Z_STEP,
        DebugEntityLane::ConfirmedGhost => CONFIRMED_OVERLAY_Z_STEP,
        DebugEntityLane::Auxiliary => REPLICATED_OVERLAY_Z_STEP,
    };
    position_xy.extend(DEBUG_OVERLAY_Z_OFFSET + z_step)
}

fn lane_color(lane: DebugEntityLane, is_controlled: bool) -> Color {
    match lane {
        DebugEntityLane::Predicted if is_controlled => Color::srgb(0.2, 1.0, 1.0),
        DebugEntityLane::Predicted => Color::srgb(0.3, 0.85, 0.85),
        DebugEntityLane::Interpolated => Color::srgb(0.2, 0.8, 0.2),
        DebugEntityLane::ConfirmedGhost => Color::srgb(1.0, 0.2, 1.0),
        DebugEntityLane::Confirmed if is_controlled => Color::srgb(1.0, 0.75, 0.2),
        DebugEntityLane::Confirmed => Color::srgb(1.0, 0.75, 0.2),
        DebugEntityLane::Auxiliary => Color::srgb(0.2, 0.8, 0.2),
    }
}

fn angle_delta_rad(a: f32, b: f32) -> f32 {
    let delta =
        (a - b + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    delta.abs()
}

#[cfg(test)]
mod tests {
    use super::{
        AuxiliaryDebugCandidate, ConfirmedGhostPose, RootDebugCandidate, angle_delta_rad,
        build_debug_text_rows, collect_debug_overlay_snapshot_system, resolve_auxiliary_candidate,
        resolve_root_candidates,
    };
    use crate::runtime::app_state::{ClientSession, LocalPlayerViewState};
    use crate::runtime::assets::{
        LocalAssetManager, RuntimeAssetDependencyState, RuntimeAssetHttpFetchState,
    };
    use crate::runtime::backdrop::{
        AsteroidSpriteShaderMaterial, PlanetVisualMaterial, RuntimeEffectMaterial,
        StreamedSpriteShaderMaterial,
    };
    use crate::runtime::components::{WeaponImpactSparkPool, WeaponTracerPool, WorldEntity};
    use crate::runtime::resources::{
        ControlBootstrapState, DebugCollisionShape, DebugEntityLane, DebugOverlayEntity,
        DebugOverlayMode, DebugOverlaySnapshot, DebugOverlayState, DebugOverlayStats,
        DuplicateVisualResolutionState, HudPerfCounters, NativePredictionRecoveryState,
        PredictionCorrectionTuning, RenderLayerPerfCounters, RuntimeAssetPerfCounters,
        RuntimeStallDiagnostics,
    };
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use lightyear::prelude::{Interpolated, LocalTimeline, Replicated};
    use sidereal_game::EntityGuid;
    use std::collections::HashMap;

    #[allow(clippy::too_many_arguments)]
    fn root_candidate(
        raw: u32,
        guid: uuid::Uuid,
        is_controlled: bool,
        is_replicated: bool,
        is_interpolated: bool,
        interpolated_ready: bool,
        is_predicted: bool,
        confirmed_pose: Option<ConfirmedGhostPose>,
    ) -> RootDebugCandidate {
        RootDebugCandidate {
            overlay_entity: DebugOverlayEntity {
                entity: Entity::from_bits(raw as u64),
                guid,
                label: "ENTITY".to_string(),
                lane: DebugEntityLane::Auxiliary,
                position_xy: Vec2::new(raw as f32, 0.0),
                rotation_rad: 0.0,
                velocity_xy: Vec2::ZERO,
                angular_velocity_rps: 0.0,
                collision: DebugCollisionShape::None,
                is_controlled,
                is_component: false,
            },
            is_replicated,
            is_interpolated,
            is_predicted,
            interpolated_ready,
            has_confirmed_wrappers: confirmed_pose.is_some(),
            confirmed_pose,
            confirmed_tick: None,
        }
    }

    fn auxiliary_candidate(
        raw: u32,
        guid: uuid::Uuid,
        parent_root_guid: uuid::Uuid,
        is_replicated: bool,
        is_interpolated: bool,
        interpolated_ready: bool,
        is_predicted: bool,
    ) -> AuxiliaryDebugCandidate {
        AuxiliaryDebugCandidate {
            guid,
            parent_root_guid,
            overlay_entity: DebugOverlayEntity {
                entity: Entity::from_bits(raw as u64),
                guid,
                label: "COMPONENT".to_string(),
                lane: DebugEntityLane::Auxiliary,
                position_xy: Vec2::new(raw as f32, 0.0),
                rotation_rad: 0.0,
                velocity_xy: Vec2::ZERO,
                angular_velocity_rps: 0.0,
                collision: DebugCollisionShape::HardpointMarker,
                is_controlled: false,
                is_component: true,
            },
            is_replicated,
            is_interpolated,
            is_predicted,
            interpolated_ready,
        }
    }

    #[test]
    fn root_lane_resolution_prefers_expected_primary_candidate() {
        struct Case {
            name: &'static str,
            candidates: Vec<RootDebugCandidate>,
            expected_lane: DebugEntityLane,
            expected_entity_bits: u64,
            expected_confirmed_ghost_position: Option<Vec2>,
        }

        let controlled_guid = uuid::Uuid::nil();
        let remote_ready_guid = uuid::Uuid::new_v4();
        let remote_unready_guid = uuid::Uuid::new_v4();
        let cases = vec![
            Case {
                name: "controlled predicted lane wins and spawns confirmed ghost",
                candidates: vec![
                    root_candidate(
                        2,
                        controlled_guid,
                        true,
                        true,
                        false,
                        false,
                        true,
                        Some(ConfirmedGhostPose {
                            position_xy: Vec2::new(10.0, 20.0),
                            rotation_rad: 0.3,
                        }),
                    ),
                    root_candidate(1, controlled_guid, true, true, false, false, false, None),
                ],
                expected_lane: DebugEntityLane::Predicted,
                expected_entity_bits: 2,
                expected_confirmed_ghost_position: Some(Vec2::new(10.0, 20.0)),
            },
            Case {
                name: "remote ready interpolated lane beats confirmed",
                candidates: vec![
                    root_candidate(4, remote_ready_guid, false, true, false, false, false, None),
                    root_candidate(3, remote_ready_guid, false, true, true, true, false, None),
                ],
                expected_lane: DebugEntityLane::Interpolated,
                expected_entity_bits: 3,
                expected_confirmed_ghost_position: None,
            },
            Case {
                name: "remote confirmed lane beats unready interpolated",
                candidates: vec![
                    root_candidate(
                        4,
                        remote_unready_guid,
                        false,
                        true,
                        false,
                        false,
                        false,
                        None,
                    ),
                    root_candidate(
                        3,
                        remote_unready_guid,
                        false,
                        true,
                        true,
                        false,
                        false,
                        None,
                    ),
                ],
                expected_lane: DebugEntityLane::Confirmed,
                expected_entity_bits: 4,
                expected_confirmed_ghost_position: None,
            },
        ];

        for case in cases {
            let resolved = resolve_root_candidates(&case.candidates);
            assert_eq!(resolved.primary_lane, case.expected_lane, "{}", case.name);
            assert_eq!(
                resolved
                    .primary
                    .expect("primary candidate")
                    .overlay_entity
                    .entity,
                Entity::from_bits(case.expected_entity_bits),
                "{}",
                case.name
            );
            match case.expected_confirmed_ghost_position {
                Some(expected_position) => assert_eq!(
                    resolved
                        .confirmed_ghost
                        .expect("confirmed ghost")
                        .overlay_entity
                        .position_xy,
                    expected_position,
                    "{}",
                    case.name
                ),
                None => assert!(resolved.confirmed_ghost.is_none(), "{}", case.name),
            }
        }
    }

    #[test]
    fn angle_delta_wraps_across_tau() {
        let delta = angle_delta_rad(0.05, std::f32::consts::TAU - 0.05);
        assert!(delta < 0.11, "delta was {delta}");
    }

    #[test]
    fn auxiliary_lane_resolution_prefers_expected_candidate() {
        struct Case {
            name: &'static str,
            parent_lane: DebugEntityLane,
            candidates: Vec<AuxiliaryDebugCandidate>,
            expected_entity_bits: u64,
        }

        let predicted_parent_guid = uuid::Uuid::new_v4();
        let interpolated_parent_guid = uuid::Uuid::new_v4();
        let cases = vec![
            Case {
                name: "predicted parent keeps predicted child",
                parent_lane: DebugEntityLane::Predicted,
                candidates: vec![
                    auxiliary_candidate(
                        4,
                        uuid::Uuid::new_v4(),
                        predicted_parent_guid,
                        true,
                        false,
                        false,
                        false,
                    ),
                    auxiliary_candidate(
                        3,
                        uuid::Uuid::new_v4(),
                        predicted_parent_guid,
                        true,
                        false,
                        false,
                        true,
                    ),
                ],
                expected_entity_bits: 3,
            },
            Case {
                name: "interpolated parent keeps confirmed child over unready interpolated",
                parent_lane: DebugEntityLane::Interpolated,
                candidates: vec![
                    auxiliary_candidate(
                        4,
                        uuid::Uuid::new_v4(),
                        interpolated_parent_guid,
                        true,
                        false,
                        false,
                        false,
                    ),
                    auxiliary_candidate(
                        3,
                        uuid::Uuid::new_v4(),
                        interpolated_parent_guid,
                        true,
                        true,
                        false,
                        false,
                    ),
                ],
                expected_entity_bits: 4,
            },
        ];

        for case in cases {
            let parent_guid = case.candidates[0].parent_root_guid;
            let mut resolved_root_lanes = HashMap::new();
            resolved_root_lanes.insert(parent_guid, case.parent_lane);
            let resolved = resolve_auxiliary_candidate(&case.candidates, &resolved_root_lanes)
                .expect("auxiliary winner");
            assert_eq!(
                resolved.overlay_entity.entity,
                Entity::from_bits(case.expected_entity_bits),
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn snapshot_skips_explicitly_hidden_root_candidates() {
        let mut app = App::new();
        app.insert_resource(DebugOverlayState {
            enabled: true,
            mode: DebugOverlayMode::Minimal,
        });
        app.insert_resource(ClientSession::default());
        app.insert_resource(LocalPlayerViewState::default());
        app.insert_resource(WeaponTracerPool::default());
        app.insert_resource(WeaponImpactSparkPool::default());
        app.insert_resource(LocalAssetManager::default());
        app.insert_resource(RuntimeAssetDependencyState::default());
        app.insert_resource(RuntimeAssetHttpFetchState::default());
        app.insert_resource(RuntimeAssetPerfCounters::default());
        app.insert_resource(HudPerfCounters::default());
        app.insert_resource(PredictionCorrectionTuning::from_env());
        app.insert_resource(ControlBootstrapState::default());
        app.insert_resource(NativePredictionRecoveryState::default());
        app.insert_resource(LocalTimeline::default());
        app.insert_resource(RenderLayerPerfCounters::default());
        app.insert_resource(RuntimeStallDiagnostics::default());
        app.insert_resource(DuplicateVisualResolutionState::default());
        app.insert_resource(DebugOverlaySnapshot::default());
        app.insert_resource(Time::<Real>::default());
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StreamedSpriteShaderMaterial>::default());
        app.insert_resource(Assets::<AsteroidSpriteShaderMaterial>::default());
        app.insert_resource(Assets::<PlanetVisualMaterial>::default());
        app.insert_resource(Assets::<RuntimeEffectMaterial>::default());

        let receiver = app.world_mut().spawn_empty().id();
        let guid = uuid::Uuid::new_v4();

        app.world_mut().spawn((
            WorldEntity,
            EntityGuid(guid),
            GlobalTransform::from(Transform::from_xyz(10.0, 0.0, 0.0)),
            Visibility::Hidden,
            Replicated { receiver },
            Interpolated,
        ));
        app.world_mut().spawn((
            WorldEntity,
            EntityGuid(guid),
            GlobalTransform::from(Transform::from_xyz(20.0, 0.0, 0.0)),
            Visibility::Visible,
            Replicated { receiver },
        ));

        let result = app
            .world_mut()
            .run_system_once(collect_debug_overlay_snapshot_system);
        assert!(
            result.is_ok(),
            "snapshot collection should succeed: {result:?}"
        );

        let snapshot = app.world().resource::<DebugOverlaySnapshot>();
        assert_eq!(snapshot.entities.len(), 1);
        assert_eq!(snapshot.entities[0].lane, DebugEntityLane::Confirmed);
        assert_eq!(snapshot.entities[0].position_xy, Vec2::new(20.0, 0.0));
    }

    #[test]
    fn debug_text_rows_include_asset_and_hud_perf_metrics() {
        let stats = DebugOverlayStats {
            window_focused: true,
            last_update_delta_ms: 16.7,
            fixed_runs_last_frame: 1,
            fixed_runs_max_frame: 3,
            fixed_overstep_ms: 2.0,
            rollback_budget_ticks: 100,
            rollback_budget_ms: 1666.7,
            runtime_pending_fetch_count: 2,
            runtime_pending_persist_count: 1,
            runtime_asset_fetch_poll_last_ms: 1.5,
            runtime_asset_fetch_poll_max_ms: 3.0,
            runtime_asset_persist_task_last_ms: 4.5,
            runtime_asset_persist_task_max_ms: 6.0,
            runtime_asset_save_index_last_ms: 2.5,
            runtime_asset_save_index_max_ms: 5.0,
            tactical_contacts_last: 7,
            tactical_markers_last: 8,
            tactical_marker_spawns_last: 2,
            tactical_marker_updates_last: 5,
            tactical_marker_despawns_last: 1,
            tactical_overlay_last_ms: 0.8,
            tactical_overlay_max_ms: 1.6,
            nameplate_targets_last: 6,
            nameplate_visible_last: 4,
            nameplate_hidden_last: 2,
            nameplate_health_updates_last: 4,
            nameplate_sync_last_ms: 0.3,
            nameplate_sync_max_ms: 0.9,
            nameplate_position_last_ms: 1.2,
            nameplate_position_max_ms: 2.4,
            ..DebugOverlayStats::default()
        };

        let rows = build_debug_text_rows(&stats, None, &[]);
        let labels = rows
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>();

        assert!(labels.contains(&"Fetch/Persist"));
        assert!(labels.contains(&"Asset Poll ms"));
        assert!(labels.contains(&"Persist ms"));
        assert!(labels.contains(&"SaveIdx ms"));
        assert!(labels.contains(&"Tact Mk/Cont"));
        assert!(labels.contains(&"Tact ms"));
        assert!(labels.contains(&"Window Focus"));
        assert!(labels.contains(&"Stall Gap"));
        assert!(labels.contains(&"Rollback Win"));
        assert!(labels.contains(&"Plates"));
        assert!(labels.contains(&"HP Updates"));
        assert!(labels.contains(&"Plate Pos ms"));
        assert!(labels.contains(&"Plate Cam"));
        assert!(labels.contains(&"Plate Miss/Proj"));
    }
}

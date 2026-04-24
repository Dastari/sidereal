//! F3 debug overlay: toggle, snapshot collection, and snapshot-driven gizmo drawing.

use avian2d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prelude::{ConfirmedTick, LocalTimeline};
use sidereal_core::SIM_TICK_HZ;
use sidereal_game::{
    CollisionAabbM, CollisionOutlineM, EntityGuid, Hardpoint, MountedOn, ParentGuid,
    PlanetBodyShaderSettings, PlayerTag, SizeM, WorldPosition, WorldRotation,
};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use std::collections::HashMap;

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::assets::{LocalAssetManager, RuntimeAssetDependencyState, RuntimeAssetHttpFetchState};
use super::backdrop::{
    AsteroidSpriteShaderMaterial, PlanetVisualMaterial, RuntimeEffectMaterial,
    StreamedSpriteShaderMaterial,
};
use super::components::{
    ControlledEntity, DebugVelocityArrowHeadLower, DebugVelocityArrowHeadUpper,
    DebugVelocityArrowShaft, RuntimeWorldVisualFamily, RuntimeWorldVisualPass, StreamedVisualChild,
    SuppressedPredictedDuplicateVisual, WeaponImpactSpark, WeaponImpactSparkPool, WeaponTracerBolt,
    WeaponTracerPool, WorldEntity,
};
use super::dev_console::{DevConsoleState, is_console_open};
use super::resources::{
    ControlBootstrapPhase, ControlBootstrapState, DebugCollisionShape, DebugControlledLane,
    DebugEntityLane, DebugOverlayEntity, DebugOverlaySnapshot, DebugOverlayState,
    DebugOverlayStats, DebugSeverity, DebugTextRow, DuplicateVisualResolutionState,
    HudPerfCounters, NativePredictionRecoveryState, PredictionCorrectionTuning,
    RenderLayerPerfCounters, RuntimeAssetPerfCounters, RuntimeStallDiagnostics,
};
use super::transforms::interpolated_presentation_ready;

const DEBUG_OVERLAY_Z_OFFSET: f32 = 6.0;
const REPLICATED_OVERLAY_Z_STEP: f32 = 0.0;
const INTERPOLATED_OVERLAY_Z_STEP: f32 = 0.18;
const PREDICTED_OVERLAY_Z_STEP: f32 = 0.36;
const CONFIRMED_OVERLAY_Z_STEP: f32 = 0.54;
const CONFIRMED_OVERLAY_POSITION_EPSILON_M: f32 = 0.05;
const CONFIRMED_OVERLAY_ROTATION_EPSILON_RAD: f32 = 0.01;
const VELOCITY_ARROW_SCALE: f32 = 0.5;
const HARDPOINT_CROSS_HALF_SIZE: f32 = 2.0;
const VELOCITY_ARROW_SHAFT_THICKNESS: f32 = 0.18;
const VELOCITY_ARROW_HEAD_LENGTH: f32 = 0.7;
const VELOCITY_ARROW_HEAD_THICKNESS: f32 = 0.12;
const VELOCITY_ARROW_HEAD_SPREAD_RAD: f32 = 0.7;
const DEBUG_STALL_GAP_THRESHOLD_MS: f64 = 100.0;

#[derive(SystemParam)]
pub(crate) struct DebugOverlayStatsInputs<'w, 's> {
    tracer_pool: Res<'w, WeaponTracerPool>,
    spark_pool: Res<'w, WeaponImpactSparkPool>,
    asset_manager: Res<'w, LocalAssetManager>,
    runtime_asset_dependency_state: Res<'w, RuntimeAssetDependencyState>,
    runtime_asset_fetch_state: Res<'w, RuntimeAssetHttpFetchState>,
    runtime_asset_perf: Res<'w, RuntimeAssetPerfCounters>,
    hud_perf: Res<'w, HudPerfCounters>,
    render_layer_perf: Res<'w, RenderLayerPerfCounters>,
    duplicate_resolution: Res<'w, DuplicateVisualResolutionState>,
    mesh_assets: Res<'w, Assets<Mesh>>,
    generic_sprite_materials: Res<'w, Assets<StreamedSpriteShaderMaterial>>,
    asteroid_materials: Res<'w, Assets<AsteroidSpriteShaderMaterial>>,
    planet_materials: Res<'w, Assets<PlanetVisualMaterial>>,
    effect_materials: Res<'w, Assets<RuntimeEffectMaterial>>,
    cameras: Query<'w, 's, &'static Camera>,
    visual_passes: Query<'w, 's, &'static RuntimeWorldVisualPass>,
    streamed_visual_children: Query<'w, 's, (), With<StreamedVisualChild>>,
    tracer_entities: Query<'w, 's, &'static Visibility, With<WeaponTracerBolt>>,
    spark_entities: Query<'w, 's, &'static Visibility, With<WeaponImpactSpark>>,
}

pub(crate) fn toggle_debug_overlay_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    mut debug_overlay: ResMut<'_, DebugOverlayState>,
) {
    if is_console_open(dev_console_state.as_deref()) {
        return;
    }
    if input.just_pressed(KeyCode::F3) {
        debug_overlay.enabled = !debug_overlay.enabled;
    }
}

pub(crate) fn debug_overlay_enabled(debug_overlay: Res<'_, DebugOverlayState>) -> bool {
    debug_overlay.enabled
}

pub(crate) fn count_fixed_update_runs_for_debug_diagnostics_system(
    mut diagnostics: ResMut<'_, RuntimeStallDiagnostics>,
) {
    diagnostics.fixed_runs_current_frame = diagnostics.fixed_runs_current_frame.saturating_add(1);
}

pub(crate) fn track_runtime_stall_diagnostics_system(
    real_time: Res<'_, Time<Real>>,
    fixed_time: Res<'_, Time<Fixed>>,
    windows: Query<'_, '_, &'_ Window, With<PrimaryWindow>>,
    mut diagnostics: ResMut<'_, RuntimeStallDiagnostics>,
) {
    let now_s = real_time.elapsed_secs_f64();
    let update_delta_ms = real_time.delta_secs_f64() * 1000.0;
    diagnostics.last_update_delta_ms = update_delta_ms;
    diagnostics.max_update_delta_ms = diagnostics.max_update_delta_ms.max(update_delta_ms);
    diagnostics.fixed_runs_last_frame = diagnostics.fixed_runs_current_frame;
    diagnostics.fixed_runs_max_frame = diagnostics
        .fixed_runs_max_frame
        .max(diagnostics.fixed_runs_last_frame);
    diagnostics.fixed_runs_current_frame = 0;
    diagnostics.fixed_overstep_ms = fixed_time.overstep().as_secs_f64() * 1000.0;

    let window_focused = windows
        .single()
        .map(|window| window.focused)
        .unwrap_or(true);
    if !diagnostics.focus_initialized {
        diagnostics.window_focused = window_focused;
        diagnostics.focus_initialized = true;
        diagnostics.last_focus_change_at_s = now_s;
    } else if diagnostics.window_focused != window_focused {
        diagnostics.window_focused = window_focused;
        diagnostics.focus_transitions = diagnostics.focus_transitions.saturating_add(1);
        diagnostics.last_focus_change_at_s = now_s;
    }

    if !window_focused {
        diagnostics.observed_unfocused_duration_s += real_time.delta_secs_f64();
        diagnostics.observed_unfocused_frames =
            diagnostics.observed_unfocused_frames.saturating_add(1);
    }

    if update_delta_ms >= DEBUG_STALL_GAP_THRESHOLD_MS {
        let estimated_ticks = (real_time.delta_secs_f64() * f64::from(SIM_TICK_HZ)).ceil() as u32;
        diagnostics.last_stall_gap_ms = update_delta_ms;
        diagnostics.last_stall_gap_estimated_ticks = estimated_ticks;
        if update_delta_ms > diagnostics.max_stall_gap_ms {
            diagnostics.max_stall_gap_ms = update_delta_ms;
            diagnostics.max_stall_gap_estimated_ticks = estimated_ticks;
        }
    }
}

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
        ControlBootstrapPhase::ActiveAnchor {
            target_entity_id,
            generation,
        } => format!("Anchor {target_entity_id} g{generation}"),
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
        (mounted_on, hardpoint, parent_guid, player_tag, controlled_marker, visibility),
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
        if is_planet {
            continue;
        }
        if !debug_overlay_candidate_visible(visibility) {
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
        let overlay_entity = DebugOverlayEntity {
            entity,
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

pub(crate) fn draw_debug_overlay_system(
    debug_overlay: Res<'_, DebugOverlayState>,
    snapshot: Res<'_, DebugOverlaySnapshot>,
    mut gizmos: Gizmos,
) {
    if !debug_overlay.enabled {
        return;
    }

    let hardpoint_color = Color::srgb(1.0, 0.8, 0.2);
    let prediction_error_color = Color::srgb(1.0, 0.2, 0.2);

    let mut controlled_predicted = None;
    let mut controlled_confirmed_ghost = None;

    for entity in &snapshot.entities {
        let pos = overlay_world_position(entity.position_xy, entity.lane);
        let rot = Quat::from_rotation_z(entity.rotation_rad);
        let draw_color = lane_color(entity.lane, entity.is_controlled);

        match &entity.collision {
            DebugCollisionShape::Outline { points } if points.len() >= 2 => {
                for idx in 0..points.len() {
                    let a = points[idx];
                    let b = points[(idx + 1) % points.len()];
                    let world_a = pos + (rot * a.extend(0.0));
                    let world_b = pos + (rot * b.extend(0.0));
                    gizmos.line(world_a, world_b, draw_color);
                }
            }
            DebugCollisionShape::Aabb { half_extents } => {
                let aabb = bevy::math::bounding::Aabb3d::new(Vec3::ZERO, *half_extents);
                let transform = Transform::from_translation(pos).with_rotation(rot);
                gizmos.aabb_3d(aabb, transform, draw_color);
            }
            DebugCollisionShape::HardpointMarker => {
                let isometry = bevy::math::Isometry3d::new(pos, rot);
                gizmos.cross(isometry, HARDPOINT_CROSS_HALF_SIZE, hardpoint_color);
            }
            DebugCollisionShape::None => {}
            DebugCollisionShape::Outline { .. } => {}
        }

        if entity.is_controlled && entity.lane == DebugEntityLane::Predicted {
            controlled_predicted = Some((entity.position_xy, entity.rotation_rad));
        } else if entity.is_controlled && entity.lane == DebugEntityLane::ConfirmedGhost {
            controlled_confirmed_ghost = Some((entity.position_xy, entity.rotation_rad));
        }
    }

    if let Some((predicted_pos, predicted_rot)) = controlled_predicted
        && let Some((confirmed_pos, confirmed_rot)) = controlled_confirmed_ghost
    {
        let predicted_pos = overlay_world_position(predicted_pos, DebugEntityLane::Predicted);
        let confirmed_pos = overlay_world_position(confirmed_pos, DebugEntityLane::ConfirmedGhost);
        if predicted_pos.distance(confirmed_pos) > CONFIRMED_OVERLAY_POSITION_EPSILON_M
            || angle_delta_rad(predicted_rot, confirmed_rot)
                > CONFIRMED_OVERLAY_ROTATION_EPSILON_RAD
        {
            gizmos.line(predicted_pos, confirmed_pos, prediction_error_color);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_debug_velocity_arrow_mesh_system(
    debug_overlay: Res<'_, DebugOverlayState>,
    snapshot: Res<'_, DebugOverlaySnapshot>,
    mut arrow_queries: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, (&'_ mut Transform, &'_ mut Visibility), With<DebugVelocityArrowShaft>>,
            Query<
                '_,
                '_,
                (&'_ mut Transform, &'_ mut Visibility),
                With<DebugVelocityArrowHeadUpper>,
            >,
            Query<
                '_,
                '_,
                (&'_ mut Transform, &'_ mut Visibility),
                With<DebugVelocityArrowHeadLower>,
            >,
        ),
    >,
) {
    if !debug_overlay.enabled {
        if let Ok((_, mut visibility)) = arrow_queries.p0().single_mut() {
            *visibility = Visibility::Hidden;
        }
        if let Ok((_, mut visibility)) = arrow_queries.p1().single_mut() {
            *visibility = Visibility::Hidden;
        }
        if let Ok((_, mut visibility)) = arrow_queries.p2().single_mut() {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let Some(entity) = snapshot.entities.iter().find(|entity| {
        entity.is_controlled
            && entity.lane != DebugEntityLane::Auxiliary
            && entity.lane != DebugEntityLane::ConfirmedGhost
            && entity.velocity_xy.length() > 0.01
    }) else {
        if let Ok((_, mut visibility)) = arrow_queries.p0().single_mut() {
            *visibility = Visibility::Hidden;
        }
        if let Ok((_, mut visibility)) = arrow_queries.p1().single_mut() {
            *visibility = Visibility::Hidden;
        }
        if let Ok((_, mut visibility)) = arrow_queries.p2().single_mut() {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    let start = overlay_world_position(entity.position_xy, entity.lane);
    let velocity_world = entity.velocity_xy.extend(0.0) * VELOCITY_ARROW_SCALE;
    let len = velocity_world.length();
    if len <= 0.01 {
        if let Ok((_, mut visibility)) = arrow_queries.p0().single_mut() {
            *visibility = Visibility::Hidden;
        }
        if let Ok((_, mut visibility)) = arrow_queries.p1().single_mut() {
            *visibility = Visibility::Hidden;
        }
        if let Ok((_, mut visibility)) = arrow_queries.p2().single_mut() {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let direction = velocity_world / len;
    let direction_2d = direction.truncate();
    let angle = direction_2d.to_angle();
    let head_length = VELOCITY_ARROW_HEAD_LENGTH.min(len * 0.5);
    let shaft_length = (len - head_length * 0.55).max(0.15);
    let shaft_center = start + direction * (shaft_length * 0.5);
    let tip = start + direction * len;
    let head_center = tip - direction * (head_length * 0.5);

    // Keep the velocity arrow on a plain mesh path. The prior gizmo arrow path reintroduced
    // visible flashing during lane churn, while the mesh version stayed visually stable.
    let shaft_transform = Transform::from_translation(shaft_center)
        .with_rotation(Quat::from_rotation_z(angle))
        .with_scale(Vec3::new(shaft_length, VELOCITY_ARROW_SHAFT_THICKNESS, 1.0));
    let upper_head_transform = Transform::from_translation(head_center)
        .with_rotation(Quat::from_rotation_z(
            angle + VELOCITY_ARROW_HEAD_SPREAD_RAD,
        ))
        .with_scale(Vec3::new(head_length, VELOCITY_ARROW_HEAD_THICKNESS, 1.0));
    let lower_head_transform = Transform::from_translation(head_center)
        .with_rotation(Quat::from_rotation_z(
            angle - VELOCITY_ARROW_HEAD_SPREAD_RAD,
        ))
        .with_scale(Vec3::new(head_length, VELOCITY_ARROW_HEAD_THICKNESS, 1.0));

    if let Ok((mut transform, mut visibility)) = arrow_queries.p0().single_mut() {
        *transform = shaft_transform;
        *visibility = Visibility::Visible;
    }
    if let Ok((mut transform, mut visibility)) = arrow_queries.p1().single_mut() {
        *transform = upper_head_transform;
        *visibility = Visibility::Visible;
    }
    if let Ok((mut transform, mut visibility)) = arrow_queries.p2().single_mut() {
        *transform = lower_head_transform;
        *visibility = Visibility::Visible;
    }
}

#[derive(Clone)]
struct RootDebugCandidate {
    overlay_entity: DebugOverlayEntity,
    is_replicated: bool,
    is_interpolated: bool,
    is_predicted: bool,
    interpolated_ready: bool,
    has_confirmed_wrappers: bool,
    confirmed_pose: Option<ConfirmedGhostPose>,
    confirmed_tick: Option<u16>,
}

#[derive(Clone)]
struct AuxiliaryDebugCandidate {
    guid: uuid::Uuid,
    parent_root_guid: uuid::Uuid,
    overlay_entity: DebugOverlayEntity,
    is_replicated: bool,
    is_interpolated: bool,
    is_predicted: bool,
    interpolated_ready: bool,
}

#[derive(Clone, Copy)]
struct ConfirmedGhostPose {
    position_xy: Vec2,
    rotation_rad: f32,
}

struct ResolvedRootCandidates<'a> {
    primary: Option<&'a RootDebugCandidate>,
    primary_lane: DebugEntityLane,
    confirmed_ghost: Option<RootDebugCandidate>,
}

fn build_collision_shape(
    size_m: Option<&SizeM>,
    collision_aabb: Option<&CollisionAabbM>,
    collision_outline: Option<&CollisionOutlineM>,
    is_hardpoint: bool,
) -> DebugCollisionShape {
    if is_hardpoint {
        return DebugCollisionShape::HardpointMarker;
    }
    if let Some(outline) = collision_outline {
        return DebugCollisionShape::Outline {
            points: outline.points.clone(),
        };
    }
    collision_aabb
        .map(|aabb| DebugCollisionShape::Aabb {
            half_extents: aabb.half_extents,
        })
        .or_else(|| {
            size_m.map(|size| DebugCollisionShape::Aabb {
                half_extents: Vec3::new(size.width * 0.5, size.length * 0.5, size.height * 0.5),
            })
        })
        .unwrap_or(DebugCollisionShape::None)
}

fn debug_overlay_candidate_visible(visibility: Option<&Visibility>) -> bool {
    !matches!(visibility, Some(Visibility::Hidden))
}

fn resolve_root_candidates(candidates: &[RootDebugCandidate]) -> ResolvedRootCandidates<'_> {
    let controlled = candidates
        .iter()
        .any(|candidate| candidate.overlay_entity.is_controlled);
    let primary = if controlled {
        pick_best_candidate(candidates, |candidate| candidate.is_predicted)
            .or_else(|| pick_best_candidate(candidates, root_candidate_is_confirmed_lane))
            .or_else(|| {
                pick_best_candidate(candidates, |candidate| {
                    candidate.is_interpolated && candidate.interpolated_ready
                })
            })
            .or_else(|| pick_best_candidate(candidates, |candidate| candidate.is_interpolated))
    } else {
        pick_best_candidate(candidates, |candidate| {
            candidate.is_interpolated && candidate.interpolated_ready
        })
        .or_else(|| pick_best_candidate(candidates, root_candidate_is_confirmed_lane))
        .or_else(|| pick_best_candidate(candidates, |candidate| candidate.is_predicted))
        .or_else(|| pick_best_candidate(candidates, |candidate| candidate.is_interpolated))
    };

    let primary_lane = primary
        .map(|candidate| candidate_primary_lane(candidate, controlled))
        .unwrap_or(DebugEntityLane::Confirmed);
    let confirmed_ghost = if controlled {
        primary.and_then(build_confirmed_ghost_entity)
    } else {
        None
    };

    ResolvedRootCandidates {
        primary,
        primary_lane,
        confirmed_ghost,
    }
}

fn pick_best_candidate(
    candidates: &[RootDebugCandidate],
    predicate: impl Fn(&RootDebugCandidate) -> bool,
) -> Option<&RootDebugCandidate> {
    candidates
        .iter()
        .filter(|candidate| predicate(candidate))
        .min_by_key(|candidate| candidate.overlay_entity.entity.to_bits())
}

fn root_candidate_is_confirmed_lane(candidate: &RootDebugCandidate) -> bool {
    candidate.is_replicated && !candidate.is_predicted && !candidate.is_interpolated
}

fn resolve_auxiliary_candidate<'a>(
    candidates: &'a [AuxiliaryDebugCandidate],
    resolved_root_lanes: &HashMap<uuid::Uuid, DebugEntityLane>,
) -> Option<&'a AuxiliaryDebugCandidate> {
    let parent_lane = candidates
        .first()
        .and_then(|candidate| resolved_root_lanes.get(&candidate.parent_root_guid))
        .copied()
        .unwrap_or(DebugEntityLane::Confirmed);

    pick_best_auxiliary_candidate(candidates, |candidate| match parent_lane {
        DebugEntityLane::Predicted => candidate.is_predicted,
        DebugEntityLane::Interpolated => candidate.is_interpolated && candidate.interpolated_ready,
        DebugEntityLane::Confirmed
        | DebugEntityLane::ConfirmedGhost
        | DebugEntityLane::Auxiliary => auxiliary_candidate_is_confirmed_lane(candidate),
    })
    .or_else(|| pick_best_auxiliary_candidate(candidates, |candidate| candidate.is_predicted))
    .or_else(|| pick_best_auxiliary_candidate(candidates, auxiliary_candidate_is_confirmed_lane))
    .or_else(|| {
        pick_best_auxiliary_candidate(candidates, |candidate| {
            candidate.is_interpolated && candidate.interpolated_ready
        })
    })
    .or_else(|| pick_best_auxiliary_candidate(candidates, |candidate| candidate.is_interpolated))
    .or_else(|| pick_best_auxiliary_candidate(candidates, |candidate| candidate.is_replicated))
}

fn pick_best_auxiliary_candidate(
    candidates: &[AuxiliaryDebugCandidate],
    predicate: impl Fn(&AuxiliaryDebugCandidate) -> bool,
) -> Option<&AuxiliaryDebugCandidate> {
    candidates
        .iter()
        .filter(|candidate| predicate(candidate))
        .min_by_key(|candidate| candidate.overlay_entity.entity.to_bits())
}

fn auxiliary_candidate_is_confirmed_lane(candidate: &AuxiliaryDebugCandidate) -> bool {
    candidate.is_replicated && !candidate.is_predicted && !candidate.is_interpolated
}

fn candidate_primary_lane(candidate: &RootDebugCandidate, controlled: bool) -> DebugEntityLane {
    if controlled {
        if candidate.is_predicted {
            DebugEntityLane::Predicted
        } else {
            DebugEntityLane::Confirmed
        }
    } else if candidate.is_interpolated {
        DebugEntityLane::Interpolated
    } else {
        DebugEntityLane::Confirmed
    }
}

fn build_confirmed_ghost_entity(primary: &RootDebugCandidate) -> Option<RootDebugCandidate> {
    if primary.is_predicted {
        return primary.confirmed_pose.map(|pose| {
            let mut overlay_entity = primary.overlay_entity.clone();
            overlay_entity.position_xy = pose.position_xy;
            overlay_entity.rotation_rad = pose.rotation_rad;
            overlay_entity.velocity_xy = Vec2::ZERO;
            overlay_entity.angular_velocity_rps = 0.0;
            RootDebugCandidate {
                overlay_entity,
                is_replicated: true,
                is_interpolated: false,
                is_predicted: false,
                interpolated_ready: false,
                has_confirmed_wrappers: true,
                confirmed_pose: None,
                confirmed_tick: primary.confirmed_tick,
            }
        });
    }

    if primary.is_replicated && !primary.has_confirmed_wrappers {
        let mut overlay_entity = primary.overlay_entity.clone();
        overlay_entity.velocity_xy = Vec2::ZERO;
        overlay_entity.angular_velocity_rps = 0.0;
        return Some(RootDebugCandidate {
            overlay_entity,
            is_replicated: true,
            is_interpolated: false,
            is_predicted: false,
            interpolated_ready: false,
            has_confirmed_wrappers: false,
            confirmed_pose: None,
            confirmed_tick: primary.confirmed_tick,
        });
    }

    None
}

fn push_snapshot_entity(
    snapshot: &mut DebugOverlaySnapshot,
    overlay_entity: &DebugOverlayEntity,
    lane: DebugEntityLane,
) {
    let mut overlay_entity = overlay_entity.clone();
    overlay_entity.lane = lane;
    match lane {
        DebugEntityLane::Predicted => snapshot.stats.predicted_count += 1,
        DebugEntityLane::Confirmed | DebugEntityLane::ConfirmedGhost => {
            snapshot.stats.confirmed_count += 1;
        }
        DebugEntityLane::Interpolated => snapshot.stats.interpolated_count += 1,
        DebugEntityLane::Auxiliary => snapshot.stats.auxiliary_count += 1,
    }
    snapshot.entities.push(overlay_entity);
}

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
        _guid: uuid::Uuid,
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
                lane: DebugEntityLane::Auxiliary,
                position_xy: Vec2::new(raw as f32, 0.0),
                rotation_rad: 0.0,
                velocity_xy: Vec2::ZERO,
                angular_velocity_rps: 0.0,
                collision: DebugCollisionShape::None,
                is_controlled,
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
                lane: DebugEntityLane::Auxiliary,
                position_xy: Vec2::new(raw as f32, 0.0),
                rotation_rad: 0.0,
                velocity_xy: Vec2::ZERO,
                angular_velocity_rps: 0.0,
                collision: DebugCollisionShape::HardpointMarker,
                is_controlled: false,
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

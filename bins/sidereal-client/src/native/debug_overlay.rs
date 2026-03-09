//! F3 debug overlay: toggle, snapshot collection, and snapshot-driven gizmo drawing.

use avian2d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::math::Isometry2d;
use bevy::prelude::*;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prediction::correction::VisualCorrection;
use lightyear::prediction::prelude::{PredictionHistory, PredictionManager};
use sidereal_game::{
    CollisionAabbM, CollisionOutlineM, EntityGuid, Hardpoint, MountedOn, ParentGuid, PlayerTag,
    SizeM,
};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use std::collections::HashMap;

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::components::{ControlledEntity, SuppressedPredictedDuplicateVisual, WorldEntity};
use super::dev_console::{DevConsoleState, is_console_open};
use super::resources::{
    BootstrapWatchdogState, DebugCollisionShape, DebugControlledLane, DebugEntityLane,
    DebugOverlayEntity, DebugOverlaySnapshot, DebugOverlayState, DebugOverlayStats, DebugSeverity,
    DebugTextRow, DeferredPredictedAdoptionState, LocalSimulationDebugMode,
    PredictionBootstrapTuning, PredictionLifecycleAuditConfig, PredictionLifecycleAuditState,
};

const DEBUG_OVERLAY_Z_OFFSET: f32 = 6.0;
const REPLICATED_OVERLAY_Z_STEP: f32 = 0.0;
const INTERPOLATED_OVERLAY_Z_STEP: f32 = 0.18;
const PREDICTED_OVERLAY_Z_STEP: f32 = 0.36;
const CONFIRMED_OVERLAY_Z_STEP: f32 = 0.54;
const CONFIRMED_OVERLAY_POSITION_EPSILON_M: f32 = 0.05;
const CONFIRMED_OVERLAY_ROTATION_EPSILON_RAD: f32 = 0.01;
const VELOCITY_ARROW_SCALE: f32 = 0.5;
const HARDPOINT_CROSS_HALF_SIZE: f32 = 2.0;

#[derive(Default)]
pub(crate) struct RollbackSampleState {
    last_active: bool,
    total_entries: u64,
    entries_since_log: u64,
    active_frames_since_log: u64,
    last_confirmed_tick: Option<u16>,
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

#[allow(clippy::type_complexity)]
pub(crate) fn collect_debug_overlay_snapshot_system(
    debug_overlay: Res<'_, DebugOverlayState>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
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
                Option<&'_ lightyear::prelude::Confirmed<Position>>,
                Option<&'_ lightyear::prelude::Confirmed<Rotation>>,
            ),
        ),
        With<WorldEntity>,
    >,
) {
    snapshot.frame_index = snapshot.frame_index.saturating_add(1);
    snapshot.entities.clear();
    snapshot.controlled_lane = None;
    snapshot.stats = DebugOverlayStats::default();
    snapshot.text_rows.clear();

    if !debug_overlay.enabled {
        return;
    }

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
        (mounted_on, hardpoint, parent_guid, player_tag, controlled_marker, _visibility),
        (
            is_replicated,
            is_interpolated,
            is_predicted,
            is_suppressed_duplicate,
            confirmed_position,
            confirmed_rotation,
        ),
    ) in &entities
    {
        if player_tag.is_some() {
            continue;
        }
        if is_suppressed_duplicate {
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
            velocity_xy: linear_velocity.map(|value| value.0).unwrap_or(Vec2::ZERO),
            angular_velocity_rps: angular_velocity.map(|value| value.0).unwrap_or_default(),
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
                has_confirmed_wrappers: confirmed_position.is_some()
                    && confirmed_rotation.is_some(),
                confirmed_pose: confirmed_position.zip(confirmed_rotation).map(
                    |(position, rotation)| ConfirmedGhostPose {
                        position_xy: position.0.0,
                        rotation_rad: rotation.0.as_radians(),
                    },
                ),
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

    let velocity_color = Color::srgb(0.2, 0.5, 1.0);
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

        if entity.is_controlled
            && entity.lane != DebugEntityLane::Auxiliary
            && entity.lane != DebugEntityLane::ConfirmedGhost
        {
            let len = entity.velocity_xy.length();
            if len > 0.01 {
                let end = pos + entity.velocity_xy.extend(0.0) * VELOCITY_ARROW_SCALE;
                gizmos.arrow(pos, end, velocity_color);
            }
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

#[derive(Clone)]
struct RootDebugCandidate {
    overlay_entity: DebugOverlayEntity,
    is_replicated: bool,
    is_interpolated: bool,
    is_predicted: bool,
    has_confirmed_wrappers: bool,
    confirmed_pose: Option<ConfirmedGhostPose>,
}

#[derive(Clone)]
struct AuxiliaryDebugCandidate {
    guid: uuid::Uuid,
    parent_root_guid: uuid::Uuid,
    overlay_entity: DebugOverlayEntity,
    is_replicated: bool,
    is_interpolated: bool,
    is_predicted: bool,
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

fn resolve_root_candidates(candidates: &[RootDebugCandidate]) -> ResolvedRootCandidates<'_> {
    let controlled = candidates
        .iter()
        .any(|candidate| candidate.overlay_entity.is_controlled);
    let primary = if controlled {
        pick_best_candidate(candidates, |candidate| candidate.is_predicted)
            .or_else(|| {
                pick_best_candidate(candidates, |candidate| {
                    candidate.is_replicated && !candidate.is_interpolated
                })
            })
            .or_else(|| pick_best_candidate(candidates, |candidate| candidate.is_interpolated))
    } else {
        pick_best_candidate(candidates, |candidate| candidate.is_interpolated)
            .or_else(|| {
                pick_best_candidate(candidates, |candidate| {
                    candidate.is_replicated && !candidate.is_predicted
                })
            })
            .or_else(|| pick_best_candidate(candidates, |candidate| candidate.is_predicted))
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
        DebugEntityLane::Interpolated => candidate.is_interpolated,
        DebugEntityLane::Confirmed
        | DebugEntityLane::ConfirmedGhost
        | DebugEntityLane::Auxiliary => {
            candidate.is_replicated && !candidate.is_predicted && !candidate.is_interpolated
        }
    })
    .or_else(|| pick_best_auxiliary_candidate(candidates, |candidate| candidate.is_predicted))
    .or_else(|| pick_best_auxiliary_candidate(candidates, |candidate| candidate.is_interpolated))
    .or_else(|| pick_best_auxiliary_candidate(candidates, |candidate| candidate.is_replicated))
}

fn pick_best_auxiliary_candidate<'a>(
    candidates: &'a [AuxiliaryDebugCandidate],
    predicate: impl Fn(&AuxiliaryDebugCandidate) -> bool,
) -> Option<&'a AuxiliaryDebugCandidate> {
    candidates
        .iter()
        .filter(|candidate| predicate(candidate))
        .min_by_key(|candidate| candidate.overlay_entity.entity.to_bits())
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
                has_confirmed_wrappers: true,
                confirmed_pose: None,
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
            has_confirmed_wrappers: false,
            confirmed_pose: None,
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
    let mut rows = vec![
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
            label: "Anomalies".to_string(),
            value: format!("{:>4}", stats.anomaly_count),
            severity: if stats.anomaly_count > 0 {
                DebugSeverity::Warn
            } else {
                DebugSeverity::Normal
            },
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

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(crate) fn log_prediction_runtime_state(
    time: Res<'_, Time>,
    tuning: Res<'_, PredictionBootstrapTuning>,
    local_mode: Res<'_, LocalSimulationDebugMode>,
    watchdog: Res<'_, BootstrapWatchdogState>,
    mut adoption_state: ResMut<'_, DeferredPredictedAdoptionState>,
    world_entities: Query<'_, '_, Entity, With<WorldEntity>>,
    replicated_entities: Query<'_, '_, Entity, With<lightyear::prelude::Replicated>>,
    predicted_entities: Query<'_, '_, Entity, With<lightyear::prelude::Predicted>>,
    interpolated_entities: Query<'_, '_, Entity, With<lightyear::prelude::Interpolated>>,
    controlled_entities: Query<'_, '_, Entity, With<ControlledEntity>>,
    interpolated_spatial_entities: Query<
        '_,
        '_,
        Entity,
        (With<lightyear::prelude::Interpolated>, With<Position>),
    >,
    interpolation_history_probe: Query<
        '_,
        '_,
        Has<ConfirmedHistory<Position>>,
        (With<lightyear::prelude::Interpolated>, With<Position>),
    >,
    controlled_prediction_state: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
            Option<&'_ lightyear::prelude::Confirmed<Position>>,
            Option<&'_ lightyear::prelude::Confirmed<Rotation>>,
            Option<&'_ lightyear::prelude::Confirmed<LinearVelocity>>,
            Option<&'_ lightyear::prelude::ConfirmedTick>,
            Has<PredictionHistory<Position>>,
            Has<PredictionHistory<Rotation>>,
            Has<PredictionHistory<LinearVelocity>>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ VisualCorrection<Isometry2d>>,
        ),
        With<ControlledEntity>,
    >,
    prediction_managers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ PredictionManager,
            Has<lightyear::prelude::client::Client>,
            Has<lightyear::prelude::Rollback>,
        ),
    >,
    mut rollback_sample: Local<'_, RollbackSampleState>,
) {
    let now_s = time.elapsed_secs_f64();
    let mut rollback_active = false;
    for (_, manager, _, _) in &prediction_managers {
        rollback_active |= manager.is_rollback();
    }
    if rollback_active {
        rollback_sample.active_frames_since_log =
            rollback_sample.active_frames_since_log.saturating_add(1);
        if !rollback_sample.last_active {
            rollback_sample.total_entries = rollback_sample.total_entries.saturating_add(1);
            rollback_sample.entries_since_log = rollback_sample.entries_since_log.saturating_add(1);
        }
    }
    rollback_sample.last_active = rollback_active;

    if now_s - adoption_state.last_runtime_summary_at_s < tuning.defer_summary_interval_s {
        return;
    }
    adoption_state.last_runtime_summary_at_s = now_s;
    let world_count = world_entities.iter().count();
    let replicated_count = replicated_entities.iter().count();
    let predicted_count = predicted_entities.iter().count();
    let interpolated_count = interpolated_entities.iter().count();
    let controlled_count = controlled_entities.iter().count();
    let manager_count = prediction_managers.iter().count();
    let manager_with_client_count = prediction_managers
        .iter()
        .filter(|(_, _, has_client, _)| *has_client)
        .count();
    let manager_with_rollback_count = prediction_managers
        .iter()
        .filter(|(_, _, _, has_rollback)| *has_rollback)
        .count();
    let manager_state = prediction_managers
        .iter()
        .next()
        .map(|(entity, manager, _, _)| {
            (
                entity,
                manager.rollback_policy.state,
                manager.rollback_policy.input,
                manager.rollback_policy.max_rollback_ticks,
                manager.get_rollback_start_tick().map(|tick| tick.0),
            )
        });
    let mode = if local_mode.0 { "local" } else { "predicted" };
    bevy::log::info!(
        "prediction runtime summary mode={} world={} replicated={} predicted={} interpolated={} controlled={} rollback_active={} rollback_entries_since_log={} rollback_entries_total={} rollback_active_frames_since_log={} managers={} managers_with_client={} managers_with_rollback={} manager_entity={:?} rollback_policy={:?}/{:?} rollback_max_ticks={} rollback_start_tick={:?} deferred_waiting={}",
        mode,
        world_count,
        replicated_count,
        predicted_count,
        interpolated_count,
        controlled_count,
        rollback_active,
        rollback_sample.entries_since_log,
        rollback_sample.total_entries,
        rollback_sample.active_frames_since_log,
        manager_count,
        manager_with_client_count,
        manager_with_rollback_count,
        manager_state.map(|v| v.0),
        manager_state.map(|v| v.1),
        manager_state.map(|v| v.2),
        manager_state.map(|v| v.3).unwrap_or_default(),
        manager_state.and_then(|v| v.4),
        adoption_state
            .waiting_entity_id
            .as_deref()
            .unwrap_or("<none>")
    );
    let interpolated_spatial_count = interpolated_spatial_entities.iter().count();
    let interp_with_history = interpolation_history_probe
        .iter()
        .filter(|has| *has)
        .count();
    let interp_without_history = interpolated_spatial_count.saturating_sub(interp_with_history);
    if interpolated_count > 0 {
        bevy::log::info!(
            "interpolation pipeline: interpolated={} spatial={} with_confirmed_history={} missing_history={}",
            interpolated_count,
            interpolated_spatial_count,
            interp_with_history,
            interp_without_history,
        );
    }
    if let Ok((
        controlled_entity,
        guid,
        is_predicted_marker,
        is_interpolated_marker,
        confirmed_position,
        confirmed_rotation,
        confirmed_velocity,
        confirmed_tick,
        has_position_history,
        has_rotation_history,
        has_velocity_history,
        current_position,
        current_rotation,
        current_velocity,
        visual_correction,
    )) = controlled_prediction_state.single()
    {
        let confirmed_tick_value = confirmed_tick.map(|tick| tick.tick.0);
        let confirmed_tick_advanced = confirmed_tick_value
            .zip(rollback_sample.last_confirmed_tick)
            .is_some_and(|(current, previous)| current != previous);
        let correction_translation_magnitude =
            visual_correction.map(|value| value.error.translation.length());
        let correction_rotation_rad =
            visual_correction.map(|value| value.error.rotation.as_radians());
        bevy::log::info!(
            "prediction controlled entity={} guid={} predicted_marker={} interpolated_marker={} confirmed_pos={} confirmed_rot={} confirmed_vel={} confirmed_tick={:?} confirmed_tick_advanced={} hist_pos={} hist_rot={} hist_vel={} current_pos={:?} current_rot_rad={:?} current_vel={:?} visual_correction_active={} visual_correction_translation_m={:?} visual_correction_rotation_rad={:?}",
            controlled_entity,
            guid.map(|v| v.0.to_string())
                .unwrap_or_else(|| "<none>".to_string()),
            is_predicted_marker,
            is_interpolated_marker,
            confirmed_position.is_some(),
            confirmed_rotation.is_some(),
            confirmed_velocity.is_some(),
            confirmed_tick_value,
            confirmed_tick_advanced,
            has_position_history,
            has_rotation_history,
            has_velocity_history,
            current_position.map(|v| v.0),
            current_rotation.map(|v| v.as_radians()),
            current_velocity.map(|v| v.0),
            visual_correction.is_some(),
            correction_translation_magnitude,
            correction_rotation_rad,
        );
        if !is_predicted_marker && is_interpolated_marker {
            bevy::log::warn!(
                "prediction runtime anomaly: controlled entity {} is interpolated instead of predicted; local motion should stay disabled until a Predicted clone exists",
                controlled_entity
            );
        }
        rollback_sample.last_confirmed_tick = confirmed_tick_value;
    }
    rollback_sample.entries_since_log = 0;
    rollback_sample.active_frames_since_log = 0;
    if !local_mode.0 && watchdog.replication_state_seen {
        let in_world_age_s = watchdog
            .in_world_entered_at_s
            .map(|entered_at_s| (now_s - entered_at_s).max(0.0))
            .unwrap_or_default();
        if in_world_age_s > tuning.defer_dialog_after_s && controlled_count == 0 {
            bevy::log::warn!(
                "prediction runtime anomaly: no controlled entity after {:.2}s in predicted mode (replicated={} predicted={} interpolated={})",
                in_world_age_s,
                replicated_count,
                predicted_count,
                interpolated_count
            );
        }
        if replicated_count > 0 && predicted_count == 0 {
            bevy::log::warn!(
                "prediction runtime anomaly: replicated entities present but zero Predicted markers (replicated={} interpolated={})",
                replicated_count,
                interpolated_count
            );
        }
        if let Some(missing_predicted) = adoption_state
            .missing_predicted_control_entity_id
            .as_deref()
        {
            bevy::log::warn!(
                "prediction runtime waiting for predicted control clone for {}",
                missing_predicted
            );
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn audit_prediction_entity_lifecycle(
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    config: Res<'_, PredictionLifecycleAuditConfig>,
    mut state: ResMut<'_, PredictionLifecycleAuditState>,
    world_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
            Has<ControlledEntity>,
            Has<super::components::SuppressedPredictedDuplicateVisual>,
            &'_ Visibility,
            Option<&'_ Transform>,
        ),
        With<WorldEntity>,
    >,
) {
    if !config.enabled {
        return;
    }
    let now = time.elapsed_secs_f64();
    if now - state.last_logged_at_s < config.interval_s {
        return;
    }
    state.last_logged_at_s = now;

    let target_guid = config.target_guid.or_else(|| {
        player_view_state
            .controlled_entity_id
            .as_deref()
            .and_then(sidereal_runtime_sync::parse_guid_from_entity_id)
            .or_else(|| {
                session
                    .player_entity_id
                    .as_deref()
                    .and_then(sidereal_runtime_sync::parse_guid_from_entity_id)
            })
    });
    let Some(target_guid) = target_guid else {
        return;
    };

    let mut lines = Vec::new();
    for (
        entity,
        guid,
        is_predicted,
        is_interpolated,
        is_controlled,
        is_suppressed,
        visibility,
        transform,
    ) in &world_entities
    {
        if guid.0 != target_guid {
            continue;
        }
        let pos = transform.map(|value| value.translation.truncate());
        lines.push(format!(
            "entity={entity:?} predicted={is_predicted} interpolated={is_interpolated} controlled={is_controlled} suppressed={is_suppressed} visibility={visibility:?} pos={pos:?}"
        ));
    }
    if lines.is_empty() {
        info!(
            "lifecycle_audit guid={} no runtime entity candidates",
            target_guid
        );
        return;
    }
    info!(
        "lifecycle_audit guid={} candidates={}",
        target_guid,
        lines.join(" | ")
    );
}

#[cfg(test)]
mod tests {
    use super::{
        AuxiliaryDebugCandidate, ConfirmedGhostPose, RootDebugCandidate, angle_delta_rad,
        resolve_auxiliary_candidate, resolve_root_candidates,
    };
    use crate::native::resources::{DebugCollisionShape, DebugEntityLane, DebugOverlayEntity};
    use bevy::prelude::*;
    use std::collections::HashMap;

    fn root_candidate(
        raw: u32,
        _guid: uuid::Uuid,
        is_controlled: bool,
        is_replicated: bool,
        is_interpolated: bool,
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
            has_confirmed_wrappers: confirmed_pose.is_some(),
            confirmed_pose,
        }
    }

    fn auxiliary_candidate(
        raw: u32,
        guid: uuid::Uuid,
        parent_root_guid: uuid::Uuid,
        is_replicated: bool,
        is_interpolated: bool,
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
        }
    }

    #[test]
    fn controlled_guid_prefers_predicted_lane_and_confirmed_ghost() {
        let guid = uuid::Uuid::nil();
        let candidates = vec![
            root_candidate(
                2,
                guid,
                true,
                true,
                false,
                true,
                Some(ConfirmedGhostPose {
                    position_xy: Vec2::new(10.0, 20.0),
                    rotation_rad: 0.3,
                }),
            ),
            root_candidate(1, guid, true, true, false, false, None),
        ];

        let resolved = resolve_root_candidates(&candidates);

        assert_eq!(resolved.primary_lane, DebugEntityLane::Predicted);
        assert_eq!(
            resolved.primary.unwrap().overlay_entity.entity,
            Entity::from_bits(2)
        );
        assert!(resolved.confirmed_ghost.is_some());
        assert_eq!(
            resolved.confirmed_ghost.unwrap().overlay_entity.position_xy,
            Vec2::new(10.0, 20.0)
        );
    }

    #[test]
    fn remote_guid_prefers_interpolated_over_confirmed() {
        let guid = uuid::Uuid::new_v4();
        let candidates = vec![
            root_candidate(4, guid, false, true, false, false, None),
            root_candidate(3, guid, false, true, true, false, None),
        ];

        let resolved = resolve_root_candidates(&candidates);

        assert_eq!(resolved.primary_lane, DebugEntityLane::Interpolated);
        assert_eq!(
            resolved.primary.unwrap().overlay_entity.entity,
            Entity::from_bits(3)
        );
        assert!(resolved.confirmed_ghost.is_none());
    }

    #[test]
    fn angle_delta_wraps_across_tau() {
        let delta = angle_delta_rad(0.05, std::f32::consts::TAU - 0.05);
        assert!(delta < 0.11, "delta was {delta}");
    }

    #[test]
    fn auxiliary_guid_follows_parent_predicted_lane() {
        let parent_guid = uuid::Uuid::new_v4();
        let child_guid = uuid::Uuid::new_v4();
        let mut resolved_root_lanes = HashMap::new();
        resolved_root_lanes.insert(parent_guid, DebugEntityLane::Predicted);
        let candidates = vec![
            auxiliary_candidate(4, child_guid, parent_guid, true, false, false),
            auxiliary_candidate(3, child_guid, parent_guid, true, false, true),
        ];

        let resolved = resolve_auxiliary_candidate(&candidates, &resolved_root_lanes)
            .expect("predicted auxiliary winner");

        assert_eq!(resolved.overlay_entity.entity, Entity::from_bits(3));
    }
}

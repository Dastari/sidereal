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

fn debug_overlay_entity_label(
    display_name: Option<&DisplayName>,
    entity_labels: Option<&EntityLabels>,
    hardpoint: Option<&Hardpoint>,
    mounted_on: Option<&MountedOn>,
    has_engine: bool,
    ballistic_weapon: Option<&BallisticWeapon>,
) -> String {
    if let Some(name) = display_name {
        let trimmed = name.0.trim();
        if !trimmed.is_empty() {
            return trimmed.to_ascii_uppercase();
        }
    }
    if let Some(weapon) = ballistic_weapon {
        let trimmed = weapon.weapon_name.trim();
        if !trimmed.is_empty() {
            return format!("WEAPON {trimmed}").to_ascii_uppercase();
        }
        return "WEAPON".to_string();
    }
    if has_engine {
        return "ENGINE".to_string();
    }
    if let Some(hardpoint) = hardpoint {
        let trimmed = hardpoint.hardpoint_id.trim();
        if !trimmed.is_empty() {
            return format!("HARDPOINT {trimmed}").to_ascii_uppercase();
        }
        return "HARDPOINT".to_string();
    }
    if let Some(labels) = entity_labels
        && let Some(label) = labels.0.iter().find(|label| !label.trim().is_empty())
    {
        return label.trim().to_ascii_uppercase();
    }
    if let Some(mounted_on) = mounted_on {
        let trimmed = mounted_on.hardpoint_id.trim();
        if !trimmed.is_empty() {
            return format!("MOUNT {trimmed}").to_ascii_uppercase();
        }
        return "MOUNTED COMPONENT".to_string();
    }
    "ENTITY".to_string()
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
    .or_else(|| {
        candidates
            .iter()
            .min_by_key(|candidate| candidate.overlay_entity.entity.to_bits())
    })
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


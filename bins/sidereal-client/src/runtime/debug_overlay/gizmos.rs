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

        if entity.is_component {
            draw_component_marker_square(&mut gizmos, pos, component_marker_color());
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

fn draw_component_marker_square(gizmos: &mut Gizmos<'_, '_>, center: Vec3, color: Color) {
    let half = COMPONENT_MARKER_HALF_SIZE;
    let z = center.z + 0.08;
    let corners = [
        Vec3::new(center.x - half, center.y - half, z),
        Vec3::new(center.x + half, center.y - half, z),
        Vec3::new(center.x + half, center.y + half, z),
        Vec3::new(center.x - half, center.y + half, z),
    ];
    for index in 0..corners.len() {
        gizmos.line(corners[index], corners[(index + 1) % corners.len()], color);
    }
}

fn component_marker_color() -> Color {
    Color::srgb(0.2, 1.0, 0.35)
}

fn hardpoint_debug_offset_m(hardpoint: &Hardpoint) -> Vec3 {
    let offset = hardpoint.offset_m;
    let longitudinal_m = if offset.z.abs() > offset.y.abs() {
        offset.z
    } else {
        offset.y
    };
    Vec3::new(offset.x, longitudinal_m, 0.0)
}


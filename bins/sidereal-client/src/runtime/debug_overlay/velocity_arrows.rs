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


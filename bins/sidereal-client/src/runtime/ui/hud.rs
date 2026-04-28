#[allow(clippy::type_complexity)]
pub(super) fn update_hud_system(
    mut fuel_baseline_by_parent: Local<'_, HashMap<uuid::Uuid, f32>>,
    controlled_query: Query<
        '_,
        '_,
        (
            &EntityGuid,
            &Transform,
            Option<&Rotation>,
            Option<&LinearVelocity>,
            Option<&HealthPool>,
        ),
        (With<ControlledEntity>, Without<GameplayCamera>),
    >,
    fuel_tank_query: Query<'_, '_, (&MountedOn, &FuelTank)>,
    camera_query: Query<'_, '_, &Transform, With<GameplayCamera>>,
    mut text_queries: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, &mut Text, With<HudSpeedValueText>>,
            Query<'_, '_, &mut Text, With<HudPositionValueText>>,
        ),
    >,
    mut bar_value_queries: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, &mut SegmentedBarValue, With<HudHealthBarFill>>,
            Query<'_, '_, &mut SegmentedBarValue, With<HudFuelBarFill>>,
        ),
    >,
) {
    let (pos, _heading_rad, vel, health_ratio, fuel_ratio) =
        if let Ok((guid, transform, maybe_rotation, maybe_velocity, maybe_health)) =
            controlled_query.single()
        {
            let vel = maybe_velocity.map_or(Vec2::ZERO, |velocity| velocity.0.as_vec2());
            let heading_rad = maybe_rotation
                .map(|rotation| rotation.as_radians() as f32)
                .unwrap_or_else(|| vel.to_angle());
            let health_ratio = if let Some(health) = maybe_health {
                if health.maximum > 0.0 {
                    (health.current / health.maximum).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let mut fuel_current = 0.0_f32;
            for (mounted_on, fuel_tank) in &fuel_tank_query {
                if mounted_on.parent_entity_id == guid.0 {
                    fuel_current += fuel_tank.fuel_kg.max(0.0);
                }
            }
            let baseline_entry = fuel_baseline_by_parent
                .entry(guid.0)
                .or_insert(fuel_current);
            *baseline_entry = baseline_entry.max(fuel_current);
            let fuel_capacity = (*baseline_entry).max(1.0);
            let fuel_ratio = if fuel_current > 0.0 || fuel_capacity > 1.0 {
                (fuel_current / fuel_capacity).clamp(0.0, 1.0)
            } else {
                0.0
            };

            (
                transform.translation,
                heading_rad,
                vel,
                health_ratio,
                fuel_ratio,
            )
        } else {
            let Ok(camera_transform) = camera_query.single() else {
                return;
            };
            (camera_transform.translation, 0.0, Vec2::ZERO, 0.0, 0.0)
        };
    let speed = vel.length();

    if let Ok(mut text) = text_queries.p0().single_mut() {
        text.0 = format!("{:.1} m/s", speed);
    }
    if let Ok(mut text) = text_queries.p1().single_mut() {
        text.0 = format!("SECTOR {}", format_sector_code(pos.x, pos.y));
    }
    if let Ok(mut fill) = bar_value_queries.p0().single_mut() {
        fill.ratio = health_ratio;
    }
    if let Ok(mut fill) = bar_value_queries.p1().single_mut() {
        fill.ratio = fuel_ratio;
    }
}


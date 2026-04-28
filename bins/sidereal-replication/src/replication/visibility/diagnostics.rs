fn motion_replication_diagnostics_enabled() -> bool {
    debug_env("SIDEREAL_DEBUG_MOTION_REPLICATION")
}

#[allow(clippy::type_complexity)]
pub fn log_motion_replication_diagnostics(
    time: Res<'_, Time>,
    mut log_state: ResMut<'_, MotionReplicationDiagnosticsLogState>,
    membership_cache: Res<'_, VisibilityMembershipCache>,
    entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Ref<'_, Position>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ AngularVelocity>,
            Option<&'_ Mass>,
            Option<&'_ AngularInertia>,
            Has<Replicate>,
            Option<&'_ ReplicationState>,
        ),
        Or<(With<SimulatedControlledEntity>, With<PlayerTag>)>,
    >,
) {
    if !motion_replication_diagnostics_enabled() {
        return;
    }
    const LOG_INTERVAL_S: f64 = 1.0;
    let now_s = time.elapsed_secs_f64();
    if now_s - log_state.last_logged_at_s < LOG_INTERVAL_S {
        return;
    }
    log_state.last_logged_at_s = now_s;

    for (
        entity,
        guid,
        position,
        rotation,
        linear_velocity,
        angular_velocity,
        mass,
        angular_inertia,
        has_replicate,
        replication_state,
    ) in &entities
    {
        let visible_clients = membership_cache
            .visible_clients(entity)
            .map(|clients| {
                let mut values = clients
                    .iter()
                    .map(|client| format!("{client:?}"))
                    .collect::<Vec<_>>();
                values.sort();
                values
            })
            .unwrap_or_default();
        let velocity = linear_velocity.map(|value| value.0);
        let rotation_rad = rotation.map(|value| value.as_radians());
        let angular_velocity_rad_s = angular_velocity.map(|value| value.0);
        let mass_kg = mass.map(|value| value.0);
        let angular_inertia_kg_m2 = angular_inertia.map(|value| value.0);
        info!(
            entity = ?entity,
            guid = %guid.0,
            position_x = position.0.x,
            position_y = position.0.y,
            position_changed = position.is_changed(),
            rotation_rad,
            velocity = ?velocity,
            angular_velocity_rad_s,
            mass_kg,
            angular_inertia_kg_m2,
            has_replicate,
            has_replication_state = replication_state.is_some(),
            visible_clients = ?visible_clients,
            "server motion replication diagnostic"
        );
    }
}


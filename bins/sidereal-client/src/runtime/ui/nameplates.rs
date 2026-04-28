#[allow(clippy::type_complexity)]
pub(super) fn update_segmented_bars_system(
    bar_roots: Query<'_, '_, (&SegmentedBarValue, &SegmentedBarStyle, &Children)>,
    mut segments: Query<'_, '_, (&SegmentedBarSegment, &mut BackgroundColor)>,
) {
    for (value, style, children) in &bar_roots {
        let seg_count = style.segments.max(1);
        let ratio = value.ratio.clamp(0.0, 1.0);
        let active_segments =
            ((ratio * seg_count as f32).round() as i32).clamp(0, seg_count as i32);
        for child in children.iter() {
            let Ok((segment, mut color)) = segments.get_mut(child) else {
                continue;
            };
            color.0 = if (segment.index as i32) < active_segments {
                style.active_color
            } else {
                style.inactive_color
            };
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_entity_nameplates_system(
    nameplate_state: Res<'_, NameplateUiState>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    mut commands: Commands<'_, '_>,
    mut hud_perf: ResMut<'_, HudPerfCounters>,
    mut registry: ResMut<'_, NameplateRegistry>,
    world_entities: Query<
        '_,
        '_,
        Entity,
        (
            With<WorldEntity>,
            With<CanonicalPresentationEntity>,
            With<HealthPool>,
        ),
    >,
    existing: Query<'_, '_, (Entity, &EntityNameplateRoot)>,
) {
    let started_at = Instant::now();
    hud_perf.nameplate_sync_runs = hud_perf.nameplate_sync_runs.saturating_add(1);
    hud_perf.nameplate_targets_last = 0;
    hud_perf.nameplate_spawned_last = 0;
    hud_perf.nameplate_despawned_last = 0;
    registry
        .active_by_target
        .retain(|_, entry| existing.get(entry.root).is_ok());
    registry
        .free_entries
        .retain(|entry| existing.get(entry.root).is_ok());

    if !nameplate_state.enabled || tactical_map_state.enabled {
        let released = registry
            .active_by_target
            .drain()
            .map(|(_, entry)| entry)
            .collect::<Vec<_>>();
        for entry in released {
            release_nameplate_entry(&mut commands, &mut registry, entry);
            hud_perf.nameplate_despawned_last = hud_perf.nameplate_despawned_last.saturating_add(1);
        }
        let elapsed_ms = elapsed_ms(started_at);
        hud_perf.nameplate_sync_last_ms = elapsed_ms;
        hud_perf.nameplate_sync_max_ms = hud_perf.nameplate_sync_max_ms.max(elapsed_ms);
        return;
    }

    let mut desired_targets = world_entities.iter().collect::<Vec<_>>();
    desired_targets.sort_unstable_by_key(|entity| entity.to_bits());
    let desired_target_set = desired_targets.iter().copied().collect::<HashSet<_>>();
    let stale_targets = registry
        .active_by_target
        .keys()
        .copied()
        .filter(|target| !desired_target_set.contains(target))
        .collect::<Vec<_>>();
    for target in stale_targets {
        if let Some(entry) = registry.active_by_target.remove(&target) {
            release_nameplate_entry(&mut commands, &mut registry, entry);
            hud_perf.nameplate_despawned_last = hud_perf.nameplate_despawned_last.saturating_add(1);
        }
    }

    for target in desired_targets {
        if registry.active_by_target.contains_key(&target) {
            continue;
        }
        let entry = registry.free_entries.pop().unwrap_or_else(|| {
            registry.allocated_entries = registry.allocated_entries.saturating_add(1);
            spawn_nameplate_entry(&mut commands)
        });
        if let Ok(mut root_commands) = commands.get_entity(entry.root) {
            root_commands.insert((
                ActiveNameplateEntry,
                EntityNameplateRoot {
                    target: Some(target),
                    health_fill: entry.health_fill,
                },
            ));
        }
        registry.active_by_target.insert(target, entry);
        hud_perf.nameplate_spawned_last = hud_perf.nameplate_spawned_last.saturating_add(1);
    }

    hud_perf.nameplate_targets_last = registry.active_by_target.len();
    let elapsed_ms = elapsed_ms(started_at);
    hud_perf.nameplate_sync_last_ms = elapsed_ms;
    hud_perf.nameplate_sync_max_ms = hud_perf.nameplate_sync_max_ms.max(elapsed_ms);
}

#[allow(clippy::type_complexity)]
pub(super) fn update_entity_nameplate_positions_system(
    nameplate_state: Res<'_, NameplateUiState>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    mut hud_perf: ResMut<'_, HudPerfCounters>,
    mut nameplate_nodes: ParamSet<
        '_,
        '_,
        (
            Query<
                '_,
                '_,
                (&EntityNameplateRoot, &mut Node, &mut Visibility),
                (With<ActiveNameplateEntry>, Without<WorldEntity>),
            >,
            Query<'_, '_, &'_ mut Node, With<EntityNameplateHealthFill>>,
        ),
    >,
    world_entities: Query<
        '_,
        '_,
        (
            &GlobalTransform,
            Option<&Visibility>,
            Option<&SizeM>,
            &HealthPool,
        ),
        (With<WorldEntity>, With<CanonicalPresentationEntity>),
    >,
    gameplay_camera: Query<'_, '_, (Entity, &Camera, &Transform), With<GameplayCamera>>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
) {
    let started_at = Instant::now();
    hud_perf.nameplate_position_runs = hud_perf.nameplate_position_runs.saturating_add(1);
    hud_perf.nameplate_camera_candidates_last = 0;
    hud_perf.nameplate_camera_active_last = 0;
    hud_perf.nameplate_entity_data_last = 0;
    hud_perf.nameplate_visible_last = 0;
    hud_perf.nameplate_hidden_last = 0;
    hud_perf.nameplate_health_updates_last = 0;
    hud_perf.nameplate_missing_target_last = 0;
    hud_perf.nameplate_projection_failures_last = 0;
    hud_perf.nameplate_viewport_culled_last = 0;
    if !nameplate_state.enabled || tactical_map_state.enabled {
        for (_, _, mut visibility) in &mut nameplate_nodes.p0() {
            *visibility = Visibility::Hidden;
            hud_perf.nameplate_hidden_last = hud_perf.nameplate_hidden_last.saturating_add(1);
        }
        let elapsed_ms = elapsed_ms(started_at);
        hud_perf.nameplate_position_last_ms = elapsed_ms;
        hud_perf.nameplate_position_max_ms = hud_perf.nameplate_position_max_ms.max(elapsed_ms);
        return;
    }

    let mut selected_camera: Option<(Entity, bool, &Camera, &Transform)> = None;
    for (entity, camera, transform) in &gameplay_camera {
        hud_perf.nameplate_camera_candidates_last =
            hud_perf.nameplate_camera_candidates_last.saturating_add(1);
        if camera.is_active {
            hud_perf.nameplate_camera_active_last =
                hud_perf.nameplate_camera_active_last.saturating_add(1);
        }
        let candidate = (entity, camera.is_active, camera, transform);
        if selected_camera.is_none_or(|(current_entity, current_active, _, _)| {
            if camera.is_active != current_active {
                return camera.is_active;
            }
            entity.to_bits() < current_entity.to_bits()
        }) {
            selected_camera = Some(candidate);
        }
    }
    let Some((_camera_entity, _camera_is_active, camera, camera_transform)) = selected_camera
    else {
        let elapsed_ms = elapsed_ms(started_at);
        hud_perf.nameplate_position_last_ms = elapsed_ms;
        hud_perf.nameplate_position_max_ms = hud_perf.nameplate_position_max_ms.max(elapsed_ms);
        return;
    };
    // This runs in `PostUpdate` after camera follow/interpolation and transform propagation.
    // Convert the current camera `Transform` directly so projection uses the final same-frame
    // gameplay camera state.
    let camera_global = GlobalTransform::from(*camera_transform);
    let Ok(window) = window_query.single() else {
        let elapsed_ms = elapsed_ms(started_at);
        hud_perf.nameplate_position_last_ms = elapsed_ms;
        hud_perf.nameplate_position_max_ms = hud_perf.nameplate_position_max_ms.max(elapsed_ms);
        return;
    };

    let mut pending_health_updates = Vec::new();
    for (root, mut node, mut visibility) in &mut nameplate_nodes.p0() {
        let Some(target) = root.target else {
            *visibility = Visibility::Hidden;
            hud_perf.nameplate_hidden_last = hud_perf.nameplate_hidden_last.saturating_add(1);
            hud_perf.nameplate_missing_target_last =
                hud_perf.nameplate_missing_target_last.saturating_add(1);
            continue;
        };
        let Ok((global_transform, maybe_visibility, size_m, health_pool)) =
            world_entities.get(target)
        else {
            *visibility = Visibility::Hidden;
            hud_perf.nameplate_hidden_last = hud_perf.nameplate_hidden_last.saturating_add(1);
            hud_perf.nameplate_missing_target_last =
                hud_perf.nameplate_missing_target_last.saturating_add(1);
            continue;
        };
        if maybe_visibility
            .is_some_and(|entity_visibility| *entity_visibility == Visibility::Hidden)
        {
            *visibility = Visibility::Hidden;
            hud_perf.nameplate_hidden_last = hud_perf.nameplate_hidden_last.saturating_add(1);
            continue;
        }
        hud_perf.nameplate_entity_data_last = hud_perf.nameplate_entity_data_last.saturating_add(1);
        let world_pos = global_transform.translation();
        let half_extent_world = size_m.map(|s| s.length * 0.5).unwrap_or(6.0);
        let center_world = Vec3::new(world_pos.x, world_pos.y, 0.0);
        let Ok(viewport_pos) = camera.world_to_viewport(&camera_global, center_world) else {
            *visibility = Visibility::Hidden;
            hud_perf.nameplate_hidden_last = hud_perf.nameplate_hidden_last.saturating_add(1);
            hud_perf.nameplate_projection_failures_last = hud_perf
                .nameplate_projection_failures_last
                .saturating_add(1);
            continue;
        };
        let top_world = Vec3::new(world_pos.x, world_pos.y + half_extent_world, 0.0);
        let Ok(top_viewport_pos) = camera.world_to_viewport(&camera_global, top_world) else {
            *visibility = Visibility::Hidden;
            hud_perf.nameplate_hidden_last = hud_perf.nameplate_hidden_last.saturating_add(1);
            hud_perf.nameplate_projection_failures_last = hud_perf
                .nameplate_projection_failures_last
                .saturating_add(1);
            continue;
        };
        // Hide plate once the entity itself is fully outside viewport bounds.
        // Center-only checks cause bars to linger at screen edges.
        let right_world = Vec3::new(world_pos.x + half_extent_world, world_pos.y, 0.0);
        let Ok(right_viewport_pos) = camera.world_to_viewport(&camera_global, right_world) else {
            *visibility = Visibility::Hidden;
            hud_perf.nameplate_hidden_last = hud_perf.nameplate_hidden_last.saturating_add(1);
            hud_perf.nameplate_projection_failures_last = hud_perf
                .nameplate_projection_failures_last
                .saturating_add(1);
            continue;
        };
        let extent_px_x = (right_viewport_pos.x - viewport_pos.x).abs().max(1.0);
        let extent_px_y = (top_viewport_pos.y - viewport_pos.y).abs().max(1.0);
        if viewport_pos.x < -extent_px_x
            || viewport_pos.x > window.width() + extent_px_x
            || viewport_pos.y < -extent_px_y
            || viewport_pos.y > window.height() + extent_px_y
        {
            *visibility = Visibility::Hidden;
            hud_perf.nameplate_hidden_last = hud_perf.nameplate_hidden_last.saturating_add(1);
            hud_perf.nameplate_viewport_culled_last =
                hud_perf.nameplate_viewport_culled_last.saturating_add(1);
            continue;
        }
        node.left = px(viewport_pos.x - NAMEPLATE_BAR_WIDTH_PX * 0.5);
        let entity_top_y_px = viewport_pos.y.min(top_viewport_pos.y);
        node.top = px(entity_top_y_px - NAMEPLATE_BAR_HEIGHT_PX - NAMEPLATE_VERTICAL_GAP_PX);
        *visibility = Visibility::Visible;
        hud_perf.nameplate_visible_last = hud_perf.nameplate_visible_last.saturating_add(1);

        let health_ratio = if health_pool.maximum > 0.0 {
            (health_pool.current / health_pool.maximum).clamp(0.0, 1.0)
        } else {
            0.0
        };
        pending_health_updates.push((root.health_fill, health_ratio));
    }

    for (health_fill, health_ratio) in pending_health_updates {
        if let Ok(mut fill_node) = nameplate_nodes.p1().get_mut(health_fill) {
            fill_node.width = percent(health_ratio * 100.0);
            hud_perf.nameplate_health_updates_last =
                hud_perf.nameplate_health_updates_last.saturating_add(1);
        }
    }
    let elapsed_ms = elapsed_ms(started_at);
    hud_perf.nameplate_position_last_ms = elapsed_ms;
    hud_perf.nameplate_position_max_ms = hud_perf.nameplate_position_max_ms.max(elapsed_ms);
}


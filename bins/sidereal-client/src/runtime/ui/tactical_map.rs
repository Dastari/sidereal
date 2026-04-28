pub(super) fn toggle_tactical_map_mode_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    mut tactical_map_state: ResMut<'_, TacticalMapUiState>,
) {
    if is_console_open(dev_console_state.as_deref()) {
        return;
    }
    if input.just_pressed(KeyCode::KeyM) {
        tactical_map_state.enabled = !tactical_map_state.enabled;
    }
}

pub(super) fn toggle_nameplates_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    mut nameplate_state: ResMut<'_, NameplateUiState>,
) {
    if is_console_open(dev_console_state.as_deref()) {
        return;
    }
    if input.just_pressed(KeyCode::KeyV) {
        nameplate_state.enabled = !nameplate_state.enabled;
    }
}

pub(super) fn sync_tactical_map_camera_zoom_system(
    mut tactical_map_state: ResMut<'_, TacticalMapUiState>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    mut camera_query: Query<'_, '_, &mut super::components::TopDownCamera, With<GameplayCamera>>,
    map_settings_query: Query<'_, '_, &'_ TacticalMapUiSettings>,
) {
    let suppress_for_console = is_console_open(dev_console_state.as_deref());
    let map_settings = map_settings_query
        .iter()
        .next()
        .cloned()
        .unwrap_or_default();
    let mut wheel_delta_y = 0.0f32;
    for event in mouse_wheel_events.read() {
        if suppress_for_console {
            continue;
        }
        let normalized = match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / 32.0,
        };
        wheel_delta_y += normalized.clamp(-4.0, 4.0);
    }
    if tactical_map_state.enabled && wheel_delta_y != 0.0 {
        let zoom_factor = (wheel_delta_y * map_settings.map_zoom_wheel_sensitivity).exp();
        tactical_map_state.target_map_zoom =
            (tactical_map_state.target_map_zoom * zoom_factor).clamp(0.005, 4.0);
    }

    let Ok(mut camera) = camera_query.single_mut() else {
        return;
    };
    let map_distance_m = map_settings.map_distance_m.max(camera.min_distance);
    let entering_map_mode = tactical_map_state.enabled && !tactical_map_state.was_enabled;
    let exiting_map_mode = !tactical_map_state.enabled && tactical_map_state.was_enabled;

    if entering_map_mode {
        tactical_map_state.last_non_map_target_distance = camera.target_distance;
        tactical_map_state.last_non_map_max_distance = camera.max_distance;
        tactical_map_state.transition_start_distance = camera.max_distance.max(camera.min_distance);
        tactical_map_state.transition_map_zoom_start =
            map_zoom_from_camera_distance(tactical_map_state.transition_start_distance);
        tactical_map_state.transition_map_zoom_end = map_zoom_from_camera_distance(map_distance_m);
        tactical_map_state.pan_offset_world = Vec2::ZERO;
        tactical_map_state.last_pan_cursor_px = None;
        tactical_map_state.map_zoom = tactical_map_state.transition_map_zoom_start;
        tactical_map_state.target_map_zoom = tactical_map_state.transition_map_zoom_end;
        camera.max_distance = camera.max_distance.max(map_distance_m);
        camera.target_distance = map_distance_m.clamp(camera.min_distance, camera.max_distance);
    } else if exiting_map_mode {
        tactical_map_state.last_pan_cursor_px = None;
        camera.max_distance = tactical_map_state
            .last_non_map_max_distance
            .max(camera.min_distance);
        camera.target_distance = tactical_map_state
            .last_non_map_target_distance
            .clamp(camera.min_distance, camera.max_distance);
    }

    tactical_map_state.was_enabled = tactical_map_state.enabled;
}

fn map_zoom_from_camera_distance(distance: f32) -> f32 {
    let ortho_scale = (distance * ORTHO_SCALE_PER_DISTANCE).max(0.0001);
    1.0 / ortho_scale
}

fn normalized_transition_progress(value: f32, start: f32, end: f32) -> f32 {
    let span = end - start;
    if span.abs() <= f32::EPSILON {
        return 1.0;
    }
    ((value - start) / span).clamp(0.0, 1.0)
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(super) fn update_tactical_map_overlay_system(
    perf_inputs: (Res<'_, Time>, ResMut<'_, HudPerfCounters>),
    mut tactical_map_state: ResMut<'_, TacticalMapUiState>,
    contacts_cache: Res<'_, TacticalContactsCache>,
    asset_io: (
        Res<'_, super::resources::AssetRootPath>,
        Res<'_, super::resources::AssetCacheAdapter>,
    ),
    asset_manager: Res<'_, LocalAssetManager>,
    mouse_buttons: Res<'_, ButtonInput<MouseButton>>,
    camera_motion: Res<'_, CameraMotionState>,
    windows: Query<'_, '_, &'_ Window, With<PrimaryWindow>>,
    mut commands: Commands<'_, '_>,
    mut svg_assets: ResMut<'_, Assets<Svg>>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut icon_cache: Local<'_, TacticalMapIconSvgCache>,
    mut smoothing_cache: Local<'_, TacticalContactSmoothingCache>,
    mut map_queries: ParamSet<
        '_,
        '_,
        (
            Query<
                '_,
                '_,
                (&'_ mut Camera, &'_ super::components::TopDownCamera),
                (With<GameplayCamera>, Without<UiOverlayCamera>),
            >,
            Query<
                '_,
                '_,
                &'_ mut Visibility,
                (
                    With<GameplayHud>,
                    Without<TacticalMapOverlayRoot>,
                    Without<EntityNameplateRoot>,
                ),
            >,
            Query<'_, '_, &'_ mut Camera, (With<UiOverlayCamera>, Without<GameplayCamera>)>,
            Query<
                '_,
                '_,
                (
                    Entity,
                    &'_ mut BackgroundColor,
                    &'_ mut Visibility,
                    &'_ Children,
                ),
                With<TacticalMapOverlayRoot>,
            >,
            Query<
                '_,
                '_,
                &'_ mut TextColor,
                (With<TacticalMapTitle>, Without<TacticalMapCursorText>),
            >,
            Query<
                '_,
                '_,
                (&'_ mut Text, &'_ mut TextColor),
                (With<TacticalMapCursorText>, Without<TacticalMapTitle>),
            >,
            Query<
                '_,
                '_,
                (&'_ Transform, Option<&'_ MapIcon>, Option<&'_ EntityGuid>),
                (
                    With<ControlledEntity>,
                    Without<RuntimeScreenOverlayPass>,
                    Without<TacticalMapMarkerDynamic>,
                ),
            >,
            Query<'_, '_, &'_ TacticalPresentationDefaults>,
        ),
    >,
    map_settings_query: Query<'_, '_, &'_ TacticalMapUiSettings>,
    mut dynamic_markers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ TacticalMapMarkerDynamic,
            &'_ mut Svg2d,
            &'_ mut Transform,
        ),
    >,
) {
    let (time, mut hud_perf) = perf_inputs;
    let started_at = Instant::now();
    hud_perf.tactical_overlay_runs = hud_perf.tactical_overlay_runs.saturating_add(1);
    hud_perf.tactical_contacts_last = contacts_cache.contacts_by_entity_id.len();
    hud_perf.tactical_markers_last = 0;
    hud_perf.tactical_marker_spawns_last = 0;
    hud_perf.tactical_marker_updates_last = 0;
    hud_perf.tactical_marker_despawns_last = 0;
    let (asset_root, cache_adapter) = asset_io;
    if icon_cache.reload_generation != asset_manager.reload_generation {
        *icon_cache = TacticalMapIconSvgCache::default();
        icon_cache.reload_generation = asset_manager.reload_generation;
    }
    let map_settings = map_settings_query
        .iter()
        .next()
        .cloned()
        .unwrap_or_default();
    let tactical_defaults = {
        let defaults_query = map_queries.p7();
        defaults_query.iter().next().cloned()
    };
    prewarm_tactical_map_marker_svgs(
        (&asset_manager, &asset_root.0, *cache_adapter),
        (&mut svg_assets, &mut meshes),
        &mut icon_cache,
        tactical_defaults.as_ref(),
        &contacts_cache,
        &map_queries.p6(),
    );
    let Ok(window) = windows.single() else {
        let elapsed_ms = elapsed_ms(started_at);
        hud_perf.tactical_overlay_last_ms = elapsed_ms;
        hud_perf.tactical_overlay_max_ms = hud_perf.tactical_overlay_max_ms.max(elapsed_ms);
        return;
    };
    let mut camera_distance = tactical_map_state.transition_start_distance;
    {
        let mut gameplay_cameras = map_queries.p0();
        for (mut camera, topdown) in &mut gameplay_cameras {
            camera_distance = topdown.distance;
            camera.is_active = !tactical_map_state.enabled || tactical_map_state.alpha < 0.995;
        }
    }
    {
        let mut gameplay_hud = map_queries.p1();
        for mut hud_visibility in &mut gameplay_hud {
            *hud_visibility = if tactical_map_state.enabled {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        }
    }
    {
        let mut ui_cameras = map_queries.p2();
        for mut camera in &mut ui_cameras {
            camera.clear_color = if tactical_map_state.enabled
                && tactical_map_state.alpha >= map_settings.overlay_takeover_alpha
            {
                ClearColorConfig::Custom(Color::srgb(
                    map_settings.background_color_rgb.x,
                    map_settings.background_color_rgb.y,
                    map_settings.background_color_rgb.z,
                ))
            } else {
                ClearColorConfig::None
            };
        }
    }

    let map_distance_m = map_settings.map_distance_m.max(1.0);
    let computed_alpha = normalized_transition_progress(
        camera_distance,
        tactical_map_state.transition_start_distance,
        map_distance_m,
    );
    let mut alpha = if tactical_map_state.enabled {
        computed_alpha.max(tactical_map_state.alpha)
    } else {
        computed_alpha.min(tactical_map_state.alpha)
    };
    if tactical_map_state.enabled && alpha >= 0.995 {
        alpha = 1.0;
    } else if !tactical_map_state.enabled && alpha <= 0.005 {
        alpha = 0.0;
    }
    tactical_map_state.alpha = alpha;

    {
        let mut roots = map_queries.p3();
        let Ok((_root_entity, mut root_bg, mut visibility, _children)) = roots.single_mut() else {
            let elapsed_ms = elapsed_ms(started_at);
            hud_perf.tactical_overlay_last_ms = elapsed_ms;
            hud_perf.tactical_overlay_max_ms = hud_perf.tactical_overlay_max_ms.max(elapsed_ms);
            return;
        };
        if alpha < 0.01 && !tactical_map_state.enabled {
            let mut despawned = 0usize;
            *visibility = Visibility::Hidden;
            for (marker, _, _, _) in &mut dynamic_markers {
                queue_despawn_if_exists(&mut commands, marker);
                despawned = despawned.saturating_add(1);
            }
            hud_perf.tactical_marker_despawns_last = despawned;
            let elapsed_ms = elapsed_ms(started_at);
            hud_perf.tactical_overlay_last_ms = elapsed_ms;
            hud_perf.tactical_overlay_max_ms = hud_perf.tactical_overlay_max_ms.max(elapsed_ms);
            return;
        }
        *visibility = Visibility::Visible;
        // Keep root node transparent so the shader-backed map grid remains visible.
        root_bg.0 = Color::srgba(0.03, 0.04, 0.08, 0.0);
    }
    for mut color in &mut map_queries.p4() {
        color.0 = Color::srgba(0.85, 0.92, 1.0, 0.95 * alpha);
    }

    let mut existing_marker_entities = HashMap::new();
    for (entity, marker, _, _) in &mut dynamic_markers {
        existing_marker_entities.insert(marker.key.clone(), entity);
    }
    let mut seen_marker_keys = HashSet::new();

    // Tactical lane updates are low cadence; smooth contact motion/heading per-frame.
    update_tactical_contact_smoothing_cache(
        &mut smoothing_cache,
        &contacts_cache,
        time.delta_secs(),
    );

    let transition_t = alpha * alpha * (3.0 - 2.0 * alpha);
    let transition_zoom = tactical_map_state
        .transition_map_zoom_start
        .lerp(tactical_map_state.transition_map_zoom_end, transition_t);
    tactical_map_state.map_zoom = if tactical_map_state.enabled && alpha >= 0.995 {
        tactical_map_state.map_zoom.lerp(
            tactical_map_state.target_map_zoom,
            1.0 - (-10.0 * time.delta_secs()).exp(),
        )
    } else {
        // During open/close transition, map zoom follows camera transition progress exactly.
        transition_zoom
    };
    let map_zoom = tactical_map_state.map_zoom.max(1e-6);
    // UI node absolute positions and cursor coordinates are in logical window space.
    let width = window.width();
    let height = window.height();
    let screen_center = Vec2::new(width * 0.5, height * 0.5);

    if tactical_map_state.enabled {
        if mouse_buttons.pressed(MouseButton::Left) {
            if let Some(cursor_px) = window.cursor_position() {
                if let Some(last_px) = tactical_map_state.last_pan_cursor_px {
                    let delta_px = cursor_px - last_px;
                    tactical_map_state.pan_offset_world +=
                        Vec2::new(-delta_px.x, delta_px.y) / map_zoom;
                }
                tactical_map_state.last_pan_cursor_px = Some(cursor_px);
            }
        } else {
            tactical_map_state.last_pan_cursor_px = None;
        }
    } else {
        tactical_map_state.last_pan_cursor_px = None;
    }
    let controlled_world_xy = map_queries
        .p6()
        .iter()
        .next()
        .map(|(transform, _, _)| transform.translation.truncate());
    let controlled_entity_guid = map_queries
        .p6()
        .iter()
        .next()
        .and_then(|(_, _, guid)| guid)
        .map(|guid| guid.0.to_string());
    let world_center_base = controlled_world_xy.unwrap_or(camera_motion.world_position_xy);
    let world_center = world_center_base + tactical_map_state.pan_offset_world;

    let world_to_screen = |xy: Vec2| -> Option<Vec2> {
        let px = screen_center.x + (xy.x - world_center.x) * map_zoom;
        let py = screen_center.y - (xy.y - world_center.y) * map_zoom;
        if px < -16.0 || py < -16.0 || px > width + 16.0 || py > height + 16.0 {
            return None;
        }
        Some(Vec2::new(px, py))
    };

    if let Ok((mut cursor_text_value, mut cursor_text_color)) = map_queries.p5().single_mut() {
        if let Some(cursor_px) = window.cursor_position() {
            let world_x = world_center.x + (cursor_px.x - screen_center.x) / map_zoom;
            let world_y = world_center.y - (cursor_px.y - screen_center.y) / map_zoom;
            cursor_text_value.0 = format!("{world_x:.2}, {world_y:.2}");
        } else {
            cursor_text_value.0 = "--, --".to_string();
        }
        cursor_text_color.0 = Color::srgba(0.85, 0.92, 1.0, 0.95 * alpha);
    }

    if let Some((controlled_transform, controlled_map_icon, _)) = map_queries.p6().iter().next()
        && let Some(screen_xy) = world_to_screen(controlled_transform.translation.truncate())
    {
        let base_asset_id = controlled_map_icon
            .map(|icon| icon.asset_id.as_str())
            .or_else(|| {
                tactical_defaults
                    .as_ref()
                    .and_then(|defaults| defaults.map_icon_asset_id_for_kind(Some("ship")))
            });
        if let Some(base_asset_id) = base_asset_id
            && let Some(svg_handle) = resolve_tactical_marker_svg(
                (&asset_manager, &asset_root.0, *cache_adapter),
                (&mut svg_assets, &mut meshes),
                &mut icon_cache,
                base_asset_id,
                TacticalMarkerColorRole::FriendlySelf,
            )
        {
            let marker_key = "self".to_string();
            seen_marker_keys.insert(marker_key.clone());
            let (_, _, heading_rad) = controlled_transform.rotation.to_euler(EulerRot::XYZ);
            let icon_scale = tactical_svg_marker_scale(&svg_assets, &svg_handle, map_zoom)
                * tactical_marker_scale_multiplier("ship");
            let base_translation = tactical_map_marker_translation(screen_xy, width, height, -8.5);
            let marker_translation = tactical_icon_centered_translation(
                &svg_assets,
                &svg_handle,
                icon_scale,
                heading_rad,
                base_translation,
            );
            let existing_entity = existing_marker_entities.remove(marker_key.as_str());
            if existing_entity.is_some() {
                hud_perf.tactical_marker_updates_last =
                    hud_perf.tactical_marker_updates_last.saturating_add(1);
            } else {
                hud_perf.tactical_marker_spawns_last =
                    hud_perf.tactical_marker_spawns_last.saturating_add(1);
            }
            upsert_tactical_map_marker(
                &mut commands,
                existing_entity,
                marker_key,
                svg_handle,
                marker_translation,
                icon_scale,
                heading_rad,
            );
        }
    }

    for contact in contacts_cache.contacts_by_entity_id.values() {
        if controlled_entity_guid
            .as_deref()
            .is_some_and(|guid| ids_refer_to_same_guid(guid, contact.entity_id.as_str()))
        {
            continue;
        }
        let (world, heading_rad) = smoothing_cache
            .tracks_by_entity_id
            .get(contact.entity_id.as_str())
            .map(|track| (track.render_pos, track.render_heading_rad))
            .unwrap_or((
                Vec2::new(contact.position_xy[0] as f32, contact.position_xy[1] as f32),
                contact.heading_rad as f32,
            ));
        let Some(screen_xy) = world_to_screen(world) else {
            continue;
        };
        let base_asset_id = contact.map_icon_asset_id.as_deref().or_else(|| {
            tactical_defaults.as_ref().and_then(|defaults| {
                defaults.map_icon_asset_id_for_kind(Some(contact.kind.as_str()))
            })
        });
        let Some(base_asset_id) = base_asset_id else {
            continue;
        };
        let color_role = TacticalMarkerColorRole::HostileContact;
        let Some(svg_handle) = resolve_tactical_marker_svg(
            (&asset_manager, &asset_root.0, *cache_adapter),
            (&mut svg_assets, &mut meshes),
            &mut icon_cache,
            base_asset_id,
            color_role,
        ) else {
            continue;
        };
        let icon_scale = tactical_svg_marker_scale(&svg_assets, &svg_handle, map_zoom)
            * tactical_marker_scale_multiplier(contact.kind.as_str());
        let base_translation = tactical_map_marker_translation(screen_xy, width, height, -8.4);
        let marker_translation = tactical_icon_centered_translation(
            &svg_assets,
            &svg_handle,
            icon_scale,
            heading_rad,
            base_translation,
        );
        let marker_key = contact.entity_id.clone();
        seen_marker_keys.insert(marker_key.clone());
        let existing_entity = existing_marker_entities.remove(marker_key.as_str());
        if existing_entity.is_some() {
            hud_perf.tactical_marker_updates_last =
                hud_perf.tactical_marker_updates_last.saturating_add(1);
        } else {
            hud_perf.tactical_marker_spawns_last =
                hud_perf.tactical_marker_spawns_last.saturating_add(1);
        }
        upsert_tactical_map_marker(
            &mut commands,
            existing_entity,
            marker_key,
            svg_handle,
            marker_translation,
            icon_scale,
            heading_rad,
        );
    }

    for (stale_key, entity) in existing_marker_entities {
        if !seen_marker_keys.contains(stale_key.as_str()) {
            queue_despawn_if_exists(&mut commands, entity);
            hud_perf.tactical_marker_despawns_last =
                hud_perf.tactical_marker_despawns_last.saturating_add(1);
        }
    }
    hud_perf.tactical_markers_last = seen_marker_keys.len();
    let elapsed_ms = elapsed_ms(started_at);
    hud_perf.tactical_overlay_last_ms = elapsed_ms;
    hud_perf.tactical_overlay_max_ms = hud_perf.tactical_overlay_max_ms.max(elapsed_ms);
}

fn upsert_tactical_map_marker(
    commands: &mut Commands<'_, '_>,
    existing: Option<Entity>,
    key: String,
    svg_handle: Handle<Svg>,
    translation: Vec3,
    icon_scale: f32,
    heading_rad: f32,
) {
    let transform = Transform {
        translation,
        scale: Vec3::splat(icon_scale),
        rotation: Quat::from_rotation_z(heading_rad),
    };

    if let Some(entity) = existing {
        commands.entity(entity).insert((
            Svg2d(svg_handle),
            transform,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        ));
        return;
    }

    commands.spawn((
        Svg2d(svg_handle),
        transform,
        RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        TacticalMapMarkerDynamic { key },
    ));
}

#[derive(Default)]
pub(super) struct TacticalContactSmoothingCache {
    tracks_by_entity_id: HashMap<String, TacticalContactSmoothingTrack>,
    last_contacts_revision: u64,
}

struct TacticalContactSmoothingTrack {
    render_pos: Vec2,
    target_pos: Vec2,
    velocity: Option<Vec2>,
    render_heading_rad: f32,
    target_heading_rad: f32,
}

fn update_tactical_contact_smoothing_cache(
    cache: &mut TacticalContactSmoothingCache,
    contacts_cache: &TacticalContactsCache,
    delta_secs: f32,
) {
    if cache.last_contacts_revision != contacts_cache.revision {
        let mut current_ids = HashSet::with_capacity(contacts_cache.contacts_by_entity_id.len());
        for (entity_id, contact) in &contacts_cache.contacts_by_entity_id {
            current_ids.insert(entity_id.clone());
            let target_pos =
                Vec2::new(contact.position_xy[0] as f32, contact.position_xy[1] as f32);
            let velocity = contact
                .velocity_xy
                .map(|v| Vec2::new(v[0] as f32, v[1] as f32));
            if let Some(track) = cache.tracks_by_entity_id.get_mut(entity_id.as_str()) {
                track.target_pos = target_pos;
                track.velocity = velocity;
                track.target_heading_rad = contact.heading_rad as f32;
            } else {
                cache.tracks_by_entity_id.insert(
                    entity_id.clone(),
                    TacticalContactSmoothingTrack {
                        render_pos: target_pos,
                        target_pos,
                        velocity,
                        render_heading_rad: contact.heading_rad as f32,
                        target_heading_rad: contact.heading_rad as f32,
                    },
                );
            }
        }
        cache
            .tracks_by_entity_id
            .retain(|entity_id, _| current_ids.contains(entity_id));
        cache.last_contacts_revision = contacts_cache.revision;
    }

    let dt = delta_secs.clamp(0.0, 0.25);
    if dt <= 0.0 {
        return;
    }
    let follow = 1.0 - (-TACTICAL_CONTACT_SMOOTHING_RATE * dt).exp();
    for track in cache.tracks_by_entity_id.values_mut() {
        let predicted_target = track
            .velocity
            .map(|v| track.target_pos + v * TACTICAL_CONTACT_PREDICTION_HORIZON_S)
            .unwrap_or(track.target_pos);
        track.render_pos = track.render_pos.lerp(predicted_target, follow);
        let heading_delta =
            shortest_angle_delta(track.render_heading_rad, track.target_heading_rad);
        track.render_heading_rad += heading_delta * follow;
    }
}

fn shortest_angle_delta(from: f32, to: f32) -> f32 {
    let mut delta = to - from;
    let two_pi = std::f32::consts::TAU;
    while delta > std::f32::consts::PI {
        delta -= two_pi;
    }
    while delta < -std::f32::consts::PI {
        delta += two_pi;
    }
    delta
}

fn tactical_map_marker_translation(
    screen_xy: Vec2,
    viewport_width_px: f32,
    viewport_height_px: f32,
    z: f32,
) -> Vec3 {
    Vec3::new(
        screen_xy.x - viewport_width_px * 0.5,
        viewport_height_px * 0.5 - screen_xy.y,
        z,
    )
}

fn tactical_svg_marker_scale(
    svg_assets: &Assets<Svg>,
    svg_handle: &Handle<Svg>,
    map_zoom_px_per_world: f32,
) -> f32 {
    let svg_height = svg_assets
        .get(svg_handle)
        .map(|svg| svg.size.y.max(1.0))
        .unwrap_or(16.0);
    let target_height_px = (TACTICAL_ICON_WORLD_HEIGHT_M * map_zoom_px_per_world).max(2.0);
    (target_height_px / svg_height).clamp(0.08, 12.0)
}

pub(super) fn tactical_marker_scale_multiplier(kind: &str) -> f32 {
    if kind.eq_ignore_ascii_case("planet") {
        TACTICAL_PLANET_ICON_SCALE_MULTIPLIER
    } else {
        1.0
    }
}

pub(super) fn tactical_icon_centered_translation(
    svg_assets: &Assets<Svg>,
    svg_handle: &Handle<Svg>,
    icon_scale: f32,
    heading_rad: f32,
    desired_center_translation: Vec3,
) -> Vec3 {
    let (svg_width, svg_height) = svg_assets
        .get(svg_handle)
        .map(|svg| (svg.size.x.max(1.0), svg.size.y.max(1.0)))
        .unwrap_or((16.0, 16.0));
    let local_center_from_origin =
        Vec2::new(svg_width * icon_scale * 0.5, -svg_height * icon_scale * 0.5);
    let rotation = Mat2::from_angle(heading_rad);
    let rotated_center_offset = rotation * local_center_from_origin;
    desired_center_translation - rotated_center_offset.extend(0.0)
}

#[derive(Default)]
pub(super) struct TacticalMapIconSvgCache {
    pub(super) reload_generation: u64,
    base_by_asset_id: HashMap<String, Handle<Svg>>,
    tinted_by_variant_key: HashMap<String, Handle<Svg>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum TacticalMarkerColorRole {
    FriendlySelf,
    HostileContact,
}

pub(super) fn tactical_marker_color(role: TacticalMarkerColorRole) -> Color {
    match role {
        TacticalMarkerColorRole::FriendlySelf => Color::srgb(0.22, 0.62, 1.0),
        TacticalMarkerColorRole::HostileContact => Color::srgb(1.0, 0.9, 0.34),
    }
}

fn tactical_marker_role_suffix(role: TacticalMarkerColorRole) -> &'static str {
    match role {
        TacticalMarkerColorRole::FriendlySelf => "self",
        TacticalMarkerColorRole::HostileContact => "contact",
    }
}

pub(super) fn resolve_tactical_marker_svg(
    asset_io: (
        &LocalAssetManager,
        &str,
        super::resources::AssetCacheAdapter,
    ),
    render_assets: (&mut Assets<Svg>, &mut Assets<Mesh>),
    cache: &mut TacticalMapIconSvgCache,
    base_asset_id: &str,
    role: TacticalMarkerColorRole,
) -> Option<Handle<Svg>> {
    resolve_tactical_marker_svg_with_color(
        asset_io,
        render_assets,
        cache,
        base_asset_id,
        tactical_marker_role_suffix(role),
        tactical_marker_color(role),
    )
}

pub(super) fn resolve_tactical_marker_svg_with_color(
    asset_io: (
        &LocalAssetManager,
        &str,
        super::resources::AssetCacheAdapter,
    ),
    render_assets: (&mut Assets<Svg>, &mut Assets<Mesh>),
    cache: &mut TacticalMapIconSvgCache,
    base_asset_id: &str,
    variant_suffix: &str,
    marker_color: Color,
) -> Option<Handle<Svg>> {
    let (asset_manager, asset_root, cache_adapter) = asset_io;
    let (svg_assets, meshes) = render_assets;
    let base_handle = if let Some(handle) = cache.base_by_asset_id.get(base_asset_id) {
        handle.clone()
    } else {
        let handle = super::assets::cached_svg_handle(
            base_asset_id,
            asset_manager,
            asset_root,
            cache_adapter,
            svg_assets,
            meshes,
        )?;
        cache
            .base_by_asset_id
            .insert(base_asset_id.to_string(), handle.clone());
        handle
    };

    let variant_key = format!("{base_asset_id}:{variant_suffix}");
    if let Some(variant) = cache.tinted_by_variant_key.get(&variant_key) {
        return Some(variant.clone());
    }

    let base_svg = svg_assets.get(&base_handle)?.clone();
    let mut tinted_svg = base_svg;
    for path in &mut tinted_svg.paths {
        path.color = marker_color;
    }
    tinted_svg.mesh = meshes.add(tinted_svg.tessellate());
    let tinted_handle = svg_assets.add(tinted_svg);
    cache
        .tinted_by_variant_key
        .insert(variant_key, tinted_handle.clone());
    Some(tinted_handle)
}

#[allow(clippy::type_complexity)]
fn prewarm_tactical_map_marker_svgs(
    asset_io: (
        &LocalAssetManager,
        &str,
        super::resources::AssetCacheAdapter,
    ),
    mut render_assets: (&mut Assets<Svg>, &mut Assets<Mesh>),
    cache: &mut TacticalMapIconSvgCache,
    tactical_defaults: Option<&TacticalPresentationDefaults>,
    contacts_cache: &TacticalContactsCache,
    controlled_entities: &Query<
        '_,
        '_,
        (&'_ Transform, Option<&'_ MapIcon>, Option<&'_ EntityGuid>),
        (
            With<ControlledEntity>,
            Without<RuntimeScreenOverlayPass>,
            Without<TacticalMapMarkerDynamic>,
        ),
    >,
) {
    let mut prewarmed_roles = HashSet::<(String, TacticalMarkerColorRole)>::new();
    if let Some(asset_id) = controlled_entities
        .iter()
        .next()
        .and_then(|(_, map_icon, _)| {
            map_icon.map(|icon| icon.asset_id.as_str()).or_else(|| {
                tactical_defaults
                    .and_then(|defaults| defaults.map_icon_asset_id_for_kind(Some("ship")))
            })
        })
        .map(ToString::to_string)
    {
        prewarm_tactical_map_marker_svg(
            asset_io,
            (&mut render_assets.0, &mut render_assets.1),
            cache,
            &mut prewarmed_roles,
            &asset_id,
            TacticalMarkerColorRole::FriendlySelf,
        );
    }

    for contact in contacts_cache.contacts_by_entity_id.values() {
        let Some(asset_id) = contact
            .map_icon_asset_id
            .as_deref()
            .or_else(|| {
                tactical_defaults.and_then(|defaults| {
                    defaults.map_icon_asset_id_for_kind(Some(contact.kind.as_str()))
                })
            })
            .map(ToString::to_string)
        else {
            continue;
        };
        prewarm_tactical_map_marker_svg(
            asset_io,
            (&mut render_assets.0, &mut render_assets.1),
            cache,
            &mut prewarmed_roles,
            &asset_id,
            TacticalMarkerColorRole::HostileContact,
        );
    }
}

fn prewarm_tactical_map_marker_svg(
    asset_io: (
        &LocalAssetManager,
        &str,
        super::resources::AssetCacheAdapter,
    ),
    render_assets: (&mut Assets<Svg>, &mut Assets<Mesh>),
    cache: &mut TacticalMapIconSvgCache,
    prewarmed_roles: &mut HashSet<(String, TacticalMarkerColorRole)>,
    asset_id: &str,
    role: TacticalMarkerColorRole,
) {
    if !prewarmed_roles.insert((asset_id.to_string(), role)) {
        return;
    }
    let _ = resolve_tactical_marker_svg(asset_io, render_assets, cache, asset_id, role);
}

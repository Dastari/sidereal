//! World HUD and owned-entity panel systems.

use avian2d::prelude::{LinearVelocity, Rotation};
use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::window::PrimaryWindow;
use sidereal_game::{
    EntityGuid, EntityLabels, FuelTank, HealthPool, MountedOn, SizeM, TacticalMapUiSettings,
};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use std::collections::{HashMap, HashSet};

use super::app_state::{
    ClientAppState, ClientSession, LocalPlayerViewState, OwnedEntitiesPanelState,
};
use super::assets::{LocalAssetManager, RuntimeAssetStreamIndicatorState};
use super::backdrop::TacticalMapOverlayMaterial;
use super::components::{
    ControlledEntity, GameplayCamera, GameplayHud, HudFuelBarFill, HudHealthBarFill,
    HudPositionValueText, HudSpeedValueText, LoadingOverlayRoot, LoadingOverlayText,
    LoadingProgressBarFill, OwnedEntitiesPanelAction, OwnedEntitiesPanelButton,
    OwnedEntitiesPanelRoot, SegmentedBarSegment, SegmentedBarStyle, SegmentedBarValue,
    ShipNameplateHealthBar, ShipNameplateRoot, SuppressedPredictedDuplicateVisual,
    TacticalMapCursorText, TacticalMapMarkerDynamic, TacticalMapOverlayRoot, TacticalMapTitle,
    TacticalMapScreenFxOverlay, UiOverlayCamera, UiOverlayLayer, WorldEntity,
};
use super::ecs_util::queue_despawn_if_exists;
use super::platform::{ORTHO_SCALE_PER_DISTANCE, UI_OVERLAY_RENDER_LAYER};
use super::resources::{
    CameraMotionState, ClientControlRequestState, EmbeddedFonts, OwnedAssetManifestCache,
    TacticalContactsCache, TacticalMapUiState,
};

/// Propagates the UI overlay render layer to all descendants of HUD roots so they are drawn
/// by the UI overlay camera (fixed scale) instead of the gameplay camera.
pub(super) fn propagate_ui_overlay_layer_system(
    mut commands: Commands,
    roots: Query<(Entity, &Children), With<UiOverlayLayer>>,
) {
    for (_entity, children) in &roots {
        for child in children.iter() {
            commands
                .entity(child)
                .try_insert((RenderLayers::layer(UI_OVERLAY_RENDER_LAYER), UiOverlayLayer));
        }
    }
}

pub(super) fn update_loading_overlay_system(
    asset_manager: Res<'_, LocalAssetManager>,
    mut overlay_query: Query<'_, '_, &mut Visibility, With<LoadingOverlayRoot>>,
    mut text_query: Query<'_, '_, (&mut Text, &mut TextColor), With<LoadingOverlayText>>,
    mut fill_query: Query<'_, '_, (&mut Node, &mut BackgroundColor), With<LoadingProgressBarFill>>,
) {
    let Ok((mut text, mut color)) = text_query.single_mut() else {
        return;
    };
    let Ok((mut fill_node, mut fill_color)) = fill_query.single_mut() else {
        return;
    };
    if asset_manager.bootstrap_complete() {
        if let Ok(mut visibility) = overlay_query.single_mut() {
            *visibility = Visibility::Hidden;
        }
        color.0.set_alpha(0.0);
        text.0 = "".to_string();
        fill_node.width = percent(0.0);
        fill_color.0.set_alpha(0.0);
        return;
    }
    if let Ok(mut visibility) = overlay_query.single_mut() {
        *visibility = Visibility::Visible;
    }
    let pct = (asset_manager.bootstrap_progress() * 100.0).round();
    fill_node.width = percent(pct.clamp(0.0, 100.0));
    fill_color.0.set_alpha(1.0);
    text.0 = if asset_manager.bootstrap_manifest_seen {
        format!("Loading assets... {}%", pct as i32)
    } else {
        "Waiting for asset manifest...".to_string()
    };
    color.0.set_alpha(1.0);
}

pub(super) fn update_runtime_stream_icon_system(
    time: Res<'_, Time>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut indicator_state: ResMut<'_, RuntimeAssetStreamIndicatorState>,
    mut text_query: Query<
        '_,
        '_,
        &mut TextColor,
        With<super::components::RuntimeStreamingIconText>,
    >,
) {
    let Ok(mut color) = text_query.single_mut() else {
        return;
    };
    if !asset_manager.should_show_runtime_stream_indicator() {
        color.0.set_alpha(0.0);
        indicator_state.blinking_phase_s = 0.0;
        return;
    }
    indicator_state.blinking_phase_s += time.delta_secs();
    let pulse = (indicator_state.blinking_phase_s * 8.0).sin().abs();
    color.0 = Color::srgba(0.3 + pulse * 0.7, 0.85, 1.0, 0.5 + pulse * 0.5);
}

pub(super) fn toggle_tactical_map_mode_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    mut tactical_map_state: ResMut<'_, TacticalMapUiState>,
) {
    if input.just_pressed(KeyCode::KeyM) {
        tactical_map_state.enabled = !tactical_map_state.enabled;
    }
}

pub(super) fn sync_tactical_map_camera_zoom_system(
    mut tactical_map_state: ResMut<'_, TacticalMapUiState>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    mut camera_query: Query<'_, '_, &mut super::components::TopDownCamera, With<GameplayCamera>>,
    map_settings_query: Query<'_, '_, &'_ TacticalMapUiSettings>,
) {
    let map_settings = map_settings_query
        .iter()
        .next()
        .copied()
        .unwrap_or_default();
    let mut wheel_delta_y = 0.0f32;
    for event in mouse_wheel_events.read() {
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
        camera.max_distance = tactical_map_state.last_non_map_max_distance.max(camera.min_distance);
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
pub(super) fn update_tactical_map_overlay_system(
    time: Res<'_, Time>,
    mut tactical_map_state: ResMut<'_, TacticalMapUiState>,
    contacts_cache: Res<'_, TacticalContactsCache>,
    mouse_buttons: Res<'_, ButtonInput<MouseButton>>,
    camera_motion: Res<'_, CameraMotionState>,
    windows: Query<'_, '_, &'_ Window, With<PrimaryWindow>>,
    mut commands: Commands<'_, '_>,
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
            Query<'_, '_, &'_ mut Visibility, (With<GameplayHud>, Without<TacticalMapOverlayRoot>)>,
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
            Query<'_, '_, &'_ mut TextColor, (With<TacticalMapTitle>, Without<TacticalMapCursorText>)>,
            Query<'_, '_, (&'_ mut Text, &'_ mut TextColor), (With<TacticalMapCursorText>, Without<TacticalMapTitle>)>,
            Query<'_, '_, &'_ Transform, (With<ControlledEntity>, Without<TacticalMapScreenFxOverlay>)>,
        ),
    >,
    map_settings_query: Query<'_, '_, &'_ TacticalMapUiSettings>,
    dynamic_markers: Query<'_, '_, Entity, With<TacticalMapMarkerDynamic>>,
) {
    let map_settings = map_settings_query
        .iter()
        .next()
        .copied()
        .unwrap_or_default();
    let Ok(window) = windows.single() else {
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

    let root_entity = {
        let mut roots = map_queries.p3();
        let Ok((root_entity, mut root_bg, mut visibility, _children)) = roots.single_mut() else {
            return;
        };
        if alpha < 0.01 && !tactical_map_state.enabled {
            *visibility = Visibility::Hidden;
            for marker in &dynamic_markers {
                queue_despawn_if_exists(&mut commands, marker);
            }
            return;
        }
        *visibility = Visibility::Visible;
        // Keep root node transparent so the shader-backed map grid remains visible.
        root_bg.0 = Color::srgba(0.03, 0.04, 0.08, 0.0);
        root_entity
    };
    for mut color in &mut map_queries.p4() {
        color.0 = Color::srgba(0.85, 0.92, 1.0, 0.95 * alpha);
    }

    for marker in &dynamic_markers {
        queue_despawn_if_exists(&mut commands, marker);
    }

    let transition_t = alpha * alpha * (3.0 - 2.0 * alpha);
    let transition_zoom = tactical_map_state.transition_map_zoom_start.lerp(
        tactical_map_state.transition_map_zoom_end,
        transition_t,
    );
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
        .map(|transform| transform.translation.truncate());
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

    if let Some(controlled_world_xy) = controlled_world_xy
        && let Some(screen_xy) = world_to_screen(controlled_world_xy)
    {
        commands.entity(root_entity).with_children(|parent| {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(screen_xy.x - 6.0),
                    top: px(screen_xy.y - 6.0),
                    width: px(12.0),
                    height: px(12.0),
                    border_radius: BorderRadius::all(px(6.0)),
                    border: UiRect::all(px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.16, 0.38, 0.74, 0.95 * alpha)),
                BorderColor::all(Color::srgba(0.85, 0.92, 1.0, 0.95 * alpha)),
                TacticalMapMarkerDynamic,
            ));
        });
    }

    for contact in contacts_cache.contacts_by_entity_id.values() {
        let world = Vec2::new(contact.position_xy[0], contact.position_xy[1]);
        let Some(screen_xy) = world_to_screen(world) else {
            continue;
        };
        let marker_color = if contact.is_live_now {
            Color::srgba(0.95, 0.96, 0.55, 0.9 * alpha)
        } else {
            Color::srgba(0.6, 0.62, 0.72, 0.68 * alpha)
        };
        commands.entity(root_entity).with_children(|parent| {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(screen_xy.x - 4.0),
                    top: px(screen_xy.y - 4.0),
                    width: px(8.0),
                    height: px(8.0),
                    border_radius: BorderRadius::all(px(4.0)),
                    border: UiRect::all(px(1.0)),
                    ..default()
                },
                BackgroundColor(marker_color),
                BorderColor::all(Color::srgba(0.85, 0.92, 1.0, 0.7 * alpha)),
                TacticalMapMarkerDynamic,
            ));
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(screen_xy.x - 1.0),
                    top: px(screen_xy.y - 10.0),
                    width: px(6.0),
                    height: px(20.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.85, 0.92, 1.0, 0.12 * alpha)),
                TacticalMapMarkerDynamic,
            ));
        });
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_tactical_map_fx_overlay_system(
    time: Res<'_, Time>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    camera_motion: Res<'_, CameraMotionState>,
    windows: Query<'_, '_, &'_ Window, With<PrimaryWindow>>,
    mut map_queries: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, &'_ Transform, With<ControlledEntity>>,
            Query<
                '_,
                '_,
                (
                    &'_ mut Visibility,
                    &'_ mut Transform,
                    &'_ MeshMaterial2d<TacticalMapOverlayMaterial>,
                ),
                (With<TacticalMapScreenFxOverlay>, Without<ControlledEntity>),
            >,
        ),
    >,
    map_settings_query: Query<'_, '_, &'_ TacticalMapUiSettings>,
    mut fx_materials: ResMut<'_, Assets<TacticalMapOverlayMaterial>>,
) {
    let map_settings = map_settings_query
        .iter()
        .next()
        .copied()
        .unwrap_or_default();
    let Ok(window) = windows.single() else {
        return;
    };
    let controlled_world_xy = map_queries
        .p0()
        .iter()
        .next()
        .map(|transform| transform.translation.truncate());
    let alpha = tactical_map_state.alpha;
    let width = window.width();
    let height = window.height();
    let world_center_base = controlled_world_xy.unwrap_or(camera_motion.world_position_xy);
    let world_center = world_center_base + tactical_map_state.pan_offset_world;
    let map_zoom = tactical_map_state.map_zoom.max(1e-6);
    {
        let mut fx_overlay = map_queries.p1();
        let Ok((mut fx_visibility, mut fx_transform, fx_material_handle)) = fx_overlay.single_mut()
        else {
            return;
        };
        *fx_visibility = if alpha > 0.001 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        fx_transform.translation.x = 0.0;
        fx_transform.translation.y = 0.0;
        fx_transform.translation.z = -10.0;
        fx_transform.scale = Vec3::new(width, height, 1.0);

        if let Some(material) = fx_materials.get_mut(&fx_material_handle.0) {
            material.viewport_time = Vec4::new(width, height, time.elapsed_secs(), alpha);
            material.map_center_zoom_mode =
                Vec4::new(world_center.x, world_center.y, map_zoom, map_settings.fx_mode as f32);
            material.grid_major = Vec4::new(
                map_settings.grid_major_color_rgb.x,
                map_settings.grid_major_color_rgb.y,
                map_settings.grid_major_color_rgb.z,
                map_settings.grid_major_alpha * alpha,
            );
            material.grid_minor = Vec4::new(
                map_settings.grid_minor_color_rgb.x,
                map_settings.grid_minor_color_rgb.y,
                map_settings.grid_minor_color_rgb.z,
                map_settings.grid_minor_alpha * alpha,
            );
            material.grid_micro = Vec4::new(
                map_settings.grid_micro_color_rgb.x,
                map_settings.grid_micro_color_rgb.y,
                map_settings.grid_micro_color_rgb.z,
                map_settings.grid_micro_alpha * alpha,
            );
            material.grid_glow_alpha = Vec4::new(
                map_settings.grid_major_glow_alpha * alpha,
                map_settings.grid_minor_glow_alpha * alpha,
                map_settings.grid_micro_glow_alpha * alpha,
                0.0,
            );
            material.fx_params = Vec4::new(
                map_settings.fx_opacity,
                map_settings.fx_noise_amount,
                map_settings.fx_scanline_density,
                map_settings.fx_scanline_speed,
            );
            material.fx_params_b = Vec4::new(
                map_settings.fx_crt_distortion,
                map_settings.fx_vignette_strength,
                map_settings.fx_green_tint_mix,
                0.0,
            );
            material.background_color = Vec4::new(
                map_settings.background_color_rgb.x,
                map_settings.background_color_rgb.y,
                map_settings.background_color_rgb.z,
                0.0,
            );
            material.line_widths_px = Vec4::new(
                map_settings.line_width_major_px,
                map_settings.line_width_minor_px,
                map_settings.line_width_micro_px,
                0.0,
            );
            material.glow_widths_px = Vec4::new(
                map_settings.glow_width_major_px,
                map_settings.glow_width_minor_px,
                map_settings.glow_width_micro_px,
                0.0,
            );
        }
    }
}

fn ids_refer_to_same_guid(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    parse_guid_from_entity_id(left)
        .zip(parse_guid_from_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

#[allow(clippy::type_complexity)]
pub(super) fn update_owned_entities_panel_system(
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    manifest_cache: Res<'_, OwnedAssetManifestCache>,
    mut panel_state: ResMut<'_, OwnedEntitiesPanelState>,
    existing_panels: Query<'_, '_, Entity, With<OwnedEntitiesPanelRoot>>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let mut owned_ship_rows = manifest_cache
        .assets_by_entity_id
        .values()
        .filter(|asset| asset.kind.eq_ignore_ascii_case("ship"))
        .map(|asset| {
            let entity_id = asset.entity_id.clone();
            let label = if asset.display_name.trim().is_empty() {
                entity_id.clone()
            } else {
                asset.display_name.clone()
            };
            (entity_id, label)
        })
        .collect::<Vec<_>>();
    owned_ship_rows.sort_by(|a, b| {
        a.1.to_lowercase()
            .cmp(&b.1.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });
    owned_ship_rows.dedup_by(|a, b| a.0 == b.0);
    let entity_ids = owned_ship_rows
        .iter()
        .map(|(entity_id, _)| entity_id.clone())
        .collect::<Vec<_>>();
    let selected_id = player_view_state
        .desired_controlled_entity_id
        .clone()
        .or_else(|| player_view_state.controlled_entity_id.clone());

    if panel_state.last_entity_ids == entity_ids
        && panel_state.last_selected_id == selected_id
        && panel_state.last_detached_mode == player_view_state.detached_free_camera
        && !existing_panels.is_empty()
    {
        return;
    }
    panel_state.last_entity_ids = entity_ids.clone();
    panel_state.last_selected_id = selected_id.clone();
    panel_state.last_detached_mode = player_view_state.detached_free_camera;

    for panel in &existing_panels {
        queue_despawn_if_exists(&mut commands, panel);
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(12),
                width: px(280),
                padding: UiRect::all(px(10)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.07, 0.11, 0.88)),
            BorderColor::all(Color::srgba(0.22, 0.34, 0.48, 0.92)),
            OwnedEntitiesPanelRoot,
            GameplayHud,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Owned Ships"),
                TextFont {
                    font: fonts.bold.clone(),
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));

            let free_roam_selected = selected_id
                .as_deref()
                .is_some_and(|selected| ids_refer_to_same_guid(selected, local_player_entity_id))
                && !player_view_state.detached_free_camera;
            panel
                .spawn((
                    Button,
                    OwnedEntitiesPanelButton {
                        action: OwnedEntitiesPanelAction::FreeRoam,
                    },
                    Node {
                        width: percent(100.0),
                        height: px(34),
                        justify_content: JustifyContent::FlexStart,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(px(10), px(0)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BackgroundColor(if free_roam_selected {
                        Color::srgba(0.26, 0.4, 0.56, 0.96)
                    } else {
                        Color::srgba(0.15, 0.2, 0.28, 0.92)
                    }),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("Free Roam"),
                        TextFont {
                            font: fonts.regular.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.97, 1.0)),
                    ));
                });
            if owned_ship_rows.is_empty() {
                panel.spawn((
                    Text::new("No owned entities visible"),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.75, 0.82, 0.9, 0.9)),
                ));
            } else {
                for (entity_id, display_label) in owned_ship_rows {
                    let is_selected = selected_id.as_deref().is_some_and(|selected| {
                        ids_refer_to_same_guid(selected, entity_id.as_str())
                    });
                    panel
                        .spawn((
                            Button,
                            OwnedEntitiesPanelButton {
                                action: OwnedEntitiesPanelAction::ControlEntity(entity_id),
                            },
                            Node {
                                width: percent(100.0),
                                height: px(34),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(px(10), px(0)),
                                border_radius: BorderRadius::all(px(6)),
                                ..default()
                            },
                            BackgroundColor(if is_selected {
                                Color::srgba(0.26, 0.4, 0.56, 0.96)
                            } else {
                                Color::srgba(0.15, 0.2, 0.28, 0.92)
                            }),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(display_label),
                                TextFont {
                                    font: fonts.regular.clone(),
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.95, 0.97, 1.0)),
                            ));
                        });
                }
            }
        });
}

#[allow(clippy::type_complexity)]
pub(super) fn handle_owned_entities_panel_buttons(
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            &OwnedEntitiesPanelButton,
            &mut BackgroundColor,
        ),
        Changed<Interaction>,
    >,
    session: Res<'_, ClientSession>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut control_request_state: ResMut<'_, ClientControlRequestState>,
    mut panel_state: ResMut<'_, OwnedEntitiesPanelState>,
) {
    for (interaction, button, mut color) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                match &button.action {
                    OwnedEntitiesPanelAction::FreeRoam => {
                        let target = session.player_entity_id.clone();
                        player_view_state.desired_controlled_entity_id = target.clone();
                        control_request_state.next_request_seq =
                            control_request_state.next_request_seq.saturating_add(1);
                        control_request_state.pending_controlled_entity_id = target;
                        control_request_state.pending_request_seq =
                            Some(control_request_state.next_request_seq);
                        control_request_state.last_sent_request_seq = None;
                        control_request_state.last_sent_at_s = 0.0;
                        // Free roam means the player entity is the controlled entity.
                        // Keep attached camera/input flow active so player movement works.
                        player_view_state.detached_free_camera = false;
                        player_view_state.selected_entity_id = session.player_entity_id.clone();
                    }
                    OwnedEntitiesPanelAction::ControlEntity(entity_id) => {
                        player_view_state.desired_controlled_entity_id = Some(entity_id.clone());
                        control_request_state.next_request_seq =
                            control_request_state.next_request_seq.saturating_add(1);
                        control_request_state.pending_controlled_entity_id =
                            Some(entity_id.clone());
                        control_request_state.pending_request_seq =
                            Some(control_request_state.next_request_seq);
                        control_request_state.last_sent_request_seq = None;
                        control_request_state.last_sent_at_s = 0.0;
                        player_view_state.detached_free_camera = false;
                        player_view_state.selected_entity_id = Some(entity_id.clone());
                    }
                }
                panel_state.last_selected_id = None;
                *color = BackgroundColor(Color::srgba(0.26, 0.4, 0.56, 0.96));
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgba(0.2, 0.29, 0.41, 0.96));
            }
            Interaction::None => {
                let is_selected = match &button.action {
                    OwnedEntitiesPanelAction::FreeRoam => {
                        player_view_state
                            .desired_controlled_entity_id
                            .as_deref()
                            .zip(session.player_entity_id.as_deref())
                            .is_some_and(|(desired, session_player)| {
                                ids_refer_to_same_guid(desired, session_player)
                            })
                            && !player_view_state.detached_free_camera
                    }
                    OwnedEntitiesPanelAction::ControlEntity(entity_id) => {
                        player_view_state.desired_controlled_entity_id.as_ref() == Some(entity_id)
                    }
                };
                *color = BackgroundColor(if is_selected {
                    Color::srgba(0.26, 0.4, 0.56, 0.96)
                } else {
                    Color::srgba(0.15, 0.2, 0.28, 0.92)
                });
            }
        }
    }
}

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
        With<ControlledEntity>,
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
            let vel = maybe_velocity.map_or(Vec2::ZERO, |velocity| velocity.0);
            let heading_rad = maybe_rotation
                .map(|rotation| rotation.as_radians())
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
        text.0 = format!("({:.0}, {:.0})", pos.x, pos.y);
    }
    if let Ok(mut fill) = bar_value_queries.p0().single_mut() {
        fill.ratio = health_ratio;
    }
    if let Ok(mut fill) = bar_value_queries.p1().single_mut() {
        fill.ratio = fuel_ratio;
    }
}

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
pub(super) fn sync_ship_nameplates_system(
    mut commands: Commands<'_, '_>,
    ships: Query<
        '_,
        '_,
        (
            Entity,
            Option<&EntityGuid>,
            Option<&EntityLabels>,
            Has<ControlledEntity>,
            Has<lightyear::prelude::Interpolated>,
            Has<lightyear::prelude::Predicted>,
        ),
        (
            With<WorldEntity>,
            Without<ShipNameplateRoot>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
    existing: Query<'_, '_, (Entity, &ShipNameplateRoot)>,
) {
    let mut existing_targets = HashMap::<Entity, Entity>::new();
    for (entity, root) in &existing {
        existing_targets.insert(root.target, entity);
    }

    let mut best_entity_by_guid = HashMap::<uuid::Uuid, (Entity, i32)>::new();
    let mut winner_entities = HashSet::<Entity>::new();
    for (ship_entity, guid, labels, is_controlled, is_interpolated, is_predicted) in &ships {
        let is_ship = labels.is_some_and(|labels| labels.0.iter().any(|label| label == "Ship"));
        if !is_ship {
            continue;
        }
        let score = if is_controlled {
            3
        } else if is_interpolated {
            2
        } else if is_predicted {
            1
        } else {
            0
        };
        if let Some(guid) = guid {
            match best_entity_by_guid.get_mut(&guid.0) {
                Some((winner, winner_score)) => {
                    if score > *winner_score
                        || (score == *winner_score && ship_entity.to_bits() < winner.to_bits())
                    {
                        *winner = ship_entity;
                        *winner_score = score;
                    }
                }
                None => {
                    best_entity_by_guid.insert(guid.0, (ship_entity, score));
                }
            }
        } else {
            winner_entities.insert(ship_entity);
        }
    }
    winner_entities.extend(best_entity_by_guid.values().map(|(entity, _)| *entity));

    for ship_entity in &winner_entities {
        if existing_targets.contains_key(ship_entity) {
            continue;
        }
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: px(100),
                    left: px(0),
                    top: px(0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
                Visibility::Hidden,
                ShipNameplateRoot {
                    target: *ship_entity,
                },
                GameplayHud,
                UiOverlayLayer,
                RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
                WorldEntity,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .with_children(|plate| {
                plate
                    .spawn((
                        Node {
                            // 16 segments @ 5px + 15 gaps @ 1px + 2px padding = 97px total.
                            // Fixed-width segments avoid uneven fractional flex spacing.
                            width: px(97.0),
                            height: px(8.0),
                            column_gap: px(1.0),
                            align_items: AlignItems::Stretch,
                            border: UiRect::all(px(1.0)),
                            padding: UiRect::all(px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.05, 0.08, 0.05, 0.75)),
                        BorderColor::all(Color::srgba(0.18, 0.35, 0.18, 0.8)),
                        SegmentedBarStyle {
                            segments: 16,
                            active_color: Color::srgb(0.22, 0.9, 0.34),
                            inactive_color: Color::srgba(0.08, 0.22, 0.10, 0.85),
                        },
                        SegmentedBarValue { ratio: 1.0 },
                        ShipNameplateHealthBar {
                            target: *ship_entity,
                        },
                    ))
                    .with_children(|bar| {
                        for index in 0..16u8 {
                            bar.spawn((
                                Node {
                                    width: px(5.0),
                                    height: percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.85)),
                                SegmentedBarSegment { index },
                            ));
                        }
                    });
            });
    }

    for (nameplate_entity, root) in &existing {
        if !winner_entities.contains(&root.target) {
            queue_despawn_if_exists(&mut commands, nameplate_entity);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_ship_nameplate_positions_system(
    mut roots: Query<'_, '_, (&ShipNameplateRoot, &mut Node, &mut Visibility)>,
    mut health_bars: Query<'_, '_, (&ShipNameplateHealthBar, &mut SegmentedBarValue)>,
    ships: Query<
        '_,
        '_,
        (
            Entity,
            &Transform,
            Option<&SizeM>,
            Option<&HealthPool>,
            Option<&EntityLabels>,
        ),
        (
            With<WorldEntity>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
    gameplay_camera: Query<'_, '_, (&Camera, &Transform), With<GameplayCamera>>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
) {
    let Ok((camera, camera_transform)) = gameplay_camera.single() else {
        return;
    };
    // This runs in `Last` after camera transform updates. Convert the current camera
    // `Transform` directly so projection uses the final same-frame camera state.
    let camera_global = GlobalTransform::from(*camera_transform);
    let Ok(window) = window_query.single() else {
        return;
    };

    let mut ship_data_by_entity = HashMap::<Entity, (Vec3, f32, f32)>::new();
    for (entity, transform, size_m, health_pool, labels) in &ships {
        let is_ship = labels.is_some_and(|labels| labels.0.iter().any(|label| label == "Ship"));
        if !is_ship {
            continue;
        }
        let health_ratio = health_pool
            .map(|health| {
                if health.maximum > 0.0 {
                    (health.current / health.maximum).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let half_height_world = size_m.map(|s| s.length * 0.5).unwrap_or(6.0);
        ship_data_by_entity.insert(
            entity,
            (transform.translation, half_height_world, health_ratio),
        );
    }

    for (root, mut node, mut visibility) in &mut roots {
        let Some((world_pos, half_height_world, _)) = ship_data_by_entity.get(&root.target) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let center_world = Vec3::new(world_pos.x, world_pos.y, 0.0);
        let Ok(viewport_pos) = camera.world_to_viewport(&camera_global, center_world) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let top_world = Vec3::new(world_pos.x, world_pos.y + *half_height_world, 0.0);
        let Ok(top_viewport_pos) = camera.world_to_viewport(&camera_global, top_world) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if viewport_pos.x < 0.0
            || viewport_pos.x > window.width()
            || viewport_pos.y < 0.0
            || viewport_pos.y > window.height()
        {
            *visibility = Visibility::Hidden;
            continue;
        }
        let plate_width = 100.0;
        let plate_height = 8.0;
        let vertical_gap = 6.0;
        node.left = px(viewport_pos.x - plate_width * 0.5);
        let ship_top_y_px = viewport_pos.y.min(top_viewport_pos.y);
        node.top = px(ship_top_y_px - plate_height - vertical_gap);
        *visibility = Visibility::Visible;

        if let Some((_, _, health_ratio)) = ship_data_by_entity.get(&root.target) {
            for (bar_target, mut value) in &mut health_bars {
                if bar_target.target == root.target {
                    value.ratio = *health_ratio;
                }
            }
        }
    }
}

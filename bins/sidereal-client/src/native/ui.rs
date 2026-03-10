//! World HUD and owned-entity panel systems.

use avian2d::prelude::{LinearVelocity, Rotation};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::MeshMaterial2d;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::window::PrimaryWindow;
use bevy_svg::prelude::{Svg, Svg2d};
use sidereal_game::{
    EntityGuid, FuelTank, HealthPool, MapIcon, MountedOn, SizeM, TacticalMapUiSettings,
    TacticalPresentationDefaults,
};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use std::collections::{HashMap, HashSet};

use super::app_state::{
    ClientAppState, ClientSession, LocalPlayerViewState, OwnedEntitiesPanelState,
};
use super::assets::{LocalAssetManager, RuntimeAssetHttpFetchState, RuntimeAssetNetIndicatorState};
use super::backdrop::TacticalMapOverlayMaterial;
use super::components::{
    ControlledEntity, DebugOverlayPanelLabelShadowText, DebugOverlayPanelLabelText,
    DebugOverlayPanelRoot, DebugOverlayPanelValueShadowText, DebugOverlayPanelValueText,
    EntityNameplateHealthBar, EntityNameplateRoot, GameplayCamera, GameplayHud, HudFuelBarFill,
    HudHealthBarFill, HudPositionValueText, HudSpeedValueText, LoadingOverlayRoot,
    LoadingOverlayText, LoadingProgressBarFill, OwnedEntitiesPanelAction, OwnedEntitiesPanelButton,
    OwnedEntitiesPanelRoot, RuntimeScreenOverlayPass, RuntimeScreenOverlayPassKind,
    SegmentedBarSegment, SegmentedBarStyle, SegmentedBarValue, SuppressedPredictedDuplicateVisual,
    TacticalMapCursorText, TacticalMapMarkerDynamic, TacticalMapOverlayRoot, TacticalMapTitle,
    UiOverlayCamera, UiOverlayLayer, WorldEntity,
};
use super::dev_console::{DevConsoleState, is_console_open};
use super::ecs_util::queue_despawn_if_exists;
use super::platform::{ORTHO_SCALE_PER_DISTANCE, UI_OVERLAY_RENDER_LAYER};
use super::resources::{
    CameraMotionState, ClientControlRequestState, DebugOverlayDisplayMetrics, DebugOverlaySnapshot,
    DebugOverlayState, DebugSeverity, DuplicateVisualResolutionState, EmbeddedFonts,
    NameplateUiState, OwnedAssetManifestCache, TacticalContactsCache, TacticalFogCache,
    TacticalMapUiState,
};

const TACTICAL_FOG_MASK_RESOLUTION: u32 = 384;
const TACTICAL_ICON_WORLD_HEIGHT_M: f32 = 24.0;
const TACTICAL_CONTACT_SMOOTHING_RATE: f32 = 8.0;
const TACTICAL_CONTACT_PREDICTION_HORIZON_S: f32 = 0.25;

/// Propagates the UI overlay render layer to all descendants of HUD roots so they are drawn
/// by the UI overlay camera (fixed scale) instead of the gameplay camera.
pub(super) fn propagate_ui_overlay_layer_system(
    mut commands: Commands<'_, '_>,
    added_overlay_roots: Query<'_, '_, Entity, Added<UiOverlayLayer>>,
    added_children: Query<'_, '_, (Entity, &'_ ChildOf), Added<ChildOf>>,
    children_query: Query<'_, '_, &'_ Children>,
    overlay_entities: Query<'_, '_, (), With<UiOverlayLayer>>,
) {
    for entity in &added_overlay_roots {
        apply_ui_overlay_to_subtree(entity, &children_query, &overlay_entities, &mut commands);
    }
    for (entity, parent) in &added_children {
        if overlay_entities.contains(parent.parent()) {
            apply_ui_overlay_to_subtree(entity, &children_query, &overlay_entities, &mut commands);
        }
    }
}

fn apply_ui_overlay_to_subtree(
    entity: Entity,
    children_query: &Query<'_, '_, &'_ Children>,
    overlay_entities: &Query<'_, '_, (), With<UiOverlayLayer>>,
    commands: &mut Commands<'_, '_>,
) {
    if !overlay_entities.contains(entity) {
        commands
            .entity(entity)
            .insert((RenderLayers::layer(UI_OVERLAY_RENDER_LAYER), UiOverlayLayer));
    } else {
        commands
            .entity(entity)
            .insert(RenderLayers::layer(UI_OVERLAY_RENDER_LAYER));
    }
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            apply_ui_overlay_to_subtree(child, children_query, overlay_entities, commands);
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
    fetch_state: Res<'_, RuntimeAssetHttpFetchState>,
    mut indicator_state: ResMut<'_, RuntimeAssetNetIndicatorState>,
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
    if !fetch_state.has_in_flight_fetch() {
        color.0.set_alpha(0.0);
        indicator_state.blinking_phase_s = 0.0;
        return;
    }
    indicator_state.blinking_phase_s += time.delta_secs();
    let phase = (indicator_state.blinking_phase_s * 6.0).fract();
    let on = phase < 0.5;
    color.0 = if on {
        Color::srgba(0.35, 0.9, 1.0, 1.0)
    } else {
        Color::srgba(0.35, 0.9, 1.0, 0.2)
    };
}

pub(super) fn update_debug_overlay_text_ui_system(
    time: Res<'_, Time>,
    debug_overlay: Res<'_, DebugOverlayState>,
    snapshot: Res<'_, DebugOverlaySnapshot>,
    diagnostics: Res<'_, DiagnosticsStore>,
    mut display_metrics: Local<'_, DebugOverlayDisplayMetrics>,
    mut root_query: Query<'_, '_, &mut Visibility, With<DebugOverlayPanelRoot>>,
    mut text_queries: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, &mut Text, With<DebugOverlayPanelLabelText>>,
            Query<'_, '_, &mut Text, With<DebugOverlayPanelLabelShadowText>>,
            Query<'_, '_, (&mut Text, &mut TextColor), With<DebugOverlayPanelValueText>>,
            Query<'_, '_, &mut Text, With<DebugOverlayPanelValueShadowText>>,
        ),
    >,
) {
    let Ok(mut root_visibility) = root_query.single_mut() else {
        return;
    };

    if !debug_overlay.enabled {
        *root_visibility = Visibility::Hidden;
        return;
    }

    *root_visibility = Visibility::Visible;

    let now_s = time.elapsed_secs_f64();
    if !display_metrics.initialized || now_s - display_metrics.last_sample_at_s >= 1.0 {
        display_metrics.sampled_fps = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|diagnostic| diagnostic.average().or_else(|| diagnostic.smoothed()));
        display_metrics.sampled_frame_ms = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|diagnostic| diagnostic.average().or_else(|| diagnostic.smoothed()));
        display_metrics.last_sample_at_s = now_s;
        display_metrics.initialized = true;
    }

    let mut labels = Vec::with_capacity(snapshot.text_rows.len() + 2);
    let mut values = Vec::with_capacity(snapshot.text_rows.len() + 2);
    labels.push("FPS".to_string());
    values.push(
        display_metrics
            .sampled_fps
            .map(|value| format!("{value:.0}"))
            .unwrap_or_else(|| "--".to_string()),
    );
    labels.push("Frame Time".to_string());
    values.push(
        display_metrics
            .sampled_frame_ms
            .map(|value| format!("{value:.2} ms"))
            .unwrap_or_else(|| "--.-- ms".to_string()),
    );
    for row in &snapshot.text_rows {
        labels.push(row.label.clone());
        values.push(row.value.clone());
    }
    let labels_text = labels.join("\n");
    let values_text = values.join("\n");

    let highest_severity = snapshot
        .text_rows
        .iter()
        .map(|row| row.severity)
        .max_by_key(|severity| match severity {
            DebugSeverity::Normal => 0,
            DebugSeverity::Warn => 1,
            DebugSeverity::Error => 2,
        })
        .unwrap_or(DebugSeverity::Normal);

    if let Ok(mut label_text) = text_queries.p0().single_mut() {
        label_text.0 = labels_text.clone();
    }
    if let Ok(mut label_shadow_text) = text_queries.p1().single_mut() {
        label_shadow_text.0 = labels_text;
    }
    if let Ok((mut value_text, mut value_text_color)) = text_queries.p2().single_mut() {
        value_text.0 = values_text.clone();
        value_text_color.0 = match highest_severity {
            DebugSeverity::Normal => Color::srgb(0.85, 0.92, 1.0),
            DebugSeverity::Warn => Color::srgb(1.0, 0.85, 0.45),
            DebugSeverity::Error => Color::srgb(1.0, 0.55, 0.5),
        };
    }
    if let Ok(mut value_shadow_text) = text_queries.p3().single_mut() {
        value_shadow_text.0 = values_text;
    }
}

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
    time: Res<'_, Time>,
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

    {
        let mut roots = map_queries.p3();
        let Ok((_root_entity, mut root_bg, mut visibility, _children)) = roots.single_mut() else {
            return;
        };
        if alpha < 0.01 && !tactical_map_state.enabled {
            *visibility = Visibility::Hidden;
            for (marker, _, _, _) in &mut dynamic_markers {
                queue_despawn_if_exists(&mut commands, marker);
            }
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
            let icon_scale = tactical_svg_marker_scale(&svg_assets, &svg_handle, map_zoom);
            let base_translation = tactical_map_marker_translation(screen_xy, width, height, -8.5);
            let marker_translation = tactical_icon_centered_translation(
                &svg_assets,
                &svg_handle,
                icon_scale,
                heading_rad,
                base_translation,
            );
            upsert_tactical_map_marker(
                &mut commands,
                existing_marker_entities.remove(marker_key.as_str()),
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
                Vec2::new(contact.position_xy[0], contact.position_xy[1]),
                contact.heading_rad,
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
        let icon_scale = tactical_svg_marker_scale(&svg_assets, &svg_handle, map_zoom);
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
        upsert_tactical_map_marker(
            &mut commands,
            existing_marker_entities.remove(marker_key.as_str()),
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
        }
    }
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
    let mut current_ids = HashSet::with_capacity(contacts_cache.contacts_by_entity_id.len());
    for (entity_id, contact) in &contacts_cache.contacts_by_entity_id {
        current_ids.insert(entity_id.clone());
        let target_pos = Vec2::new(contact.position_xy[0], contact.position_xy[1]);
        let velocity = contact.velocity_xy.map(|v| Vec2::new(v[0], v[1]));
        if let Some(track) = cache.tracks_by_entity_id.get_mut(entity_id.as_str()) {
            track.target_pos = target_pos;
            track.velocity = velocity;
            track.target_heading_rad = contact.heading_rad;
        } else {
            cache.tracks_by_entity_id.insert(
                entity_id.clone(),
                TacticalContactSmoothingTrack {
                    render_pos: target_pos,
                    target_pos,
                    velocity,
                    render_heading_rad: contact.heading_rad,
                    target_heading_rad: contact.heading_rad,
                },
            );
        }
    }
    cache
        .tracks_by_entity_id
        .retain(|entity_id, _| current_ids.contains(entity_id));

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

fn tactical_icon_centered_translation(
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
    reload_generation: u64,
    base_by_asset_id: HashMap<String, Handle<Svg>>,
    tinted_by_variant_key: HashMap<String, Handle<Svg>>,
}

#[derive(Clone, Copy)]
enum TacticalMarkerColorRole {
    FriendlySelf,
    HostileContact,
}

fn tactical_marker_color(role: TacticalMarkerColorRole) -> Color {
    match role {
        TacticalMarkerColorRole::FriendlySelf => Color::srgb(0.22, 0.62, 1.0),
        TacticalMarkerColorRole::HostileContact => Color::srgb(1.0, 0.9, 0.34),
    }
}

fn resolve_tactical_marker_svg(
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

    let variant_key = format!(
        "{}:{}",
        base_asset_id,
        match role {
            TacticalMarkerColorRole::FriendlySelf => "self",
            TacticalMarkerColorRole::HostileContact => "contact",
        }
    );
    if let Some(variant) = cache.tinted_by_variant_key.get(&variant_key) {
        return Some(variant.clone());
    }

    let base_svg = svg_assets.get(&base_handle)?.clone();
    let mut tinted_svg = base_svg;
    let marker_color = tactical_marker_color(role);
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
#[allow(clippy::too_many_arguments)]
pub(super) fn update_runtime_screen_overlay_passes_system(
    time: Res<'_, Time>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    fog_cache: Res<'_, TacticalFogCache>,
    camera_motion: Res<'_, CameraMotionState>,
    windows: Query<'_, '_, &'_ Window, With<PrimaryWindow>>,
    mut map_queries: ParamSet<
        '_,
        '_,
        (
            Query<
                '_,
                '_,
                &'_ Transform,
                (With<ControlledEntity>, Without<RuntimeScreenOverlayPass>),
            >,
            Query<
                '_,
                '_,
                (
                    &'_ mut Visibility,
                    &'_ mut Transform,
                    &'_ RuntimeScreenOverlayPass,
                    &'_ MeshMaterial2d<TacticalMapOverlayMaterial>,
                ),
                (With<RuntimeScreenOverlayPass>, Without<ControlledEntity>),
            >,
        ),
    >,
    map_settings_query: Query<'_, '_, &'_ TacticalMapUiSettings>,
    mut fx_materials: ResMut<'_, Assets<TacticalMapOverlayMaterial>>,
    mut images: ResMut<'_, Assets<Image>>,
) {
    let map_settings = map_settings_query
        .iter()
        .next()
        .cloned()
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
    let mut fx_overlay = map_queries.p1();
    let Ok((mut fx_visibility, mut fx_transform, fx_pass, fx_material_handle)) =
        fx_overlay.single_mut()
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
        match fx_pass.kind {
            RuntimeScreenOverlayPassKind::TacticalMap => {
                update_tactical_runtime_screen_overlay_material(
                    material,
                    &mut images,
                    &fog_cache,
                    &map_settings,
                    width,
                    height,
                    time.elapsed_secs(),
                    alpha,
                    world_center,
                    map_zoom,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_tactical_runtime_screen_overlay_material(
    material: &mut TacticalMapOverlayMaterial,
    images: &mut Assets<Image>,
    fog_cache: &TacticalFogCache,
    map_settings: &TacticalMapUiSettings,
    width: f32,
    height: f32,
    time_s: f32,
    alpha: f32,
    world_center: Vec2,
    map_zoom: f32,
) {
    material.viewport_time = Vec4::new(width, height, time_s, alpha);
    material.map_center_zoom_mode = Vec4::new(
        world_center.x,
        world_center.y,
        map_zoom,
        map_settings.fx_mode as f32,
    );
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
    update_tactical_fog_mask_texture(
        images,
        material,
        fog_cache,
        width,
        height,
        world_center,
        map_zoom,
    );
}

fn update_tactical_fog_mask_texture(
    images: &mut Assets<Image>,
    material: &TacticalMapOverlayMaterial,
    fog_cache: &TacticalFogCache,
    viewport_width_px: f32,
    viewport_height_px: f32,
    world_center: Vec2,
    map_zoom_px_per_world: f32,
) {
    let Some(image) = images.get_mut(&material.fog_mask) else {
        return;
    };
    let expected_len = (TACTICAL_FOG_MASK_RESOLUTION * TACTICAL_FOG_MASK_RESOLUTION) as usize;
    let needs_rebuild = image.texture_descriptor.size.width != TACTICAL_FOG_MASK_RESOLUTION
        || image.texture_descriptor.size.height != TACTICAL_FOG_MASK_RESOLUTION
        || image.texture_descriptor.format != TextureFormat::R8Unorm
        || image.data.as_ref().map_or(0, Vec::len) != expected_len;
    if needs_rebuild {
        *image = Image::new_fill(
            Extent3d {
                width: TACTICAL_FOG_MASK_RESOLUTION,
                height: TACTICAL_FOG_MASK_RESOLUTION,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[255],
            TextureFormat::R8Unorm,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
    }
    let Some(mask) = image.data.as_mut() else {
        return;
    };
    let cell_size_m = fog_cache.cell_size_m;
    if cell_size_m <= 0.0
        || map_zoom_px_per_world <= 0.0
        || viewport_width_px <= 0.0
        || viewport_height_px <= 0.0
    {
        mask.fill(255);
        return;
    }

    let mut explored_cells =
        HashSet::with_capacity(fog_cache.explored_cells.len() + fog_cache.live_cells.len());
    explored_cells.extend(fog_cache.explored_cells.iter().copied());
    explored_cells.extend(fog_cache.live_cells.iter().copied());

    let width = TACTICAL_FOG_MASK_RESOLUTION as usize;
    let height = TACTICAL_FOG_MASK_RESOLUTION as usize;
    let width_f = TACTICAL_FOG_MASK_RESOLUTION as f32;
    let height_f = TACTICAL_FOG_MASK_RESOLUTION as f32;

    for y in 0..height {
        let sample_screen_y = ((y as f32 + 0.5) / height_f) * viewport_height_px;
        let world_y =
            world_center.y + (viewport_height_px * 0.5 - sample_screen_y) / map_zoom_px_per_world;
        let cell_y = (world_y / cell_size_m).floor() as i32;
        for x in 0..width {
            let sample_screen_x = ((x as f32 + 0.5) / width_f) * viewport_width_px;
            let world_x = world_center.x
                + (sample_screen_x - viewport_width_px * 0.5) / map_zoom_px_per_world;
            let cell_x = (world_x / cell_size_m).floor() as i32;
            let index = y * width + x;
            mask[index] = if explored_cells.contains(&sidereal_net::GridCell {
                x: cell_x,
                y: cell_y,
            }) {
                255
            } else {
                0
            };
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
pub(super) fn sync_entity_nameplates_system(
    mut commands: Commands<'_, '_>,
    duplicate_visuals: Res<'_, DuplicateVisualResolutionState>,
    world_entities: Query<
        '_,
        '_,
        (Entity, Option<&EntityGuid>, Option<&HealthPool>),
        (
            With<WorldEntity>,
            Without<EntityNameplateRoot>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
    existing: Query<'_, '_, (Entity, &EntityNameplateRoot)>,
) {
    let mut existing_targets = HashMap::<Entity, Entity>::new();
    for (entity, root) in &existing {
        existing_targets.insert(root.target, entity);
        // UI nameplate roots are HUD-only entities and must not be tagged as world entities.
        // This keeps UI/world queries disjoint and avoids stale top-left plate positions.
        commands.entity(entity).remove::<WorldEntity>();
        commands.entity(entity).remove::<GameplayHud>();
    }

    let mut winner_entities = HashSet::<Entity>::new();
    for (entity, guid, health_pool) in &world_entities {
        if health_pool.is_none() {
            continue;
        }
        if let Some(guid) = guid {
            if duplicate_visuals.winner_by_guid.get(&guid.0) == Some(&entity) {
                winner_entities.insert(entity);
            }
        } else {
            winner_entities.insert(entity);
        }
    }

    for entity in &winner_entities {
        if existing_targets.contains_key(entity) {
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
                EntityNameplateRoot { target: *entity },
                UiOverlayLayer,
                RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
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
                        EntityNameplateHealthBar { target: *entity },
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
pub(super) fn update_entity_nameplate_positions_system(
    nameplate_state: Res<'_, NameplateUiState>,
    mut roots: Query<
        '_,
        '_,
        (&EntityNameplateRoot, &mut Node, &mut Visibility),
        Without<WorldEntity>,
    >,
    mut health_bars: Query<'_, '_, (&EntityNameplateHealthBar, &mut SegmentedBarValue)>,
    world_entities: Query<
        '_,
        '_,
        (
            Entity,
            &GlobalTransform,
            Option<&Visibility>,
            Option<&ViewVisibility>,
            Option<&SizeM>,
            Option<&HealthPool>,
        ),
        (
            With<WorldEntity>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
    gameplay_camera: Query<'_, '_, (&Camera, &Transform), With<GameplayCamera>>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
) {
    if !nameplate_state.enabled {
        for (_, _, mut visibility) in &mut roots {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let Ok((camera, camera_transform)) = gameplay_camera.single() else {
        return;
    };
    // This runs in `Last` after camera transform updates. Convert the current camera
    // `Transform` directly so projection uses the final same-frame camera state.
    let camera_global = GlobalTransform::from(*camera_transform);
    let Ok(window) = window_query.single() else {
        return;
    };

    let mut entity_data_by_entity = HashMap::<Entity, (Vec3, f32, f32)>::new();
    for (entity, global_transform, visibility, view_visibility, size_m, health_pool) in
        &world_entities
    {
        let Some(health_pool) = health_pool else {
            continue;
        };
        if visibility.is_some_and(|visibility| *visibility == Visibility::Hidden) {
            continue;
        }
        if view_visibility.is_some_and(|view_visibility| !view_visibility.get()) {
            continue;
        }
        let health_ratio = if health_pool.maximum > 0.0 {
            (health_pool.current / health_pool.maximum).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let half_extent_world = size_m.map(|s| s.length * 0.5).unwrap_or(6.0);
        entity_data_by_entity.insert(
            entity,
            (
                global_transform.translation(),
                half_extent_world,
                health_ratio,
            ),
        );
    }

    for (root, mut node, mut visibility) in &mut roots {
        let Some((world_pos, half_extent_world, _)) = entity_data_by_entity.get(&root.target)
        else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let center_world = Vec3::new(world_pos.x, world_pos.y, 0.0);
        let Ok(viewport_pos) = camera.world_to_viewport(&camera_global, center_world) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let top_world = Vec3::new(world_pos.x, world_pos.y + *half_extent_world, 0.0);
        let Ok(top_viewport_pos) = camera.world_to_viewport(&camera_global, top_world) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        // Hide plate once the entity itself is fully outside viewport bounds.
        // Center-only checks cause bars to linger at screen edges.
        let right_world = Vec3::new(world_pos.x + *half_extent_world, world_pos.y, 0.0);
        let Ok(right_viewport_pos) = camera.world_to_viewport(&camera_global, right_world) else {
            *visibility = Visibility::Hidden;
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
            continue;
        }
        let plate_width = 100.0;
        let plate_height = 8.0;
        let vertical_gap = 6.0;
        node.left = px(viewport_pos.x - plate_width * 0.5);
        let entity_top_y_px = viewport_pos.y.min(top_viewport_pos.y);
        node.top = px(entity_top_y_px - plate_height - vertical_gap);
        *visibility = Visibility::Visible;

        if let Some((_, _, health_ratio)) = entity_data_by_entity.get(&root.target) {
            for (bar_target, mut value) in &mut health_bars {
                if bar_target.target == root.target {
                    value.ratio = *health_ratio;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::propagate_ui_overlay_layer_system;
    use crate::native::components::UiOverlayLayer;
    use crate::native::platform::UI_OVERLAY_RENDER_LAYER;
    use bevy::camera::visibility::RenderLayers;
    use bevy::prelude::*;

    #[test]
    fn ui_overlay_layer_propagates_to_new_children_only_when_needed() {
        let mut app = App::new();
        app.add_systems(Update, propagate_ui_overlay_layer_system);

        let root = app.world_mut().spawn(UiOverlayLayer).id();
        let child = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(root).add_child(child);

        app.update();

        let child_ref = app.world().entity(child);
        assert!(child_ref.contains::<UiOverlayLayer>());
        let layers = child_ref
            .get::<RenderLayers>()
            .expect("child render layers should be propagated");
        assert!(layers.intersects(&RenderLayers::layer(UI_OVERLAY_RENDER_LAYER)));
    }
}

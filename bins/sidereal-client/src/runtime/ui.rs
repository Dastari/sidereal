//! World HUD and owned-entity panel systems.

use avian2d::prelude::{LinearVelocity, Rotation};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::log::info;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::MeshMaterial2d;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::window::PrimaryWindow;
use bevy_svg::prelude::{Svg, Svg2d};
use sidereal_game::{
    EntityAction, EntityGuid, FuelTank, HealthPool, MapIcon, MountedOn, PlanetBodyShaderSettings,
    SizeM, TacticalMapUiSettings, TacticalPresentationDefaults,
};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use sidereal_ui::layout;
use sidereal_ui::theme::{ActiveUiTheme, UiVisualSettings, theme_definition};
use sidereal_ui::typography::text_font;
use sidereal_ui::widgets::{
    UiButtonVariant, UiInteractionState, button_surface, panel_surface, spawn_hud_frame_chrome,
};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use super::app_state::{
    ClientAppState, ClientSession, LocalPlayerViewState, OwnedEntitiesPanelState,
};
use super::assets::{LocalAssetManager, RuntimeAssetHttpFetchState, RuntimeAssetNetIndicatorState};
use super::backdrop::TacticalMapOverlayMaterial;
use super::components::{
    ActiveNameplateEntry, AnnotationCalloutLine, AnnotationCalloutPlacement, AnnotationCalloutRoot,
    AnnotationCalloutText, CanonicalPresentationEntity, ControlledEntity,
    DebugOverlayPanelLabelShadowText, DebugOverlayPanelLabelText, DebugOverlayPanelRoot,
    DebugOverlayPanelSecondaryLabelShadowText, DebugOverlayPanelSecondaryLabelText,
    DebugOverlayPanelSecondaryValueShadowText, DebugOverlayPanelSecondaryValueText,
    DebugOverlayPanelTertiaryLabelShadowText, DebugOverlayPanelTertiaryLabelText,
    DebugOverlayPanelTertiaryValueShadowText, DebugOverlayPanelTertiaryValueText,
    DebugOverlayPanelText, DebugOverlayPanelValueShadowText, DebugOverlayPanelValueText,
    EntityNameplateHealthFill, EntityNameplateRoot, GameplayCamera, GameplayHud, HudFuelBarFill,
    HudHealthBarFill, HudPositionValueText, HudSpeedValueText, LoadingOverlayRoot,
    LoadingOverlayText, LoadingProgressBarFill, OwnedEntitiesPanelAction, OwnedEntitiesPanelButton,
    OwnedEntitiesPanelRoot, ResolvedRuntimeRenderLayer, RuntimeScreenOverlayPass,
    RuntimeScreenOverlayPassKind, SegmentedBarSegment, SegmentedBarStyle, SegmentedBarValue,
    TacticalMapCursorText, TacticalMapMarkerDynamic, TacticalMapOverlayRoot, TacticalMapTitle,
    UiOverlayCamera, UiOverlayLayer, WorldEntity,
};
use super::dev_console::{DevConsoleState, is_console_open};
use super::ecs_util::queue_despawn_if_exists;
use super::platform::{ORTHO_SCALE_PER_DISTANCE, UI_OVERLAY_RENDER_LAYER};
use super::resources::{
    AnnotationCalloutEntry, AnnotationCalloutRegistry, CameraMotionState,
    ClientControlRequestState, ClientInputSendState, DebugOverlayDisplayMetrics,
    DebugOverlaySnapshot, DebugOverlayState, EmbeddedFonts, HudPerfCounters, NameplateRegistry,
    NameplateRegistryEntry, NameplateUiState, OwnedAssetManifestCache, TacticalContactsCache,
    TacticalFogCache, TacticalMapUiState,
};

type AnnotationCalloutLineQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Node,
        &'static mut UiTransform,
        &'static mut Visibility,
    ),
    (With<AnnotationCalloutLine>, Without<AnnotationCalloutRoot>),
>;

type AnnotationCalloutTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static GlobalTransform>,
        Option<&'static Visibility>,
        Option<&'static SizeM>,
        Option<&'static PlanetBodyShaderSettings>,
        Option<&'static ResolvedRuntimeRenderLayer>,
    ),
    (
        Without<AnnotationCalloutRoot>,
        Without<AnnotationCalloutLine>,
    ),
>;

#[allow(clippy::type_complexity)]
#[derive(SystemParam)]
pub(super) struct DebugOverlayTextUiQueries<'w, 's> {
    root_query: Query<'w, 's, &'static mut Visibility, With<DebugOverlayPanelRoot>>,
    text_query: Query<
        'w,
        's,
        (
            &'static mut Text,
            Option<&'static mut TextColor>,
            Option<&'static DebugOverlayPanelLabelText>,
            Option<&'static DebugOverlayPanelLabelShadowText>,
            Option<&'static DebugOverlayPanelValueText>,
            Option<&'static DebugOverlayPanelValueShadowText>,
            Option<&'static DebugOverlayPanelSecondaryLabelText>,
            Option<&'static DebugOverlayPanelSecondaryLabelShadowText>,
            Option<&'static DebugOverlayPanelSecondaryValueText>,
            Option<&'static DebugOverlayPanelSecondaryValueShadowText>,
            Option<&'static DebugOverlayPanelTertiaryLabelText>,
            Option<&'static DebugOverlayPanelTertiaryLabelShadowText>,
            Option<&'static DebugOverlayPanelTertiaryValueText>,
            Option<&'static DebugOverlayPanelTertiaryValueShadowText>,
        ),
        With<DebugOverlayPanelText>,
    >,
}

#[allow(clippy::type_complexity)]
#[derive(SystemParam)]
pub(super) struct AnnotationCalloutUiQueries<'w, 's> {
    registry: ResMut<'w, AnnotationCalloutRegistry>,
    root_query: Query<
        'w,
        's,
        (
            &'static AnnotationCalloutRoot,
            &'static mut Node,
            &'static mut Visibility,
        ),
        Without<AnnotationCalloutLine>,
    >,
    text_query: Query<'w, 's, &'static mut Text, With<AnnotationCalloutText>>,
    line_query: AnnotationCalloutLineQuery<'w, 's>,
    target_query: AnnotationCalloutTargetQuery<'w, 's>,
    gameplay_camera:
        Query<'w, 's, (Entity, &'static Camera, &'static Transform), With<GameplayCamera>>,
    window_query: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
}

const TACTICAL_FOG_MASK_RESOLUTION: u32 = 384;
const TACTICAL_ICON_WORLD_HEIGHT_M: f32 = 24.0;
const TACTICAL_PLANET_ICON_SCALE_MULTIPLIER: f32 = 8.0;
const TACTICAL_GRAVITY_WELL_COUNT: usize = 4;
const TACTICAL_GRAVITY_WELL_MIN_RADIUS_M: f32 = 180.0;
const TACTICAL_GRAVITY_WELL_MAX_RADIUS_M: f32 = 4_500.0;
const TACTICAL_CONTACT_SMOOTHING_RATE: f32 = 8.0;
const TACTICAL_CONTACT_PREDICTION_HORIZON_S: f32 = 0.25;
const DEBUG_OVERLAY_TEXT_COLUMN_COUNT: usize = 3;
const DEBUG_OVERLAY_VALUE_MAX_CHARS: usize = 30;
const DEBUG_CALLOUT_WIDTH_PX: f32 = 178.0;
const DEBUG_CALLOUT_ROW_HEIGHT_PX: f32 = 11.0;
const DEBUG_CALLOUT_PADDING_PX: f32 = 5.0;
const DEBUG_CALLOUT_TARGET_GAP_PX: f32 = 38.0;
const DEBUG_CALLOUT_LINE_THICKNESS_PX: f32 = 1.5;
const DEBUG_CALLOUT_HOVER_RADIUS_PX: f32 = 16.0;
const DEBUG_CALLOUT_VIEWPORT_MARGIN_PX: f32 = 8.0;
const NAMEPLATE_BAR_WIDTH_PX: f32 = 100.0;
const NAMEPLATE_BAR_HEIGHT_PX: f32 = 8.0;
const NAMEPLATE_VERTICAL_GAP_PX: f32 = 6.0;

#[derive(Debug, Default, PartialEq, Eq)]
struct DebugOverlayTextColumn {
    labels: Vec<String>,
    values: Vec<String>,
}

fn elapsed_ms(started_at: Instant) -> f64 {
    started_at.elapsed().as_secs_f64() * 1000.0
}

#[derive(Default)]
pub(super) struct TacticalFogMaskUpdateState {
    initialized: bool,
    fog_revision: u64,
    viewport_width_px: u32,
    viewport_height_px: u32,
    world_center: Vec2,
    map_zoom: f32,
}

fn nameplate_shell_color() -> Color {
    Color::srgba(0.05, 0.08, 0.05, 0.75)
}

fn nameplate_border_color() -> Color {
    Color::srgba(0.18, 0.35, 0.18, 0.8)
}

fn nameplate_fill_color() -> Color {
    Color::srgb(0.22, 0.9, 0.34)
}

fn spawn_nameplate_entry(commands: &mut Commands<'_, '_>) -> NameplateRegistryEntry {
    let root = commands
        .spawn((
            Name::new("Nameplate"),
            Node {
                position_type: PositionType::Absolute,
                width: px(NAMEPLATE_BAR_WIDTH_PX),
                height: px(NAMEPLATE_BAR_HEIGHT_PX),
                left: px(0.0),
                top: px(0.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Visibility::Hidden,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .id();
    let shell = commands
        .spawn((
            Name::new("NameplateShell"),
            Node {
                width: percent(100.0),
                height: percent(100.0),
                border: UiRect::all(px(1.0)),
                padding: UiRect::all(px(1.0)),
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(nameplate_shell_color()),
            BorderColor::all(nameplate_border_color()),
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        ))
        .id();
    let health_fill = commands
        .spawn((
            Name::new("NameplateFill"),
            Node {
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(nameplate_fill_color()),
            EntityNameplateHealthFill,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        ))
        .id();
    commands.entity(shell).add_child(health_fill);
    commands.entity(root).add_child(shell);
    commands.entity(root).insert(EntityNameplateRoot {
        target: None,
        health_fill,
    });
    NameplateRegistryEntry { root, health_fill }
}

fn release_nameplate_entry(
    commands: &mut Commands<'_, '_>,
    registry: &mut NameplateRegistry,
    entry: NameplateRegistryEntry,
) {
    registry.free_entries.push(entry);
    if let Ok(mut root_commands) = commands.get_entity(entry.root) {
        root_commands.insert((
            Visibility::Hidden,
            EntityNameplateRoot {
                target: None,
                health_fill: entry.health_fill,
            },
        ));
        root_commands.remove::<ActiveNameplateEntry>();
    }
}

fn split_debug_overlay_text_columns(
    row_pairs: &[(String, String)],
) -> [DebugOverlayTextColumn; DEBUG_OVERLAY_TEXT_COLUMN_COUNT] {
    let mut columns = std::array::from_fn(|_| DebugOverlayTextColumn::default());
    let mut dynamic_rows = Vec::new();

    for (label, value) in row_pairs {
        if let Some(column_index) = preferred_debug_overlay_text_column(label) {
            columns[column_index].labels.push(label.clone());
            columns[column_index].values.push(value.clone());
        } else {
            dynamic_rows.push((label, value));
        }
    }

    let rows_per_dynamic_column = dynamic_rows
        .len()
        .div_ceil(DEBUG_OVERLAY_TEXT_COLUMN_COUNT)
        .max(1);
    for (index, (label, value)) in dynamic_rows.into_iter().enumerate() {
        let column_index =
            (index / rows_per_dynamic_column).min(DEBUG_OVERLAY_TEXT_COLUMN_COUNT - 1);
        columns[column_index].labels.push(label.clone());
        columns[column_index].values.push(value.clone());
    }
    columns
}

fn preferred_debug_overlay_text_column(label: &str) -> Option<usize> {
    match label {
        "Sent Input" | "Recover Input" | "Control Lane" | "Ctrl Bootstrap" | "Control GUID"
        | "Confirmed Ghost" => Some(DEBUG_OVERLAY_TEXT_COLUMN_COUNT - 1),
        _ => None,
    }
}

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
    input_send_state: Res<'_, ClientInputSendState>,
    mut display_metrics: Local<'_, DebugOverlayDisplayMetrics>,
    mut ui_queries: DebugOverlayTextUiQueries<'_, '_>,
) {
    let Ok(mut root_visibility) = ui_queries.root_query.single_mut() else {
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

    let mut header_row_pairs = Vec::with_capacity(2);
    header_row_pairs.push((
        "FPS".to_string(),
        display_metrics
            .sampled_fps
            .map(|value| format!("{value:.0}"))
            .unwrap_or_else(|| "--".to_string()),
    ));
    header_row_pairs.push((
        "Frame Time".to_string(),
        display_metrics
            .sampled_frame_ms
            .map(|value| format!("{value:.2} ms"))
            .unwrap_or_else(|| "--.-- ms".to_string()),
    ));
    let mut row_pairs = Vec::with_capacity(snapshot.text_rows.len() + 1);
    row_pairs.push((
        "Sent Input".to_string(),
        format_sent_input_actions(&input_send_state.last_sent_actions),
    ));
    for row in &snapshot.text_rows {
        row_pairs.push((
            row.label.clone(),
            truncate_debug_overlay_value(&row.value, DEBUG_OVERLAY_VALUE_MAX_CHARS),
        ));
    }
    let columns = split_debug_overlay_text_columns(&row_pairs);
    let header_labels_text = header_row_pairs
        .iter()
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let header_values_text = header_row_pairs
        .iter()
        .map(|(_, value)| value.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let primary_labels_text = if columns[0].labels.is_empty() {
        header_labels_text.clone()
    } else {
        format!("{header_labels_text}\n{}", columns[0].labels.join("\n"))
    };
    let primary_values_text = if columns[0].values.is_empty() {
        header_values_text.clone()
    } else {
        format!("{header_values_text}\n{}", columns[0].values.join("\n"))
    };
    let secondary_labels_text = columns[1].labels.join("\n");
    let secondary_values_text = columns[1].values.join("\n");
    let tertiary_labels_text = columns[2].labels.join("\n");
    let tertiary_values_text = columns[2].values.join("\n");

    let debug_value_color = Color::srgb(0.85, 0.92, 1.0);
    for (
        mut text,
        color,
        primary_label,
        primary_label_shadow,
        primary_value,
        primary_value_shadow,
        secondary_label,
        secondary_label_shadow,
        secondary_value,
        secondary_value_shadow,
        tertiary_label,
        tertiary_label_shadow,
        tertiary_value,
        tertiary_value_shadow,
    ) in &mut ui_queries.text_query
    {
        if primary_label.is_some() || primary_label_shadow.is_some() {
            text.0 = primary_labels_text.clone();
        } else if primary_value.is_some() {
            text.0 = primary_values_text.clone();
            if let Some(mut color) = color {
                color.0 = debug_value_color;
            }
        } else if primary_value_shadow.is_some() {
            text.0 = primary_values_text.clone();
        } else if secondary_label.is_some() || secondary_label_shadow.is_some() {
            text.0 = secondary_labels_text.clone();
        } else if secondary_value.is_some() {
            text.0 = secondary_values_text.clone();
            if let Some(mut color) = color {
                color.0 = debug_value_color;
            }
        } else if secondary_value_shadow.is_some() {
            text.0 = secondary_values_text.clone();
        } else if tertiary_label.is_some() || tertiary_label_shadow.is_some() {
            text.0 = tertiary_labels_text.clone();
        } else if tertiary_value.is_some() {
            text.0 = tertiary_values_text.clone();
            if let Some(mut color) = color {
                color.0 = debug_value_color;
            }
        } else if tertiary_value_shadow.is_some() {
            text.0 = tertiary_values_text.clone();
        }
    }
}

fn format_sent_input_actions(actions: &[EntityAction]) -> String {
    if actions.is_empty() {
        return "[]".to_string();
    }

    let names: Vec<&'static str> = actions.iter().map(describe_entity_action).collect();
    let value = format!("[{}]", names.join(", "));
    truncate_debug_overlay_value(&value, DEBUG_OVERLAY_VALUE_MAX_CHARS)
}

fn truncate_debug_overlay_value(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(3);
    let truncated = value.chars().take(keep).collect::<String>();
    format!("{truncated}...")
}

fn describe_entity_action(action: &EntityAction) -> &'static str {
    match action {
        EntityAction::Forward => "Forward",
        EntityAction::Backward => "Backward",
        EntityAction::LongitudinalNeutral => "Long Neutral",
        EntityAction::Left => "Left",
        EntityAction::Right => "Right",
        EntityAction::LateralNeutral => "Turn Neutral",
        EntityAction::Brake => "Brake",
        EntityAction::AfterburnerOn => "Afterburner On",
        EntityAction::AfterburnerOff => "Afterburner Off",
        EntityAction::FirePrimary => "Fire Primary",
        EntityAction::FireSecondary => "Fire Secondary",
        EntityAction::ActivateShield => "Shield On",
        EntityAction::DeactivateShield => "Shield Off",
        EntityAction::ActivateTractor => "Tractor On",
        EntityAction::DeactivateTractor => "Tractor Off",
        EntityAction::ActivateScanner => "Scanner On",
        EntityAction::DeployCargo => "Deploy Cargo",
        EntityAction::EngageAutopilot => "Autopilot On",
        EntityAction::DisengageAutopilot => "Autopilot Off",
        EntityAction::InitiateDocking => "Dock",
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

fn tactical_marker_scale_multiplier(kind: &str) -> f32 {
    if kind.eq_ignore_ascii_case("planet") {
        TACTICAL_PLANET_ICON_SCALE_MULTIPLIER
    } else {
        1.0
    }
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn update_runtime_screen_overlay_passes_system(
    time: Res<'_, Time>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    fog_cache: Res<'_, TacticalFogCache>,
    contacts_cache: Res<'_, TacticalContactsCache>,
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
    mut fog_mask_state: Local<'_, TacticalFogMaskUpdateState>,
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
    if alpha <= 0.001 {
        return;
    }
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
                    &contacts_cache,
                    &map_settings,
                    width,
                    height,
                    time.elapsed_secs(),
                    alpha,
                    world_center,
                    map_zoom,
                    &mut fog_mask_state,
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
    contacts_cache: &TacticalContactsCache,
    map_settings: &TacticalMapUiSettings,
    width: f32,
    height: f32,
    time_s: f32,
    alpha: f32,
    world_center: Vec2,
    map_zoom: f32,
    fog_mask_state: &mut TacticalFogMaskUpdateState,
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
    update_tactical_gravity_well_uniforms(material, contacts_cache, world_center);
    update_tactical_fog_mask_texture(
        images,
        material,
        fog_cache,
        width,
        height,
        world_center,
        map_zoom,
        fog_mask_state,
    );
}

fn update_tactical_gravity_well_uniforms(
    material: &mut TacticalMapOverlayMaterial,
    contacts_cache: &TacticalContactsCache,
    world_center: Vec2,
) {
    let mut wells = contacts_cache
        .contacts_by_entity_id
        .values()
        .filter_map(tactical_gravity_well_from_contact)
        .collect::<Vec<_>>();
    wells.sort_by(|left, right| {
        let left_score = left.radius_m * left.mass_scale
            / left.center.distance_squared(world_center).max(1.0).sqrt();
        let right_score = right.radius_m * right.mass_scale
            / right.center.distance_squared(world_center).max(1.0).sqrt();
        right_score.total_cmp(&left_score)
    });

    let mut uniforms = [Vec4::ZERO; TACTICAL_GRAVITY_WELL_COUNT];
    for (index, well) in wells.iter().take(TACTICAL_GRAVITY_WELL_COUNT).enumerate() {
        uniforms[index] = Vec4::new(well.center.x, well.center.y, well.radius_m, well.mass_scale);
    }

    material.gravity_well_params = Vec4::new(
        wells.len().min(TACTICAL_GRAVITY_WELL_COUNT) as f32,
        0.18,
        0.45,
        0.0,
    );
    material.gravity_well_0 = uniforms[0];
    material.gravity_well_1 = uniforms[1];
    material.gravity_well_2 = uniforms[2];
    material.gravity_well_3 = uniforms[3];
}

struct TacticalGravityWell {
    center: Vec2,
    radius_m: f32,
    mass_scale: f32,
}

fn tactical_gravity_well_from_contact(
    contact: &sidereal_net::TacticalContact,
) -> Option<TacticalGravityWell> {
    if !contact.is_live_now || !tactical_contact_has_gravity_well(contact.kind.as_str()) {
        return None;
    }

    let size_radius = contact
        .size_m
        .map(|size| size.into_iter().fold(0.0_f32, f32::max) * 0.5)
        .unwrap_or(0.0);
    let mass_scale = contact
        .mass_kg
        .filter(|mass| *mass > 0.0)
        .map(|mass| (mass.log10() / 12.0).clamp(0.75, 2.5))
        .unwrap_or(1.0);
    let radius_m = (size_radius * (5.0 + mass_scale * 2.0))
        .max(TACTICAL_GRAVITY_WELL_MIN_RADIUS_M)
        .clamp(
            TACTICAL_GRAVITY_WELL_MIN_RADIUS_M,
            TACTICAL_GRAVITY_WELL_MAX_RADIUS_M,
        );

    Some(TacticalGravityWell {
        center: Vec2::new(contact.position_xy[0] as f32, contact.position_xy[1] as f32),
        radius_m,
        mass_scale,
    })
}

fn tactical_contact_has_gravity_well(kind: &str) -> bool {
    matches!(
        kind.to_ascii_lowercase().as_str(),
        "planet" | "star" | "blackhole" | "black_hole"
    )
}

#[allow(clippy::too_many_arguments)]
fn update_tactical_fog_mask_texture(
    images: &mut Assets<Image>,
    material: &TacticalMapOverlayMaterial,
    fog_cache: &TacticalFogCache,
    viewport_width_px: f32,
    viewport_height_px: f32,
    world_center: Vec2,
    map_zoom_px_per_world: f32,
    build_state: &mut TacticalFogMaskUpdateState,
) {
    let Some(image) = images.get_mut(&material.fog_mask) else {
        return;
    };
    let expected_len = (TACTICAL_FOG_MASK_RESOLUTION * TACTICAL_FOG_MASK_RESOLUTION) as usize;
    let needs_rebuild = image.texture_descriptor.size.width != TACTICAL_FOG_MASK_RESOLUTION
        || image.texture_descriptor.size.height != TACTICAL_FOG_MASK_RESOLUTION
        || image.texture_descriptor.format != TextureFormat::R8Unorm
        || image.data.as_ref().map_or(0, Vec::len) != expected_len;
    let viewport_width_u32 = viewport_width_px.max(0.0).round() as u32;
    let viewport_height_u32 = viewport_height_px.max(0.0).round() as u32;
    let params_changed = !build_state.initialized
        || build_state.fog_revision != fog_cache.revision
        || build_state.viewport_width_px != viewport_width_u32
        || build_state.viewport_height_px != viewport_height_u32
        || build_state.world_center.distance_squared(world_center) > 0.0001
        || (build_state.map_zoom - map_zoom_px_per_world).abs() > 0.0001;
    if !needs_rebuild && !params_changed {
        return;
    }
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
        build_state.initialized = true;
        build_state.fog_revision = fog_cache.revision;
        build_state.viewport_width_px = viewport_width_u32;
        build_state.viewport_height_px = viewport_height_u32;
        build_state.world_center = world_center;
        build_state.map_zoom = map_zoom_px_per_world;
        return;
    }

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
            mask[index] = if fog_cache.revealed_cells.contains(&sidereal_net::GridCell {
                x: cell_x,
                y: cell_y,
            }) {
                255
            } else {
                0
            };
        }
    }
    build_state.initialized = true;
    build_state.fog_revision = fog_cache.revision;
    build_state.viewport_width_px = viewport_width_u32;
    build_state.viewport_height_px = viewport_height_u32;
    build_state.world_center = world_center;
    build_state.map_zoom = map_zoom_px_per_world;
}

fn ids_refer_to_same_guid(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    parse_guid_from_entity_id(left)
        .zip(parse_guid_from_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

fn format_sector_code(x: f32, y: f32) -> String {
    let sector_size = 1000.0;
    let sector_x = (x / sector_size).floor() as i32;
    let sector_y = (y / sector_size).floor() as i32;
    let east_west = if sector_x >= 0 { 'E' } else { 'W' };
    let north_south = if sector_y >= 0 { 'N' } else { 'S' };
    format!(
        "{east_west}{:02}-{north_south}{:02}",
        sector_x.abs(),
        sector_y.abs()
    )
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn update_owned_entities_panel_system(
    mut commands: Commands<'_, '_>,
    mut images: ResMut<'_, Assets<Image>>,
    fonts: Res<'_, EmbeddedFonts>,
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
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
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    let (panel_bg, panel_border, panel_shadow) = panel_surface(theme, glow_intensity);

    for panel in &existing_panels {
        queue_despawn_if_exists(&mut commands, panel);
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(12),
                ..layout::panel(
                    px(280),
                    10.0,
                    8.0,
                    theme.metrics.panel_radius_px,
                    theme.metrics.panel_border_px,
                )
            },
            panel_bg,
            panel_border,
            panel_shadow,
            OwnedEntitiesPanelRoot,
            GameplayHud,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|panel| {
            spawn_hud_frame_chrome(
                panel,
                &mut images,
                theme,
                Some("Owned Fleet"),
                &fonts.mono,
                glow_intensity,
            );
            panel.spawn((
                Text::new("Owned Ships"),
                text_font(fonts.bold.clone(), 18.0),
                TextColor(theme.colors.foreground_color()),
            ));

            let free_roam_selected = selected_id
                .as_deref()
                .is_some_and(|selected| ids_refer_to_same_guid(selected, local_player_entity_id))
                && !player_view_state.detached_free_camera;
            let free_roam_state = if free_roam_selected {
                UiInteractionState::Selected
            } else {
                UiInteractionState::Idle
            };
            let (free_roam_bg, free_roam_border, free_roam_shadow) = button_surface(
                theme,
                UiButtonVariant::Secondary,
                free_roam_state,
                glow_intensity,
            );
            panel
                .spawn((
                    Button,
                    OwnedEntitiesPanelButton {
                        action: OwnedEntitiesPanelAction::FreeRoam,
                    },
                    layout::leading_button(
                        percent(100.0),
                        34.0,
                        theme.metrics.input_radius_px,
                        theme.metrics.control_border_px,
                        10.0,
                    ),
                    free_roam_bg,
                    free_roam_border,
                    free_roam_shadow,
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("FREE ROAM"),
                        text_font(fonts.mono_bold.clone(), 17.0),
                        TextColor(theme.colors.panel_foreground_color()),
                    ));
                });
            if owned_ship_rows.is_empty() {
                panel.spawn((
                    Text::new("No owned entities visible"),
                    text_font(fonts.regular.clone(), 13.0),
                    TextColor(theme.colors.muted_foreground_color()),
                ));
            } else {
                for (entity_id, display_label) in owned_ship_rows {
                    let is_selected = selected_id.as_deref().is_some_and(|selected| {
                        ids_refer_to_same_guid(selected, entity_id.as_str())
                    });
                    let button_state = if is_selected {
                        UiInteractionState::Selected
                    } else {
                        UiInteractionState::Idle
                    };
                    let (button_bg, button_border, button_shadow) = button_surface(
                        theme,
                        UiButtonVariant::Secondary,
                        button_state,
                        glow_intensity,
                    );
                    panel
                        .spawn((
                            Button,
                            OwnedEntitiesPanelButton {
                                action: OwnedEntitiesPanelAction::ControlEntity(entity_id),
                            },
                            layout::leading_button(
                                percent(100.0),
                                34.0,
                                theme.metrics.input_radius_px,
                                theme.metrics.control_border_px,
                                10.0,
                            ),
                            button_bg,
                            button_border,
                            button_shadow,
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(display_label.to_ascii_uppercase()),
                                text_font(fonts.mono_bold.clone(), 17.0),
                                TextColor(theme.colors.panel_foreground_color()),
                            ));
                        });
                }
            }
        });
}

#[allow(clippy::type_complexity)]
pub(super) fn handle_owned_entities_panel_buttons(
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            &OwnedEntitiesPanelButton,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut BoxShadow,
        ),
        Changed<Interaction>,
    >,
    session: Res<'_, ClientSession>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut control_request_state: ResMut<'_, ClientControlRequestState>,
    mut panel_state: ResMut<'_, OwnedEntitiesPanelState>,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    for (interaction, button, mut color, mut border, mut shadow) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                match &button.action {
                    OwnedEntitiesPanelAction::FreeRoam => {
                        let target = session.player_entity_id.clone();
                        let next_request_seq =
                            control_request_state.next_request_seq.saturating_add(1);
                        info!(
                            "client control selection requested via owned panel player={} target={} seq={}",
                            session.player_entity_id.as_deref().unwrap_or("<none>"),
                            target.as_deref().unwrap_or("<player-anchor>"),
                            next_request_seq
                        );
                        player_view_state.desired_controlled_entity_id = target.clone();
                        control_request_state.next_request_seq = next_request_seq;
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
                        let next_request_seq =
                            control_request_state.next_request_seq.saturating_add(1);
                        info!(
                            "client control selection requested via owned panel player={} target={} seq={}",
                            session.player_entity_id.as_deref().unwrap_or("<none>"),
                            entity_id,
                            next_request_seq
                        );
                        player_view_state.desired_controlled_entity_id = Some(entity_id.clone());
                        control_request_state.next_request_seq = next_request_seq;
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
            }
            Interaction::Hovered => {}
            Interaction::None => {}
        }
        let state = match *interaction {
            Interaction::Pressed => UiInteractionState::Pressed,
            Interaction::Hovered => UiInteractionState::Hovered,
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
                if is_selected {
                    UiInteractionState::Selected
                } else {
                    UiInteractionState::Idle
                }
            }
        };
        let (next_bg, next_border, next_shadow) =
            button_surface(theme, UiButtonVariant::Secondary, state, glow_intensity);
        *color = next_bg;
        *border = next_border;
        *shadow = next_shadow;
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

#[allow(clippy::too_many_arguments)]
pub(super) fn sync_debug_entity_callouts_system(
    debug_overlay: Res<'_, DebugOverlayState>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    camera_motion: Res<'_, CameraMotionState>,
    snapshot: Res<'_, DebugOverlaySnapshot>,
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    mut ui_queries: AnnotationCalloutUiQueries<'_, '_>,
) {
    if !debug_overlay.enabled || tactical_map_state.enabled {
        for (_, _, mut visibility) in &mut ui_queries.root_query {
            *visibility = Visibility::Hidden;
        }
        for (_, _, mut visibility) in &mut ui_queries.line_query {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let Some((_camera_entity, camera, camera_transform)) =
        select_annotation_callout_camera(&ui_queries.gameplay_camera)
    else {
        for (_, _, mut visibility) in &mut ui_queries.root_query {
            *visibility = Visibility::Hidden;
        }
        for (_, _, mut visibility) in &mut ui_queries.line_query {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let camera_global = GlobalTransform::from(camera_transform);
    let (cursor_px, window_size) = {
        let Ok(window) = ui_queries.window_query.single() else {
            return;
        };
        (
            window.cursor_position(),
            Vec2::new(window.width(), window.height()),
        )
    };
    let controlled_position = snapshot
        .entities
        .iter()
        .find(|entity| {
            entity.is_controlled
                && !matches!(
                    entity.lane,
                    super::resources::DebugEntityLane::Auxiliary
                        | super::resources::DebugEntityLane::ConfirmedGhost
                )
        })
        .map(|entity| entity.position_xy);
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();

    let desired_targets = snapshot
        .entities
        .iter()
        .map(|entity| entity.entity)
        .collect::<HashSet<_>>();
    let stale_targets = ui_queries
        .registry
        .active_by_target
        .keys()
        .copied()
        .filter(|target| !desired_targets.contains(target))
        .collect::<Vec<_>>();
    for target in stale_targets {
        if let Some(entry) = ui_queries.registry.active_by_target.remove(&target) {
            release_annotation_callout_entry(&mut commands, &mut ui_queries.registry, entry);
        }
    }

    for entity in &snapshot.entities {
        let Some(callout_target) = annotation_callout_target(
            entity,
            &ui_queries.target_query,
            &camera,
            &camera_global,
            camera_motion.parallax_position_xy,
        ) else {
            if let Some(entry) = ui_queries
                .registry
                .active_by_target
                .get(&entity.entity)
                .copied()
            {
                hide_annotation_callout_entry(&mut ui_queries, entry);
            }
            continue;
        };
        let viewport_pos = callout_target.center_viewport_pos;
        if viewport_pos.x < -DEBUG_CALLOUT_VIEWPORT_MARGIN_PX
            || viewport_pos.x > window_size.x + DEBUG_CALLOUT_VIEWPORT_MARGIN_PX
            || viewport_pos.y < -DEBUG_CALLOUT_VIEWPORT_MARGIN_PX
            || viewport_pos.y > window_size.y + DEBUG_CALLOUT_VIEWPORT_MARGIN_PX
        {
            if let Some(entry) = ui_queries
                .registry
                .active_by_target
                .get(&entity.entity)
                .copied()
            {
                hide_annotation_callout_entry(&mut ui_queries, entry);
            }
            continue;
        }

        let entry = if let Some(entry) = ui_queries
            .registry
            .active_by_target
            .get(&entity.entity)
            .copied()
        {
            entry
        } else {
            ui_queries.registry.allocated_entries =
                ui_queries.registry.allocated_entries.saturating_add(1);
            let entry = ui_queries.registry.free_entries.pop().unwrap_or_else(|| {
                spawn_annotation_callout_entry(&mut commands, &fonts, theme, glow_intensity)
            });
            ui_queries
                .registry
                .active_by_target
                .insert(entity.entity, entry);
            entry
        };
        let hovered = cursor_px.is_some_and(|cursor_px| {
            cursor_px.distance(viewport_pos) <= DEBUG_CALLOUT_HOVER_RADIUS_PX
        });
        let placement = if entity.is_component {
            AnnotationCalloutPlacement::BottomRight
        } else {
            AnnotationCalloutPlacement::TopLeft
        };
        if entity.is_component && !hovered {
            hide_annotation_callout_entry(&mut ui_queries, entry);
            continue;
        }
        if let Ok((root, mut node, mut visibility)) = ui_queries.root_query.get_mut(entry.root) {
            let text = annotation_callout_text(entity, controlled_position);
            let line_count = text.lines().count().max(1) as f32;
            let height_px =
                line_count * DEBUG_CALLOUT_ROW_HEIGHT_PX + DEBUG_CALLOUT_PADDING_PX * 2.0 + 2.0;
            let callout_rect = annotation_callout_rect(
                callout_target.anchor_viewport_pos,
                height_px,
                window_size,
                placement,
            );
            node.left = px(callout_rect.min.x);
            node.top = px(callout_rect.min.y);
            node.height = px(height_px);
            *visibility = Visibility::Visible;
            if (root.target, root.placement) != (Some(entity.entity), placement)
                && let Ok(mut root_commands) = commands.get_entity(entry.root)
            {
                root_commands.insert(AnnotationCalloutRoot {
                    target: Some(entity.entity),
                    placement,
                });
            }
            if let Ok(mut text_value) = ui_queries.text_query.get_mut(entry.text) {
                text_value.0 = text;
            }
            sync_annotation_callout_line(
                &mut ui_queries.line_query,
                entry.line,
                callout_rect,
                viewport_pos,
            );
        }
    }
}

#[derive(Clone, Copy)]
struct AnnotationCalloutTarget {
    center_viewport_pos: Vec2,
    anchor_viewport_pos: Vec2,
}

#[derive(Clone, Copy)]
struct AnnotationCalloutRect {
    min: Vec2,
    max: Vec2,
}

fn annotation_callout_target(
    entity: &super::resources::DebugOverlayEntity,
    target_query: &AnnotationCalloutTargetQuery<'_, '_>,
    camera: &Camera,
    camera_global: &GlobalTransform,
    parallax_position_xy: Vec2,
) -> Option<AnnotationCalloutTarget> {
    if entity.is_component {
        let (center_world, anchor_world) = annotation_callout_snapshot_world_positions(entity);
        return project_annotation_callout_target(
            camera,
            camera_global,
            center_world,
            anchor_world,
        );
    }
    let (center_world, anchor_world) = if let Ok((
        global_transform,
        maybe_visibility,
        size_m,
        planet_settings,
        resolved_render_layer,
    )) = target_query.get(entity.entity)
    {
        if maybe_visibility
            .is_some_and(|entity_visibility| *entity_visibility == Visibility::Hidden)
        {
            return None;
        }
        if let Some(global_transform) = global_transform {
            let world_pos = global_transform.translation();
            if planet_settings.is_some_and(|settings| settings.enabled) {
                let projected_center_world = super::visuals::planet_camera_relative_translation(
                    resolved_render_layer,
                    world_pos.truncate(),
                    parallax_position_xy,
                );
                let radius_m = size_m
                    .map(|size| size.width.max(size.length) * 0.5)
                    .unwrap_or(128.0);
                let layer_screen_scale = resolved_render_layer
                    .map(|layer| {
                        super::visuals::runtime_layer_screen_scale_factor(&layer.definition)
                    })
                    .unwrap_or(1.0);
                let projected_radius_m = radius_m * layer_screen_scale;
                return project_annotation_callout_target(
                    camera,
                    camera_global,
                    projected_center_world.extend(0.0),
                    Vec3::new(
                        projected_center_world.x - projected_radius_m,
                        projected_center_world.y + projected_radius_m,
                        0.0,
                    ),
                );
            }
            let anchor_world = size_m
                .map(|size| {
                    Vec3::new(
                        world_pos.x - size.width * 0.5,
                        world_pos.y + size.length * 0.5,
                        0.0,
                    )
                })
                .unwrap_or(Vec3::new(world_pos.x, world_pos.y, 0.0));
            (Vec3::new(world_pos.x, world_pos.y, 0.0), anchor_world)
        } else {
            annotation_callout_snapshot_world_positions(entity)
        }
    } else {
        annotation_callout_snapshot_world_positions(entity)
    };
    project_annotation_callout_target(camera, camera_global, center_world, anchor_world)
}

fn project_annotation_callout_target(
    camera: &Camera,
    camera_global: &GlobalTransform,
    center_world: Vec3,
    anchor_world: Vec3,
) -> Option<AnnotationCalloutTarget> {
    let center_viewport_pos = camera.world_to_viewport(camera_global, center_world).ok()?;
    let anchor_viewport_pos = camera
        .world_to_viewport(camera_global, anchor_world)
        .unwrap_or(center_viewport_pos);
    Some(AnnotationCalloutTarget {
        center_viewport_pos,
        anchor_viewport_pos,
    })
}

fn annotation_callout_snapshot_world_positions(
    entity: &super::resources::DebugOverlayEntity,
) -> (Vec3, Vec3) {
    let center = entity.position_xy.extend(0.0);
    let anchor = match &entity.collision {
        super::resources::DebugCollisionShape::Aabb { half_extents } => Vec3::new(
            entity.position_xy.x - half_extents.x,
            entity.position_xy.y + half_extents.y,
            0.0,
        ),
        super::resources::DebugCollisionShape::Outline { points } if !points.is_empty() => {
            let min_x = points
                .iter()
                .map(|point| point.x)
                .fold(f32::INFINITY, f32::min);
            let max_y = points
                .iter()
                .map(|point| point.y)
                .fold(f32::NEG_INFINITY, f32::max);
            Vec3::new(
                entity.position_xy.x + min_x,
                entity.position_xy.y + max_y,
                0.0,
            )
        }
        _ => center,
    };
    (center, anchor)
}

fn annotation_callout_rect(
    anchor_viewport_pos: Vec2,
    height_px: f32,
    window_size: Vec2,
    placement: AnnotationCalloutPlacement,
) -> AnnotationCalloutRect {
    let (unclamped_left, unclamped_top) = match placement {
        AnnotationCalloutPlacement::TopLeft => (
            anchor_viewport_pos.x - DEBUG_CALLOUT_WIDTH_PX - DEBUG_CALLOUT_TARGET_GAP_PX,
            anchor_viewport_pos.y - height_px - DEBUG_CALLOUT_TARGET_GAP_PX,
        ),
        AnnotationCalloutPlacement::BottomRight => (
            anchor_viewport_pos.x + DEBUG_CALLOUT_TARGET_GAP_PX,
            anchor_viewport_pos.y + DEBUG_CALLOUT_TARGET_GAP_PX,
        ),
    };
    let max_left = (window_size.x - DEBUG_CALLOUT_WIDTH_PX - DEBUG_CALLOUT_VIEWPORT_MARGIN_PX)
        .max(DEBUG_CALLOUT_VIEWPORT_MARGIN_PX);
    let max_top = (window_size.y - height_px - DEBUG_CALLOUT_VIEWPORT_MARGIN_PX)
        .max(DEBUG_CALLOUT_VIEWPORT_MARGIN_PX);
    let min = Vec2::new(
        unclamped_left.clamp(DEBUG_CALLOUT_VIEWPORT_MARGIN_PX, max_left),
        unclamped_top.clamp(DEBUG_CALLOUT_VIEWPORT_MARGIN_PX, max_top),
    );
    AnnotationCalloutRect {
        min,
        max: min + Vec2::new(DEBUG_CALLOUT_WIDTH_PX, height_px),
    }
}

fn sync_annotation_callout_line(
    line_query: &mut AnnotationCalloutLineQuery<'_, '_>,
    line_entity: Entity,
    callout_rect: AnnotationCalloutRect,
    target_viewport_pos: Vec2,
) {
    let Ok((mut node, mut transform, mut visibility)) = line_query.get_mut(line_entity) else {
        return;
    };
    let start = closest_point_on_annotation_callout_rect(callout_rect, target_viewport_pos);
    let delta = target_viewport_pos - start;
    let length = delta.length();
    if length <= 1.0 {
        *visibility = Visibility::Hidden;
        return;
    }
    let midpoint = start + delta * 0.5;
    node.left = px(midpoint.x - length * 0.5);
    node.top = px(midpoint.y - DEBUG_CALLOUT_LINE_THICKNESS_PX * 0.5);
    node.width = px(length);
    node.height = px(DEBUG_CALLOUT_LINE_THICKNESS_PX);
    *transform = UiTransform::from_rotation(Rot2::radians(delta.y.atan2(delta.x)));
    *visibility = Visibility::Visible;
}

fn closest_point_on_annotation_callout_rect(rect: AnnotationCalloutRect, target: Vec2) -> Vec2 {
    Vec2::new(
        target.x.clamp(rect.min.x, rect.max.x),
        target.y.clamp(rect.min.y, rect.max.y),
    )
}

fn hide_annotation_callout_entry(
    ui_queries: &mut AnnotationCalloutUiQueries<'_, '_>,
    entry: AnnotationCalloutEntry,
) {
    if let Ok((_, _, mut visibility)) = ui_queries.root_query.get_mut(entry.root) {
        *visibility = Visibility::Hidden;
    }
    if let Ok((_, _, mut visibility)) = ui_queries.line_query.get_mut(entry.line) {
        *visibility = Visibility::Hidden;
    }
}

fn select_annotation_callout_camera(
    gameplay_camera: &Query<'_, '_, (Entity, &'_ Camera, &'_ Transform), With<GameplayCamera>>,
) -> Option<(Entity, Camera, Transform)> {
    let mut selected_camera: Option<(Entity, bool, Camera, Transform)> = None;
    for (entity, camera, transform) in gameplay_camera {
        let candidate = (entity, camera.is_active, camera.clone(), *transform);
        if selected_camera
            .as_ref()
            .is_none_or(|(current_entity, current_active, _, _)| {
                if camera.is_active != *current_active {
                    return camera.is_active;
                }
                entity.to_bits() < current_entity.to_bits()
            })
        {
            selected_camera = Some(candidate);
        }
    }
    selected_camera.map(|(entity, _, camera, transform)| (entity, camera, transform))
}

fn spawn_annotation_callout_entry(
    commands: &mut Commands<'_, '_>,
    fonts: &EmbeddedFonts,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
) -> AnnotationCalloutEntry {
    let (panel_bg, panel_border, panel_shadow) = panel_surface(theme, glow_intensity);
    let root = commands
        .spawn((
            Name::new("AnnotationCallout"),
            Node {
                position_type: PositionType::Absolute,
                width: px(DEBUG_CALLOUT_WIDTH_PX),
                height: px(48.0),
                left: px(0.0),
                top: px(0.0),
                ..layout::panel(
                    px(DEBUG_CALLOUT_WIDTH_PX),
                    DEBUG_CALLOUT_PADDING_PX,
                    0.0,
                    theme.metrics.panel_radius_px,
                    theme.metrics.panel_border_px,
                )
            },
            panel_bg,
            panel_border,
            panel_shadow,
            Visibility::Hidden,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            AnnotationCalloutRoot {
                target: None,
                placement: AnnotationCalloutPlacement::TopLeft,
            },
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(""),
            text_font(fonts.mono.clone(), 9.5),
            TextColor(Color::srgb(0.78, 1.0, 0.82)),
            AnnotationCalloutText,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        ))
        .id();
    let line = commands
        .spawn((
            Name::new("DebugEntityCalloutLine"),
            Node {
                position_type: PositionType::Absolute,
                width: px(1.0),
                height: px(DEBUG_CALLOUT_LINE_THICKNESS_PX),
                left: px(0.0),
                top: px(0.0),
                ..default()
            },
            UiTransform::IDENTITY,
            BackgroundColor(Color::srgba(0.22, 1.0, 0.4, 0.78)),
            Visibility::Hidden,
            UiOverlayLayer,
            AnnotationCalloutLine,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .id();
    commands.entity(root).add_child(text);
    AnnotationCalloutEntry { root, text, line }
}

fn release_annotation_callout_entry(
    commands: &mut Commands<'_, '_>,
    registry: &mut AnnotationCalloutRegistry,
    entry: AnnotationCalloutEntry,
) {
    registry.free_entries.push(entry);
    if let Ok(mut root_commands) = commands.get_entity(entry.root) {
        root_commands.insert((
            Visibility::Hidden,
            AnnotationCalloutRoot {
                target: None,
                placement: AnnotationCalloutPlacement::TopLeft,
            },
        ));
    }
    if let Ok(mut line_commands) = commands.get_entity(entry.line) {
        line_commands.insert(Visibility::Hidden);
    }
}

fn annotation_callout_text(
    entity: &super::resources::DebugOverlayEntity,
    controlled_position: Option<Vec2>,
) -> String {
    let mut lines = Vec::with_capacity(10);
    lines.push(entity.label.clone());
    lines.push(format!("ID {}", short_uuid(entity.guid)));
    lines.push(format!(
        "POS {:>7.1} {:>7.1}",
        entity.position_xy.x, entity.position_xy.y
    ));
    lines.push(format!("ROT {:>6.1} DEG", entity.rotation_rad.to_degrees()));
    if let Some(controlled_position) = controlled_position {
        let relative = entity.position_xy - controlled_position;
        lines.push(format!("REL {:>7.1} {:>7.1}", relative.x, relative.y));
    }
    lines.push(format!("LANE {:?}", entity.lane).to_ascii_uppercase());
    lines.push(format!("ECS {}", entity.entity.to_bits()));
    lines.push(format!(
        "VEL {:>6.1} {:>6.1}",
        entity.velocity_xy.x, entity.velocity_xy.y
    ));
    lines.push(format!("ANG {:>6.2}", entity.angular_velocity_rps));
    lines.push(format!(
        "COMP {}",
        if entity.is_component { "YES" } else { "NO" }
    ));
    lines.join("\n")
}

fn short_uuid(guid: uuid::Uuid) -> String {
    guid.to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{
        propagate_ui_overlay_layer_system, split_debug_overlay_text_columns,
        sync_entity_nameplates_system, update_debug_overlay_text_ui_system,
    };
    use crate::runtime::components::{
        CanonicalPresentationEntity, DebugOverlayPanelLabelShadowText, DebugOverlayPanelLabelText,
        DebugOverlayPanelRoot, DebugOverlayPanelSecondaryLabelShadowText,
        DebugOverlayPanelSecondaryLabelText, DebugOverlayPanelSecondaryValueShadowText,
        DebugOverlayPanelSecondaryValueText, DebugOverlayPanelTertiaryLabelShadowText,
        DebugOverlayPanelTertiaryLabelText, DebugOverlayPanelTertiaryValueShadowText,
        DebugOverlayPanelTertiaryValueText, DebugOverlayPanelText,
        DebugOverlayPanelValueShadowText, DebugOverlayPanelValueText, EntityNameplateRoot,
        UiOverlayLayer, WorldEntity,
    };
    use crate::runtime::platform::UI_OVERLAY_RENDER_LAYER;
    use crate::runtime::resources::{
        ClientInputSendState, DebugOverlaySnapshot, DebugOverlayState, HudPerfCounters,
        NameplateRegistry, NameplateUiState, TacticalMapUiState,
    };
    use bevy::camera::visibility::RenderLayers;
    use bevy::diagnostic::DiagnosticsStore;
    use bevy::prelude::*;
    use sidereal_game::{EntityAction, HealthPool};

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

    #[test]
    fn debug_overlay_text_rows_split_evenly_across_two_columns() {
        let rows = vec![
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "2".to_string()),
            ("C".to_string(), "3".to_string()),
            ("D".to_string(), "4".to_string()),
            ("E".to_string(), "5".to_string()),
        ];

        let columns = split_debug_overlay_text_columns(&rows);

        assert_eq!(columns[0].labels, vec!["A", "B"]);
        assert_eq!(columns[0].values, vec!["1", "2"]);
        assert_eq!(columns[1].labels, vec!["C", "D"]);
        assert_eq!(columns[1].values, vec!["3", "4"]);
        assert_eq!(columns[2].labels, vec!["E"]);
        assert_eq!(columns[2].values, vec!["5"]);
    }

    #[test]
    fn debug_overlay_text_rows_pin_control_data_to_right_column() {
        let rows = vec![
            ("Sent Input".to_string(), "[Long Neutral]".to_string()),
            ("A".to_string(), "1".to_string()),
            (
                "Control GUID".to_string(),
                "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
            ),
            ("B".to_string(), "2".to_string()),
        ];

        let columns = split_debug_overlay_text_columns(&rows);

        assert!(!columns[0].labels.contains(&"Sent Input".to_string()));
        assert!(!columns[1].labels.contains(&"Control GUID".to_string()));
        assert!(columns[2].labels.contains(&"Sent Input".to_string()));
        assert!(columns[2].labels.contains(&"Control GUID".to_string()));
    }

    #[test]
    fn debug_overlay_text_ui_system_initializes_without_query_conflicts() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<DiagnosticsStore>();
        app.init_resource::<DebugOverlaySnapshot>();
        app.insert_resource(DebugOverlayState {
            enabled: true,
            ..Default::default()
        });
        app.init_resource::<ClientInputSendState>();
        app.add_systems(Update, update_debug_overlay_text_ui_system);

        app.world_mut()
            .spawn((DebugOverlayPanelRoot, Visibility::Hidden));
        app.world_mut().spawn((
            DebugOverlayPanelLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));

        app.update();
    }

    #[test]
    fn debug_overlay_sent_input_moves_to_right_column() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<DiagnosticsStore>();
        app.insert_resource(DebugOverlayState {
            enabled: true,
            ..Default::default()
        });
        app.insert_resource(DebugOverlaySnapshot {
            text_rows: vec![
                super::super::resources::DebugTextRow {
                    label: "Predicted".to_string(),
                    value: "1".to_string(),
                    severity: super::super::resources::DebugSeverity::Normal,
                },
                super::super::resources::DebugTextRow {
                    label: "Confirmed".to_string(),
                    value: "2".to_string(),
                    severity: super::super::resources::DebugSeverity::Normal,
                },
                super::super::resources::DebugTextRow {
                    label: "Interpolated".to_string(),
                    value: "3".to_string(),
                    severity: super::super::resources::DebugSeverity::Normal,
                },
                super::super::resources::DebugTextRow {
                    label: "Cameras".to_string(),
                    value: "7".to_string(),
                    severity: super::super::resources::DebugSeverity::Normal,
                },
            ],
            ..Default::default()
        });
        app.insert_resource(ClientInputSendState {
            last_sent_actions: vec![
                EntityAction::Left,
                EntityAction::LongitudinalNeutral,
                EntityAction::AfterburnerOff,
            ],
            ..Default::default()
        });
        app.add_systems(Update, update_debug_overlay_text_ui_system);

        app.world_mut()
            .spawn((DebugOverlayPanelRoot, Visibility::Hidden));
        app.world_mut().spawn((
            DebugOverlayPanelLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));

        app.update();

        let primary_labels_value = {
            let world = app.world_mut();
            world
                .query_filtered::<&Text, With<DebugOverlayPanelLabelText>>()
                .single(world)
                .expect("primary labels")
                .0
                .clone()
        };
        let secondary_labels_value = {
            let world = app.world_mut();
            world
                .query_filtered::<&Text, With<DebugOverlayPanelSecondaryLabelText>>()
                .single(world)
                .expect("secondary labels")
                .0
                .clone()
        };

        let tertiary_labels_value = {
            let world = app.world_mut();
            world
                .query_filtered::<&Text, With<DebugOverlayPanelTertiaryLabelText>>()
                .single(world)
                .expect("tertiary labels")
                .0
                .clone()
        };

        assert!(!primary_labels_value.contains("Sent Input"));
        assert!(!secondary_labels_value.contains("Sent Input"));
        assert!(tertiary_labels_value.contains("Sent Input"));
    }

    #[test]
    fn nameplates_default_to_enabled() {
        assert!(NameplateUiState::default().enabled);
    }

    #[test]
    fn sync_entity_nameplates_system_names_spawned_roots() {
        let mut app = App::new();
        app.init_resource::<HudPerfCounters>();
        app.init_resource::<NameplateRegistry>();
        app.init_resource::<NameplateUiState>();
        app.init_resource::<TacticalMapUiState>();
        app.add_systems(Update, sync_entity_nameplates_system);

        let target = app
            .world_mut()
            .spawn((
                WorldEntity,
                CanonicalPresentationEntity,
                HealthPool {
                    current: 10.0,
                    maximum: 10.0,
                },
            ))
            .id();

        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<(&Name, &EntityNameplateRoot), Without<WorldEntity>>();
        let (name, root) = query.single(app.world()).expect("spawned nameplate root");
        assert_eq!(name.as_str(), "Nameplate");
        assert_eq!(root.target, Some(target));
        assert_eq!(
            app.world()
                .resource::<NameplateRegistry>()
                .active_by_target
                .len(),
            1
        );
    }

    #[test]
    fn sync_entity_nameplates_system_reuses_pooled_entries() {
        let mut app = App::new();
        app.init_resource::<HudPerfCounters>();
        app.init_resource::<NameplateRegistry>();
        app.init_resource::<NameplateUiState>();
        app.init_resource::<TacticalMapUiState>();
        app.add_systems(Update, sync_entity_nameplates_system);

        let first_target = app
            .world_mut()
            .spawn((
                WorldEntity,
                CanonicalPresentationEntity,
                HealthPool {
                    current: 10.0,
                    maximum: 10.0,
                },
            ))
            .id();
        let second_target = app
            .world_mut()
            .spawn((
                WorldEntity,
                HealthPool {
                    current: 10.0,
                    maximum: 10.0,
                },
            ))
            .id();

        app.update();
        let first_root =
            app.world().resource::<NameplateRegistry>().active_by_target[&first_target].root;
        assert_eq!(
            app.world()
                .resource::<NameplateRegistry>()
                .allocated_entries,
            1
        );

        app.world_mut()
            .entity_mut(first_target)
            .remove::<CanonicalPresentationEntity>();
        app.world_mut()
            .entity_mut(second_target)
            .insert(CanonicalPresentationEntity);

        app.update();

        let registry = app.world().resource::<NameplateRegistry>();
        assert_eq!(
            registry.allocated_entries, 1,
            "pooled entries should be reused"
        );
        assert_eq!(registry.active_by_target[&second_target].root, first_root);
    }

    #[test]
    fn disabled_nameplates_do_not_allocate_entries() {
        let mut app = App::new();
        app.init_resource::<HudPerfCounters>();
        app.init_resource::<NameplateRegistry>();
        app.insert_resource(NameplateUiState { enabled: false });
        app.init_resource::<TacticalMapUiState>();
        app.add_systems(Update, sync_entity_nameplates_system);

        app.world_mut().spawn((
            WorldEntity,
            CanonicalPresentationEntity,
            HealthPool {
                current: 10.0,
                maximum: 10.0,
            },
        ));

        app.update();

        let registry = app.world().resource::<NameplateRegistry>();
        assert!(registry.active_by_target.is_empty());
        assert!(registry.free_entries.is_empty());
        assert_eq!(registry.allocated_entries, 0);
    }

    #[test]
    fn tactical_map_mode_suppresses_nameplate_allocation_without_disabling_preference() {
        let mut app = App::new();
        app.init_resource::<HudPerfCounters>();
        app.init_resource::<NameplateRegistry>();
        app.init_resource::<NameplateUiState>();
        app.insert_resource(TacticalMapUiState {
            enabled: true,
            ..Default::default()
        });
        app.add_systems(Update, sync_entity_nameplates_system);

        app.world_mut().spawn((
            WorldEntity,
            CanonicalPresentationEntity,
            HealthPool {
                current: 10.0,
                maximum: 10.0,
            },
        ));

        app.update();

        assert!(app.world().resource::<NameplateUiState>().enabled);
        let registry = app.world().resource::<NameplateRegistry>();
        assert!(registry.active_by_target.is_empty());
        assert!(registry.free_entries.is_empty());
        assert_eq!(registry.allocated_entries, 0);
    }
}

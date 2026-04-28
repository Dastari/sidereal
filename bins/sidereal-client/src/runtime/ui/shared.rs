// World HUD and owned-entity panel systems.

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


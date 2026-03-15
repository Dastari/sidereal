//! World scene bootstrap systems.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::state_scoped::DespawnOnExit;
use sidereal_ui::theme::{ActiveUiTheme, theme_definition};
use sidereal_ui::widgets::spawn_scanline_overlay;

use super::app_state::{ClientAppState, ClientSession};
use super::assets::LocalAssetManager;
use super::backdrop::TacticalMapOverlayMaterial;
use super::components::{
    BackdropCamera, ClientSceneEntity, DebugOverlayCamera, DebugOverlayPanelLabelShadowText,
    DebugOverlayPanelLabelText, DebugOverlayPanelRoot, DebugOverlayPanelSecondaryLabelShadowText,
    DebugOverlayPanelSecondaryLabelText, DebugOverlayPanelSecondaryValueShadowText,
    DebugOverlayPanelSecondaryValueText, DebugOverlayPanelValueShadowText,
    DebugOverlayPanelValueText, DebugVelocityArrowHeadLower, DebugVelocityArrowHeadUpper,
    DebugVelocityArrowShaft, FullscreenForegroundCamera, GameplayCamera, GameplayHud,
    HudFuelBarFill, HudHealthBarFill, HudPositionValueText, HudSpeedValueText, LoadingOverlayRoot,
    LoadingOverlayText, LoadingProgressBarFill, PlanetBodyCamera, PostProcessCamera,
    RuntimeScreenOverlayPass, RuntimeScreenOverlayPassKind, RuntimeStreamingIconText,
    SegmentedBarSegment, SegmentedBarStyle, SegmentedBarValue, SpaceBackdropFallback,
    TacticalMapCursorText, TacticalMapOverlayRoot, TacticalMapTitle, TopDownCamera, UiOverlayLayer,
};
use super::platform::{
    BACKDROP_RENDER_LAYER, DEBUG_OVERLAY_RENDER_LAYER, FULLSCREEN_FOREGROUND_RENDER_LAYER,
    PLANET_BODY_RENDER_LAYER, POST_PROCESS_RENDER_LAYER, UI_OVERLAY_RENDER_LAYER,
};
use super::resources::{
    AssetCacheAdapter, AssetRootPath, CameraMotionState, EmbeddedFonts, StarfieldMotionState,
};
use super::shaders;

const TACTICAL_FOG_MASK_RESOLUTION: u32 = 384;
const HUD_TELEMETRY_LABEL_WIDTH_PX: f32 = 84.0;

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_world_scene(
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
    active_theme: Res<'_, ActiveUiTheme>,
    mut session: ResMut<'_, ClientSession>,
    mut shaders_assets: ResMut<'_, Assets<bevy::shader::Shader>>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut images: ResMut<'_, Assets<Image>>,
    mut color_materials: ResMut<'_, Assets<ColorMaterial>>,
    mut tactical_map_materials: ResMut<'_, Assets<TacticalMapOverlayMaterial>>,
    mut starfield_motion: ResMut<'_, StarfieldMotionState>,
    mut camera_motion: ResMut<'_, CameraMotionState>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    shader_assignments: Res<'_, shaders::RuntimeShaderAssignments>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
) {
    let theme = theme_definition(active_theme.0);
    *starfield_motion = StarfieldMotionState::default();
    *camera_motion = CameraMotionState::default();
    shaders::reload_streamed_shaders(
        &mut shaders_assets,
        &asset_root.0,
        &asset_manager,
        *cache_adapter,
        &shader_assignments,
    );
    commands.spawn((
        Camera2d,
        Camera {
            order: -2,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        BackdropCamera,
        RenderLayers::layer(BACKDROP_RENDER_LAYER),
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    let fallback_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let fallback_material = color_materials.add(ColorMaterial::from(Color::BLACK));
    commands.spawn((
        Mesh2d(fallback_mesh),
        MeshMaterial2d(fallback_material),
        Transform::from_xyz(0.0, 0.0, -210.0),
        RenderLayers::layer(BACKDROP_RENDER_LAYER),
        SpaceBackdropFallback,
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 80.0),
        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
        PlanetBodyCamera,
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 80.0),
        RenderLayers::layer(0),
        GameplayCamera,
        TopDownCamera {
            distance: 30.0,
            target_distance: 30.0,
            min_distance: 10.0,
            max_distance: 30.0,
            zoom_units_per_wheel: 2.0,
            zoom_smoothness: 8.0,
            look_ahead_offset: Vec2::ZERO,
            filtered_focus_xy: Vec2::ZERO,
            focus_initialized: false,
        },
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        Camera2d,
        Camera {
            order: 50,
            is_active: true,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 80.0),
        RenderLayers::layer(DEBUG_OVERLAY_RENDER_LAYER),
        DebugOverlayCamera,
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    let debug_arrow_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let debug_arrow_material = color_materials.add(ColorMaterial::from(Color::srgb(0.2, 0.5, 1.0)));
    commands.spawn((
        Mesh2d(debug_arrow_mesh.clone()),
        MeshMaterial2d(debug_arrow_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Hidden,
        RenderLayers::layer(DEBUG_OVERLAY_RENDER_LAYER),
        DebugVelocityArrowShaft,
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    commands.spawn((
        Mesh2d(debug_arrow_mesh.clone()),
        MeshMaterial2d(debug_arrow_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Hidden,
        RenderLayers::layer(DEBUG_OVERLAY_RENDER_LAYER),
        DebugVelocityArrowHeadUpper,
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    commands.spawn((
        Mesh2d(debug_arrow_mesh),
        MeshMaterial2d(debug_arrow_material),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Hidden,
        RenderLayers::layer(DEBUG_OVERLAY_RENDER_LAYER),
        DebugVelocityArrowHeadLower,
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            is_active: true,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        FullscreenForegroundCamera,
        RenderLayers::layer(FULLSCREEN_FOREGROUND_RENDER_LAYER),
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        Camera2d,
        Camera {
            order: 2,
            is_active: true,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        PostProcessCamera,
        RenderLayers::layer(POST_PROCESS_RENDER_LAYER),
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 20_000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(14),
                top: px(14),
                width: px(820),
                ..default()
            },
            Visibility::Hidden,
            DebugOverlayPanelRoot,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            ClientSceneEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|panel| {
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(1),
                    top: px(1),
                    ..default()
                },
                Text::new("FPS\nFrame"),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
                DebugOverlayPanelLabelShadowText,
            ));
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(181),
                    top: px(1),
                    ..default()
                },
                Text::new("--\n--.-- ms"),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
                DebugOverlayPanelValueShadowText,
            ));
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(321),
                    top: px(1),
                    ..default()
                },
                Text::new(""),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
                DebugOverlayPanelSecondaryLabelShadowText,
            ));
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(501),
                    top: px(1),
                    ..default()
                },
                Text::new(""),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
                DebugOverlayPanelSecondaryValueShadowText,
            ));
            panel.spawn((
                Text::new("FPS\nFrame"),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.92, 1.0)),
                DebugOverlayPanelLabelText,
            ));
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(180),
                    top: px(0),
                    ..default()
                },
                Text::new("--\n--.-- ms"),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.92, 1.0)),
                DebugOverlayPanelValueText,
            ));
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(320),
                    top: px(0),
                    ..default()
                },
                Text::new(""),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.92, 1.0)),
                DebugOverlayPanelSecondaryLabelText,
            ));
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(500),
                    top: px(0),
                    ..default()
                },
                Text::new(""),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.92, 1.0)),
                DebugOverlayPanelSecondaryValueText,
            ));
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(14),
                bottom: px(14),
                width: px(360),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            GameplayHud,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            ClientSceneEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|panel| {
            panel
                .spawn((Node {
                    width: percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(8),
                    align_items: AlignItems::Center,
                    ..default()
                },))
                .with_children(|row| {
                    row.spawn((
                        Text::new("SPEED"),
                        Node {
                            width: px(HUD_TELEMETRY_LABEL_WIDTH_PX),
                            ..default()
                        },
                        TextFont {
                            font: fonts.mono_bold.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(theme.colors.muted_foreground_color()),
                    ));
                    row.spawn((
                        Text::new("--.- m/s"),
                        TextFont {
                            font: fonts.display.clone(),
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(theme.colors.primary_color()),
                        HudSpeedValueText,
                    ));
                });
            panel
                .spawn((Node {
                    width: percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(8),
                    align_items: AlignItems::Center,
                    ..default()
                },))
                .with_children(|row| {
                    row.spawn((
                        Text::new("POSITION"),
                        Node {
                            width: px(HUD_TELEMETRY_LABEL_WIDTH_PX),
                            ..default()
                        },
                        TextFont {
                            font: fonts.mono_bold.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(theme.colors.muted_foreground_color()),
                    ));
                    row.spawn((
                        Text::new("SECTOR E00-N00"),
                        TextFont {
                            font: fonts.display.clone(),
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(theme.colors.primary_color()),
                        HudPositionValueText,
                    ));
                });
            panel
                .spawn((Node {
                    width: percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                },))
                .with_children(|row| {
                    let health_style = SegmentedBarStyle {
                        segments: 20,
                        active_color: Color::srgb(0.28, 0.9, 0.4),
                        inactive_color: Color::srgba(0.15, 0.2, 0.28, 0.85),
                        shell_color: Color::srgba(0.08, 0.12, 0.09, 0.88),
                        border_color: Color::srgba(0.22, 0.65, 0.32, 0.88),
                        corner_color: Color::srgba(0.32, 0.95, 0.46, 0.78),
                        scanline_primary_color: Color::srgba(0.2, 0.8, 0.32, 0.018),
                        scanline_secondary_color: Color::srgba(0.16, 0.55, 0.26, 0.009),
                        segment_width_px: 9.0,
                        segment_gap_px: 2.0,
                    };
                    row.spawn((
                        Node {
                            width: px(HUD_TELEMETRY_LABEL_WIDTH_PX),
                            ..default()
                        },
                        Text::new("HEALTH"),
                        TextFont {
                            font: fonts.mono_bold.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(theme.colors.muted_foreground_color()),
                    ));
                    row.spawn((
                        Node {
                            // 20 segments @ 9px + 19 gaps @ 2px + 2px padding = 220px total.
                            // Keep integer segment widths to avoid fractional flex distribution jitter.
                            width: px(220),
                            height: px(14),
                            column_gap: px(health_style.segment_gap_px),
                            align_items: AlignItems::Stretch,
                            border: UiRect::all(px(1.0)),
                            padding: UiRect::all(px(1.0)),
                            ..default()
                        },
                        BackgroundColor(health_style.shell_color),
                        BorderColor::all(health_style.border_color),
                        health_style,
                        SegmentedBarValue { ratio: 1.0 },
                        HudHealthBarFill,
                    ))
                    .with_children(|bar| {
                        spawn_scanline_overlay(
                            bar,
                            &mut images,
                            health_style.scanline_primary_color,
                            health_style.scanline_secondary_color,
                            1.0,
                            3.0,
                            2,
                        );
                        for index in 0..health_style.segments {
                            bar.spawn((
                                Node {
                                    width: px(health_style.segment_width_px),
                                    height: percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(health_style.inactive_color),
                                SegmentedBarSegment { index },
                            ));
                        }
                    });
                });
            panel
                .spawn((Node {
                    width: percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                },))
                .with_children(|row| {
                    let fuel_style = SegmentedBarStyle {
                        segments: 20,
                        active_color: Color::srgb(0.3, 0.78, 1.0),
                        inactive_color: Color::srgba(0.15, 0.2, 0.28, 0.85),
                        shell_color: Color::srgba(0.08, 0.11, 0.16, 0.88),
                        border_color: Color::srgba(0.22, 0.58, 0.88, 0.88),
                        corner_color: Color::srgba(0.4, 0.85, 1.0, 0.78),
                        scanline_primary_color: Color::srgba(0.2, 0.7, 1.0, 0.02),
                        scanline_secondary_color: Color::srgba(0.16, 0.5, 0.78, 0.01),
                        segment_width_px: 9.0,
                        segment_gap_px: 2.0,
                    };
                    row.spawn((
                        Node {
                            width: px(HUD_TELEMETRY_LABEL_WIDTH_PX),
                            ..default()
                        },
                        Text::new("FUEL"),
                        TextFont {
                            font: fonts.mono_bold.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(theme.colors.muted_foreground_color()),
                    ));
                    row.spawn((
                        Node {
                            // 20 segments @ 9px + 19 gaps @ 2px + 2px padding = 220px total.
                            // Keep integer segment widths to avoid fractional flex distribution jitter.
                            width: px(220),
                            height: px(14),
                            column_gap: px(fuel_style.segment_gap_px),
                            align_items: AlignItems::Stretch,
                            border: UiRect::all(px(1.0)),
                            padding: UiRect::all(px(1.0)),
                            ..default()
                        },
                        BackgroundColor(fuel_style.shell_color),
                        BorderColor::all(fuel_style.border_color),
                        fuel_style,
                        SegmentedBarValue { ratio: 1.0 },
                        HudFuelBarFill,
                    ))
                    .with_children(|bar| {
                        spawn_scanline_overlay(
                            bar,
                            &mut images,
                            fuel_style.scanline_primary_color,
                            fuel_style.scanline_secondary_color,
                            1.0,
                            3.0,
                            2,
                        );
                        for index in 0..fuel_style.segments {
                            bar.spawn((
                                Node {
                                    width: px(fuel_style.segment_width_px),
                                    height: percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(fuel_style.inactive_color),
                                SegmentedBarSegment { index },
                            ));
                        }
                    });
                });
        });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.0),
                top: percent(50.0),
                width: px(460),
                margin: UiRect::all(px(-230.0)),
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                ..default()
            },
            Visibility::Visible,
            LoadingOverlayRoot,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Loading world assets..."),
                TextFont {
                    font: fonts.bold.clone(),
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                LoadingOverlayText,
            ));
            parent
                .spawn((
                    Node {
                        width: percent(100.0),
                        height: px(16),
                        border: UiRect::all(px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.1, 0.1, 0.14, 0.85)),
                    BorderColor::all(Color::srgba(0.8, 0.9, 1.0, 0.8)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Node {
                            width: percent(0.0),
                            height: percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.35, 0.85, 1.0)),
                        LoadingProgressBarFill,
                    ));
                });
        });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(20),
            bottom: px(16),
            ..default()
        },
        Text::new("NET"),
        TextFont {
            font: fonts.regular.clone(),
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
        RuntimeStreamingIconText,
        UiOverlayLayer,
        RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        DespawnOnExit(ClientAppState::InWorld),
    ));
    let tactical_overlay_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let fog_mask = images.add(Image::new_fill(
        Extent3d {
            width: TACTICAL_FOG_MASK_RESOLUTION,
            height: TACTICAL_FOG_MASK_RESOLUTION,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255],
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    ));
    let tactical_overlay_material = tactical_map_materials.add(TacticalMapOverlayMaterial {
        fog_mask,
        ..default()
    });
    commands.spawn((
        Mesh2d(tactical_overlay_mesh),
        MeshMaterial2d(tactical_overlay_material),
        Transform::from_xyz(0.0, 0.0, -10.0),
        RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        Visibility::Hidden,
        RuntimeScreenOverlayPass {
            kind: RuntimeScreenOverlayPassKind::TacticalMap,
        },
        ClientSceneEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.08, 0.0)),
            Visibility::Hidden,
            TacticalMapOverlayRoot,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(16.0),
                    top: px(12.0),
                    ..default()
                },
                Text::new("TACTICAL MAP"),
                TextFont {
                    font: fonts.bold.clone(),
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgba(0.68, 0.92, 1.0, 0.0)),
                TacticalMapTitle,
            ));
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: px(16.0),
                    top: px(12.0),
                    ..default()
                },
                Text::new("0.00, 0.00"),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgba(0.85, 0.92, 1.0, 0.0)),
                TacticalMapCursorText,
            ));
        });
    session.status = "Scene ready. Waiting for replicated entities...".to_string();
}

//! World scene bootstrap systems.

use bevy::camera::visibility::RenderLayers;
use bevy::log::info;
use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::state_scoped::DespawnOnExit;
use sidereal_game::{SpaceBackgroundFullscreenLayerBundle, StarfieldFullscreenLayerBundle};

use super::app_state::{ClientAppState, ClientSession};
use super::components::{
    DebugBlueBackdrop, FallbackFullscreenLayer, GameplayCamera, GameplayHud, HudFpsText,
    HudFuelBarFill, HudHealthBarFill, HudPositionValueText, HudSpeedValueText, LoadingOverlayRoot,
    LoadingOverlayText, LoadingProgressBarFill, RuntimeStreamingIconText, SegmentedBarSegment,
    SegmentedBarStyle, SegmentedBarValue, SpaceBackdropFallback, TopDownCamera, UiOverlayLayer,
    WorldEntity,
};
use super::platform::{BACKDROP_RENDER_LAYER, UI_OVERLAY_RENDER_LAYER};
use super::resources::{
    AssetRootPath, CameraMotionState, DebugBlueOverlayEnabled, EmbeddedFonts, StarfieldMotionState,
};
use super::shaders;

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_world_scene(
    mut commands: Commands<'_, '_>,
    asset_server: Res<'_, AssetServer>,
    fonts: Res<'_, EmbeddedFonts>,
    mut session: ResMut<'_, ClientSession>,
    mut shaders_assets: ResMut<'_, Assets<bevy::shader::Shader>>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut color_materials: ResMut<'_, Assets<ColorMaterial>>,
    mut starfield_motion: ResMut<'_, StarfieldMotionState>,
    mut camera_motion: ResMut<'_, CameraMotionState>,
    asset_root: Res<'_, AssetRootPath>,
    debug_blue_overlay: Res<'_, DebugBlueOverlayEnabled>,
) {
    *starfield_motion = StarfieldMotionState::default();
    *camera_motion = CameraMotionState::default();
    shaders::reload_streamed_shaders(&asset_server, &mut shaders_assets, &asset_root.0);
    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        RenderLayers::layer(BACKDROP_RENDER_LAYER),
        WorldEntity,
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
        WorldEntity,
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
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 20_000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(12),
            top: px(12),
            ..default()
        },
        Text::new(""),
        TextFont {
            font: fonts.bold.clone(),
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.92, 1.0)),
        HudFpsText,
        GameplayHud,
        UiOverlayLayer,
        RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(14),
                bottom: px(14),
                width: px(330),
                padding: UiRect::all(px(12)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.92)),
            BorderColor::all(Color::srgba(0.2, 0.3, 0.45, 0.8)),
            GameplayHud,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|panel| {
            panel
                .spawn((
                    Node {
                        width: percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(8),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new("SPEED"),
                        TextFont {
                            font: fonts.bold.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.85, 0.92, 1.0)),
                    ));
                    row.spawn((
                        Text::new("--.- m/s"),
                        TextFont {
                            font: fonts.bold.clone(),
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.35, 0.85, 1.0)),
                        HudSpeedValueText,
                    ));
                });
            panel
                .spawn((
                    Node {
                        width: percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(8),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new("POSITION"),
                        TextFont {
                            font: fonts.bold.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.85, 0.92, 1.0)),
                    ));
                    row.spawn((
                        Text::new("(--, --)"),
                        TextFont {
                            font: fonts.bold.clone(),
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.35, 0.85, 1.0)),
                        HudPositionValueText,
                    ));
                });
            panel
                .spawn((
                    Node {
                        width: percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|row| {
                    row.spawn((
                        Node {
                            width: px(56),
                            ..default()
                        },
                        Text::new("HEALTH"),
                        TextFont {
                            font: fonts.bold.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.83, 0.89, 0.95, 0.95)),
                    ));
                    row.spawn((
                        Node {
                            width: px(230),
                            height: px(14),
                            column_gap: px(2.0),
                            align_items: AlignItems::Stretch,
                            border: UiRect::all(px(1.0)),
                            padding: UiRect::all(px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.1, 0.1, 0.14, 0.85)),
                        BorderColor::all(Color::srgba(0.2, 0.3, 0.45, 0.8)),
                        SegmentedBarStyle {
                            segments: 20,
                            active_color: Color::srgb(0.35, 0.85, 1.0),
                            inactive_color: Color::srgba(0.15, 0.2, 0.28, 0.85),
                        },
                        SegmentedBarValue { ratio: 1.0 },
                        HudHealthBarFill,
                    ))
                    .with_children(|bar| {
                        for index in 0..20u8 {
                            bar.spawn((
                                Node {
                                    flex_grow: 1.0,
                                    height: percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.85)),
                                SegmentedBarSegment { index },
                            ));
                        }
                    });
                });
            panel
                .spawn((
                    Node {
                        width: percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|row| {
                    row.spawn((
                        Node {
                            width: px(56),
                            ..default()
                        },
                        Text::new("FUEL"),
                        TextFont {
                            font: fonts.bold.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.83, 0.89, 0.95, 0.95)),
                    ));
                    row.spawn((
                        Node {
                            width: px(230),
                            height: px(14),
                            column_gap: px(2.0),
                            align_items: AlignItems::Stretch,
                            border: UiRect::all(px(1.0)),
                            padding: UiRect::all(px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.1, 0.1, 0.14, 0.85)),
                        BorderColor::all(Color::srgba(0.2, 0.3, 0.45, 0.8)),
                        SegmentedBarStyle {
                            segments: 20,
                            active_color: Color::srgb(0.3, 0.78, 1.0),
                            inactive_color: Color::srgba(0.15, 0.2, 0.28, 0.85),
                        },
                        SegmentedBarValue { ratio: 1.0 },
                        HudFuelBarFill,
                    ))
                    .with_children(|bar| {
                        for index in 0..20u8 {
                            bar.spawn((
                                Node {
                                    flex_grow: 1.0,
                                    height: percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.85)),
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
    if debug_blue_overlay.0 {
        let mesh = meshes.add(Rectangle::new(1.0, 1.0));
        let material = color_materials.add(ColorMaterial::from(Color::srgb(0.1, 0.35, 1.0)));
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_xyz(0.0, 0.0, -180.0),
            RenderLayers::layer(BACKDROP_RENDER_LAYER),
            DebugBlueBackdrop,
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ));
        info!("client debug blue fullscreen overlay enabled");
    }
    commands.spawn((
        SpaceBackgroundFullscreenLayerBundle::default(),
        FallbackFullscreenLayer,
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    commands.spawn((
        StarfieldFullscreenLayerBundle::default(),
        FallbackFullscreenLayer,
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    session.status = "Scene ready. Waiting for replicated entities...".to_string();
}

//! World scene bootstrap systems.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::log::info;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::state_scoped::DespawnOnExit;

use super::app_state::{ClientAppState, ClientSession};
use super::assets::LocalAssetManager;
use super::backdrop::TacticalMapOverlayMaterial;
use super::components::{
    BackdropCamera, ClientSceneEntity, DebugBlueBackdrop, DebugOverlayCamera,
    DebugOverlayPanelLabelShadowText, DebugOverlayPanelLabelText, DebugOverlayPanelRoot,
    DebugOverlayPanelValueShadowText, DebugOverlayPanelValueText, FullscreenForegroundCamera,
    GameplayCamera, GameplayHud, HudFuelBarFill, HudHealthBarFill, HudPositionValueText,
    HudSpeedValueText, LoadingOverlayRoot, LoadingOverlayText, LoadingProgressBarFill,
    PostProcessCamera, RuntimeScreenOverlayPass, RuntimeScreenOverlayPassKind,
    RuntimeStreamingIconText, SegmentedBarSegment, SegmentedBarStyle, SegmentedBarValue,
    SpaceBackdropFallback, TacticalMapCursorText, TacticalMapOverlayRoot, TacticalMapTitle,
    TopDownCamera, UiOverlayLayer,
};
use super::platform::{
    BACKDROP_RENDER_LAYER, DEBUG_OVERLAY_RENDER_LAYER, FULLSCREEN_FOREGROUND_RENDER_LAYER,
    PLANET_BODY_RENDER_LAYER, POST_PROCESS_RENDER_LAYER, UI_OVERLAY_RENDER_LAYER,
};
use super::resources::{
    AssetCacheAdapter, AssetRootPath, CameraMotionState, DebugBlueOverlayEnabled, EmbeddedFonts,
    StarfieldMotionState,
};
use super::shaders;

const TACTICAL_FOG_MASK_RESOLUTION: u32 = 384;

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_world_scene(
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
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
    debug_blue_overlay: Res<'_, DebugBlueOverlayEnabled>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
) {
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
            order: -1,
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
            order: 0,
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 80.0),
        RenderLayers::from_layers(&[0, PLANET_BODY_RENDER_LAYER]),
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
                width: px(680),
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
                    font: fonts.regular.clone(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
                DebugOverlayPanelLabelShadowText,
            ));
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(221),
                    top: px(1),
                    ..default()
                },
                Text::new("--\n--.-- ms"),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
                DebugOverlayPanelValueShadowText,
            ));
            panel.spawn((
                Text::new("FPS\nFrame"),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.92, 1.0)),
                DebugOverlayPanelLabelText,
            ));
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(220),
                    top: px(0),
                    ..default()
                },
                Text::new("--\n--.-- ms"),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.92, 1.0)),
                DebugOverlayPanelValueText,
            ));
        });

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
                .spawn((Node {
                    width: percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                },))
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
                            // 20 segments @ 9px + 19 gaps @ 2px + 2px padding = 220px total.
                            // Keep integer segment widths to avoid fractional flex distribution jitter.
                            width: px(220),
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
                                    width: px(9.0),
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
                .spawn((Node {
                    width: percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                },))
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
                            // 20 segments @ 9px + 19 gaps @ 2px + 2px padding = 220px total.
                            // Keep integer segment widths to avoid fractional flex distribution jitter.
                            width: px(220),
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
                                    width: px(9.0),
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
    if debug_blue_overlay.0 {
        let mesh = meshes.add(Rectangle::new(1.0, 1.0));
        let material = color_materials.add(ColorMaterial::from(Color::srgb(0.1, 0.35, 1.0)));
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_xyz(0.0, 0.0, -180.0),
            RenderLayers::layer(BACKDROP_RENDER_LAYER),
            DebugBlueBackdrop,
            ClientSceneEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ));
        info!("client debug blue fullscreen overlay enabled");
    }
    session.status = "Scene ready. Waiting for replicated entities...".to_string();
}

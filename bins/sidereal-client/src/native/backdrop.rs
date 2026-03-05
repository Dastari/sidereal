//! Fullscreen backdrop materials (starfield, space background, streamed sprite) and their update systems.

use avian2d::prelude::*;
use bevy::camera::ScalingMode;
use bevy::camera::visibility::RenderLayers;
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, MeshMaterial2d};
use sidereal_runtime_sync::RuntimeEntityHierarchy;

use sidereal_game::{
    FullscreenLayer, SPACE_BACKGROUND_LAYER_KIND, SpaceBackgroundShaderSettings,
    STARFIELD_LAYER_KIND, StarfieldShaderSettings, default_space_bg_flare_blue_asset_id,
    default_space_bg_flare_red_asset_id, default_space_bg_flare_sun_asset_id,
    default_space_bg_flare_white_asset_id,
};

use super::assets;
use super::components::{
    BackdropCamera, ControlledEntity, DebugBlueBackdrop, FullscreenLayerRenderable, GameplayCamera,
    PendingInitialVisualReady, SpaceBackdropFallback, SpaceBackgroundBackdrop, StarfieldBackdrop,
};
use super::platform::{self, BACKDROP_RENDER_LAYER};
use super::resources::AssetRootPath;
use super::resources::{FullscreenExternalWorldData, StarfieldMotionState};

fn flare_texture_asset_id_for_set(set: u32) -> &'static str {
    match set {
        1 => default_space_bg_flare_blue_asset_id(),
        2 => default_space_bg_flare_red_asset_id(),
        3 => default_space_bg_flare_sun_asset_id(),
        _ => default_space_bg_flare_white_asset_id(),
    }
}

pub(super) fn sync_fullscreen_layer_renderables_system(
    mut commands: Commands<'_, '_>,
    layers: Query<'_, '_, (Entity, &FullscreenLayer, Option<&FullscreenLayerRenderable>)>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut starfield_materials: ResMut<'_, Assets<StarfieldMaterial>>,
    mut space_background_materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, assets::LocalAssetManager>,
) {
    let shader_materials_enabled = super::shaders::shader_materials_enabled();
    if !shader_materials_enabled {
        for (entity, _, rendered) in &layers {
            if rendered.is_none() {
                continue;
            }
            let Ok(mut entity_commands) = commands.get_entity(entity) else {
                continue;
            };
            entity_commands
                .remove::<FullscreenLayerRenderable>()
                .remove::<StarfieldBackdrop>()
                .remove::<SpaceBackgroundBackdrop>()
                .remove::<Mesh2d>()
                .remove::<MeshMaterial2d<StarfieldMaterial>>()
                .remove::<MeshMaterial2d<SpaceBackgroundMaterial>>();
        }
        return;
    }

    for (entity, layer, rendered) in &layers {
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        let has_streamed_shader = super::shaders::fullscreen_layer_shader_ready(
            &asset_root.0,
            &asset_manager,
            &layer.shader_asset_id,
        );
        let is_supported_kind = layer.layer_kind == STARFIELD_LAYER_KIND
            || layer.layer_kind == SPACE_BACKGROUND_LAYER_KIND;
        let needs_rebuild = rendered.is_none_or(|existing| {
            existing.layer_kind != layer.layer_kind || existing.layer_order != layer.layer_order
        });

        if !is_supported_kind || !has_streamed_shader {
            if !is_supported_kind {
                warn!(
                    "unsupported fullscreen layer kind={} shader_asset_id={}",
                    layer.layer_kind, layer.shader_asset_id
                );
            } else {
                warn!(
                    "fullscreen layer waiting for shader readiness layer_kind={} shader_asset_id={}",
                    layer.layer_kind, layer.shader_asset_id
                );
            }
            if rendered.is_some() {
                entity_commands
                    .remove::<FullscreenLayerRenderable>()
                    .remove::<StarfieldBackdrop>()
                    .remove::<SpaceBackgroundBackdrop>()
                    .remove::<Mesh2d>()
                    .remove::<MeshMaterial2d<StarfieldMaterial>>()
                    .remove::<MeshMaterial2d<SpaceBackgroundMaterial>>();
            }
            continue;
        }

        if needs_rebuild {
            let mesh = meshes.add(Rectangle::new(1.0, 1.0));
            entity_commands
                .try_insert((
                    Mesh2d(mesh),
                    Transform::from_xyz(0.0, 0.0, layer.layer_order as f32),
                    RenderLayers::layer(BACKDROP_RENDER_LAYER),
                    Visibility::Visible,
                    FullscreenLayerRenderable {
                        layer_kind: layer.layer_kind.clone(),
                        layer_order: layer.layer_order,
                    },
                ))
                .remove::<PendingInitialVisualReady>()
                .remove::<StarfieldBackdrop>()
                .remove::<SpaceBackgroundBackdrop>()
                .remove::<MeshMaterial2d<StarfieldMaterial>>()
                .remove::<MeshMaterial2d<SpaceBackgroundMaterial>>();

            if layer.layer_kind == STARFIELD_LAYER_KIND {
                let material = starfield_materials.add(StarfieldMaterial::default());
                entity_commands.try_insert((
                    StarfieldBackdrop,
                    MeshMaterial2d(material),
                    StarfieldShaderSettings::default(),
                ));
            } else {
                let material = space_background_materials.add(SpaceBackgroundMaterial::default());
                entity_commands.try_insert((
                    SpaceBackgroundBackdrop,
                    MeshMaterial2d(material),
                    SpaceBackgroundShaderSettings::default(),
                ));
            }
            info!(
                "fullscreen layer renderable ready layer_kind={} order={} shader_asset_id={}",
                layer.layer_kind, layer.layer_order, layer.shader_asset_id
            );
        } else {
            entity_commands.try_insert(Transform::from_xyz(0.0, 0.0, layer.layer_order as f32));
        }
    }
}

pub(super) fn detach_fullscreen_layer_hierarchy_links_system(
    mut commands: Commands<'_, '_>,
    fullscreen_layers: Query<'_, '_, (Entity, &'_ ChildOf), With<FullscreenLayer>>,
) {
    for (entity, _) in &fullscreen_layers {
        commands.entity(entity).remove::<ChildOf>();
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_backdrop_fullscreen_system(
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut backdrop_query: Query<
        '_,
        '_,
        &mut Transform,
        (
            Or<(
                With<StarfieldBackdrop>,
                With<SpaceBackgroundBackdrop>,
                With<DebugBlueBackdrop>,
                With<SpaceBackdropFallback>,
            )>,
        ),
    >,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(viewport_size) = platform::safe_viewport_size(window) else {
        return;
    };
    let width = viewport_size.x;
    let height = viewport_size.y;
    for mut transform in &mut backdrop_query {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
        transform.scale = Vec3::new(width, height, 1.0);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_backdrop_camera_system(
    mut cameras: Query<
        '_,
        '_,
        (&'_ mut Camera, &'_ mut Transform, &'_ mut Projection),
        With<BackdropCamera>,
    >,
) {
    for (mut camera, mut transform, mut projection) in &mut cameras {
        camera.is_active = true;
        transform.translation = Vec3::ZERO;
        transform.rotation = Quat::IDENTITY;
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode = ScalingMode::WindowSize;
            ortho.scale = 1.0;
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StarfieldMaterial {
    #[uniform(0)]
    pub viewport_time: Vec4,
    #[uniform(1)]
    pub drift_intensity: Vec4,
    #[uniform(2)]
    pub velocity_dir: Vec4,
    #[uniform(3)]
    pub starfield_params: Vec4,
    #[uniform(4)]
    pub starfield_tint: Vec4,
    #[uniform(5)]
    pub star_core_params: Vec4,
    #[uniform(6)]
    pub star_core_color: Vec4,
    #[uniform(7)]
    pub corona_params: Vec4,
    #[uniform(8)]
    pub corona_color: Vec4,
}

impl Default for StarfieldMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            drift_intensity: Vec4::new(0.0, 0.0, 1.0, 1.0),
            velocity_dir: Vec4::new(0.0, 1.0, 1.0, 0.0),
            starfield_params: Vec4::new(0.05, 3.0, 0.35, 1.0),
            starfield_tint: Vec4::new(1.0, 1.0, 1.0, 1.0),
            star_core_params: Vec4::new(1.0, 1.0, 1.0, 0.0),
            star_core_color: Vec4::new(0.72, 0.83, 1.0, 1.0),
            corona_params: Vec4::new(1.0, 1.0, 1.0, 0.0),
            corona_color: Vec4::new(0.44, 0.64, 1.0, 1.0),
        }
    }
}

impl Material2d for StarfieldMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(super::shaders::STARFIELD_SHADER_HANDLE)
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Packed uniforms for the space background shader (one buffer to stay under
/// max_uniform_buffers_per_shader_stage limits on Windows/DX).
#[derive(ShaderType, Debug, Clone)]
pub struct SpaceBackgroundUniforms {
    pub viewport_time: Vec4,
    pub drift_intensity: Vec4,
    pub velocity_dir: Vec4,
    pub space_bg_params: Vec4,
    pub space_bg_tint: Vec4,
    pub space_bg_background: Vec4,
    pub space_bg_flare: Vec4,
    pub space_bg_noise_a: Vec4,
    pub space_bg_noise_b: Vec4,
    pub space_bg_star_mask_a: Vec4,
    pub space_bg_star_mask_b: Vec4,
    pub space_bg_star_mask_c: Vec4,
    pub space_bg_blend_a: Vec4,
    pub space_bg_blend_b: Vec4,
    pub space_bg_nebula_color_a: Vec4,
    pub space_bg_nebula_color_b: Vec4,
    pub space_bg_nebula_color_c: Vec4,
    pub space_bg_star_color: Vec4,
    pub space_bg_flare_tint: Vec4,
    pub space_bg_depth_a: Vec4,
    pub space_bg_light_a: Vec4,
    pub space_bg_light_b: Vec4,
    pub space_bg_light_flags: Vec4,
    pub space_bg_shafts_a: Vec4,
    pub space_bg_shafts_b: Vec4,
    pub space_bg_backlight_color: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct SpaceBackgroundMaterial {
    #[uniform(0)]
    pub params: SpaceBackgroundUniforms,
    #[texture(1)]
    #[sampler(2)]
    pub flare_texture: Handle<Image>,
}

impl Default for SpaceBackgroundUniforms {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            drift_intensity: Vec4::new(0.0, 0.0, 1.0, 1.0),
            velocity_dir: Vec4::new(0.0, 1.0, 1.0, 0.0),
            space_bg_params: Vec4::new(0.35, 2.0, 1.0, 0.85),
            space_bg_tint: Vec4::new(1.0, 1.77, 1.24, 0.0),
            space_bg_background: Vec4::new(0.0, 0.0, 0.0, 1.0),
            space_bg_flare: Vec4::new(1.0, 4.0, 0.54, 0.0),
            space_bg_noise_a: Vec4::new(0.0, 5.0, 0.52, 2.0),
            space_bg_noise_b: Vec4::new(1.0, 0.42, 1.0, 0.0),
            space_bg_star_mask_a: Vec4::new(1.0, 0.0, 4.0, 3.1),
            space_bg_star_mask_b: Vec4::new(0.35, 1.25, 0.42, 1.75),
            space_bg_star_mask_c: Vec4::new(0.83, 5.0, 0.019, 0.022),
            space_bg_blend_a: Vec4::new(1.0, 0.5, 2.0, 1.0),
            space_bg_blend_b: Vec4::new(1.0, 1.0, 1.0, 0.0),
            space_bg_nebula_color_a: Vec4::new(0.0, 0.0, 0.196, 0.0),
            space_bg_nebula_color_b: Vec4::new(0.0, 0.073, 0.082, 0.0),
            space_bg_nebula_color_c: Vec4::new(0.187, 0.16, 0.539, 0.0),
            space_bg_star_color: Vec4::new(0.698, 0.682, 2.0, 1.0),
            space_bg_flare_tint: Vec4::new(1.0, 1.0, 2.0, 1.0),
            space_bg_depth_a: Vec4::new(1.03, 0.83, 1.69, 1.08),
            space_bg_light_a: Vec4::new(-0.3, 0.10, 4.0, 0.49),
            space_bg_light_b: Vec4::new(2.2, 1.35, 0.14, 0.0),
            space_bg_light_flags: Vec4::new(1.0, 1.0, 0.0, 1.0),
            space_bg_shafts_a: Vec4::new(1.76, 0.47, 2.65, 16.0),
            space_bg_shafts_b: Vec4::new(1.15, 1.0, 1.45, 0.85),
            space_bg_backlight_color: Vec4::new(1.15, 1.0, 1.45, 1.0),
        }
    }
}

impl Material2d for SpaceBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(super::shaders::SPACE_BACKGROUND_SHADER_HANDLE)
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Opaque
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StreamedSpriteShaderMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub image: Handle<Image>,
}

impl Material2d for StreamedSpriteShaderMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(super::shaders::SPRITE_PIXEL_SHADER_HANDLE)
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ThrusterPlumeMaterial {
    #[uniform(0)]
    pub params: ThrusterPlumeUniforms,
}

#[derive(ShaderType, Debug, Clone)]
pub struct ThrusterPlumeUniforms {
    pub shape_params: Vec4,
    pub state_params: Vec4,
    pub base_color: Vec4,
    pub hot_color: Vec4,
    pub afterburner_color: Vec4,
}

impl Default for ThrusterPlumeMaterial {
    fn default() -> Self {
        Self {
            params: ThrusterPlumeUniforms {
                shape_params: Vec4::new(1.25, 1.7, 0.35, 0.0),
                state_params: Vec4::new(0.0, 0.0, 0.0, 0.0),
                base_color: Vec4::new(1.0, 0.4, 0.15, 1.0),
                hot_color: Vec4::new(1.0, 0.82, 0.3, 1.0),
                afterburner_color: Vec4::new(0.68, 0.88, 1.12, 1.0),
            },
        }
    }
}

impl Material2d for ThrusterPlumeMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(super::shaders::THRUSTER_PLUME_SHADER_HANDLE)
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct TacticalMapOverlayMaterial {
    #[uniform(0)]
    pub viewport_time: Vec4, // x=width, y=height, z=time_s, w=alpha
    #[uniform(1)]
    pub map_center_zoom_mode: Vec4, // x=center_x, y=center_y, z=zoom_px_per_world, w=fx_mode
    #[uniform(2)]
    pub grid_major: Vec4, // rgb + alpha
    #[uniform(3)]
    pub grid_minor: Vec4, // rgb + alpha
    #[uniform(4)]
    pub grid_micro: Vec4, // rgb + alpha
    #[uniform(5)]
    pub grid_glow_alpha: Vec4, // x=major, y=minor, z=micro, w=unused
    #[uniform(6)]
    pub fx_params: Vec4, // x=fx_opacity, y=noise_amount, z=scanline_density, w=scanline_speed
    #[uniform(7)]
    pub fx_params_b: Vec4, // x=crt_distortion, y=vignette_strength, z=green_tint_mix, w=unused
}

impl Default for TacticalMapOverlayMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            map_center_zoom_mode: Vec4::new(0.0, 0.0, 1.0, 1.0),
            grid_major: Vec4::new(0.22, 0.34, 0.48, 0.14),
            grid_minor: Vec4::new(0.22, 0.34, 0.48, 0.126),
            grid_micro: Vec4::new(0.22, 0.34, 0.48, 0.113),
            grid_glow_alpha: Vec4::new(0.02, 0.018, 0.016, 0.0),
            fx_params: Vec4::new(0.45, 0.12, 360.0, 0.65),
            fx_params_b: Vec4::new(0.02, 0.24, 0.0, 0.0),
        }
    }
}

impl Material2d for TacticalMapOverlayMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(super::shaders::TACTICAL_MAP_OVERLAY_SHADER_HANDLE)
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Computes fullscreen world-space values used by fullscreen shaders. Runs in Last.
#[allow(clippy::too_many_arguments)]
pub fn compute_fullscreen_external_world_system(
    time: Res<'_, Time>,
    player_view_state: Res<'_, super::app_state::LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    controlled_vel_query: Query<'_, '_, &'static LinearVelocity, With<ControlledEntity>>,
    gameplay_camera_projection: Query<'_, '_, &'static Projection, With<GameplayCamera>>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut motion: ResMut<'_, StarfieldMotionState>,
    mut world_data: ResMut<'_, FullscreenExternalWorldData>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(viewport_size) = platform::safe_viewport_size(window) else {
        return;
    };

    let velocity_vector = if let Some(controlled_id) = &player_view_state.controlled_entity_id {
        if let Some(&entity) = entity_registry.by_entity_id.get(controlled_id.as_str()) {
            controlled_vel_query
                .get(entity)
                .ok()
                .map(|v| v.0)
                .unwrap_or(Vec2::ZERO)
        } else {
            Vec2::ZERO
        }
    } else {
        Vec2::ZERO
    };

    let magnitude = velocity_vector.length();
    let heading = if magnitude > 0.01 {
        velocity_vector / magnitude
    } else {
        Vec2::Y
    };

    let dt = time.delta_secs().max(0.0);
    let zoom_scale = gameplay_camera_projection
        .single()
        .ok()
        .and_then(|projection| match projection {
            Projection::Orthographic(ortho) => Some(ortho.scale.max(0.01)),
            _ => None,
        })
        .unwrap_or(1.0);

    if !motion.initialized {
        motion.initialized = true;
        motion.prev_speed = magnitude;
        motion.smoothed_warp = 0.0;
    }

    // Starfield from controlled entity: vector = velocity, magnitude = speed, heading = unit direction.
    // Parallax is distance-over-time: we need the accumulator so scroll reflects integrated displacement (continual smooth motion).
    // Do not wrap at 1.0 (caused visible reset). Shader uses fract() so pattern is periodic. Wrap at large period to avoid f32 precision loss over long sessions.
    const STARFIELD_WORLD_TO_UV: f32 = 0.024;
    const SCROLL_WRAP_PERIOD: f32 = 4096.0;

    let frame_displacement = velocity_vector * dt;
    let delta_uv = frame_displacement * STARFIELD_WORLD_TO_UV;
    motion.accumulated_scroll_uv += delta_uv;
    if motion.accumulated_scroll_uv.x.abs() >= SCROLL_WRAP_PERIOD {
        motion.accumulated_scroll_uv.x -=
            motion.accumulated_scroll_uv.x.signum() * SCROLL_WRAP_PERIOD;
    }
    if motion.accumulated_scroll_uv.y.abs() >= SCROLL_WRAP_PERIOD {
        motion.accumulated_scroll_uv.y -=
            motion.accumulated_scroll_uv.y.signum() * SCROLL_WRAP_PERIOD;
    }

    let travel_uv = motion.accumulated_scroll_uv;
    motion.starfield_drift_uv = travel_uv;
    motion.background_drift_uv = travel_uv * 0.32;

    let target_warp = ((magnitude - 480.0) / 1650.0).clamp(0.0, 1.25);
    let warp_alpha = 1.0 - (-7.5 * dt).exp();
    motion.smoothed_warp = motion.smoothed_warp.lerp(target_warp, warp_alpha);

    let warp = motion.smoothed_warp;

    world_data.viewport_time =
        Vec4::new(viewport_size.x, viewport_size.y, time.elapsed_secs(), warp);
    // Y-flip so world Y-up matches screen: stars stream opposite travel (e.g. 223° -> 43°).
    world_data.drift_intensity = Vec4::new(travel_uv.x, -travel_uv.y, 1.0, 1.0);
    world_data.velocity_dir = Vec4::new(heading.x, heading.y, zoom_scale, 0.0);
}

#[allow(clippy::type_complexity)]
pub fn update_starfield_material_system(
    world_data: Res<'_, FullscreenExternalWorldData>,
    starfield_query: Query<
        '_,
        '_,
        (
            &'_ MeshMaterial2d<StarfieldMaterial>,
            Option<&'_ StarfieldShaderSettings>,
            Option<&'_ mut Visibility>,
        ),
        With<StarfieldBackdrop>,
    >,
    mut materials: ResMut<'_, Assets<StarfieldMaterial>>,
) {
    for (material_handle, settings, maybe_visibility) in starfield_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            let settings = settings.cloned().unwrap_or_default();
            if let Some(mut visibility) = maybe_visibility {
                *visibility = if settings.enabled {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
            material.viewport_time = world_data.viewport_time;
            material.drift_intensity = world_data.drift_intensity;
            material.velocity_dir = world_data.velocity_dir;
            material.starfield_params = Vec4::new(
                settings.density.clamp(0.0, 1.0),
                settings.layer_count.clamp(1, 8) as f32,
                settings.initial_z_offset.clamp(0.0, 1.0),
                settings.alpha.clamp(0.0, 1.0),
            );
            material.starfield_tint = settings.tint_rgb.extend(settings.intensity.max(0.0));
            material.star_core_params = Vec4::new(
                settings.star_size.clamp(0.1, 10.0),
                settings.star_intensity.clamp(0.0, 10.0),
                settings.star_alpha.clamp(0.0, 1.0),
                0.0,
            );
            material.star_core_color = settings.star_color_rgb.extend(1.0);
            material.corona_params = Vec4::new(
                settings.corona_size.clamp(0.1, 10.0),
                settings.corona_intensity.clamp(0.0, 10.0),
                settings.corona_alpha.clamp(0.0, 1.0),
                0.0,
            );
            material.corona_color = settings.corona_color_rgb.extend(1.0);
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn update_space_background_material_system(
    world_data: Res<'_, FullscreenExternalWorldData>,
    asset_server: Res<'_, AssetServer>,
    asset_manager: Res<'_, assets::LocalAssetManager>,
    bg_query: Query<
        '_,
        '_,
        (
            &'_ MeshMaterial2d<SpaceBackgroundMaterial>,
            Option<&'_ SpaceBackgroundShaderSettings>,
            Option<&'_ mut Visibility>,
        ),
        With<SpaceBackgroundBackdrop>,
    >,
    mut materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
) {
    for (material_handle, settings, maybe_visibility) in bg_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            let settings = settings.cloned().unwrap_or_default();
            if let Some(mut visibility) = maybe_visibility {
                *visibility = if settings.enabled {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
            material.params.viewport_time = world_data.viewport_time;
            material.params.drift_intensity = world_data.drift_intensity;
            material.params.velocity_dir = world_data.velocity_dir;
            material.params.space_bg_params = Vec4::new(
                settings.intensity.max(0.0),
                settings.drift_scale.max(0.0),
                settings.velocity_glow.max(0.0),
                settings.nebula_strength.max(0.0),
            );
            material.params.space_bg_tint = settings.tint_rgb.extend(settings.seed);
            material.params.space_bg_background = settings.background_rgb.extend(1.0);
            let flare_asset_id = flare_texture_asset_id_for_set(settings.flare_texture_set);
            let mut flare_enabled = settings.flare_enabled;
            if let Some(path) = assets::streamed_visual_asset_path(flare_asset_id, &asset_manager) {
                material.flare_texture = asset_server.load(path);
            } else {
                flare_enabled = false;
            }
            material.params.space_bg_flare = Vec4::new(
                if flare_enabled { 1.0 } else { 0.0 },
                settings.flare_intensity.max(0.0),
                settings.flare_density.clamp(0.0, 1.0),
                settings.flare_size.max(0.01),
            );
            material.params.space_bg_noise_a = Vec4::new(
                settings.nebula_noise_mode.clamp(0, 1) as f32,
                settings.nebula_octaves.clamp(1, 8) as f32,
                settings.nebula_gain.clamp(0.1, 0.95),
                settings.nebula_lacunarity.clamp(1.1, 4.0),
            );
            material.params.space_bg_noise_b = Vec4::new(
                settings.nebula_power.clamp(0.2, 4.0),
                settings.nebula_shelf.clamp(0.0, 0.95),
                settings.nebula_ridge_offset.clamp(0.5, 2.5),
                0.0,
            );
            material.params.space_bg_star_mask_a = Vec4::new(
                if settings.star_mask_enabled { 1.0 } else { 0.0 },
                settings.star_mask_mode.clamp(0, 1) as f32,
                settings.star_mask_octaves.clamp(1, 8) as f32,
                settings.star_mask_scale.clamp(0.2, 8.0),
            );
            material.params.space_bg_star_mask_b = Vec4::new(
                settings.star_mask_threshold.clamp(0.0, 0.99),
                settings.star_mask_power.clamp(0.2, 4.0),
                settings.star_mask_gain.clamp(0.1, 0.95),
                settings.star_mask_lacunarity.clamp(1.1, 4.0),
            );
            material.params.space_bg_star_mask_c = Vec4::new(
                settings.star_mask_ridge_offset.clamp(0.5, 2.5),
                settings.star_count.clamp(0.0, 5.0),
                settings.star_size_min.clamp(0.01, 0.35),
                settings.star_size_max.clamp(0.01, 0.35),
            );
            material.params.space_bg_blend_a = Vec4::new(
                settings.nebula_blend_mode.clamp(0, 2) as f32,
                settings.nebula_opacity.clamp(0.0, 1.0),
                settings.stars_blend_mode.clamp(0, 2) as f32,
                settings.stars_opacity.clamp(0.0, 1.0),
            );
            material.params.space_bg_blend_b = Vec4::new(
                settings.flares_blend_mode.clamp(0, 2) as f32,
                settings.flares_opacity.clamp(0.0, 1.0),
                settings.zoom_rate.clamp(0.0, 4.0),
                0.0,
            );
            material.params.space_bg_nebula_color_a = settings
                .nebula_color_primary_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(0.0);
            material.params.space_bg_nebula_color_b = settings
                .nebula_color_secondary_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(0.0);
            material.params.space_bg_nebula_color_c = settings
                .nebula_color_accent_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(0.0);
            material.params.space_bg_star_color = settings
                .star_color_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(1.0);
            material.params.space_bg_flare_tint = settings
                .flare_tint_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(1.0);
            material.params.space_bg_depth_a = Vec4::new(
                settings.depth_layer_separation.clamp(0.0, 2.0),
                settings.depth_parallax_scale.clamp(0.0, 2.0),
                settings.depth_haze_strength.clamp(0.0, 2.0),
                settings.depth_occlusion_strength.clamp(0.0, 3.0),
            );
            material.params.space_bg_light_a = Vec4::new(
                settings.backlight_screen_x.clamp(-1.5, 1.5),
                settings.backlight_screen_y.clamp(-1.5, 1.5),
                settings.backlight_intensity.clamp(0.0, 20.0),
                settings.backlight_wrap.clamp(0.0, 2.0),
            );
            material.params.space_bg_light_b = Vec4::new(
                settings.backlight_edge_boost.clamp(0.0, 6.0),
                settings.backlight_bloom_scale.clamp(0.0, 2.0),
                settings.backlight_bloom_threshold.clamp(0.0, 1.0),
                0.0,
            );
            material.params.space_bg_light_flags = Vec4::new(
                if settings.enable_backlight { 1.0 } else { 0.0 },
                if settings.enable_light_shafts { 1.0 } else { 0.0 },
                if settings.shafts_debug_view { 1.0 } else { 0.0 },
                settings.shaft_blend_mode.clamp(0, 2) as f32,
            );
            material.params.space_bg_shafts_a = Vec4::new(
                settings.shaft_intensity.clamp(0.0, 40.0),
                settings.shaft_length.clamp(0.05, 0.95),
                settings.shaft_falloff.clamp(0.2, 8.0),
                settings.shaft_samples.clamp(4, 24) as f32,
            );
            material.params.space_bg_shafts_b = settings
                .shaft_color_rgb
                .clamp(Vec3::ZERO, Vec3::splat(3.0))
                .extend(settings.shaft_opacity.clamp(0.0, 1.0));
            material.params.space_bg_backlight_color = settings
                .backlight_color_rgb
                .clamp(Vec3::ZERO, Vec3::splat(3.0))
                .extend(1.0);
        }
    }
}

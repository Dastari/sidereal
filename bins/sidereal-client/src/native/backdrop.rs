//! Fullscreen backdrop materials (starfield, space background, streamed sprite) and their update systems.

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, MeshMaterial2d};
use sidereal_runtime_sync::RuntimeEntityHierarchy;

use sidereal_game::{
    SpaceBackgroundShaderSettings, StarfieldShaderSettings, default_space_bg_flare_blue_asset_id,
    default_space_bg_flare_red_asset_id, default_space_bg_flare_sun_asset_id,
    default_space_bg_flare_white_asset_id,
};

use super::assets;
use super::components::{
    ControlledEntity, GameplayCamera, SpaceBackgroundBackdrop, StarfieldBackdrop,
};
use super::platform::{self};
use super::resources::{FullscreenExternalWorldData, StarfieldMotionState};

fn flare_texture_asset_id_for_set(set: u32) -> &'static str {
    match set {
        1 => default_space_bg_flare_blue_asset_id(),
        2 => default_space_bg_flare_red_asset_id(),
        3 => default_space_bg_flare_sun_asset_id(),
        _ => default_space_bg_flare_white_asset_id(),
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
}

impl Default for StarfieldMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            drift_intensity: Vec4::new(0.0, 0.0, 1.0, 1.0),
            velocity_dir: Vec4::new(0.0, 1.0, 1.0, 0.0),
            starfield_params: Vec4::new(0.05, 3.0, 0.35, 1.0),
            starfield_tint: Vec4::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

impl Material2d for StarfieldMaterial {
    fn fragment_shader() -> ShaderRef {
        "data/cache_stream/shaders/starfield.wgsl".into()
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
    pub space_bg_flare_tint: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
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
            space_bg_params: Vec4::new(1.0, 1.0, 1.0, 1.0),
            space_bg_tint: Vec4::ONE,
            space_bg_background: Vec4::new(0.004, 0.007, 0.018, 1.0),
            space_bg_flare: Vec4::new(1.0, 0.2, 0.85, 0.0),
            space_bg_noise_a: Vec4::new(0.0, 5.0, 0.52, 2.0),
            space_bg_noise_b: Vec4::new(1.0, 0.42, 1.0, 0.0),
            space_bg_star_mask_a: Vec4::new(0.0, 0.0, 4.0, 1.4),
            space_bg_star_mask_b: Vec4::new(0.35, 1.2, 0.55, 2.0),
            space_bg_star_mask_c: Vec4::new(1.0, 0.0, 0.0, 0.0),
            space_bg_blend_a: Vec4::new(1.0, 1.0, 0.0, 1.0),
            space_bg_blend_b: Vec4::new(1.0, 1.0, 0.0, 0.0),
            space_bg_nebula_color_a: Vec4::new(0.07, 0.13, 0.28, 0.0),
            space_bg_nebula_color_b: Vec4::new(0.12, 0.24, 0.40, 0.0),
            space_bg_nebula_color_c: Vec4::new(0.18, 0.16, 0.36, 0.0),
            space_bg_flare_tint: Vec4::ONE,
        }
    }
}

impl Default for SpaceBackgroundMaterial {
    fn default() -> Self {
        Self {
            params: SpaceBackgroundUniforms::default(),
            flare_texture: Handle::default(),
        }
    }
}

impl Material2d for SpaceBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        "data/cache_stream/shaders/space_background.wgsl".into()
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
        platform::STREAMED_SPRITE_PIXEL_SHADER_PATH.into()
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
        }
    }
}

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
                0.0,
                0.0,
                0.0,
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
                0.0,
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
            material.params.space_bg_flare_tint = settings
                .flare_tint_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(1.0);
        }
    }
}

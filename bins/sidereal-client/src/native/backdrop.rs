//! Fullscreen backdrop materials (starfield, space background, streamed sprite) and their update systems.

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, MeshMaterial2d};
use sidereal_runtime_sync::RuntimeEntityHierarchy;

use super::components::{ControlledEntity, SpaceBackgroundBackdrop, StarfieldBackdrop};
use super::platform::{self};
use super::resources::{CameraMotionState, StarfieldMotionState};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StarfieldMaterial {
    #[uniform(0)]
    pub viewport_time: Vec4,
    #[uniform(1)]
    pub drift_intensity: Vec4,
    #[uniform(2)]
    pub velocity_dir: Vec4,
}

impl Default for StarfieldMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            drift_intensity: Vec4::new(0.0, 0.0, 1.0, 1.0),
            velocity_dir: Vec4::new(0.0, 1.0, 0.0, 0.0),
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

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SpaceBackgroundMaterial {
    #[uniform(0)]
    pub viewport_time: Vec4,
    #[uniform(1)]
    pub colors: Vec4,
    #[uniform(2)]
    pub motion: Vec4,
}

impl Default for SpaceBackgroundMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 1.0),
            colors: Vec4::new(0.05, 0.08, 0.15, 1.0),
            motion: Vec4::ZERO,
        }
    }
}

impl Material2d for SpaceBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        "data/cache_stream/shaders/simple_space_background.wgsl".into()
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

/// Updates starfield material from the controlled entity's velocity: vector → magnitude + heading, accumulated scroll (distance-over-time), and warp. Runs in Last.
#[allow(clippy::too_many_arguments)]
pub fn update_starfield_material_system(
    time: Res<'_, Time>,
    player_view_state: Res<'_, super::state::LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    controlled_vel_query: Query<'_, '_, &'static LinearVelocity, With<ControlledEntity>>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut motion: ResMut<'_, StarfieldMotionState>,
    starfield_query: Query<'_, '_, &MeshMaterial2d<StarfieldMaterial>, With<StarfieldBackdrop>>,
    mut materials: ResMut<'_, Assets<StarfieldMaterial>>,
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
        motion.accumulated_scroll_uv.x -= motion.accumulated_scroll_uv.x.signum() * SCROLL_WRAP_PERIOD;
    }
    if motion.accumulated_scroll_uv.y.abs() >= SCROLL_WRAP_PERIOD {
        motion.accumulated_scroll_uv.y -= motion.accumulated_scroll_uv.y.signum() * SCROLL_WRAP_PERIOD;
    }

    let travel_uv = motion.accumulated_scroll_uv;

    let target_warp = ((magnitude - 480.0) / 1650.0).clamp(0.0, 1.25);
    let warp_alpha = 1.0 - (-7.5 * dt).exp();
    motion.smoothed_warp = motion.smoothed_warp.lerp(target_warp, warp_alpha);

    let warp = motion.smoothed_warp;

    for material_handle in &starfield_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.viewport_time =
                Vec4::new(viewport_size.x, viewport_size.y, time.elapsed_secs(), warp);
            // Y-flip so world Y-up matches screen: stars stream opposite travel (e.g. 223° → 43°).
            material.drift_intensity = Vec4::new(travel_uv.x, -travel_uv.y, 1.0, 1.0);
            material.velocity_dir = Vec4::new(heading.x, heading.y, magnitude, 0.0);
        }
    }
}

pub fn update_space_background_material_system(
    time: Res<'_, Time>,
    camera_motion: Res<'_, CameraMotionState>,
    starfield_motion: Res<'_, StarfieldMotionState>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    bg_query: Query<
        '_,
        '_,
        &MeshMaterial2d<SpaceBackgroundMaterial>,
        With<SpaceBackgroundBackdrop>,
    >,
    mut materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(viewport_size) = platform::safe_viewport_size(window) else {
        return;
    };
    if !camera_motion.initialized {
        return;
    }

    let drift_xy = starfield_motion.background_drift_uv;
    let velocity_xy = camera_motion.smoothed_velocity_xy;
    let speed = velocity_xy.length();
    let velocity_dir = if speed > 0.001 {
        velocity_xy / speed
    } else {
        Vec2::Y
    };

    for material_handle in &bg_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.viewport_time =
                Vec4::new(viewport_size.x, viewport_size.y, time.elapsed_secs(), 0.0);
            material.motion = Vec4::new(drift_xy.x, drift_xy.y, velocity_dir.x, velocity_dir.y);
        }
    }
}

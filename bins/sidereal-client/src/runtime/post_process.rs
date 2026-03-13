use bevy::{
    core_pipeline::{
        FullscreenShader,
        core_2d::graph::{Core2d, Node2d},
    },
    ecs::query::QueryItem,
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_graph::{
            NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            BindGroupEntries, BindGroupLayoutDescriptor, CachedRenderPipelineId, ColorTargetState,
            ColorWrites, FragmentState, Operations, PipelineCache, RenderPassColorAttachment,
            RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
            SamplerDescriptor, ShaderStages, ShaderType, TextureFormat,
            binding_types::{sampler, texture_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice},
        view::ViewTarget,
    },
};

use super::app_state::ClientAppState;
use super::components::{GameplayCamera, WeaponImpactExplosion};

const EXPLOSION_DISTORTION_POST_PROCESS_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/explosion_distortion_post_process.wgsl");
#[cfg(test)]
const SHADER_ASSET_PATH: &str = "data/shaders/explosion_distortion_post_process.wgsl";
const MAX_EXPLOSION_SHOCKWAVES: usize = 8;
const BASE_SHOCKWAVE_STRENGTH: f32 = 0.012;
const SHOCKWAVE_RADIUS_WORLD_FACTOR: f32 = 0.5;

pub const EXPLOSION_DISTORTION_POST_PROCESS_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("361b5057-b81d-4b3d-a26a-7d72af30e5d0");

pub(crate) struct ExplosionDistortionPostProcessPlugin;

impl Plugin for ExplosionDistortionPostProcessPlugin {
    fn build(&self, app: &mut App) {
        install_explosion_distortion_shader(app);
        app.add_plugins((
            ExtractComponentPlugin::<ExplosionDistortionSettings>::default(),
            UniformComponentPlugin::<ExplosionDistortionSettings>::default(),
        ));
        app.add_systems(
            PostUpdate,
            update_explosion_distortion_settings_system
                .after(bevy::transform::TransformSystems::Propagate)
                .run_if(in_state(ClientAppState::InWorld)),
        );

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.add_systems(RenderStartup, init_explosion_distortion_pipeline);
        render_app
            .add_render_graph_node::<ViewNodeRunner<ExplosionDistortionNode>>(
                Core2d,
                ExplosionDistortionLabel,
            )
            .add_render_graph_edges(
                Core2d,
                (
                    Node2d::Tonemapping,
                    ExplosionDistortionLabel,
                    Node2d::EndMainPassPostProcessing,
                ),
            );
    }
}

#[derive(Component, Clone, Copy, Debug, ExtractComponent, ShaderType, PartialEq)]
pub(crate) struct ExplosionDistortionSettings {
    metadata: Vec4,
    shockwaves: [Vec4; MAX_EXPLOSION_SHOCKWAVES],
}

impl Default for ExplosionDistortionSettings {
    fn default() -> Self {
        Self {
            metadata: Vec4::ZERO,
            shockwaves: [Vec4::ZERO; MAX_EXPLOSION_SHOCKWAVES],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct EncodedShockwave {
    center_uv: Vec2,
    radius_uv: f32,
    strength: f32,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct ExplosionDistortionLabel;

#[derive(Default)]
struct ExplosionDistortionNode;

impl ViewNode for ExplosionDistortionNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ExplosionDistortionSettings,
        &'static DynamicUniformIndex<ExplosionDistortionSettings>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, settings, settings_index): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if settings.metadata.x <= 0.0 {
            return Ok(());
        }

        let pipeline = world.resource::<ExplosionDistortionPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
            return Ok(());
        };

        let settings_uniforms = world.resource::<ComponentUniforms<ExplosionDistortionSettings>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            return Ok(());
        };

        let post_process = view_target.post_process_write();
        let bind_group = render_context.render_device().create_bind_group(
            "explosion_distortion_post_process_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline.sampler,
                settings_binding.clone(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("explosion_distortion_post_process_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_render_pipeline(render_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[settings_index.index()]);
        render_pass.draw(0..3, 0..1);
        Ok(())
    }
}

#[derive(Resource)]
struct ExplosionDistortionPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

fn init_explosion_distortion_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "explosion_distortion_post_process_bind_group_layout",
        &bevy::render::render_resource::BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(bevy::render::render_resource::TextureSampleType::Float {
                    filterable: true,
                }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<ExplosionDistortionSettings>(true),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("explosion_distortion_post_process_sampler"),
        ..default()
    });
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("explosion_distortion_post_process_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: EXPLOSION_DISTORTION_POST_PROCESS_SHADER_HANDLE,
            entry_point: Some("fragment_main".into()),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::bevy_default(),
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(ExplosionDistortionPipeline {
        layout,
        sampler,
        pipeline_id,
    });
}

fn install_explosion_distortion_shader(app: &mut App) {
    let mut shaders = app
        .world_mut()
        .resource_mut::<Assets<bevy::shader::Shader>>();
    let _ = shaders.insert(
        EXPLOSION_DISTORTION_POST_PROCESS_SHADER_HANDLE.id(),
        bevy::shader::Shader::from_wgsl(
            EXPLOSION_DISTORTION_POST_PROCESS_SHADER_SOURCE,
            "sidereal://shader/explosion_distortion_post_process",
        ),
    );
}

#[allow(clippy::type_complexity)]
fn update_explosion_distortion_settings_system(
    mut commands: Commands<'_, '_>,
    cameras: Query<
        '_,
        '_,
        (
            Entity,
            &'_ Camera,
            &'_ GlobalTransform,
            Option<&'_ ExplosionDistortionSettings>,
        ),
        With<GameplayCamera>,
    >,
    explosions: Query<
        '_,
        '_,
        (
            &'_ WeaponImpactExplosion,
            &'_ GlobalTransform,
            &'_ Transform,
            &'_ Visibility,
        ),
    >,
) {
    let Ok((camera_entity, camera, camera_transform, current_settings)) = cameras.single() else {
        return;
    };
    let Some(viewport_size) = camera.logical_viewport_size() else {
        if current_settings.is_some() {
            commands
                .entity(camera_entity)
                .remove::<ExplosionDistortionSettings>();
        }
        return;
    };
    if viewport_size.x <= 1.0 || viewport_size.y <= 1.0 {
        if current_settings.is_some() {
            commands
                .entity(camera_entity)
                .remove::<ExplosionDistortionSettings>();
        }
        return;
    }

    let mut encoded = Vec::<EncodedShockwave>::new();
    for (explosion, global_transform, transform, visibility) in &explosions {
        if *visibility == Visibility::Hidden
            || explosion.ttl_s <= 0.0
            || explosion.max_ttl_s <= 0.0
            || explosion.screen_distortion_scale <= 0.0
        {
            continue;
        }
        let world_translation = global_transform.translation();
        let center_world = Vec3::new(world_translation.x, world_translation.y, 0.0);
        let Ok(center_viewport) = camera.world_to_viewport(camera_transform, center_world) else {
            continue;
        };
        let world_radius =
            transform.scale.x.max(0.01) * explosion.domain_scale * SHOCKWAVE_RADIUS_WORLD_FACTOR;
        let edge_world = Vec3::new(world_translation.x + world_radius, world_translation.y, 0.0);
        let Ok(edge_viewport) = camera.world_to_viewport(camera_transform, edge_world) else {
            continue;
        };
        let age_norm = 1.0 - (explosion.ttl_s / explosion.max_ttl_s).clamp(0.0, 1.0);
        let phase_strength = shockwave_phase_strength(age_norm);
        if phase_strength <= 0.0 {
            continue;
        }
        let radius_px = (edge_viewport.x - center_viewport.x).abs().max(1.0);
        let strength = BASE_SHOCKWAVE_STRENGTH
            * explosion.intensity_scale
            * explosion.screen_distortion_scale
            * phase_strength;
        if let Some(shockwave) =
            encode_shockwave(center_viewport, viewport_size, radius_px, strength)
        {
            encoded.push(shockwave);
        }
    }

    encoded.sort_by(|left, right| {
        let left_weight = left.strength * left.radius_uv;
        let right_weight = right.strength * right.radius_uv;
        right_weight.total_cmp(&left_weight)
    });

    if encoded.is_empty() {
        if current_settings.is_some() {
            commands
                .entity(camera_entity)
                .remove::<ExplosionDistortionSettings>();
        }
        return;
    }

    let mut next = ExplosionDistortionSettings::default();
    let active_count = encoded.len().min(MAX_EXPLOSION_SHOCKWAVES);
    next.metadata.x = active_count as f32;
    for (index, shockwave) in encoded
        .into_iter()
        .take(MAX_EXPLOSION_SHOCKWAVES)
        .enumerate()
    {
        next.shockwaves[index] = Vec4::new(
            shockwave.center_uv.x,
            shockwave.center_uv.y,
            shockwave.radius_uv,
            shockwave.strength,
        );
    }
    if current_settings.is_none_or(|current| *current != next) {
        commands.entity(camera_entity).insert(next);
    }
}

fn shockwave_phase_strength(age_norm: f32) -> f32 {
    let ring_window = 1.0 - ((age_norm - 0.36).abs() / 0.36);
    ring_window.clamp(0.0, 1.0)
}

fn encode_shockwave(
    center_viewport: Vec2,
    viewport_size: Vec2,
    radius_px: f32,
    strength: f32,
) -> Option<EncodedShockwave> {
    if viewport_size.x <= 1.0 || viewport_size.y <= 1.0 || strength <= 0.0 {
        return None;
    }
    let center_uv = Vec2::new(
        center_viewport.x / viewport_size.x,
        center_viewport.y / viewport_size.y,
    );
    let radius_uv = (radius_px / viewport_size.y).max(0.001);
    if center_uv.x < -radius_uv
        || center_uv.x > 1.0 + radius_uv
        || center_uv.y < -radius_uv
        || center_uv.y > 1.0 + radius_uv
    {
        return None;
    }
    Some(EncodedShockwave {
        center_uv,
        radius_uv,
        strength: strength.clamp(0.0, 0.08),
    })
}

#[cfg(test)]
mod tests {
    use super::{MAX_EXPLOSION_SHOCKWAVES, encode_shockwave, shockwave_phase_strength};
    use bevy::prelude::*;
    use std::path::PathBuf;

    #[test]
    fn encode_shockwave_rejects_fully_offscreen_entries() {
        let encoded = encode_shockwave(
            Vec2::new(-120.0, 300.0),
            Vec2::new(1280.0, 720.0),
            20.0,
            0.02,
        );
        assert!(
            encoded.is_none(),
            "far offscreen shockwaves should be discarded"
        );
    }

    #[test]
    fn encode_shockwave_normalizes_screen_space_values() {
        let encoded = encode_shockwave(
            Vec2::new(640.0, 360.0),
            Vec2::new(1280.0, 720.0),
            72.0,
            0.03,
        )
        .expect("centered shockwave should encode");
        assert!((encoded.center_uv.x - 0.5).abs() < 0.0001);
        assert!((encoded.center_uv.y - 0.5).abs() < 0.0001);
        assert!((encoded.radius_uv - 0.1).abs() < 0.0001);
        assert!(encoded.strength <= 0.08);
    }

    #[test]
    fn shockwave_phase_strength_peaks_mid_life() {
        let early = shockwave_phase_strength(0.05);
        let peak = shockwave_phase_strength(0.36);
        let late = shockwave_phase_strength(0.82);
        assert!(peak > early);
        assert!(peak > late);
    }

    #[test]
    fn post_process_capacity_constant_stays_small() {
        assert_eq!(MAX_EXPLOSION_SHOCKWAVES, 8);
    }

    #[test]
    fn explosion_distortion_shader_path_exists_in_repo() {
        let shader_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(super::SHADER_ASSET_PATH);
        assert!(
            shader_path.exists(),
            "explosion distortion shader path should exist: {}",
            shader_path.display()
        );
    }
}

//! Streamed shader placeholder creation and reload.

use bevy::prelude::*;

use super::platform::STREAMED_SPRITE_PIXEL_SHADER_PATH;
use super::resources::LocalAssetManager;

const STREAMED_SHADER_PATHS: &[&str] = &[
    "data/cache_stream/shaders/starfield.wgsl",
    "data/cache_stream/shaders/simple_space_background.wgsl",
    STREAMED_SPRITE_PIXEL_SHADER_PATH,
];

const LOCAL_SHADER_FALLBACK_PATHS: &[&str] = &[
    "data/shaders/starfield.wgsl",
    "data/shaders/simple_space_background.wgsl",
    "data/shaders/sprite_pixel_effect.wgsl",
];

pub fn ensure_shader_placeholders(asset_root: &str) {
    const STARFIELD_PLACEHOLDER: &str = "\
#import bevy_sprite::mesh2d_vertex_output::VertexOutput
@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> drift_intensity: vec4<f32>;
@group(2) @binding(2) var<uniform> velocity_dir: vec4<f32>;
@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
";

    const BACKGROUND_PLACEHOLDER: &str = "\
#import bevy_sprite::mesh2d_vertex_output::VertexOutput
@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> colors: vec4<f32>;
@group(2) @binding(2) var<uniform> motion: vec4<f32>;
@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(colors.r, colors.g, colors.b, 1.0);
}
";

    const SPRITE_PIXEL_PLACEHOLDER: &str = "\
#import bevy_sprite::mesh2d_vertex_output::VertexOutput
@group(2) @binding(0) var image: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;
@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(image, image_sampler, mesh.uv);
}
";

    let placeholders: &[(&str, &str, &str)] = &[
        (
            STREAMED_SHADER_PATHS[0],
            LOCAL_SHADER_FALLBACK_PATHS[0],
            STARFIELD_PLACEHOLDER,
        ),
        (
            STREAMED_SHADER_PATHS[1],
            LOCAL_SHADER_FALLBACK_PATHS[1],
            BACKGROUND_PLACEHOLDER,
        ),
        (
            STREAMED_SHADER_PATHS[2],
            LOCAL_SHADER_FALLBACK_PATHS[2],
            SPRITE_PIXEL_PLACEHOLDER,
        ),
    ];

    for &(cache_rel_path, source_rel_path, placeholder_content) in placeholders {
        let cache_path = std::path::PathBuf::from(asset_root).join(cache_rel_path);
        if cache_path.exists() {
            continue;
        }
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let source_path = std::path::PathBuf::from(asset_root).join(source_rel_path);
        let content = std::fs::read_to_string(&source_path)
            .ok()
            .unwrap_or_else(|| placeholder_content.to_string());
        std::fs::write(&cache_path, content).ok();
    }
}

pub fn reload_streamed_shaders(
    asset_server: &AssetServer,
    shaders: &mut Assets<bevy::shader::Shader>,
    asset_root: &str,
) {
    for (idx, &path) in STREAMED_SHADER_PATHS.iter().enumerate() {
        let cache_path = std::path::PathBuf::from(asset_root).join(path);
        let local_fallback_path = std::path::PathBuf::from(asset_root).join(
            LOCAL_SHADER_FALLBACK_PATHS
                .get(idx)
                .copied()
                .unwrap_or(path),
        );

        let selected_path = if cache_path.exists() {
            cache_path
        } else {
            local_fallback_path
        };

        if let Ok(content) = std::fs::read_to_string(&selected_path) {
            let handle: Handle<bevy::shader::Shader> = asset_server.load(path);
            let _ = shaders.insert(handle.id(), bevy::shader::Shader::from_wgsl(content, path));
        }
    }
}

pub fn streamed_shader_path_for_asset_id(shader_asset_id: &str) -> Option<&'static str> {
    match shader_asset_id {
        "starfield_wgsl" => Some(STREAMED_SHADER_PATHS[0]),
        "space_background_wgsl" => Some(STREAMED_SHADER_PATHS[1]),
        _ => None,
    }
}

pub fn fullscreen_layer_shader_ready(
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    shader_asset_id: &str,
) -> bool {
    if let Some(relative_cache_path) = asset_manager.cached_relative_path(shader_asset_id) {
        let rooted_stream_path = std::path::PathBuf::from(asset_root)
            .join("data/cache_stream")
            .join(relative_cache_path);
        let rooted_direct_path = std::path::PathBuf::from(asset_root).join(relative_cache_path);
        if rooted_stream_path.exists() || rooted_direct_path.exists() {
            return true;
        }
    }

    let Some(streamed_shader_rel_path) = streamed_shader_path_for_asset_id(shader_asset_id) else {
        return false;
    };
    std::path::PathBuf::from(asset_root)
        .join(streamed_shader_rel_path)
        .exists()
}

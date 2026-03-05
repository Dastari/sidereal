//! Streamed shader reload helpers.

use bevy::prelude::*;

use super::assets::LocalAssetManager;

const DEFAULT_STARFIELD_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/starfield.wgsl");
const DEFAULT_SPACE_BACKGROUND_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/space_background.wgsl");
const DEFAULT_SPRITE_PIXEL_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/sprite_pixel_effect.wgsl");
const DEFAULT_THRUSTER_PLUME_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/thruster_plume.wgsl");
const DEFAULT_TACTICAL_MAP_OVERLAY_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/tactical_map_overlay.wgsl");

const STARFIELD_SHADER_OVERRIDE_PATH: &str = "data/cache_stream/shaders/starfield.wgsl";
const SPACE_BACKGROUND_SHADER_OVERRIDE_PATH: &str =
    "data/cache_stream/shaders/space_background.wgsl";
const SPRITE_PIXEL_SHADER_OVERRIDE_PATH: &str =
    "data/cache_stream/shaders/sprite_pixel_effect.wgsl";
const THRUSTER_PLUME_SHADER_OVERRIDE_PATH: &str = "data/cache_stream/shaders/thruster_plume.wgsl";
const TACTICAL_MAP_OVERLAY_SHADER_OVERRIDE_PATH: &str =
    "data/cache_stream/shaders/tactical_map_overlay.wgsl";

const STARFIELD_SHADER_LABEL: &str = "sidereal://shader/starfield";
const SPACE_BACKGROUND_SHADER_LABEL: &str = "sidereal://shader/space_background";
const SPRITE_PIXEL_SHADER_LABEL: &str = "sidereal://shader/sprite_pixel_effect";
const THRUSTER_PLUME_SHADER_LABEL: &str = "sidereal://shader/thruster_plume";
const TACTICAL_MAP_OVERLAY_SHADER_LABEL: &str = "sidereal://shader/tactical_map_overlay";

pub const STARFIELD_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("ee54757d-14a2-4f84-8fdb-cdf547be8401");
pub const SPACE_BACKGROUND_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("3491ffc9-a955-4a2e-bdf5-7d2cef546f35");
pub const SPRITE_PIXEL_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("33999a9f-c09f-4ce2-b7d2-65c7fe640a48");
pub const THRUSTER_PLUME_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("0cae863f-b918-4470-b7ee-f30749186a34");
pub const TACTICAL_MAP_OVERLAY_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("f7de7110-00a1-41f2-b498-ec705dbd2d22");

fn env_flag_with_default(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .map(|value| matches!(value.as_str(), "1" | "true"))
        .unwrap_or(default)
}

pub fn shader_materials_enabled() -> bool {
    env_flag_with_default("SIDEREAL_ENABLE_SHADER_MATERIALS", true)
}

pub fn streamed_shader_overrides_enabled() -> bool {
    env_flag_with_default("SIDEREAL_CLIENT_ENABLE_STREAMED_SHADER_OVERRIDES", false)
}

fn read_override_or_default(
    asset_root: &str,
    override_rel_path: &str,
    default_source: &str,
) -> String {
    if !streamed_shader_overrides_enabled() {
        return default_source.to_string();
    }
    let rooted_override = std::path::PathBuf::from(asset_root).join(override_rel_path);
    std::fs::read_to_string(rooted_override).unwrap_or_else(|_| default_source.to_string())
}

fn install_shader(
    shaders: &mut Assets<bevy::shader::Shader>,
    handle: Handle<bevy::shader::Shader>,
    label: &str,
    source: String,
) {
    let _ = shaders.insert(handle.id(), bevy::shader::Shader::from_wgsl(source, label));
}

pub fn install_runtime_shaders(shaders: &mut Assets<bevy::shader::Shader>, asset_root: &str) {
    install_shader(
        shaders,
        STARFIELD_SHADER_HANDLE,
        STARFIELD_SHADER_LABEL,
        read_override_or_default(
            asset_root,
            STARFIELD_SHADER_OVERRIDE_PATH,
            DEFAULT_STARFIELD_SHADER_SOURCE,
        ),
    );
    install_shader(
        shaders,
        SPACE_BACKGROUND_SHADER_HANDLE,
        SPACE_BACKGROUND_SHADER_LABEL,
        read_override_or_default(
            asset_root,
            SPACE_BACKGROUND_SHADER_OVERRIDE_PATH,
            DEFAULT_SPACE_BACKGROUND_SHADER_SOURCE,
        ),
    );
    install_shader(
        shaders,
        SPRITE_PIXEL_SHADER_HANDLE,
        SPRITE_PIXEL_SHADER_LABEL,
        read_override_or_default(
            asset_root,
            SPRITE_PIXEL_SHADER_OVERRIDE_PATH,
            DEFAULT_SPRITE_PIXEL_SHADER_SOURCE,
        ),
    );
    install_shader(
        shaders,
        THRUSTER_PLUME_SHADER_HANDLE,
        THRUSTER_PLUME_SHADER_LABEL,
        read_override_or_default(
            asset_root,
            THRUSTER_PLUME_SHADER_OVERRIDE_PATH,
            DEFAULT_THRUSTER_PLUME_SHADER_SOURCE,
        ),
    );
    install_shader(
        shaders,
        TACTICAL_MAP_OVERLAY_SHADER_HANDLE,
        TACTICAL_MAP_OVERLAY_SHADER_LABEL,
        read_override_or_default(
            asset_root,
            TACTICAL_MAP_OVERLAY_SHADER_OVERRIDE_PATH,
            DEFAULT_TACTICAL_MAP_OVERLAY_SHADER_SOURCE,
        ),
    );
}

pub fn reload_streamed_shaders(shaders: &mut Assets<bevy::shader::Shader>, asset_root: &str) {
    install_runtime_shaders(shaders, asset_root);
}

pub fn streamed_shader_path_for_asset_id(shader_asset_id: &str) -> Option<&'static str> {
    match shader_asset_id {
        "starfield_wgsl" => Some(STARFIELD_SHADER_OVERRIDE_PATH),
        "space_background_wgsl" => Some(SPACE_BACKGROUND_SHADER_OVERRIDE_PATH),
        "sprite_pixel_effect_wgsl" => Some(SPRITE_PIXEL_SHADER_OVERRIDE_PATH),
        "thruster_plume_wgsl" => Some(THRUSTER_PLUME_SHADER_OVERRIDE_PATH),
        "tactical_map_overlay_wgsl" => Some(TACTICAL_MAP_OVERLAY_SHADER_OVERRIDE_PATH),
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

    matches!(
        shader_asset_id,
        "starfield_wgsl"
            | "space_background_wgsl"
            | "sprite_pixel_effect_wgsl"
            | "thruster_plume_wgsl"
            | "tactical_map_overlay_wgsl"
    ) || streamed_shader_path_for_asset_id(shader_asset_id).is_some_and(
        |streamed_shader_rel_path| {
            std::path::PathBuf::from(asset_root)
                .join(streamed_shader_rel_path)
                .exists()
        },
    )
}

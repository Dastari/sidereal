//! Runtime shader install/reload helpers.

use bevy::log::warn;
use bevy::prelude::*;

use super::assets::LocalAssetManager;

// Material2d fragment shaders can rely on Bevy's default vertex path.
const FALLBACK_FRAGMENT_SHADER_SOURCE: &str = r#"
@fragment
fn fragment() -> @location(0) vec4<f32> {
  return vec4<f32>(1.0, 0.0, 1.0, 1.0);
}
"#;

pub const STARFIELD_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("ee54757d-14a2-4f84-8fdb-cdf547be8401");
pub const SPACE_BACKGROUND_BASE_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("3491ffc9-a955-4a2e-bdf5-7d2cef546f35");
pub const SPACE_BACKGROUND_NEBULA_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("84fc7002-8686-4fb5-a79c-e62048fe3b78");
pub const SPRITE_PIXEL_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("33999a9f-c09f-4ce2-b7d2-65c7fe640a48");
pub const ASTEROID_SPRITE_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("5ac93fc1-e198-4a11-a3a2-3d6ca6f121d3");
pub const PLANET_VISUAL_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("cb5dd61d-270f-4dca-bc4d-7f7329a8c41b");
pub const RUNTIME_EFFECT_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("0cae863f-b918-4470-b7ee-f30749186a34");
pub const TACTICAL_MAP_OVERLAY_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("f7de7110-00a1-41f2-b498-ec705dbd2d22");

#[derive(Clone, Copy)]
enum RuntimeShaderFamily {
    Fullscreen,
    WorldSprite,
    WorldPolygon,
    Effect,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFullscreenShaderKind {
    Starfield,
    SpaceBackgroundBase,
    SpaceBackgroundNebula,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWorldSpriteShaderKind {
    GenericSprite,
    Asteroid,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWorldPolygonShaderKind {
    PlanetVisual,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEffectShaderKind {
    RuntimeEffect,
    TacticalMapOverlay,
}

#[derive(Clone)]
struct RuntimeShaderSpec {
    asset_id: &'static str,
    label: &'static str,
    handle: Handle<bevy::shader::Shader>,
    family: RuntimeShaderFamily,
    fullscreen_kind: Option<RuntimeFullscreenShaderKind>,
}

const RUNTIME_SHADER_SPECS: &[RuntimeShaderSpec] = &[
    RuntimeShaderSpec {
        asset_id: "starfield_wgsl",
        label: "sidereal://shader/starfield",
        handle: STARFIELD_SHADER_HANDLE,
        family: RuntimeShaderFamily::Fullscreen,
        fullscreen_kind: Some(RuntimeFullscreenShaderKind::Starfield),
    },
    RuntimeShaderSpec {
        asset_id: "space_background_base_wgsl",
        label: "sidereal://shader/space_background_base",
        handle: SPACE_BACKGROUND_BASE_SHADER_HANDLE,
        family: RuntimeShaderFamily::Fullscreen,
        fullscreen_kind: Some(RuntimeFullscreenShaderKind::SpaceBackgroundBase),
    },
    RuntimeShaderSpec {
        asset_id: "space_background_nebula_wgsl",
        label: "sidereal://shader/space_background_nebula",
        handle: SPACE_BACKGROUND_NEBULA_SHADER_HANDLE,
        family: RuntimeShaderFamily::Fullscreen,
        fullscreen_kind: Some(RuntimeFullscreenShaderKind::SpaceBackgroundNebula),
    },
    RuntimeShaderSpec {
        asset_id: "sprite_pixel_shader_wgsl",
        label: "sidereal://shader/sprite_pixel_effect",
        handle: SPRITE_PIXEL_SHADER_HANDLE,
        family: RuntimeShaderFamily::WorldSprite,
        fullscreen_kind: None,
    },
    RuntimeShaderSpec {
        asset_id: "asteroid_wgsl",
        label: "sidereal://shader/asteroid_sprite",
        handle: ASTEROID_SPRITE_SHADER_HANDLE,
        family: RuntimeShaderFamily::WorldSprite,
        fullscreen_kind: None,
    },
    RuntimeShaderSpec {
        asset_id: "planet_visual_wgsl",
        label: "sidereal://shader/planet_visual",
        handle: PLANET_VISUAL_SHADER_HANDLE,
        family: RuntimeShaderFamily::WorldPolygon,
        fullscreen_kind: None,
    },
    RuntimeShaderSpec {
        asset_id: "runtime_effect_wgsl",
        label: "sidereal://shader/runtime_effect",
        handle: RUNTIME_EFFECT_SHADER_HANDLE,
        family: RuntimeShaderFamily::Effect,
        fullscreen_kind: None,
    },
    RuntimeShaderSpec {
        asset_id: "tactical_map_overlay_wgsl",
        label: "sidereal://shader/tactical_map_overlay",
        handle: TACTICAL_MAP_OVERLAY_SHADER_HANDLE,
        family: RuntimeShaderFamily::Fullscreen,
        fullscreen_kind: None,
    },
];

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
    env_flag_with_default("SIDEREAL_CLIENT_ENABLE_STREAMED_SHADER_OVERRIDES", true)
}

fn install_shader(
    shaders: &mut Assets<bevy::shader::Shader>,
    handle: Handle<bevy::shader::Shader>,
    label: &str,
    source: &str,
) {
    let _ = shaders.insert(
        handle.id(),
        bevy::shader::Shader::from_wgsl(source.to_string(), label),
    );
}

fn fallback_shader_source_for_family(_family: RuntimeShaderFamily) -> &'static str {
    FALLBACK_FRAGMENT_SHADER_SOURCE
}

pub fn fullscreen_shader_kind(shader_asset_id: &str) -> Option<RuntimeFullscreenShaderKind> {
    RUNTIME_SHADER_SPECS
        .iter()
        .find(|spec| spec.asset_id == shader_asset_id)
        .and_then(|spec| spec.fullscreen_kind)
}

pub fn world_sprite_shader_kind(shader_asset_id: &str) -> Option<RuntimeWorldSpriteShaderKind> {
    RUNTIME_SHADER_SPECS
        .iter()
        .find(|spec| spec.asset_id == shader_asset_id)
        .and_then(|spec| match spec.asset_id {
            "sprite_pixel_shader_wgsl" => Some(RuntimeWorldSpriteShaderKind::GenericSprite),
            "asteroid_wgsl" => Some(RuntimeWorldSpriteShaderKind::Asteroid),
            _ => None,
        })
}

pub fn world_polygon_shader_kind(shader_asset_id: &str) -> Option<RuntimeWorldPolygonShaderKind> {
    RUNTIME_SHADER_SPECS
        .iter()
        .find(|spec| spec.asset_id == shader_asset_id)
        .and_then(|spec| match spec.asset_id {
            "planet_visual_wgsl" => Some(RuntimeWorldPolygonShaderKind::PlanetVisual),
            _ => None,
        })
}

pub fn world_polygon_shader_handle(
    kind: RuntimeWorldPolygonShaderKind,
) -> Handle<bevy::shader::Shader> {
    match kind {
        RuntimeWorldPolygonShaderKind::PlanetVisual => runtime_shader_handle("planet_visual_wgsl")
            .expect("planet visual shader handle must be registered"),
    }
}

pub fn runtime_effect_shader_handle(kind: RuntimeEffectShaderKind) -> Handle<bevy::shader::Shader> {
    match kind {
        RuntimeEffectShaderKind::RuntimeEffect => runtime_shader_handle("runtime_effect_wgsl")
            .expect("runtime effect shader handle must be registered"),
        RuntimeEffectShaderKind::TacticalMapOverlay => {
            runtime_shader_handle("tactical_map_overlay_wgsl")
                .expect("tactical overlay shader handle must be registered")
        }
    }
}

pub fn runtime_shader_handle(shader_asset_id: &str) -> Option<Handle<bevy::shader::Shader>> {
    RUNTIME_SHADER_SPECS
        .iter()
        .find(|spec| spec.asset_id == shader_asset_id)
        .map(|spec| spec.handle.clone())
}

fn read_shader_source_for_asset(
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    shader_asset_id: &str,
) -> Option<String> {
    let entry = asset_manager.catalog_by_asset_id.get(shader_asset_id)?;

    let rooted_stream_path = std::path::PathBuf::from(asset_root)
        .join("data/cache_stream")
        .join(&entry.relative_cache_path);
    if let Ok(source) = std::fs::read_to_string(&rooted_stream_path) {
        return Some(source);
    }
    let rooted_direct_path = std::path::PathBuf::from(asset_root).join(&entry.relative_cache_path);
    std::fs::read_to_string(rooted_direct_path).ok()
}

fn shader_asset_ready(
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    shader_asset_id: &str,
) -> bool {
    let Some(relative_cache_path) = asset_manager.cached_relative_path(shader_asset_id) else {
        return false;
    };
    let rooted_stream_path = std::path::PathBuf::from(asset_root)
        .join("data/cache_stream")
        .join(relative_cache_path);
    let rooted_direct_path = std::path::PathBuf::from(asset_root).join(relative_cache_path);
    rooted_stream_path.exists() || rooted_direct_path.exists()
}

fn install_runtime_shader(
    shaders: &mut Assets<bevy::shader::Shader>,
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    spec: &RuntimeShaderSpec,
) {
    let source = read_shader_source_for_asset(asset_root, asset_manager, spec.asset_id);
    if let Some(source) = source {
        install_shader(shaders, spec.handle.clone(), spec.label, &source);
    } else {
        warn!(
            "runtime shader asset missing from cache/catalog asset_id={} label={}; installing shared fallback shader",
            spec.asset_id, spec.label
        );
        install_shader(
            shaders,
            spec.handle.clone(),
            spec.label,
            fallback_shader_source_for_family(spec.family),
        );
    }
}

pub fn install_runtime_shaders(
    shaders: &mut Assets<bevy::shader::Shader>,
    asset_root: &str,
    asset_manager: &LocalAssetManager,
) {
    if !streamed_shader_overrides_enabled() {
        for spec in RUNTIME_SHADER_SPECS {
            install_shader(
                shaders,
                spec.handle.clone(),
                spec.label,
                fallback_shader_source_for_family(spec.family),
            );
        }
        return;
    }

    for spec in RUNTIME_SHADER_SPECS {
        install_runtime_shader(shaders, asset_root, asset_manager, spec);
    }
}

pub fn reload_streamed_shaders(
    shaders: &mut Assets<bevy::shader::Shader>,
    asset_root: &str,
    asset_manager: &LocalAssetManager,
) {
    install_runtime_shaders(shaders, asset_root, asset_manager);
}

pub fn fullscreen_layer_shader_ready(
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    shader_asset_id: &str,
) -> bool {
    if shader_asset_ready(asset_root, asset_manager, shader_asset_id) {
        return true;
    }

    // Fullscreen materials have installed shader handles, so absence of a streamed
    // file should not block rendering.
    true
}

pub fn world_sprite_shader_ready(
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    shader_asset_id: &str,
) -> bool {
    shader_asset_ready(asset_root, asset_manager, shader_asset_id)
}

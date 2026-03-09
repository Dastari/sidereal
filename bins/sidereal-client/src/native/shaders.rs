//! Runtime shader install/reload helpers.

use bevy::log::warn;
use bevy::prelude::*;
use sidereal_game::{
    PlanetBodyShaderSettings, ProceduralSprite, RuntimeRenderLayerDefinition, SpriteShaderAssetId,
    TacticalMapUiSettings,
};

use super::assets::LocalAssetManager;
use super::components::StreamedSpriteShaderAssetId;
use super::resources::{AssetCacheAdapter, AssetRootPath};

// Material2d fragment shaders can rely on Bevy's default vertex path.
const FALLBACK_FRAGMENT_SHADER_SOURCE: &str = r#"
@fragment
fn fragment() -> @location(0) vec4<f32> {
  return vec4<f32>(1.0, 0.0, 1.0, 1.0);
}
"#;

// Browser WebGPU currently rejects the streamed fullscreen shaders even though the
// same sources render correctly on native clients. Keep browser world entry
// usable by installing simple fullscreen shaders for those slots on wasm.
#[cfg(target_arch = "wasm32")]
const WASM_STARFIELD_FALLBACK_SHADER_SOURCE: &str = r#"
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> drift_intensity: vec4<f32>;
@group(2) @binding(2) var<uniform> velocity_dir: vec4<f32>;
@group(2) @binding(3) var<uniform> starfield_params: vec4<f32>;
@group(2) @binding(4) var<uniform> starfield_tint: vec4<f32>;
@group(2) @binding(5) var<uniform> star_core_params: vec4<f32>;
@group(2) @binding(6) var<uniform> star_core_color: vec4<f32>;
@group(2) @binding(7) var<uniform> corona_params: vec4<f32>;
@group(2) @binding(8) var<uniform> corona_color: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let viewport = max(viewport_time.xy, vec2<f32>(1.0, 1.0));
    let aspect = viewport.x / max(viewport.y, 1.0);
    let centered = (in.uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);
    let dist = length(centered);
    let vignette = clamp(1.0 - dist * 0.8, 0.0, 1.0);
    let twinkle = 0.85 + 0.15 * sin(viewport_time.z * 0.5 + centered.x * 12.0 + centered.y * 8.0);
    let rgb = starfield_tint.rgb * twinkle * vignette;
    let alpha = clamp(starfield_params.w * vignette, 0.0, 1.0);
    return vec4<f32>(rgb, alpha);
}
"#;

#[cfg(target_arch = "wasm32")]
const WASM_SPACE_BACKGROUND_BASE_FALLBACK_SHADER_SOURCE: &str = r#"
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct SpaceBackgroundParams {
    viewport_time: vec4<f32>,
    drift_intensity: vec4<f32>,
    velocity_dir: vec4<f32>,
    space_bg_params: vec4<f32>,
    space_bg_tint: vec4<f32>,
    space_bg_background: vec4<f32>,
    space_bg_flare: vec4<f32>,
    space_bg_noise_a: vec4<f32>,
    space_bg_noise_b: vec4<f32>,
    space_bg_star_mask_a: vec4<f32>,
    space_bg_star_mask_b: vec4<f32>,
    space_bg_star_mask_c: vec4<f32>,
    space_bg_blend_a: vec4<f32>,
    space_bg_blend_b: vec4<f32>,
    space_bg_section_flags: vec4<f32>,
    space_bg_nebula_color_a: vec4<f32>,
    space_bg_nebula_color_b: vec4<f32>,
    space_bg_nebula_color_c: vec4<f32>,
    space_bg_star_color: vec4<f32>,
    space_bg_flare_tint: vec4<f32>,
    space_bg_depth_a: vec4<f32>,
    space_bg_light_a: vec4<f32>,
    space_bg_light_b: vec4<f32>,
    space_bg_light_flags: vec4<f32>,
    space_bg_shafts_a: vec4<f32>,
    space_bg_shafts_b: vec4<f32>,
    space_bg_backlight_color: vec4<f32>,
}

@group(2) @binding(0) var<uniform> params: SpaceBackgroundParams;
@group(2) @binding(1) var flare_texture: texture_2d<f32>;
@group(2) @binding(2) var flare_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let viewport = max(params.viewport_time.xy, vec2<f32>(1.0, 1.0));
    let aspect = viewport.x / max(viewport.y, 1.0);
    let centered = (in.uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);
    let vignette = clamp(1.0 - dot(centered, centered) * 0.8, 0.0, 1.0);
    let flare = textureSample(flare_texture, flare_sampler, in.uv).rgb;
    let gradient = mix(params.space_bg_background.rgb, params.space_bg_tint.rgb, in.uv.y);
    let rgb = clamp(
        (gradient + flare * params.space_bg_flare_tint.rgb * 0.15) * vignette,
        vec3<f32>(0.0),
        vec3<f32>(1.0)
    );
    return vec4<f32>(rgb, 1.0);
}
"#;

#[cfg(target_arch = "wasm32")]
const WASM_SPACE_BACKGROUND_NEBULA_FALLBACK_SHADER_SOURCE: &str = r#"
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct SpaceBackgroundParams {
    viewport_time: vec4<f32>,
    drift_intensity: vec4<f32>,
    velocity_dir: vec4<f32>,
    space_bg_params: vec4<f32>,
    space_bg_tint: vec4<f32>,
    space_bg_background: vec4<f32>,
    space_bg_flare: vec4<f32>,
    space_bg_noise_a: vec4<f32>,
    space_bg_noise_b: vec4<f32>,
    space_bg_star_mask_a: vec4<f32>,
    space_bg_star_mask_b: vec4<f32>,
    space_bg_star_mask_c: vec4<f32>,
    space_bg_blend_a: vec4<f32>,
    space_bg_blend_b: vec4<f32>,
    space_bg_section_flags: vec4<f32>,
    space_bg_nebula_color_a: vec4<f32>,
    space_bg_nebula_color_b: vec4<f32>,
    space_bg_nebula_color_c: vec4<f32>,
    space_bg_star_color: vec4<f32>,
    space_bg_flare_tint: vec4<f32>,
    space_bg_depth_a: vec4<f32>,
    space_bg_light_a: vec4<f32>,
    space_bg_light_b: vec4<f32>,
    space_bg_light_flags: vec4<f32>,
    space_bg_shafts_a: vec4<f32>,
    space_bg_shafts_b: vec4<f32>,
    space_bg_backlight_color: vec4<f32>,
}

@group(2) @binding(0) var<uniform> params: SpaceBackgroundParams;
@group(2) @binding(1) var flare_texture: texture_2d<f32>;
@group(2) @binding(2) var flare_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let viewport = max(params.viewport_time.xy, vec2<f32>(1.0, 1.0));
    let aspect = viewport.x / max(viewport.y, 1.0);
    let centered = (in.uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);
    let vignette = clamp(1.0 - dot(centered, centered) * 0.8, 0.0, 1.0);
    let flare = textureSample(flare_texture, flare_sampler, in.uv).rgb;
    let nebula = mix(params.space_bg_nebula_color_a.rgb, params.space_bg_nebula_color_c.rgb, in.uv.x);
    let rgb = clamp(
        (nebula + flare * params.space_bg_flare_tint.rgb * 0.1) * vignette,
        vec3<f32>(0.0),
        vec3<f32>(1.0)
    );
    let alpha = clamp(params.space_bg_params.w * (0.35 + 0.65 * in.uv.y) * vignette, 0.0, 1.0);
    return vec4<f32>(rgb, alpha);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeShaderSlot {
    Starfield,
    SpaceBackgroundBase,
    SpaceBackgroundNebula,
    GenericSprite,
    AsteroidSprite,
    PlanetVisual,
    RuntimeEffect,
    TacticalMapOverlay,
}

#[derive(Clone)]
struct RuntimeShaderSpec {
    slot: RuntimeShaderSlot,
    label: &'static str,
    handle: Handle<bevy::shader::Shader>,
    family: RuntimeShaderFamily,
}

const RUNTIME_SHADER_SPECS: &[RuntimeShaderSpec] = &[
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::Starfield,
        label: "sidereal://shader/starfield",
        handle: STARFIELD_SHADER_HANDLE,
        family: RuntimeShaderFamily::Fullscreen,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::SpaceBackgroundBase,
        label: "sidereal://shader/space_background_base",
        handle: SPACE_BACKGROUND_BASE_SHADER_HANDLE,
        family: RuntimeShaderFamily::Fullscreen,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::SpaceBackgroundNebula,
        label: "sidereal://shader/space_background_nebula",
        handle: SPACE_BACKGROUND_NEBULA_SHADER_HANDLE,
        family: RuntimeShaderFamily::Fullscreen,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::GenericSprite,
        label: "sidereal://shader/sprite_pixel_effect",
        handle: SPRITE_PIXEL_SHADER_HANDLE,
        family: RuntimeShaderFamily::WorldSprite,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::AsteroidSprite,
        label: "sidereal://shader/asteroid_sprite",
        handle: ASTEROID_SPRITE_SHADER_HANDLE,
        family: RuntimeShaderFamily::WorldSprite,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::PlanetVisual,
        label: "sidereal://shader/planet_visual",
        handle: PLANET_VISUAL_SHADER_HANDLE,
        family: RuntimeShaderFamily::WorldPolygon,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::RuntimeEffect,
        label: "sidereal://shader/runtime_effect",
        handle: RUNTIME_EFFECT_SHADER_HANDLE,
        family: RuntimeShaderFamily::Effect,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::TacticalMapOverlay,
        label: "sidereal://shader/tactical_map_overlay",
        handle: TACTICAL_MAP_OVERLAY_SHADER_HANDLE,
        family: RuntimeShaderFamily::Fullscreen,
    },
];

#[derive(Debug, Clone, Resource, PartialEq, Eq, Default)]
pub struct RuntimeShaderAssignments {
    starfield_asset_id: Option<String>,
    space_background_base_asset_id: Option<String>,
    space_background_nebula_asset_id: Option<String>,
    generic_sprite_asset_id: Option<String>,
    asteroid_sprite_asset_id: Option<String>,
    planet_visual_asset_id: Option<String>,
    runtime_effect_asset_id: Option<String>,
    tactical_map_overlay_asset_id: Option<String>,
}

impl RuntimeShaderAssignments {
    fn asset_id_for_slot(&self, slot: RuntimeShaderSlot) -> Option<&str> {
        match slot {
            RuntimeShaderSlot::Starfield => self.starfield_asset_id.as_deref(),
            RuntimeShaderSlot::SpaceBackgroundBase => {
                self.space_background_base_asset_id.as_deref()
            }
            RuntimeShaderSlot::SpaceBackgroundNebula => {
                self.space_background_nebula_asset_id.as_deref()
            }
            RuntimeShaderSlot::GenericSprite => self.generic_sprite_asset_id.as_deref(),
            RuntimeShaderSlot::AsteroidSprite => self.asteroid_sprite_asset_id.as_deref(),
            RuntimeShaderSlot::PlanetVisual => self.planet_visual_asset_id.as_deref(),
            RuntimeShaderSlot::RuntimeEffect => self.runtime_effect_asset_id.as_deref(),
            RuntimeShaderSlot::TacticalMapOverlay => self.tactical_map_overlay_asset_id.as_deref(),
        }
    }
}

fn env_flag_with_default(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .map(|value| matches!(value.as_str(), "1" | "true"))
        .unwrap_or(default)
}

pub fn shader_materials_enabled() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        env_flag_with_default("SIDEREAL_ENABLE_SHADER_MATERIALS", false)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        env_flag_with_default("SIDEREAL_ENABLE_SHADER_MATERIALS", true)
    }
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

#[cfg(target_arch = "wasm32")]
fn wasm_browser_safe_shader_source_for_slot(slot: RuntimeShaderSlot) -> Option<&'static str> {
    match slot {
        RuntimeShaderSlot::Starfield => Some(WASM_STARFIELD_FALLBACK_SHADER_SOURCE),
        RuntimeShaderSlot::SpaceBackgroundBase => {
            Some(WASM_SPACE_BACKGROUND_BASE_FALLBACK_SHADER_SOURCE)
        }
        RuntimeShaderSlot::SpaceBackgroundNebula => {
            Some(WASM_SPACE_BACKGROUND_NEBULA_FALLBACK_SHADER_SOURCE)
        }
        _ => None,
    }
}

pub fn fullscreen_shader_kind(
    assignments: &RuntimeShaderAssignments,
    shader_asset_id: &str,
) -> Option<RuntimeFullscreenShaderKind> {
    if assignments.starfield_asset_id.as_deref() == Some(shader_asset_id) {
        Some(RuntimeFullscreenShaderKind::Starfield)
    } else if assignments.space_background_base_asset_id.as_deref() == Some(shader_asset_id) {
        Some(RuntimeFullscreenShaderKind::SpaceBackgroundBase)
    } else if assignments.space_background_nebula_asset_id.as_deref() == Some(shader_asset_id) {
        Some(RuntimeFullscreenShaderKind::SpaceBackgroundNebula)
    } else {
        None
    }
}

pub fn world_sprite_shader_kind(
    assignments: &RuntimeShaderAssignments,
    shader_asset_id: &str,
) -> Option<RuntimeWorldSpriteShaderKind> {
    if assignments.generic_sprite_asset_id.as_deref() == Some(shader_asset_id) {
        Some(RuntimeWorldSpriteShaderKind::GenericSprite)
    } else if assignments.asteroid_sprite_asset_id.as_deref() == Some(shader_asset_id) {
        Some(RuntimeWorldSpriteShaderKind::Asteroid)
    } else {
        None
    }
}

pub fn world_polygon_shader_kind(
    assignments: &RuntimeShaderAssignments,
    shader_asset_id: &str,
) -> Option<RuntimeWorldPolygonShaderKind> {
    if assignments.planet_visual_asset_id.as_deref() == Some(shader_asset_id) {
        Some(RuntimeWorldPolygonShaderKind::PlanetVisual)
    } else {
        None
    }
}

pub fn world_polygon_shader_handle(
    kind: RuntimeWorldPolygonShaderKind,
) -> Handle<bevy::shader::Shader> {
    match kind {
        RuntimeWorldPolygonShaderKind::PlanetVisual => {
            runtime_shader_handle(RuntimeShaderSlot::PlanetVisual)
        }
    }
}

pub fn runtime_effect_shader_handle(kind: RuntimeEffectShaderKind) -> Handle<bevy::shader::Shader> {
    match kind {
        RuntimeEffectShaderKind::RuntimeEffect => {
            runtime_shader_handle(RuntimeShaderSlot::RuntimeEffect)
        }
        RuntimeEffectShaderKind::TacticalMapOverlay => {
            runtime_shader_handle(RuntimeShaderSlot::TacticalMapOverlay)
        }
    }
}

pub fn runtime_shader_handle(slot: RuntimeShaderSlot) -> Handle<bevy::shader::Shader> {
    RUNTIME_SHADER_SPECS
        .iter()
        .find(|spec| spec.slot == slot)
        .map(|spec| spec.handle.clone())
        .expect("runtime shader slot handle must be registered")
}

fn read_shader_source_for_asset(
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    cache_adapter: AssetCacheAdapter,
    shader_asset_id: &str,
) -> Option<String> {
    if let Some(source) = super::assets::cached_shader_source(
        shader_asset_id,
        asset_manager,
        asset_root,
        cache_adapter,
    ) {
        return Some(source);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let entry = asset_manager.catalog_by_asset_id.get(shader_asset_id)?;
        let rooted_direct_path =
            std::path::PathBuf::from(asset_root).join(&entry.relative_cache_path);
        std::fs::read_to_string(rooted_direct_path).ok()
    }

    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

fn shader_asset_ready(
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    cache_adapter: AssetCacheAdapter,
    shader_asset_id: &str,
) -> bool {
    if super::assets::cached_asset_bytes(shader_asset_id, asset_manager, asset_root, cache_adapter)
        .is_some()
    {
        return true;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(relative_cache_path) = asset_manager.cached_relative_path(shader_asset_id) else {
            return false;
        };
        let rooted_direct_path = std::path::PathBuf::from(asset_root).join(relative_cache_path);
        rooted_direct_path.exists()
    }

    #[cfg(target_arch = "wasm32")]
    {
        false
    }
}

fn install_runtime_shader(
    shaders: &mut Assets<bevy::shader::Shader>,
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    cache_adapter: AssetCacheAdapter,
    assignments: &RuntimeShaderAssignments,
    spec: &RuntimeShaderSpec,
) {
    #[cfg(target_arch = "wasm32")]
    if let Some(source) = wasm_browser_safe_shader_source_for_slot(spec.slot) {
        info!(
            "wasm runtime shader override using browser-safe fallback slot={:?} label={}",
            spec.slot, spec.label
        );
        install_shader(shaders, spec.handle.clone(), spec.label, source);
        return;
    }

    let Some(shader_asset_id) = assignments.asset_id_for_slot(spec.slot) else {
        install_shader(
            shaders,
            spec.handle.clone(),
            spec.label,
            fallback_shader_source_for_family(spec.family),
        );
        return;
    };
    let source =
        read_shader_source_for_asset(asset_root, asset_manager, cache_adapter, shader_asset_id);
    if let Some(source) = source {
        install_shader(shaders, spec.handle.clone(), spec.label, &source);
    } else {
        warn!(
            "runtime shader asset missing from cache/catalog asset_id={} label={}; installing shared fallback shader",
            shader_asset_id, spec.label
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
    cache_adapter: AssetCacheAdapter,
    assignments: &RuntimeShaderAssignments,
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
        install_runtime_shader(
            shaders,
            asset_root,
            asset_manager,
            cache_adapter,
            assignments,
            spec,
        );
    }
}

pub fn reload_streamed_shaders(
    shaders: &mut Assets<bevy::shader::Shader>,
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    cache_adapter: AssetCacheAdapter,
    assignments: &RuntimeShaderAssignments,
) {
    install_runtime_shaders(
        shaders,
        asset_root,
        asset_manager,
        cache_adapter,
        assignments,
    );
}

pub fn fullscreen_layer_shader_ready(
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    cache_adapter: AssetCacheAdapter,
    shader_asset_id: &str,
) -> bool {
    if shader_asset_ready(asset_root, asset_manager, cache_adapter, shader_asset_id) {
        return true;
    }

    // Fullscreen materials have installed shader handles, so absence of a streamed
    // file should not block rendering.
    true
}

pub fn world_sprite_shader_ready(
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    cache_adapter: AssetCacheAdapter,
    shader_asset_id: &str,
) -> bool {
    shader_asset_ready(asset_root, asset_manager, cache_adapter, shader_asset_id)
}

fn non_empty_asset_id(asset_id: &str) -> Option<String> {
    (!asset_id.trim().is_empty()).then(|| asset_id.to_string())
}

fn first_sprite_shader_asset_id<'a, I>(
    entries: I,
    exclude_asset_ids: &[Option<&str>],
) -> Option<String>
where
    I: IntoIterator<Item = (&'a str, bool, bool)>,
{
    entries
        .into_iter()
        .find_map(|(asset_id, is_planet, is_asteroid)| {
            if is_planet || is_asteroid {
                return None;
            }
            let asset_id = asset_id.trim();
            if asset_id.is_empty()
                || exclude_asset_ids
                    .iter()
                    .flatten()
                    .any(|value| *value == asset_id)
            {
                return None;
            }
            Some(asset_id.to_string())
        })
}

#[allow(clippy::too_many_arguments)]
pub fn sync_runtime_shader_assignments_system(
    mut assignments: ResMut<'_, RuntimeShaderAssignments>,
    runtime_render_layers: Query<'_, '_, &'_ RuntimeRenderLayerDefinition>,
    sprite_shader_asset_ids: Query<
        '_,
        '_,
        (
            &'_ SpriteShaderAssetId,
            Option<&'_ PlanetBodyShaderSettings>,
            Option<&'_ ProceduralSprite>,
        ),
    >,
    streamed_sprite_shader_asset_ids: Query<
        '_,
        '_,
        (
            &'_ StreamedSpriteShaderAssetId,
            Option<&'_ PlanetBodyShaderSettings>,
            Option<&'_ ProceduralSprite>,
        ),
    >,
    tactical_map_settings: Query<'_, '_, &'_ TacticalMapUiSettings>,
    mut shaders_assets: ResMut<'_, Assets<bevy::shader::Shader>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut last_reload_generation: Local<'_, u64>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
) {
    let mut next = RuntimeShaderAssignments::default();

    for layer in &runtime_render_layers {
        let Some(shader_asset_id) = non_empty_asset_id(&layer.shader_asset_id) else {
            continue;
        };
        match layer.layer_id.as_str() {
            "bg_starfield" => next.starfield_asset_id = Some(shader_asset_id),
            "bg_space_background_base" => {
                next.space_background_base_asset_id = Some(shader_asset_id)
            }
            "bg_space_background_nebula" => {
                next.space_background_nebula_asset_id = Some(shader_asset_id)
            }
            _ if layer.material_domain == "world_polygon"
                && next.planet_visual_asset_id.is_none() =>
            {
                next.planet_visual_asset_id = Some(shader_asset_id)
            }
            _ => {}
        }
    }

    for settings in &tactical_map_settings {
        if let Some(shader_asset_id) = non_empty_asset_id(&settings.shader_asset_id) {
            next.tactical_map_overlay_asset_id = Some(shader_asset_id);
            break;
        }
    }

    if next.runtime_effect_asset_id.is_none() {
        next.runtime_effect_asset_id = asset_manager
            .catalog_by_asset_id
            .iter()
            .find(|(_, entry)| entry.shader_family.as_deref() == Some("effect"))
            .map(|(asset_id, _)| asset_id.clone());
    }

    for (shader_asset_id, _planet, procedural_sprite) in &sprite_shader_asset_ids {
        let Some(shader_asset_id) = shader_asset_id.0.as_deref() else {
            continue;
        };
        if next.asteroid_sprite_asset_id.is_none()
            && procedural_sprite.is_some_and(|sprite| sprite.generator_id == "asteroid_rocky_v1")
        {
            next.asteroid_sprite_asset_id = non_empty_asset_id(shader_asset_id);
        }
    }
    for (shader_asset_id, _planet, procedural_sprite) in &streamed_sprite_shader_asset_ids {
        if next.asteroid_sprite_asset_id.is_some() {
            break;
        }
        if procedural_sprite.is_some_and(|sprite| sprite.generator_id == "asteroid_rocky_v1") {
            next.asteroid_sprite_asset_id = non_empty_asset_id(&shader_asset_id.0);
        }
    }

    if next.planet_visual_asset_id.is_none() {
        for (shader_asset_id, planet_settings, _) in &sprite_shader_asset_ids {
            if planet_settings.is_some() {
                next.planet_visual_asset_id = shader_asset_id.0.clone();
                if next.planet_visual_asset_id.is_some() {
                    break;
                }
            }
        }
    }

    next.generic_sprite_asset_id = first_sprite_shader_asset_id(
        sprite_shader_asset_ids.iter().filter_map(
            |(shader_asset_id, planet_settings, procedural_sprite)| {
                shader_asset_id.0.as_deref().map(|asset_id| {
                    (
                        asset_id,
                        planet_settings.is_some(),
                        procedural_sprite
                            .is_some_and(|sprite| sprite.generator_id == "asteroid_rocky_v1"),
                    )
                })
            },
        ),
        &[
            next.planet_visual_asset_id.as_deref(),
            next.asteroid_sprite_asset_id.as_deref(),
        ],
    )
    .or_else(|| {
        first_sprite_shader_asset_id(
            streamed_sprite_shader_asset_ids.iter().map(
                |(shader_asset_id, planet_settings, procedural_sprite)| {
                    (
                        shader_asset_id.0.as_str(),
                        planet_settings.is_some(),
                        procedural_sprite
                            .is_some_and(|sprite| sprite.generator_id == "asteroid_rocky_v1"),
                    )
                },
            ),
            &[
                next.planet_visual_asset_id.as_deref(),
                next.asteroid_sprite_asset_id.as_deref(),
            ],
        )
    });

    let catalog_reloaded = *last_reload_generation != asset_manager.reload_generation;
    *last_reload_generation = asset_manager.reload_generation;
    if *assignments != next || catalog_reloaded {
        *assignments = next;
        reload_streamed_shaders(
            &mut shaders_assets,
            &asset_root.0,
            &asset_manager,
            *cache_adapter,
            &assignments,
        );
    }
}

#[cfg(test)]
mod tests {
    use serde_json::from_str;
    use sidereal_asset_runtime::{
        AssetCacheIndex, asset_version_from_sha256_hex, generated_asset_guid,
        generated_relative_cache_path, sha256_hex,
    };
    use std::fs;
    use std::path::PathBuf;

    fn asset_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    #[test]
    fn fullscreen_shader_cache_entries_match_sources() {
        let asset_root = asset_root();
        let index_path = asset_root.join("cache_stream/index.json");
        let index: AssetCacheIndex =
            from_str(&fs::read_to_string(index_path).expect("cache index should be readable"))
                .expect("cache index should decode");

        for (asset_id, source_path) in [
            ("starfield_wgsl", "shaders/starfield.wgsl"),
            (
                "space_background_base_wgsl",
                "shaders/space_background_base.wgsl",
            ),
            (
                "space_background_nebula_wgsl",
                "shaders/space_background_nebula.wgsl",
            ),
            (
                "tactical_map_overlay_wgsl",
                "shaders/tactical_map_overlay.wgsl",
            ),
        ] {
            let source_bytes =
                fs::read(asset_root.join(source_path)).expect("shader source should exist");
            let sha256 = sha256_hex(&source_bytes);
            let guid = generated_asset_guid(asset_id, &sha256);
            let relative_cache_path =
                generated_relative_cache_path(&guid, source_path, "text/wgsl");
            let cached_bytes = fs::read(asset_root.join("cache_stream").join(&relative_cache_path))
                .expect("cached shader should exist");
            let index_record = index
                .by_asset_id
                .get(asset_id)
                .expect("cache index record should exist");

            assert_eq!(
                cached_bytes, source_bytes,
                "cached shader payload diverged for {asset_id}"
            );
            assert_eq!(
                index_record.sha256_hex, sha256,
                "sha mismatch for {asset_id}"
            );
            assert_eq!(
                index_record.asset_version,
                asset_version_from_sha256_hex(&sha256),
                "asset version mismatch for {asset_id}"
            );
        }
    }
}

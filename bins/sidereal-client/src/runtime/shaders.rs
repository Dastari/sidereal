//! Runtime shader install/reload helpers.

use bevy::log::{error, warn};
use bevy::prelude::*;
use naga::front::wgsl;
use naga::proc::{GlobalCtx, Layouter};
use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga::{AddressSpace, ShaderStage, TypeInner};
use sidereal_game::{
    PlanetBodyShaderSettings, ProceduralSprite, RuntimeRenderLayerDefinition, SpriteShaderAssetId,
    TacticalMapUiSettings,
};

use super::assets::LocalAssetManager;
use super::components::StreamedSpriteShaderAssetId;
use super::resources::{AssetCacheAdapter, AssetRootPath};

// Material2d fragment shaders can rely on Bevy's default vertex path. Keep this
// as a last-resort diagnostic only; normal runtime fallback should use bundled
// shader sources so streamed asset/catalog churn does not produce magenta quads.
const DEBUG_FALLBACK_FRAGMENT_SHADER_SOURCE: &str = r#"
@fragment
fn fragment() -> @location(0) vec4<f32> {
  return vec4<f32>(1.0, 0.0, 1.0, 1.0);
}
"#;

const BUNDLED_STARFIELD_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/starfield.wgsl");
const BUNDLED_SPACE_BACKGROUND_BASE_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/space_background_base.wgsl");
const BUNDLED_SPACE_BACKGROUND_NEBULA_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/space_background_nebula.wgsl");
const BUNDLED_GENERIC_SPRITE_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/sprite_pixel_effect.wgsl");
const BUNDLED_ASTEROID_SPRITE_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/asteroid.wgsl");
const BUNDLED_PLANET_VISUAL_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/planet_visual.wgsl");
const BUNDLED_STAR_VISUAL_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/star_visual.wgsl");
const BUNDLED_RUNTIME_EFFECT_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/runtime_effect.wgsl");
const BUNDLED_TACTICAL_MAP_OVERLAY_SHADER_SOURCE: &str =
    include_str!("../../../../data/shaders/tactical_map_overlay.wgsl");

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
pub const STAR_VISUAL_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("597f3d01-83d2-46f2-a57f-13a15e4202a7");
pub const RUNTIME_EFFECT_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("0cae863f-b918-4470-b7ee-f30749186a34");
pub const TACTICAL_MAP_OVERLAY_SHADER_HANDLE: Handle<bevy::shader::Shader> =
    bevy::asset::uuid_handle!("f7de7110-00a1-41f2-b498-ec705dbd2d22");

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFullscreenShaderKind {
    Starfield,
    SpaceBackgroundBase,
    SpaceBackgroundNebula,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWorldSpriteShaderKind {
    GenericSprite,
    Asteroid,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWorldPolygonShaderKind {
    PlanetVisual,
    StarVisual,
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
    StarVisual,
    RuntimeEffect,
    TacticalMapOverlay,
}

#[derive(Clone)]
struct RuntimeShaderSpec {
    slot: RuntimeShaderSlot,
    label: &'static str,
    handle: Handle<bevy::shader::Shader>,
}

const RUNTIME_SHADER_SPECS: &[RuntimeShaderSpec] = &[
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::Starfield,
        label: "sidereal://shader/starfield",
        handle: STARFIELD_SHADER_HANDLE,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::SpaceBackgroundBase,
        label: "sidereal://shader/space_background_base",
        handle: SPACE_BACKGROUND_BASE_SHADER_HANDLE,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::SpaceBackgroundNebula,
        label: "sidereal://shader/space_background_nebula",
        handle: SPACE_BACKGROUND_NEBULA_SHADER_HANDLE,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::GenericSprite,
        label: "sidereal://shader/sprite_pixel_effect",
        handle: SPRITE_PIXEL_SHADER_HANDLE,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::AsteroidSprite,
        label: "sidereal://shader/asteroid_sprite",
        handle: ASTEROID_SPRITE_SHADER_HANDLE,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::PlanetVisual,
        label: "sidereal://shader/planet_visual",
        handle: PLANET_VISUAL_SHADER_HANDLE,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::StarVisual,
        label: "sidereal://shader/star_visual",
        handle: STAR_VISUAL_SHADER_HANDLE,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::RuntimeEffect,
        label: "sidereal://shader/runtime_effect",
        handle: RUNTIME_EFFECT_SHADER_HANDLE,
    },
    RuntimeShaderSpec {
        slot: RuntimeShaderSlot::TacticalMapOverlay,
        label: "sidereal://shader/tactical_map_overlay",
        handle: TACTICAL_MAP_OVERLAY_SHADER_HANDLE,
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
    star_visual_asset_id: Option<String>,
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
            RuntimeShaderSlot::StarVisual => self.star_visual_asset_id.as_deref(),
            RuntimeShaderSlot::RuntimeEffect => self.runtime_effect_asset_id.as_deref(),
            RuntimeShaderSlot::TacticalMapOverlay => self.tactical_map_overlay_asset_id.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Resource)]
pub(crate) struct RuntimeShaderAssignmentSyncState {
    pub dirty: bool,
    pub last_catalog_reload_generation: u64,
}

impl Default for RuntimeShaderAssignmentSyncState {
    fn default() -> Self {
        Self {
            dirty: true,
            last_catalog_reload_generation: 0,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeShaderBindingKind {
    Uniform,
    Texture,
    Sampler,
}

#[derive(Clone, Copy)]
struct RuntimeShaderBinding {
    binding: u32,
    kind: RuntimeShaderBindingKind,
}

const STARFIELD_BINDINGS: &[RuntimeShaderBinding] = &[
    RuntimeShaderBinding {
        binding: 0,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 1,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 2,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 3,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 4,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 5,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 6,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 7,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 8,
        kind: RuntimeShaderBindingKind::Uniform,
    },
];
const SPACE_BACKGROUND_BINDINGS: &[RuntimeShaderBinding] = &[
    RuntimeShaderBinding {
        binding: 0,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 1,
        kind: RuntimeShaderBindingKind::Texture,
    },
    RuntimeShaderBinding {
        binding: 2,
        kind: RuntimeShaderBindingKind::Sampler,
    },
];
const GENERIC_SPRITE_BINDINGS: &[RuntimeShaderBinding] = &[
    RuntimeShaderBinding {
        binding: 0,
        kind: RuntimeShaderBindingKind::Texture,
    },
    RuntimeShaderBinding {
        binding: 1,
        kind: RuntimeShaderBindingKind::Sampler,
    },
    RuntimeShaderBinding {
        binding: 2,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 3,
        kind: RuntimeShaderBindingKind::Uniform,
    },
];
const ASTEROID_SPRITE_BINDINGS: &[RuntimeShaderBinding] = &[
    RuntimeShaderBinding {
        binding: 0,
        kind: RuntimeShaderBindingKind::Texture,
    },
    RuntimeShaderBinding {
        binding: 1,
        kind: RuntimeShaderBindingKind::Sampler,
    },
    RuntimeShaderBinding {
        binding: 2,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 3,
        kind: RuntimeShaderBindingKind::Texture,
    },
    RuntimeShaderBinding {
        binding: 4,
        kind: RuntimeShaderBindingKind::Sampler,
    },
    RuntimeShaderBinding {
        binding: 5,
        kind: RuntimeShaderBindingKind::Uniform,
    },
];
const PLANET_VISUAL_BINDINGS: &[RuntimeShaderBinding] = &[RuntimeShaderBinding {
    binding: 0,
    kind: RuntimeShaderBindingKind::Uniform,
}];
const RUNTIME_EFFECT_BINDINGS: &[RuntimeShaderBinding] = &[
    RuntimeShaderBinding {
        binding: 0,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 1,
        kind: RuntimeShaderBindingKind::Uniform,
    },
];
const TACTICAL_MAP_OVERLAY_BINDINGS: &[RuntimeShaderBinding] = &[
    RuntimeShaderBinding {
        binding: 0,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 1,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 2,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 3,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 4,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 5,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 6,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 7,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 8,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 9,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 10,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 11,
        kind: RuntimeShaderBindingKind::Texture,
    },
    RuntimeShaderBinding {
        binding: 12,
        kind: RuntimeShaderBindingKind::Sampler,
    },
    RuntimeShaderBinding {
        binding: 13,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 14,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 15,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 16,
        kind: RuntimeShaderBindingKind::Uniform,
    },
    RuntimeShaderBinding {
        binding: 17,
        kind: RuntimeShaderBindingKind::Uniform,
    },
];

fn bundled_shader_source_for_slot(slot: RuntimeShaderSlot) -> &'static str {
    match slot {
        RuntimeShaderSlot::Starfield => BUNDLED_STARFIELD_SHADER_SOURCE,
        RuntimeShaderSlot::SpaceBackgroundBase => BUNDLED_SPACE_BACKGROUND_BASE_SHADER_SOURCE,
        RuntimeShaderSlot::SpaceBackgroundNebula => BUNDLED_SPACE_BACKGROUND_NEBULA_SHADER_SOURCE,
        RuntimeShaderSlot::GenericSprite => BUNDLED_GENERIC_SPRITE_SHADER_SOURCE,
        RuntimeShaderSlot::AsteroidSprite => BUNDLED_ASTEROID_SPRITE_SHADER_SOURCE,
        RuntimeShaderSlot::PlanetVisual => BUNDLED_PLANET_VISUAL_SHADER_SOURCE,
        RuntimeShaderSlot::StarVisual => BUNDLED_STAR_VISUAL_SHADER_SOURCE,
        RuntimeShaderSlot::RuntimeEffect => BUNDLED_RUNTIME_EFFECT_SHADER_SOURCE,
        RuntimeShaderSlot::TacticalMapOverlay => BUNDLED_TACTICAL_MAP_OVERLAY_SHADER_SOURCE,
    }
}

fn install_bundled_runtime_shader(
    shaders: &mut Assets<bevy::shader::Shader>,
    spec: &RuntimeShaderSpec,
) {
    let source = bundled_shader_source_for_slot(spec.slot);
    match validate_runtime_shader_source(spec.slot, source) {
        Ok(()) => install_shader(shaders, spec.handle.clone(), spec.label, source),
        Err(message) => {
            error!(
                "bundled runtime shader rejected slot={:?} label={}: {}; installing diagnostic fallback",
                spec.slot, spec.label, message
            );
            install_shader(
                shaders,
                spec.handle.clone(),
                spec.label,
                DEBUG_FALLBACK_FRAGMENT_SHADER_SOURCE,
            );
        }
    }
}

fn normalize_runtime_wgsl(source: &str) -> String {
    const VERTEX_OUTPUT: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
"#;

    let adapted = source.replace(
        "#import bevy_sprite::mesh2d_vertex_output::VertexOutput",
        "",
    );

    if adapted.contains("struct VertexOutput") {
        adapted
    } else {
        format!("{VERTEX_OUTPUT}\n{adapted}")
    }
}

fn expected_bindings_for_slot(slot: RuntimeShaderSlot) -> &'static [RuntimeShaderBinding] {
    match slot {
        RuntimeShaderSlot::Starfield => STARFIELD_BINDINGS,
        RuntimeShaderSlot::SpaceBackgroundBase | RuntimeShaderSlot::SpaceBackgroundNebula => {
            SPACE_BACKGROUND_BINDINGS
        }
        RuntimeShaderSlot::GenericSprite => GENERIC_SPRITE_BINDINGS,
        RuntimeShaderSlot::AsteroidSprite => ASTEROID_SPRITE_BINDINGS,
        RuntimeShaderSlot::PlanetVisual | RuntimeShaderSlot::StarVisual => PLANET_VISUAL_BINDINGS,
        RuntimeShaderSlot::RuntimeEffect => RUNTIME_EFFECT_BINDINGS,
        RuntimeShaderSlot::TacticalMapOverlay => TACTICAL_MAP_OVERLAY_BINDINGS,
    }
}

fn resource_binding_kind(
    module: &naga::Module,
    global: &naga::GlobalVariable,
) -> Option<RuntimeShaderBindingKind> {
    match global.space {
        AddressSpace::Uniform => Some(RuntimeShaderBindingKind::Uniform),
        AddressSpace::Handle => match module.types[global.ty].inner {
            TypeInner::Image { .. } => Some(RuntimeShaderBindingKind::Texture),
            TypeInner::Sampler { .. } => Some(RuntimeShaderBindingKind::Sampler),
            _ => None,
        },
        _ => None,
    }
}

fn expected_uniform_size_bytes(slot: RuntimeShaderSlot, binding: u32) -> Option<u32> {
    match slot {
        RuntimeShaderSlot::Starfield => (binding <= 8).then_some(16),
        RuntimeShaderSlot::SpaceBackgroundBase | RuntimeShaderSlot::SpaceBackgroundNebula => {
            (binding == 0).then_some(432)
        }
        RuntimeShaderSlot::AsteroidSprite => match binding {
            2 => Some(384),
            5 => Some(16),
            _ => None,
        },
        RuntimeShaderSlot::GenericSprite => match binding {
            2 => Some(384),
            3 => Some(16),
            _ => None,
        },
        RuntimeShaderSlot::PlanetVisual | RuntimeShaderSlot::StarVisual => {
            (binding == 0).then_some(736)
        }
        RuntimeShaderSlot::RuntimeEffect => match binding {
            0 => Some(96),
            1 => Some(384),
            _ => None,
        },
        RuntimeShaderSlot::TacticalMapOverlay => {
            ((binding <= 10) || (13..=17).contains(&binding)).then_some(16)
        }
    }
}

fn uniform_type_size_bytes(
    module: &naga::Module,
    global: &naga::GlobalVariable,
) -> Result<u32, String> {
    let mut layouter = Layouter::default();
    let gctx = GlobalCtx {
        types: &module.types,
        constants: &module.constants,
        overrides: &module.overrides,
        global_expressions: &module.global_expressions,
    };
    layouter
        .update(gctx)
        .map_err(|error| format!("could not compute uniform layout: {error}"))?;
    Ok(layouter[global.ty].size)
}

fn validate_runtime_shader_source(slot: RuntimeShaderSlot, source: &str) -> Result<(), String> {
    let adapted_source = normalize_runtime_wgsl(source);
    let module = wgsl::parse_str(&adapted_source).map_err(|error| {
        format!(
            "WGSL parse failed: {}",
            error.emit_to_string(&adapted_source)
        )
    })?;

    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    validator.validate(&module).map_err(|error| {
        format!(
            "WGSL validation failed: {}",
            error.emit_to_string(&adapted_source)
        )
    })?;

    if !module.entry_points.iter().any(|entry_point| {
        entry_point.stage == ShaderStage::Fragment && entry_point.name == "fragment"
    }) {
        return Err("runtime shader must define @fragment fn fragment(...)".to_string());
    }

    let expected = expected_bindings_for_slot(slot);
    for (_, global) in module.global_variables.iter() {
        let Some(binding) = global.binding else {
            continue;
        };
        if binding.group != 2 {
            return Err(format!(
                "shader resource '{}' uses @group({}) @binding({}); runtime material shaders may only bind group 2 resources",
                global.name.as_deref().unwrap_or("<unnamed>"),
                binding.group,
                binding.binding
            ));
        }
        let Some(actual_kind) = resource_binding_kind(&module, global) else {
            return Err(format!(
                "shader resource '{}' at @group(2) @binding({}) uses an unsupported resource kind",
                global.name.as_deref().unwrap_or("<unnamed>"),
                binding.binding
            ));
        };
        let Some(expected_binding) = expected
            .iter()
            .find(|expected| expected.binding == binding.binding)
        else {
            return Err(format!(
                "shader resource '{}' uses @group(2) @binding({}), but slot {:?} does not expose that binding",
                global.name.as_deref().unwrap_or("<unnamed>"),
                binding.binding,
                slot
            ));
        };
        if expected_binding.kind != actual_kind {
            return Err(format!(
                "shader resource '{}' uses @group(2) @binding({}) as {:?}, but slot {:?} expects {:?}",
                global.name.as_deref().unwrap_or("<unnamed>"),
                binding.binding,
                actual_kind,
                slot,
                expected_binding.kind
            ));
        }
        if actual_kind == RuntimeShaderBindingKind::Uniform {
            let expected_size =
                expected_uniform_size_bytes(slot, binding.binding).ok_or_else(|| {
                    format!(
                        "slot {slot:?} does not expose a uniform buffer at @group(2) @binding({})",
                        binding.binding
                    )
                })?;
            let actual_size = uniform_type_size_bytes(&module, global)?;
            if actual_size > expected_size {
                return Err(format!(
                    "shader uniform '{}' at @group(2) @binding({}) requires {} bytes, but slot {:?} provides {} bytes",
                    global.name.as_deref().unwrap_or("<unnamed>"),
                    binding.binding,
                    actual_size,
                    slot,
                    expected_size
                ));
            }
        }
    }

    Ok(())
}

fn install_validated_runtime_shader(
    shaders: &mut Assets<bevy::shader::Shader>,
    spec: &RuntimeShaderSpec,
    shader_asset_id: &str,
    source: &str,
) {
    match validate_runtime_shader_source(spec.slot, source) {
        Ok(()) => install_shader(shaders, spec.handle.clone(), spec.label, source),
        Err(message) => {
            error!(
                "runtime shader asset rejected asset_id={} slot={:?} label={}: {}",
                shader_asset_id, spec.slot, spec.label, message
            );
            if !shaders.contains(spec.handle.id()) {
                install_bundled_runtime_shader(shaders, spec);
            }
        }
    }
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
    if assignments.asteroid_sprite_asset_id.as_deref() == Some(shader_asset_id) {
        Some(RuntimeWorldSpriteShaderKind::Asteroid)
    } else if assignments.generic_sprite_asset_id.as_deref() == Some(shader_asset_id) {
        Some(RuntimeWorldSpriteShaderKind::GenericSprite)
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
    } else if assignments.star_visual_asset_id.as_deref() == Some(shader_asset_id) {
        Some(RuntimeWorldPolygonShaderKind::StarVisual)
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
        RuntimeWorldPolygonShaderKind::StarVisual => {
            runtime_shader_handle(RuntimeShaderSlot::StarVisual)
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
        install_bundled_runtime_shader(shaders, spec);
        return;
    };
    let source =
        read_shader_source_for_asset(asset_root, asset_manager, cache_adapter, shader_asset_id);
    if let Some(source) = source {
        install_validated_runtime_shader(shaders, spec, shader_asset_id, &source);
    } else {
        warn!(
            "runtime shader asset missing from cache/catalog asset_id={} label={}; installing bundled shader fallback",
            shader_asset_id, spec.label
        );
        install_bundled_runtime_shader(shaders, spec);
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
            install_bundled_runtime_shader(shaders, spec);
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

fn catalog_shader_asset_id_by_family(
    asset_manager: &LocalAssetManager,
    shader_family: &str,
) -> Option<String> {
    asset_manager
        .catalog_by_asset_id
        .iter()
        .find(|(_, entry)| entry.shader_family.as_deref() == Some(shader_family))
        .map(|(asset_id, _)| asset_id.clone())
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn mark_runtime_shader_assignments_dirty_system(
    mut state: ResMut<'_, RuntimeShaderAssignmentSyncState>,
    runtime_render_layer_changed: Query<
        '_,
        '_,
        (),
        Or<(
            Added<RuntimeRenderLayerDefinition>,
            Changed<RuntimeRenderLayerDefinition>,
        )>,
    >,
    sprite_shader_changed: Query<
        '_,
        '_,
        (),
        Or<(
            Added<SpriteShaderAssetId>,
            Changed<SpriteShaderAssetId>,
            Added<PlanetBodyShaderSettings>,
            Changed<PlanetBodyShaderSettings>,
            Added<ProceduralSprite>,
            Changed<ProceduralSprite>,
        )>,
    >,
    streamed_sprite_shader_changed: Query<
        '_,
        '_,
        (),
        Or<(
            Added<StreamedSpriteShaderAssetId>,
            Changed<StreamedSpriteShaderAssetId>,
            Added<PlanetBodyShaderSettings>,
            Changed<PlanetBodyShaderSettings>,
            Added<ProceduralSprite>,
            Changed<ProceduralSprite>,
        )>,
    >,
    tactical_map_settings_changed: Query<
        '_,
        '_,
        (),
        Or<(Added<TacticalMapUiSettings>, Changed<TacticalMapUiSettings>)>,
    >,
    mut removed_runtime_render_layer: RemovedComponents<'_, '_, RuntimeRenderLayerDefinition>,
    mut removed_sprite_shader: RemovedComponents<'_, '_, SpriteShaderAssetId>,
    mut removed_streamed_sprite_shader: RemovedComponents<'_, '_, StreamedSpriteShaderAssetId>,
    mut removed_tactical_map_settings: RemovedComponents<'_, '_, TacticalMapUiSettings>,
    mut removed_planet_shader_settings: RemovedComponents<'_, '_, PlanetBodyShaderSettings>,
    mut removed_procedural_sprite: RemovedComponents<'_, '_, ProceduralSprite>,
) {
    let changed = runtime_render_layer_changed.iter().next().is_some()
        || sprite_shader_changed.iter().next().is_some()
        || streamed_sprite_shader_changed.iter().next().is_some()
        || tactical_map_settings_changed.iter().next().is_some()
        || removed_runtime_render_layer.read().next().is_some()
        || removed_sprite_shader.read().next().is_some()
        || removed_streamed_sprite_shader.read().next().is_some()
        || removed_tactical_map_settings.read().next().is_some()
        || removed_planet_shader_settings.read().next().is_some()
        || removed_procedural_sprite.read().next().is_some();
    if changed {
        state.dirty = true;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sync_runtime_shader_assignments_system(
    mut assignments: ResMut<'_, RuntimeShaderAssignments>,
    mut sync_state: ResMut<'_, RuntimeShaderAssignmentSyncState>,
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
    cache_adapter: Res<'_, AssetCacheAdapter>,
) {
    let catalog_reloaded =
        sync_state.last_catalog_reload_generation != asset_manager.reload_generation;
    if !sync_state.dirty && !catalog_reloaded {
        return;
    }
    sync_state.dirty = false;
    sync_state.last_catalog_reload_generation = asset_manager.reload_generation;

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
        next.runtime_effect_asset_id = catalog_shader_asset_id_by_family(&asset_manager, "effect");
    }

    if next.star_visual_asset_id.is_none() {
        next.star_visual_asset_id =
            catalog_shader_asset_id_by_family(&asset_manager, "world_polygon_star");
    }

    if next.asteroid_sprite_asset_id.is_none() {
        next.asteroid_sprite_asset_id =
            catalog_shader_asset_id_by_family(&asset_manager, "world_sprite_asteroid");
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
            if planet_settings.is_some_and(|settings| settings.body_kind != 1) {
                next.planet_visual_asset_id = shader_asset_id.0.clone();
                if next.planet_visual_asset_id.is_some() {
                    break;
                }
            }
        }
    }

    if next.star_visual_asset_id.is_none() {
        for (shader_asset_id, planet_settings, _) in &sprite_shader_asset_ids {
            if planet_settings.is_some_and(|settings| settings.body_kind == 1) {
                next.star_visual_asset_id = shader_asset_id.0.clone();
                if next.star_visual_asset_id.is_some() {
                    break;
                }
            }
        }
    }

    next.generic_sprite_asset_id =
        catalog_shader_asset_id_by_family(&asset_manager, "world_sprite_generic");

    if next.generic_sprite_asset_id.is_none() {
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
                next.star_visual_asset_id.as_deref(),
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
                    next.star_visual_asset_id.as_deref(),
                    next.asteroid_sprite_asset_id.as_deref(),
                ],
            )
        });
    }

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
    use super::{
        RuntimeShaderAssignments, RuntimeShaderSlot, RuntimeWorldSpriteShaderKind,
        bundled_shader_source_for_slot, validate_runtime_shader_source, world_sprite_shader_kind,
    };
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
        if !index_path.exists() {
            return;
        }
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
            ("runtime_effect_wgsl", "shaders/runtime_effect.wgsl"),
            (
                "sprite_pixel_effect_wgsl",
                "shaders/sprite_pixel_effect.wgsl",
            ),
            ("asteroid_wgsl", "shaders/asteroid.wgsl"),
            ("planet_visual_wgsl", "shaders/planet_visual.wgsl"),
            ("star_visual_wgsl", "shaders/star_visual.wgsl"),
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

    #[test]
    fn world_sprite_shader_kind_prefers_asteroid_when_assignments_overlap() {
        let assignments = RuntimeShaderAssignments {
            generic_sprite_asset_id: Some("asteroid_wgsl".to_string()),
            asteroid_sprite_asset_id: Some("asteroid_wgsl".to_string()),
            ..Default::default()
        };

        assert_eq!(
            world_sprite_shader_kind(&assignments, "asteroid_wgsl"),
            Some(RuntimeWorldSpriteShaderKind::Asteroid)
        );
    }

    #[test]
    fn generic_sprite_shader_rejects_asteroid_only_binding() {
        let source = r#"
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var image: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;
@group(2) @binding(5) var<uniform> local_rotation: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(image, image_sampler, mesh.uv) + local_rotation;
}
"#;

        let error = validate_runtime_shader_source(RuntimeShaderSlot::GenericSprite, source)
            .expect_err("generic sprite ABI should reject asteroid-only binding");

        assert!(
            error.contains("binding(5)") && error.contains("GenericSprite"),
            "unexpected validation error: {error}"
        );
    }

    #[test]
    fn bundled_runtime_shaders_match_material_binding_abis() {
        let asset_root = asset_root();
        for (slot, source_path) in [
            (RuntimeShaderSlot::Starfield, "shaders/starfield.wgsl"),
            (
                RuntimeShaderSlot::SpaceBackgroundBase,
                "shaders/space_background_base.wgsl",
            ),
            (
                RuntimeShaderSlot::SpaceBackgroundNebula,
                "shaders/space_background_nebula.wgsl",
            ),
            (
                RuntimeShaderSlot::GenericSprite,
                "shaders/sprite_pixel_effect.wgsl",
            ),
            (RuntimeShaderSlot::AsteroidSprite, "shaders/asteroid.wgsl"),
            (
                RuntimeShaderSlot::PlanetVisual,
                "shaders/planet_visual.wgsl",
            ),
            (
                RuntimeShaderSlot::RuntimeEffect,
                "shaders/runtime_effect.wgsl",
            ),
            (
                RuntimeShaderSlot::TacticalMapOverlay,
                "shaders/tactical_map_overlay.wgsl",
            ),
        ] {
            let source = fs::read_to_string(asset_root.join(source_path))
                .expect("bundled runtime shader should exist");

            validate_runtime_shader_source(slot, &source).unwrap_or_else(|error| {
                panic!("{source_path} should match {slot:?} material ABI: {error}")
            });
        }
    }

    #[test]
    fn bundled_runtime_shader_fallbacks_match_material_binding_abis() {
        for slot in [
            RuntimeShaderSlot::Starfield,
            RuntimeShaderSlot::SpaceBackgroundBase,
            RuntimeShaderSlot::SpaceBackgroundNebula,
            RuntimeShaderSlot::GenericSprite,
            RuntimeShaderSlot::AsteroidSprite,
            RuntimeShaderSlot::PlanetVisual,
            RuntimeShaderSlot::RuntimeEffect,
            RuntimeShaderSlot::TacticalMapOverlay,
        ] {
            validate_runtime_shader_source(slot, bundled_shader_source_for_slot(slot))
                .unwrap_or_else(|error| panic!("bundled fallback for {slot:?} failed: {error}"));
        }
    }
}

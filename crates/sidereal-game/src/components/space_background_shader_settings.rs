use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

/// Paste exported JSON here to override space background defaults.
/// The JSON must match `SpaceBackgroundShaderSettings` and use arrays for `Vec3` fields.
pub const DEFAULT_SPACE_BACKGROUND_SHADER_SETTINGS_JSON: &str = r#"{
  "enabled": true,
  "intensity": 1.3,
  "drift_scale": 1,
  "velocity_glow": 1,
  "nebula_strength": 1,
  "seed": 0,
  "background_rgb": [
    0,
    0,
    0
  ],
  "nebula_color_primary_rgb": [
    1.425,
    0,
    0.023
  ],
  "nebula_color_secondary_rgb": [
    0,
    0,
    1.3980000000000001
  ],
  "nebula_color_accent_rgb": [
    0.913,
    0.16,
    0.36
  ],
  "flare_enabled": true,
  "flare_tint_rgb": [
    1.124,
    0,
    0.462
  ],
  "flare_intensity": 1.58,
  "flare_density": 0.42,
  "flare_size": 2.89,
  "flare_texture_set": 0,
  "nebula_noise_mode": 0,
  "nebula_octaves": 5,
  "nebula_gain": 0.52,
  "nebula_lacunarity": 2,
  "nebula_power": 1,
  "nebula_shelf": 0.42,
  "nebula_ridge_offset": 1,
  "star_mask_enabled": false,
  "star_mask_mode": 0,
  "star_mask_octaves": 4,
  "star_mask_gain": 0.55,
  "star_mask_lacunarity": 1.9500000000000002,
  "star_mask_threshold": 0.35000000000000003,
  "star_mask_power": 1.2000000000000002,
  "star_mask_ridge_offset": 1,
  "star_mask_scale": 1.4000000000000001,
  "nebula_blend_mode": 1,
  "nebula_opacity": 0.99,
  "stars_blend_mode": 0,
  "stars_opacity": 0.09,
  "flares_blend_mode": 1,
  "flares_opacity": 0.9500000000000001,
  "tint_rgb": [
    1,
    1,
    1
  ]
}"#;

#[sidereal_component_macros::sidereal_component(
    kind = "space_background_shader_settings",
    persist = false,
    replicate = false,
    visibility = [OwnerOnly]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SpaceBackgroundShaderSettings {
    pub enabled: bool,
    pub intensity: f32,
    pub drift_scale: f32,
    pub velocity_glow: f32,
    pub nebula_strength: f32,
    pub seed: f32,
    pub background_rgb: Vec3,
    pub nebula_color_primary_rgb: Vec3,
    pub nebula_color_secondary_rgb: Vec3,
    pub nebula_color_accent_rgb: Vec3,
    pub flare_enabled: bool,
    pub flare_tint_rgb: Vec3,
    pub flare_intensity: f32,
    pub flare_density: f32,
    pub flare_size: f32,
    pub flare_texture_set: u32,
    pub nebula_noise_mode: u32,
    pub nebula_octaves: u32,
    pub nebula_gain: f32,
    pub nebula_lacunarity: f32,
    pub nebula_power: f32,
    pub nebula_shelf: f32,
    pub nebula_ridge_offset: f32,
    pub star_mask_enabled: bool,
    pub star_mask_mode: u32,
    pub star_mask_octaves: u32,
    pub star_mask_gain: f32,
    pub star_mask_lacunarity: f32,
    pub star_mask_threshold: f32,
    pub star_mask_power: f32,
    pub star_mask_ridge_offset: f32,
    pub star_mask_scale: f32,
    pub nebula_blend_mode: u32,
    pub nebula_opacity: f32,
    pub stars_blend_mode: u32,
    pub stars_opacity: f32,
    pub flares_blend_mode: u32,
    pub flares_opacity: f32,
    pub tint_rgb: Vec3,
}

fn builtin_space_background_defaults() -> SpaceBackgroundShaderSettings {
    SpaceBackgroundShaderSettings {
        enabled: true,
        intensity: 1.0,
        drift_scale: 1.0,
        velocity_glow: 1.0,
        nebula_strength: 1.0,
        seed: 73.421,
        background_rgb: Vec3::new(0.004, 0.007, 0.018),
        nebula_color_primary_rgb: Vec3::new(0.07, 0.13, 0.28),
        nebula_color_secondary_rgb: Vec3::new(0.12, 0.24, 0.40),
        nebula_color_accent_rgb: Vec3::new(0.18, 0.16, 0.36),
        flare_enabled: true,
        flare_tint_rgb: Vec3::ONE,
        flare_intensity: 0.18,
        flare_density: 0.22,
        flare_size: 0.85,
        flare_texture_set: 0,
        nebula_noise_mode: 0,
        nebula_octaves: 5,
        nebula_gain: 0.52,
        nebula_lacunarity: 2.0,
        nebula_power: 1.0,
        nebula_shelf: 0.42,
        nebula_ridge_offset: 1.0,
        star_mask_enabled: false,
        star_mask_mode: 0,
        star_mask_octaves: 4,
        star_mask_gain: 0.55,
        star_mask_lacunarity: 2.0,
        star_mask_threshold: 0.35,
        star_mask_power: 1.2,
        star_mask_ridge_offset: 1.0,
        star_mask_scale: 1.4,
        nebula_blend_mode: 1,
        nebula_opacity: 1.0,
        stars_blend_mode: 0,
        stars_opacity: 1.0,
        flares_blend_mode: 1,
        flares_opacity: 0.85,
        tint_rgb: Vec3::ONE,
    }
}

impl Default for SpaceBackgroundShaderSettings {
    fn default() -> Self {
        match serde_json::from_str::<SpaceBackgroundShaderSettings>(
            DEFAULT_SPACE_BACKGROUND_SHADER_SETTINGS_JSON,
        ) {
            Ok(settings) => settings,
            Err(err) => {
                eprintln!(
                    "[sidereal-game] invalid DEFAULT_SPACE_BACKGROUND_SHADER_SETTINGS_JSON: {}",
                    err
                );
                builtin_space_background_defaults()
            }
        }
    }
}

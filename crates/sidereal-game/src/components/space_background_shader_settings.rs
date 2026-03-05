use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

/// Paste exported JSON here to override space background defaults.
/// The JSON must match `SpaceBackgroundShaderSettings` and use arrays for `Vec3` fields.
pub const DEFAULT_SPACE_BACKGROUND_SHADER_SETTINGS_JSON: &str = r#"{
  "enabled": true,
  "intensity": 0.35000000000000003,
  "drift_scale": 2,
  "zoom_rate": 1,
  "velocity_glow": 1,
  "nebula_strength": 0.8500000000000001,
  "seed": 0,
  "background_rgb": [
    0,
    0,
    0
  ],
  "nebula_color_primary_rgb": [
    0,
    0,
    0.196
  ],
  "nebula_color_secondary_rgb": [
    0,
    0.073,
    0.082
  ],
  "nebula_color_accent_rgb": [
    0.187,
    0.16,
    0.539
  ],
  "flare_enabled": true,
  "flare_tint_rgb": [
    1,
    1,
    2
  ],
  "flare_intensity": 4,
  "flare_density": 0.54,
  "flare_size": 2.29,
  "flare_texture_set": 0,
  "nebula_noise_mode": 0,
  "nebula_octaves": 5,
  "nebula_gain": 0.52,
  "nebula_lacunarity": 2,
  "nebula_power": 1,
  "nebula_shelf": 0.42,
  "nebula_ridge_offset": 1,
  "star_mask_enabled": true,
  "star_mask_mode": 0,
  "star_mask_octaves": 4,
  "star_mask_gain": 0.42,
  "star_mask_lacunarity": 1.75,
  "star_mask_threshold": 0.35000000000000003,
  "star_mask_power": 1.25,
  "star_mask_ridge_offset": 0.8300000000000001,
  "star_mask_scale": 3.1,
  "nebula_blend_mode": 1,
  "nebula_opacity": 0.5,
  "stars_blend_mode": 2,
  "stars_opacity": 1,
  "star_count": 5,
  "star_size_min": 0.019,
  "star_size_max": 0.022,
  "star_color_rgb": [
    0.6980000000000001,
    0.682,
    2
  ],
  "flares_blend_mode": 1,
  "flares_opacity": 1,
  "depth_layer_separation": 1.03,
  "depth_parallax_scale": 0.8300000000000001,
  "depth_haze_strength": 1.69,
  "depth_occlusion_strength": 1.08,
  "backlight_screen_x": -0.3,
  "backlight_screen_y": 0.1,
  "backlight_intensity": 4,
  "backlight_wrap": 0.49,
  "backlight_edge_boost": 2.2,
  "backlight_bloom_scale": 1.35,
  "backlight_bloom_threshold": 0.14,
  "enable_backlight": true,
  "enable_light_shafts": true,
  "shafts_debug_view": false,
  "shaft_intensity": 1.76,
  "shaft_length": 0.47000000000000003,
  "shaft_falloff": 2.65,
  "shaft_samples": 16,
  "shaft_blend_mode": 1,
  "shaft_opacity": 0.85,
  "shaft_color_rgb": [
    1.15,
    1,
    1.45
  ],
  "backlight_color_rgb": [
    1.15,
    1,
    1.45
  ],
  "tint_rgb": [
    1,
    1.77,
    1.24
  ]
}"#;

/*{
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
    0.386,
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
  "star_mask_enabled": true,
  "star_mask_mode": 0,
  "star_mask_octaves": 4,
  "star_mask_gain": 0.63,
  "star_mask_lacunarity": 2.4000000000000004,
  "star_mask_threshold": 0.35000000000000003,
  "star_mask_power": 1.25,
  "star_mask_ridge_offset": 0.99,
  "star_mask_scale": 1.4000000000000001,
  "nebula_blend_mode": 1,
  "nebula_opacity": 0.75,
  "stars_blend_mode": 2,
  "stars_opacity": 0.79,
  "star_count": 5,
  "star_size_min": 0.034,
  "star_size_max": 0.035,
  "star_color_rgb": [
    1.086,
    1,
    1.487
  ],
  "flares_blend_mode": 1,
  "flares_opacity": 0.9500000000000001,
  "tint_rgb": [
    1,
    1,
    1
  ]
}"#;
*/

#[sidereal_component_macros::sidereal_component(
    kind = "space_background_shader_settings",
    persist = false,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SpaceBackgroundShaderSettings {
    pub enabled: bool,
    pub intensity: f32,
    pub drift_scale: f32,
    pub zoom_rate: f32,
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
    pub star_count: f32,
    pub star_size_min: f32,
    pub star_size_max: f32,
    pub star_color_rgb: Vec3,
    pub flares_blend_mode: u32,
    pub flares_opacity: f32,
    pub depth_layer_separation: f32,
    pub depth_parallax_scale: f32,
    pub depth_haze_strength: f32,
    pub depth_occlusion_strength: f32,
    pub backlight_screen_x: f32,
    pub backlight_screen_y: f32,
    pub backlight_intensity: f32,
    pub backlight_wrap: f32,
    pub backlight_edge_boost: f32,
    pub backlight_bloom_scale: f32,
    pub backlight_bloom_threshold: f32,
    pub enable_backlight: bool,
    pub enable_light_shafts: bool,
    pub shafts_debug_view: bool,
    pub shaft_intensity: f32,
    pub shaft_length: f32,
    pub shaft_falloff: f32,
    pub shaft_samples: u32,
    pub shaft_blend_mode: u32,
    pub shaft_opacity: f32,
    pub shaft_color_rgb: Vec3,
    pub backlight_color_rgb: Vec3,
    pub tint_rgb: Vec3,
}

fn builtin_space_background_defaults() -> SpaceBackgroundShaderSettings {
    SpaceBackgroundShaderSettings {
        enabled: true,
        intensity: 0.35,
        drift_scale: 2.0,
        zoom_rate: 1.0,
        velocity_glow: 1.0,
        nebula_strength: 0.85,
        seed: 0.0,
        background_rgb: Vec3::ZERO,
        nebula_color_primary_rgb: Vec3::new(0.0, 0.0, 0.196),
        nebula_color_secondary_rgb: Vec3::new(0.0, 0.073, 0.082),
        nebula_color_accent_rgb: Vec3::new(0.187, 0.16, 0.539),
        flare_enabled: true,
        flare_tint_rgb: Vec3::new(1.0, 1.0, 2.0),
        flare_intensity: 4.0,
        flare_density: 0.54,
        flare_size: 2.29,
        flare_texture_set: 0,
        nebula_noise_mode: 0,
        nebula_octaves: 5,
        nebula_gain: 0.52,
        nebula_lacunarity: 2.0,
        nebula_power: 1.0,
        nebula_shelf: 0.42,
        nebula_ridge_offset: 1.0,
        star_mask_enabled: true,
        star_mask_mode: 0,
        star_mask_octaves: 4,
        star_mask_gain: 0.42,
        star_mask_lacunarity: 1.75,
        star_mask_threshold: 0.35,
        star_mask_power: 1.25,
        star_mask_ridge_offset: 0.83,
        star_mask_scale: 3.1,
        nebula_blend_mode: 1,
        nebula_opacity: 0.5,
        stars_blend_mode: 2,
        stars_opacity: 1.0,
        star_count: 5.0,
        star_size_min: 0.019,
        star_size_max: 0.022,
        star_color_rgb: Vec3::new(0.698, 0.682, 2.0),
        flares_blend_mode: 1,
        flares_opacity: 1.0,
        depth_layer_separation: 1.03,
        depth_parallax_scale: 0.83,
        depth_haze_strength: 1.69,
        depth_occlusion_strength: 1.08,
        backlight_screen_x: -0.3,
        backlight_screen_y: 0.1,
        backlight_intensity: 4.0,
        backlight_wrap: 0.49,
        backlight_edge_boost: 2.2,
        backlight_bloom_scale: 1.35,
        backlight_bloom_threshold: 0.14,
        enable_backlight: true,
        enable_light_shafts: true,
        shafts_debug_view: false,
        shaft_intensity: 1.76,
        shaft_length: 0.47,
        shaft_falloff: 2.65,
        shaft_samples: 16,
        shaft_blend_mode: 1,
        shaft_opacity: 0.85,
        shaft_color_rgb: Vec3::new(1.15, 1.0, 1.45),
        backlight_color_rgb: Vec3::new(1.15, 1.0, 1.45),
        tint_rgb: Vec3::new(1.0, 1.77, 1.24),
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

#[cfg(test)]
mod tests {
    use super::SpaceBackgroundShaderSettings;

    #[test]
    fn default_space_background_settings_builds_without_recursive_default() {
        let settings = SpaceBackgroundShaderSettings::default();
        assert!(settings.intensity.is_finite());
    }
}

use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

/// Paste exported JSON here to override starfield defaults.
/// The JSON must match `StarfieldShaderSettings` and use arrays for `Vec3` fields.
pub const DEFAULT_STARFIELD_SHADER_SETTINGS_JSON: &str = r#"{
  "enabled": true,
  "density": 0.07,
  "layer_count": 4,
  "initial_z_offset": 0.5,
  "intensity": 4,
  "alpha": 0.32,
  "tint_rgb": [
    1,
    1,
    1.34
  ],
  "star_size": 0.3,
  "star_intensity": 6.65,
  "star_alpha": 1,
  "star_color_rgb": [
    0.33,
    0.33,
    1.49
  ],
  "corona_size": 2.68,
  "corona_intensity": 1.35,
  "corona_alpha": 1,
  "corona_color_rgb": [
    0.42,
    0.42,
    1.83
  ]
}"#;

#[sidereal_component_macros::sidereal_component(
    kind = "starfield_shader_settings",
    persist = false,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct StarfieldShaderSettings {
    pub enabled: bool,
    pub density: f32,
    pub layer_count: u32,
    /// Pushes initial starfield layers farther "back" in depth space.
    /// 0.0 = unchanged depth distribution, 1.0 = all layers forced far.
    pub initial_z_offset: f32,
    pub intensity: f32,
    pub alpha: f32,
    pub tint_rgb: Vec3,
    pub star_size: f32,
    pub star_intensity: f32,
    pub star_alpha: f32,
    pub star_color_rgb: Vec3,
    pub corona_size: f32,
    pub corona_intensity: f32,
    pub corona_alpha: f32,
    pub corona_color_rgb: Vec3,
}

fn builtin_starfield_defaults() -> StarfieldShaderSettings {
    StarfieldShaderSettings {
        enabled: true,
        density: 0.05,
        layer_count: 3,
        initial_z_offset: 0.35,
        intensity: 1.0,
        alpha: 1.0,
        tint_rgb: Vec3::ONE,
        star_size: 1.0,
        star_intensity: 1.0,
        star_alpha: 1.0,
        star_color_rgb: Vec3::new(0.72, 0.83, 1.0),
        corona_size: 1.0,
        corona_intensity: 1.0,
        corona_alpha: 1.0,
        corona_color_rgb: Vec3::new(0.44, 0.64, 1.0),
    }
}

impl Default for StarfieldShaderSettings {
    fn default() -> Self {
        match serde_json::from_str::<StarfieldShaderSettings>(
            DEFAULT_STARFIELD_SHADER_SETTINGS_JSON,
        ) {
            Ok(settings) => settings,
            Err(err) => {
                eprintln!(
                    "[sidereal-game] invalid DEFAULT_STARFIELD_SHADER_SETTINGS_JSON: {}",
                    err
                );
                builtin_starfield_defaults()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StarfieldShaderSettings;

    #[test]
    fn default_starfield_settings_builds_without_recursive_default() {
        let settings = StarfieldShaderSettings::default();
        assert!(settings.intensity.is_finite());
    }
}

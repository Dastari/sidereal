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
  ]
}"#;

#[sidereal_component_macros::sidereal_component(
    kind = "starfield_shader_settings",
    persist = false,
    replicate = false,
    visibility = [OwnerOnly]
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

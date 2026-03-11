use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

/// Paste exported JSON here to override thruster plume shader defaults.
/// The JSON must match `ThrusterPlumeShaderSettings` and use arrays for `Vec3` fields.
pub const DEFAULT_THRUSTER_PLUME_SHADER_SETTINGS_JSON: &str = r#"{
  "enabled": true,
  "debug_override_enabled": false,
  "debug_forced_thrust_alpha": 0.0,
  "debug_force_afterburner": false,
  "base_length_m": 0.0,
  "max_length_m": 14.0,
  "base_width_m": 1.35,
  "max_width_m": 4.1,
  "idle_core_alpha": 0.22,
  "max_alpha": 0.9,
  "falloff": 1.35,
  "edge_softness": 1.8,
  "noise_strength": 0.4,
  "flicker_hz": 18.0,
  "reactive_length_scale": 1.0,
  "reactive_alpha_scale": 1.0,
  "afterburner_length_scale": 1.5,
  "afterburner_alpha_boost": 0.25,
  "base_color_rgb": [
    0.35,
    0.68,
    1.2
  ],
  "hot_color_rgb": [
    0.7,
    0.92,
    1.3
  ],
  "afterburner_color_rgb": [
    1.0,
    1.0,
    1.4
  ]
}"#;

#[sidereal_component_macros::sidereal_component(
    kind = "thruster_plume_shader_settings",
    persist = true,
    replicate = true,
    visibility = [OwnerOnly]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ThrusterPlumeShaderSettings {
    pub enabled: bool,
    #[serde(default)]
    pub debug_override_enabled: bool,
    #[serde(default)]
    pub debug_forced_thrust_alpha: f32,
    #[serde(default)]
    pub debug_force_afterburner: bool,
    pub base_length_m: f32,
    pub max_length_m: f32,
    pub base_width_m: f32,
    pub max_width_m: f32,
    pub idle_core_alpha: f32,
    pub max_alpha: f32,
    pub falloff: f32,
    pub edge_softness: f32,
    pub noise_strength: f32,
    pub flicker_hz: f32,
    pub reactive_length_scale: f32,
    pub reactive_alpha_scale: f32,
    pub afterburner_length_scale: f32,
    pub afterburner_alpha_boost: f32,
    pub base_color_rgb: Vec3,
    pub hot_color_rgb: Vec3,
    pub afterburner_color_rgb: Vec3,
}

fn builtin_thruster_plume_defaults() -> ThrusterPlumeShaderSettings {
    ThrusterPlumeShaderSettings {
        enabled: true,
        debug_override_enabled: false,
        debug_forced_thrust_alpha: 0.0,
        debug_force_afterburner: false,
        base_length_m: 0.0,
        max_length_m: 14.0,
        base_width_m: 1.35,
        max_width_m: 4.1,
        idle_core_alpha: 0.2,
        max_alpha: 0.9,
        falloff: 1.25,
        edge_softness: 1.7,
        noise_strength: 0.35,
        flicker_hz: 16.0,
        reactive_length_scale: 1.0,
        reactive_alpha_scale: 1.0,
        afterburner_length_scale: 1.4,
        afterburner_alpha_boost: 0.2,
        base_color_rgb: Vec3::new(0.35, 0.68, 1.2),
        hot_color_rgb: Vec3::new(0.7, 0.92, 1.3),
        afterburner_color_rgb: Vec3::new(1.0, 1.0, 1.4),
    }
}

impl Default for ThrusterPlumeShaderSettings {
    fn default() -> Self {
        match serde_json::from_str::<ThrusterPlumeShaderSettings>(
            DEFAULT_THRUSTER_PLUME_SHADER_SETTINGS_JSON,
        ) {
            Ok(settings) => settings,
            Err(err) => {
                tracing::error!(
                    "[sidereal-game] invalid DEFAULT_THRUSTER_PLUME_SHADER_SETTINGS_JSON: {}",
                    err
                );
                builtin_thruster_plume_defaults()
            }
        }
    }
}

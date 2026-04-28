use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

/// Paste exported JSON here to override environment lighting defaults.
/// The JSON must match `EnvironmentLightingState` and use arrays for `Vec2`/`Vec3` fields.
pub const DEFAULT_ENVIRONMENT_LIGHTING_STATE_JSON: &str = r#"{
  "primary_direction_xy": [0.76, 0.58],
  "primary_elevation": 0.36,
  "primary_color_rgb": [1.0, 0.92, 0.78],
  "primary_intensity": 1.15,
  "ambient_color_rgb": [0.16, 0.20, 0.27],
  "ambient_intensity": 0.12,
  "backlight_color_rgb": [0.28, 0.42, 0.62],
  "backlight_intensity": 0.08,
  "event_flash_color_rgb": [1.0, 0.95, 0.88],
  "event_flash_intensity": 0.0
}"#;

#[sidereal_component_macros::sidereal_component(
    kind = "environment_lighting_state",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EnvironmentLightingState {
    pub primary_direction_xy: Vec2,
    pub primary_elevation: f32,
    pub primary_color_rgb: Vec3,
    pub primary_intensity: f32,
    pub ambient_color_rgb: Vec3,
    pub ambient_intensity: f32,
    pub backlight_color_rgb: Vec3,
    pub backlight_intensity: f32,
    pub event_flash_color_rgb: Vec3,
    pub event_flash_intensity: f32,
}

impl Default for EnvironmentLightingState {
    fn default() -> Self {
        serde_json::from_str(DEFAULT_ENVIRONMENT_LIGHTING_STATE_JSON)
            .expect("default environment lighting state JSON must be valid")
    }
}

#[cfg(test)]
mod tests {
    use super::EnvironmentLightingState;

    #[test]
    fn default_environment_lighting_state_parse() {
        let lighting = EnvironmentLightingState::default();
        assert!(lighting.primary_intensity > 0.0);
        assert!(lighting.primary_elevation > 0.0);
        assert!(lighting.ambient_intensity >= 0.0);
    }
}

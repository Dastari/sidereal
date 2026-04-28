use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

/// Paste exported JSON here to override stellar light defaults.
/// The JSON must match `StellarLightSource` and use arrays for `Vec3` fields.
pub const DEFAULT_STELLAR_LIGHT_SOURCE_JSON: &str = r#"{
  "enabled": true,
  "color_rgb": [1.0, 0.86, 0.48],
  "intensity": 1.25,
  "inner_radius_m": 3500.0,
  "outer_radius_m": 18000.0,
  "elevation": 0.36,
  "priority": 1.0
}"#;

#[sidereal_component_macros::sidereal_component(
    kind = "stellar_light_source",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct StellarLightSource {
    pub enabled: bool,
    pub color_rgb: Vec3,
    pub intensity: f32,
    pub inner_radius_m: f32,
    pub outer_radius_m: f32,
    pub elevation: f32,
    pub priority: f32,
}

impl Default for StellarLightSource {
    fn default() -> Self {
        serde_json::from_str(DEFAULT_STELLAR_LIGHT_SOURCE_JSON)
            .expect("default stellar light source JSON must be valid")
    }
}

#[cfg(test)]
mod tests {
    use super::StellarLightSource;

    #[test]
    fn default_stellar_light_source_parse() {
        let light = StellarLightSource::default();
        assert!(light.enabled);
        assert!(light.intensity > 0.0);
        assert!(light.outer_radius_m > light.inner_radius_m);
        assert!(light.elevation > 0.0);
    }
}

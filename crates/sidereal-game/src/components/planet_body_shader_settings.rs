use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

/// Paste exported JSON here to override planet body defaults.
/// The JSON must match `PlanetBodyShaderSettings` and use arrays for `Vec3` fields.
pub const DEFAULT_PLANET_BODY_SHADER_SETTINGS_JSON: &str = r#"{
  "enabled": true,
  "enable_surface_detail": true,
  "enable_craters": true,
  "enable_clouds": true,
  "enable_atmosphere": true,
  "enable_specular": true,
  "enable_night_lights": true,
  "enable_emissive": true,
  "enable_ocean_specular": true,
  "body_kind": 0,
  "planet_type": 0,
  "seed": 1,
  "base_radius_scale": 0.5,
  "normal_strength": 0.55,
  "detail_level": 0.3,
  "rotation_speed": 0.004,
  "light_wrap": 0.2,
  "ambient_strength": 0.16,
  "specular_strength": 0.12,
  "specular_power": 18.0,
  "rim_strength": 0.28,
  "rim_power": 3.6,
  "fresnel_strength": 0.4,
  "cloud_shadow_strength": 0.18,
  "night_glow_strength": 0.05,
  "continent_size": 0.58,
  "ocean_level": 0.46,
  "mountain_height": 0.34,
  "roughness": 0.44,
  "terrain_octaves": 5,
  "terrain_lacunarity": 2.1,
  "terrain_gain": 0.5,
  "crater_density": 0.18,
  "crater_size": 0.33,
  "volcano_density": 0.04,
  "ice_cap_size": 0.18,
  "storm_intensity": 0.1,
  "bands_count": 6.0,
  "spot_density": 0.08,
  "surface_activity": 0.12,
  "corona_intensity": 0.0,
  "cloud_coverage": 0.34,
  "cloud_scale": 1.3,
  "cloud_speed": 0.18,
  "cloud_alpha": 0.42,
  "atmosphere_thickness": 0.12,
  "atmosphere_falloff": 2.8,
  "atmosphere_alpha": 0.48,
  "city_lights": 0.04,
  "emissive_strength": 0.0,
  "sun_intensity": 1.0,
  "surface_saturation": 1.12,
  "surface_contrast": 1.08,
  "light_color_mix": 0.14,
  "sun_direction_xy": [0.74, 0.52],
  "color_primary_rgb": [0.24, 0.48, 0.22],
  "color_secondary_rgb": [0.52, 0.42, 0.28],
  "color_tertiary_rgb": [0.08, 0.2, 0.48],
  "color_atmosphere_rgb": [0.36, 0.62, 1.0],
  "color_clouds_rgb": [0.95, 0.97, 1.0],
  "color_night_lights_rgb": [1.0, 0.76, 0.4],
  "color_emissive_rgb": [1.0, 0.42, 0.18]
}"#;

#[sidereal_component_macros::sidereal_component(
    kind = "planet_body_shader_settings",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct PlanetBodyShaderSettings {
    pub enabled: bool,
    pub enable_surface_detail: bool,
    pub enable_craters: bool,
    pub enable_clouds: bool,
    pub enable_atmosphere: bool,
    pub enable_specular: bool,
    pub enable_night_lights: bool,
    pub enable_emissive: bool,
    pub enable_ocean_specular: bool,
    pub body_kind: u32,
    pub planet_type: u32,
    pub seed: u32,
    pub base_radius_scale: f32,
    pub normal_strength: f32,
    pub detail_level: f32,
    pub rotation_speed: f32,
    pub light_wrap: f32,
    pub ambient_strength: f32,
    pub specular_strength: f32,
    pub specular_power: f32,
    pub rim_strength: f32,
    pub rim_power: f32,
    pub fresnel_strength: f32,
    pub cloud_shadow_strength: f32,
    pub night_glow_strength: f32,
    pub continent_size: f32,
    pub ocean_level: f32,
    pub mountain_height: f32,
    pub roughness: f32,
    pub terrain_octaves: u32,
    pub terrain_lacunarity: f32,
    pub terrain_gain: f32,
    pub crater_density: f32,
    pub crater_size: f32,
    pub volcano_density: f32,
    pub ice_cap_size: f32,
    pub storm_intensity: f32,
    pub bands_count: f32,
    pub spot_density: f32,
    pub surface_activity: f32,
    pub corona_intensity: f32,
    pub cloud_coverage: f32,
    pub cloud_scale: f32,
    pub cloud_speed: f32,
    pub cloud_alpha: f32,
    pub atmosphere_thickness: f32,
    pub atmosphere_falloff: f32,
    pub atmosphere_alpha: f32,
    pub city_lights: f32,
    pub emissive_strength: f32,
    pub sun_intensity: f32,
    pub surface_saturation: f32,
    pub surface_contrast: f32,
    pub light_color_mix: f32,
    pub sun_direction_xy: Vec2,
    pub color_primary_rgb: Vec3,
    pub color_secondary_rgb: Vec3,
    pub color_tertiary_rgb: Vec3,
    pub color_atmosphere_rgb: Vec3,
    pub color_clouds_rgb: Vec3,
    pub color_night_lights_rgb: Vec3,
    pub color_emissive_rgb: Vec3,
}

impl Default for PlanetBodyShaderSettings {
    fn default() -> Self {
        serde_json::from_str(DEFAULT_PLANET_BODY_SHADER_SETTINGS_JSON)
            .expect("default planet body shader settings JSON must be valid")
    }
}

#[cfg(test)]
mod tests {
    use super::PlanetBodyShaderSettings;

    #[test]
    fn default_planet_body_shader_settings_parse() {
        let settings = PlanetBodyShaderSettings::default();
        assert!(settings.enabled);
        assert!(settings.enable_surface_detail);
        assert_eq!(settings.body_kind, 0);
        assert_eq!(settings.planet_type, 0);
        assert_eq!(settings.terrain_octaves, 5);
        assert!(settings.atmosphere_thickness > 0.0);
    }
}

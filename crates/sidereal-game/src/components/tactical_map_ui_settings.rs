use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[sidereal_component_macros::sidereal_component(
    kind = "tactical_map_ui_settings",
    persist = true,
    replicate = true,
    visibility = [OwnerOnly]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TacticalMapUiSettings {
    pub shader_asset_id: String,
    pub map_distance_m: f32,
    pub map_zoom_wheel_sensitivity: f32,
    pub overlay_takeover_alpha: f32,
    pub grid_major_color_rgb: Vec3,
    pub grid_minor_color_rgb: Vec3,
    pub grid_micro_color_rgb: Vec3,
    pub grid_major_alpha: f32,
    pub grid_minor_alpha: f32,
    pub grid_micro_alpha: f32,
    pub grid_major_glow_alpha: f32,
    pub grid_minor_glow_alpha: f32,
    pub grid_micro_glow_alpha: f32,
    pub background_color_rgb: Vec3,
    pub line_width_major_px: f32,
    pub line_width_minor_px: f32,
    pub line_width_micro_px: f32,
    pub glow_width_major_px: f32,
    pub glow_width_minor_px: f32,
    pub glow_width_micro_px: f32,
    pub fx_mode: u32,
    pub fx_opacity: f32,
    pub fx_noise_amount: f32,
    pub fx_scanline_density: f32,
    pub fx_scanline_speed: f32,
    pub fx_crt_distortion: f32,
    pub fx_vignette_strength: f32,
    pub fx_green_tint_mix: f32,
}

impl Default for TacticalMapUiSettings {
    fn default() -> Self {
        Self {
            shader_asset_id: String::new(),
            map_distance_m: 90.0,
            map_zoom_wheel_sensitivity: 0.12,
            overlay_takeover_alpha: 0.995,
            grid_major_color_rgb: Vec3::new(0.22, 0.34, 0.48),
            grid_minor_color_rgb: Vec3::new(0.22, 0.34, 0.48),
            grid_micro_color_rgb: Vec3::new(0.22, 0.34, 0.48),
            grid_major_alpha: 0.14,
            grid_minor_alpha: 0.126,
            grid_micro_alpha: 0.113,
            grid_major_glow_alpha: 0.02,
            grid_minor_glow_alpha: 0.018,
            grid_micro_glow_alpha: 0.016,
            background_color_rgb: Vec3::new(0.005, 0.008, 0.02),
            line_width_major_px: 1.4,
            line_width_minor_px: 0.95,
            line_width_micro_px: 0.75,
            glow_width_major_px: 2.0,
            glow_width_minor_px: 1.5,
            glow_width_micro_px: 1.2,
            fx_mode: 1,
            fx_opacity: 0.45,
            fx_noise_amount: 0.12,
            fx_scanline_density: 360.0,
            fx_scanline_speed: 0.65,
            fx_crt_distortion: 0.02,
            fx_vignette_strength: 0.24,
            fx_green_tint_mix: 0.0,
        }
    }
}

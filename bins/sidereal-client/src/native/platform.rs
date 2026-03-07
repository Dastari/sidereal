//! Platform and render config: constants, wgpu, viewport.

use bevy::prelude::*;
use bevy::render::settings::{Backends, WgpuSettings};
use std::fs;

pub const BACKDROP_RENDER_LAYER: usize = 1;
pub const PLANET_BODY_RENDER_LAYER: usize = 2;
pub const FULLSCREEN_FOREGROUND_RENDER_LAYER: usize = 3;
pub const POST_PROCESS_RENDER_LAYER: usize = 4;
pub const UI_OVERLAY_RENDER_LAYER: usize = 31;
pub const ORTHO_SCALE_PER_DISTANCE: f32 = 0.02;
/// Not used when UI overlay uses true screen space (scale derived from window height).
#[allow(dead_code)]
pub const UI_OVERLAY_ORTHO_SCALE: f32 = 0.6;
pub const MIN_WINDOW_WIDTH: f32 = 960.0;
pub const MIN_WINDOW_HEIGHT: f32 = 540.0;

pub fn safe_viewport_size(window: &bevy::window::Window) -> Option<Vec2> {
    let width = window.resolution.width();
    let height = window.resolution.height();
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some(Vec2::new(width, height))
}

pub fn safe_render_target_size(window: &bevy::window::Window) -> Option<Vec2> {
    let width = window.physical_width() as f32;
    let height = window.physical_height() as f32;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some(Vec2::new(width, height))
}

pub fn preferred_backends() -> Backends {
    if let Ok(raw_value) = std::env::var("SIDEREAL_CLIENT_WGPU_BACKENDS") {
        let parsed = Backends::from_comma_list(&raw_value);
        if parsed.is_empty() {
            bevy::log::warn!(
                "SIDEREAL_CLIENT_WGPU_BACKENDS='{}' did not contain any valid backend values; falling back to WGPU_BACKEND/default backend set",
                raw_value
            );
        } else {
            return parsed;
        }
    }
    if let Some(from_env) = Backends::from_env() {
        return from_env;
    }
    if is_wsl_runtime() {
        bevy::log::warn!(
            "WSL runtime detected; defaulting client backend to VULKAN. Override with SIDEREAL_CLIENT_WGPU_BACKENDS or WGPU_BACKEND."
        );
        return Backends::VULKAN;
    }
    Backends::PRIMARY
}

fn is_wsl_runtime() -> bool {
    let osrelease = fs::read_to_string("/proc/sys/kernel/osrelease")
        .or_else(|_| fs::read_to_string("/proc/version"))
        .unwrap_or_default();
    let lowered = osrelease.to_ascii_lowercase();
    lowered.contains("microsoft") || lowered.contains("wsl")
}

pub fn configured_wgpu_settings() -> WgpuSettings {
    let force_fallback_adapter = match std::env::var("SIDEREAL_CLIENT_FORCE_SOFTWARE_ADAPTER") {
        Ok(value) if value == "1" || value.eq_ignore_ascii_case("true") => true,
        Ok(_) => false,
        Err(_) => false,
    };
    let backends = preferred_backends();
    bevy::log::info!(
        "client render config backends={:?} force_fallback_adapter={}",
        backends,
        force_fallback_adapter
    );
    WgpuSettings {
        backends: Some(backends),
        force_fallback_adapter,
        ..Default::default()
    }
}

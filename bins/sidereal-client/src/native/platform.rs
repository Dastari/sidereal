//! Platform and render config: constants, wgpu, viewport, frame cap, auth UI helpers.

use bevy::prelude::*;
use bevy::render::settings::{Backends, WgpuSettings};
use std::net::TcpListener;
use std::time::Instant;

use super::resources::FrameRateCap;
use super::state::{ClientSession, FocusField};

pub const BACKDROP_RENDER_LAYER: usize = 1;
pub const UI_OVERLAY_RENDER_LAYER: usize = 31;
pub const ORTHO_SCALE_PER_DISTANCE: f32 = 0.02;
pub const MIN_WINDOW_WIDTH: f32 = 960.0;
pub const MIN_WINDOW_HEIGHT: f32 = 540.0;
pub const STREAMED_SPRITE_PIXEL_SHADER_PATH: &str =
    "data/cache_stream/shaders/sprite_pixel_effect.wgsl";

pub fn safe_viewport_size(window: &bevy::window::Window) -> Option<Vec2> {
    let width = window.resolution.width();
    let height = window.resolution.height();
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some(Vec2::new(width, height))
}

pub fn active_field_mut(session: &mut ClientSession) -> &mut String {
    match session.focus {
        FocusField::Email => &mut session.email,
        FocusField::Password => &mut session.password,
        FocusField::ResetToken => &mut session.reset_token,
        FocusField::NewPassword => &mut session.new_password,
    }
}

pub fn mask(value: &str) -> String {
    if value.is_empty() {
        return "".to_string();
    }
    "*".repeat(value.chars().count())
}

pub fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);
    !is_in_private_use_area && !chr.is_ascii_control()
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
    Backends::from_env().unwrap_or(Backends::PRIMARY)
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

pub fn acquire_multi_instance_guard() -> Option<TcpListener> {
    const MULTI_INSTANCE_GUARD_ADDR: &str = "127.0.0.1:62173";
    match TcpListener::bind(MULTI_INSTANCE_GUARD_ADDR) {
        Ok(listener) => Some(listener),
        Err(err) => {
            bevy::log::warn!(
                "sidereal-client multi-instance guard lock unavailable at {} ({}). Assuming secondary instance.",
                MULTI_INSTANCE_GUARD_ADDR, err
            );
            None
        }
    }
}

pub fn enforce_frame_rate_cap_system(mut frame_cap: ResMut<'_, FrameRateCap>) {
    let elapsed = frame_cap.last_frame_end.elapsed();
    if elapsed < frame_cap.frame_duration {
        std::thread::sleep(frame_cap.frame_duration - elapsed);
    }
    frame_cap.last_frame_end = Instant::now();
}

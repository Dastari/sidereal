use bevy::prelude::*;

use crate::theme::{UiTheme, color, with_alpha};

use super::shadow::glow_box_shadow;

pub fn panel_surface(
    theme: UiTheme,
    glow_intensity: f32,
) -> (BackgroundColor, BorderColor, BoxShadow) {
    let colors = theme.colors;
    let shadow_spread_px = (theme.metrics.panel_shadow_spread_px * 0.5).max(2.0);
    let shadow_blur_px = (theme.metrics.panel_shadow_blur_px * 0.34).max(9.0);
    (
        BackgroundColor(colors.panel_color()),
        BorderColor::all(color(with_alpha(colors.primary, 0.32))),
        glow_box_shadow(
            colors.glow_muted,
            0.1,
            shadow_spread_px,
            shadow_blur_px,
            glow_intensity,
        ),
    )
}

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

pub fn panel_surface_with_accent(
    theme: UiTheme,
    glow_intensity: f32,
    accent_color: Color,
) -> (BackgroundColor, BorderColor, BoxShadow) {
    let (_, _, shadow) = panel_surface(theme, glow_intensity);
    (
        BackgroundColor(blend_colors(theme.colors.panel_color(), accent_color, 0.14)),
        BorderColor::all(accent_color.with_alpha(0.56)),
        shadow,
    )
}

fn blend_colors(base: Color, tint: Color, tint_weight: f32) -> Color {
    let base = base.to_srgba();
    let tint = tint.to_srgba();
    let tint_weight = tint_weight.clamp(0.0, 1.0);
    let base_weight = 1.0 - tint_weight;
    Color::srgba(
        (base.red * base_weight) + (tint.red * tint_weight),
        (base.green * base_weight) + (tint.green * tint_weight),
        (base.blue * base_weight) + (tint.blue * tint_weight),
        base.alpha.max(tint.alpha),
    )
}

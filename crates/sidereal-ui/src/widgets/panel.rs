use bevy::prelude::*;

use crate::theme::{UiSemanticTone, UiTheme, color, with_alpha};

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

pub fn panel_surface_with_tone(
    theme: UiTheme,
    glow_intensity: f32,
    tone: UiSemanticTone,
) -> (BackgroundColor, BorderColor, BoxShadow) {
    let accent_token = tone.accent_token(theme);
    let accent_color = tone.accent_color(theme);
    let chrome_color = tone.chrome_color(theme);
    let shadow_spread_px = (theme.metrics.panel_shadow_spread_px * 0.55).max(2.0);
    let shadow_blur_px = (theme.metrics.panel_shadow_blur_px * 0.42).max(10.0);
    (
        BackgroundColor(semantic_surface_background(
            theme.colors.panel_color(),
            accent_color,
            tone,
        )),
        BorderColor::all(chrome_color.with_alpha(0.92)),
        glow_box_shadow(
            accent_token,
            0.13,
            shadow_spread_px,
            shadow_blur_px,
            glow_intensity,
        ),
    )
}

fn semantic_surface_background(base: Color, accent: Color, tone: UiSemanticTone) -> Color {
    match tone {
        UiSemanticTone::Danger => blend_colors_with_alpha(base, accent, 0.62, 0.9),
        UiSemanticTone::Warning => blend_colors_with_alpha(base, accent, 0.58, 0.9),
        UiSemanticTone::Success => blend_colors_with_alpha(base, accent, 0.5, 0.9),
        UiSemanticTone::Info => blend_colors_with_alpha(base, accent, 0.24, 0.92),
    }
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

fn blend_colors_with_alpha(base: Color, tint: Color, tint_weight: f32, alpha: f32) -> Color {
    let mut color = blend_colors(base, tint, tint_weight).to_srgba();
    color.alpha = alpha.clamp(0.0, 1.0);
    color.into()
}

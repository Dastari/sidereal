use bevy::prelude::*;

use crate::theme::{color, with_alpha};

pub(crate) fn glow_alpha(base_alpha: f32, glow_intensity: f32) -> f32 {
    (base_alpha * glow_intensity.max(0.0)).clamp(0.0, 1.0)
}

pub(crate) fn glow_box_shadow(
    color_token: bevy::color::Oklcha,
    base_alpha: f32,
    spread_px: f32,
    blur_px: f32,
    glow_intensity: f32,
) -> BoxShadow {
    let alpha = glow_alpha(base_alpha, glow_intensity);
    if alpha <= f32::EPSILON {
        return no_box_shadow();
    }

    BoxShadow::new(
        color(with_alpha(color_token, alpha)),
        Val::Px(0.0),
        Val::Px(0.0),
        Val::Px(spread_px),
        Val::Px(blur_px),
    )
}

pub(crate) fn no_box_shadow() -> BoxShadow {
    BoxShadow::new(
        Color::NONE,
        Val::Px(0.0),
        Val::Px(0.0),
        Val::Px(0.0),
        Val::Px(0.0),
    )
}

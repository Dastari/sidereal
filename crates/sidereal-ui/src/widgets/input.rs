use bevy::prelude::*;

use crate::theme::{UiTheme, color, with_alpha};

use super::shadow::glow_box_shadow;

pub fn input_surface(
    theme: UiTheme,
    focused: bool,
    glow_intensity: f32,
) -> (BackgroundColor, BorderColor, BoxShadow) {
    let colors = theme.colors;
    if focused {
        (
            BackgroundColor(color(with_alpha(colors.input, 0.98))),
            BorderColor::all(colors.ring_color()),
            glow_box_shadow(colors.glow, 0.06, 1.0, 4.0, glow_intensity),
        )
    } else {
        (
            BackgroundColor(color(with_alpha(colors.input, 0.88))),
            BorderColor::all(color(with_alpha(colors.border, 0.75))),
            glow_box_shadow(colors.glow_muted, 0.025, 0.75, 3.0, glow_intensity),
        )
    }
}

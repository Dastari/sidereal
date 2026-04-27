use bevy::prelude::*;

use crate::theme::{UiSemanticTone, UiTheme, color, with_alpha};

use super::shadow::{glow_box_shadow, no_box_shadow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiButtonVariant {
    Primary,
    Secondary,
    Outline,
    Ghost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiInteractionState {
    Idle,
    Hovered,
    Pressed,
    Selected,
    Focused,
}

pub fn button_surface(
    theme: UiTheme,
    variant: UiButtonVariant,
    state: UiInteractionState,
    glow_intensity: f32,
) -> (BackgroundColor, BorderColor, BoxShadow) {
    let colors = theme.colors;
    match (variant, state) {
        (UiButtonVariant::Primary, UiInteractionState::Idle) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.22))),
            BorderColor::all(color(with_alpha(colors.border, 0.9))),
            no_box_shadow(),
        ),
        (UiButtonVariant::Primary, UiInteractionState::Hovered) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.3))),
            BorderColor::all(colors.ring_color()),
            glow_box_shadow(colors.glow, 0.038, 1.0, 3.5, glow_intensity),
        ),
        (UiButtonVariant::Primary, UiInteractionState::Pressed) => (
            BackgroundColor(colors.primary_color()),
            BorderColor::all(colors.glow_color()),
            no_box_shadow(),
        ),
        (UiButtonVariant::Primary, UiInteractionState::Selected) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.28))),
            BorderColor::all(colors.glow_color()),
            no_box_shadow(),
        ),
        (UiButtonVariant::Primary, UiInteractionState::Focused) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.24))),
            BorderColor::all(colors.ring_color()),
            no_box_shadow(),
        ),
        (UiButtonVariant::Secondary, UiInteractionState::Idle) => (
            BackgroundColor(color(with_alpha(colors.secondary, 0.65))),
            BorderColor::all(color(with_alpha(colors.border, 0.7))),
            no_box_shadow(),
        ),
        (UiButtonVariant::Secondary, UiInteractionState::Hovered) => (
            BackgroundColor(color(with_alpha(colors.secondary, 0.76))),
            BorderColor::all(color(with_alpha(colors.glow_muted, 0.8))),
            glow_box_shadow(colors.glow_muted, 0.03, 1.0, 3.0, glow_intensity),
        ),
        (UiButtonVariant::Secondary, UiInteractionState::Pressed) => (
            BackgroundColor(color(with_alpha(colors.accent, 0.82))),
            BorderColor::all(colors.glow_color()),
            no_box_shadow(),
        ),
        (UiButtonVariant::Secondary, UiInteractionState::Selected) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.22))),
            BorderColor::all(colors.glow_color()),
            no_box_shadow(),
        ),
        (UiButtonVariant::Secondary, UiInteractionState::Focused) => (
            BackgroundColor(color(with_alpha(colors.input, 0.96))),
            BorderColor::all(colors.ring_color()),
            no_box_shadow(),
        ),
        (UiButtonVariant::Outline, UiInteractionState::Idle) => (
            BackgroundColor(color(with_alpha(colors.panel, 0.5))),
            BorderColor::all(color(with_alpha(colors.border, 0.9))),
            no_box_shadow(),
        ),
        (UiButtonVariant::Outline, UiInteractionState::Hovered) => (
            BackgroundColor(color(with_alpha(colors.secondary, 0.42))),
            BorderColor::all(colors.ring_color()),
            glow_box_shadow(colors.glow, 0.032, 1.0, 3.0, glow_intensity),
        ),
        (UiButtonVariant::Outline, UiInteractionState::Pressed) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.2))),
            BorderColor::all(colors.glow_color()),
            no_box_shadow(),
        ),
        (UiButtonVariant::Outline, UiInteractionState::Selected) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.16))),
            BorderColor::all(colors.glow_color()),
            no_box_shadow(),
        ),
        (UiButtonVariant::Outline, UiInteractionState::Focused) => (
            BackgroundColor(color(with_alpha(colors.panel, 0.64))),
            BorderColor::all(colors.ring_color()),
            no_box_shadow(),
        ),
        (UiButtonVariant::Ghost, UiInteractionState::Idle) => (
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            no_box_shadow(),
        ),
        (UiButtonVariant::Ghost, UiInteractionState::Hovered) => (
            BackgroundColor(color(with_alpha(colors.secondary, 0.22))),
            BorderColor::all(Color::NONE),
            glow_box_shadow(colors.glow_muted, 0.022, 0.5, 2.5, glow_intensity),
        ),
        (UiButtonVariant::Ghost, UiInteractionState::Pressed) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.18))),
            BorderColor::all(Color::NONE),
            no_box_shadow(),
        ),
        (UiButtonVariant::Ghost, UiInteractionState::Selected) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.14))),
            BorderColor::all(Color::NONE),
            no_box_shadow(),
        ),
        (UiButtonVariant::Ghost, UiInteractionState::Focused) => (
            BackgroundColor(color(with_alpha(colors.secondary, 0.22))),
            BorderColor::all(Color::NONE),
            no_box_shadow(),
        ),
    }
}

pub fn button_surface_with_tone(
    theme: UiTheme,
    tone: UiSemanticTone,
    state: UiInteractionState,
    glow_intensity: f32,
) -> (BackgroundColor, BorderColor, BoxShadow) {
    let accent_token = tone.accent_token(theme);
    let accent_color = tone.accent_color(theme);
    let chrome_color = tone.chrome_color(theme);
    let alpha = match state {
        UiInteractionState::Idle => 0.74,
        UiInteractionState::Hovered => 0.84,
        UiInteractionState::Pressed => 0.96,
        UiInteractionState::Selected => 0.88,
        UiInteractionState::Focused => 0.8,
    };
    let glow_alpha = match state {
        UiInteractionState::Hovered | UiInteractionState::Focused => 0.052,
        UiInteractionState::Pressed => 0.028,
        UiInteractionState::Selected => 0.036,
        UiInteractionState::Idle => 0.0,
    };
    (
        BackgroundColor(accent_color.with_alpha(alpha)),
        BorderColor::all(chrome_color.with_alpha(0.96)),
        if glow_alpha > 0.0 {
            glow_box_shadow(accent_token, glow_alpha, 1.0, 3.5, glow_intensity)
        } else {
            no_box_shadow()
        },
    )
}

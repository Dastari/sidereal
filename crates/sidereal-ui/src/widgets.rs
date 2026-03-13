use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::theme::{UiTheme, color, with_alpha};

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

const HUD_FRAME_CORNER_SIZE_PX: f32 = 18.0;
const HUD_FRAME_CORNER_STROKE_PX: f32 = 2.0;
const HUD_FRAME_TITLE_LEFT_PX: f32 = 14.0;
const HUD_FRAME_TITLE_TOP_PX: f32 = -13.0;
const HUD_FRAME_SCANLINE_COUNT: usize = 14;
const HUD_FRAME_SCANLINE_INSET_PX: f32 = 8.0;

pub fn panel_surface(
    theme: UiTheme,
    glow_intensity: f32,
) -> (BackgroundColor, BorderColor, BoxShadow) {
    let colors = theme.colors;
    let shadow_blur_px = (theme.metrics.panel_shadow_blur_px * 0.72).max(16.0);
    (
        BackgroundColor(colors.panel_color()),
        BorderColor::all(color(with_alpha(colors.primary, 0.32))),
        glow_box_shadow(colors.glow_muted, 0.1, shadow_blur_px, glow_intensity),
    )
}

pub fn spawn_hud_frame_chrome(
    parent: &mut ChildSpawnerCommands,
    theme: UiTheme,
    title: Option<&str>,
    title_font: &Handle<Font>,
    glow_intensity: f32,
) {
    let corner_color = color(with_alpha(theme.colors.primary, 0.72));
    let title_bg = color(with_alpha(theme.colors.background, 0.98));
    let title_text = color(with_alpha(theme.colors.primary, 0.86));

    spawn_corner(
        parent,
        HUDCorner::TopLeft,
        HUD_FRAME_CORNER_SIZE_PX,
        HUD_FRAME_CORNER_STROKE_PX,
        corner_color,
    );
    spawn_corner(
        parent,
        HUDCorner::TopRight,
        HUD_FRAME_CORNER_SIZE_PX,
        HUD_FRAME_CORNER_STROKE_PX,
        corner_color,
    );
    spawn_corner(
        parent,
        HUDCorner::BottomLeft,
        HUD_FRAME_CORNER_SIZE_PX,
        HUD_FRAME_CORNER_STROKE_PX,
        corner_color,
    );
    spawn_corner(
        parent,
        HUDCorner::BottomRight,
        HUD_FRAME_CORNER_SIZE_PX,
        HUD_FRAME_CORNER_STROKE_PX,
        corner_color,
    );

    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(HUD_FRAME_SCANLINE_INSET_PX),
                right: Val::Px(HUD_FRAME_SCANLINE_INSET_PX),
                bottom: Val::Px(HUD_FRAME_SCANLINE_INSET_PX),
                left: Val::Px(HUD_FRAME_SCANLINE_INSET_PX),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(Color::NONE),
            FocusPolicy::Pass,
        ))
        .with_children(|scanlines| {
            for idx in 0..HUD_FRAME_SCANLINE_COUNT {
                let alpha = if idx % 2 == 0 { 0.05 } else { 0.025 };
                scanlines.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(color(with_alpha(theme.colors.glow_muted, alpha))),
                    FocusPolicy::Pass,
                ));
            }
        });

    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(22.0),
            right: Val::Px(22.0),
            height: Val::Px(1.0),
            ..default()
        },
        BackgroundColor(color(with_alpha(
            theme.colors.glow,
            glow_alpha(0.28, glow_intensity),
        ))),
        FocusPolicy::Pass,
    ));

    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.0),
            left: Val::Px(22.0),
            right: Val::Px(22.0),
            height: Val::Px(1.0),
            ..default()
        },
        BackgroundColor(color(with_alpha(
            theme.colors.glow_muted,
            glow_alpha(0.22, glow_intensity),
        ))),
        FocusPolicy::Pass,
    ));

    if let Some(title) = title {
        parent
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(HUD_FRAME_TITLE_TOP_PX),
                    left: Val::Px(HUD_FRAME_TITLE_LEFT_PX),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(title_bg),
                FocusPolicy::Pass,
            ))
            .with_children(|title_node| {
                title_node.spawn((
                    Text::new(title.to_ascii_uppercase()),
                    TextFont {
                        font: title_font.clone(),
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(title_text),
                    FocusPolicy::Pass,
                ));
            });
    }
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
            glow_box_shadow(colors.glow, 0.16, 14.0, glow_intensity),
        ),
        (UiButtonVariant::Primary, UiInteractionState::Hovered) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.34))),
            BorderColor::all(colors.ring_color()),
            glow_box_shadow(colors.glow, 0.22, 18.0, glow_intensity),
        ),
        (UiButtonVariant::Primary, UiInteractionState::Pressed) => (
            BackgroundColor(colors.primary_color()),
            BorderColor::all(colors.glow_color()),
            glow_box_shadow(colors.glow, 0.28, 20.0, glow_intensity),
        ),
        (UiButtonVariant::Primary, UiInteractionState::Selected) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.28))),
            BorderColor::all(colors.glow_color()),
            glow_box_shadow(colors.glow, 0.22, 18.0, glow_intensity),
        ),
        (UiButtonVariant::Primary, UiInteractionState::Focused) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.24))),
            BorderColor::all(colors.ring_color()),
            glow_box_shadow(colors.glow, 0.18, 16.0, glow_intensity),
        ),
        (UiButtonVariant::Secondary, UiInteractionState::Idle) => (
            BackgroundColor(color(with_alpha(colors.secondary, 0.65))),
            BorderColor::all(color(with_alpha(colors.border, 0.7))),
            glow_box_shadow(colors.glow_muted, 0.08, 10.0, glow_intensity),
        ),
        (UiButtonVariant::Secondary, UiInteractionState::Hovered) => (
            BackgroundColor(color(with_alpha(colors.secondary, 0.85))),
            BorderColor::all(color(with_alpha(colors.glow_muted, 0.8))),
            glow_box_shadow(colors.glow_muted, 0.14, 13.0, glow_intensity),
        ),
        (UiButtonVariant::Secondary, UiInteractionState::Pressed) => (
            BackgroundColor(color(with_alpha(colors.accent, 0.82))),
            BorderColor::all(colors.glow_color()),
            glow_box_shadow(colors.glow, 0.18, 15.0, glow_intensity),
        ),
        (UiButtonVariant::Secondary, UiInteractionState::Selected) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.22))),
            BorderColor::all(colors.glow_color()),
            glow_box_shadow(colors.glow, 0.16, 14.0, glow_intensity),
        ),
        (UiButtonVariant::Secondary, UiInteractionState::Focused) => (
            BackgroundColor(color(with_alpha(colors.input, 0.96))),
            BorderColor::all(colors.ring_color()),
            glow_box_shadow(colors.glow_muted, 0.14, 13.0, glow_intensity),
        ),
        (UiButtonVariant::Outline, UiInteractionState::Idle) => (
            BackgroundColor(color(with_alpha(colors.panel, 0.5))),
            BorderColor::all(color(with_alpha(colors.border, 0.9))),
            glow_box_shadow(colors.glow_muted, 0.05, 9.0, glow_intensity),
        ),
        (UiButtonVariant::Outline, UiInteractionState::Hovered) => (
            BackgroundColor(color(with_alpha(colors.secondary, 0.55))),
            BorderColor::all(colors.ring_color()),
            glow_box_shadow(colors.glow, 0.2, 14.0, glow_intensity),
        ),
        (UiButtonVariant::Outline, UiInteractionState::Pressed) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.2))),
            BorderColor::all(colors.glow_color()),
            glow_box_shadow(colors.glow, 0.16, 14.0, glow_intensity),
        ),
        (UiButtonVariant::Outline, UiInteractionState::Selected) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.16))),
            BorderColor::all(colors.glow_color()),
            glow_box_shadow(colors.glow, 0.14, 13.0, glow_intensity),
        ),
        (UiButtonVariant::Outline, UiInteractionState::Focused) => (
            BackgroundColor(color(with_alpha(colors.panel, 0.64))),
            BorderColor::all(colors.ring_color()),
            glow_box_shadow(colors.glow_muted, 0.12, 12.0, glow_intensity),
        ),
        (UiButtonVariant::Ghost, UiInteractionState::Idle) => (
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            no_box_shadow(),
        ),
        (UiButtonVariant::Ghost, UiInteractionState::Hovered) => (
            BackgroundColor(color(with_alpha(colors.secondary, 0.3))),
            BorderColor::all(Color::NONE),
            glow_box_shadow(colors.glow_muted, 0.08, 10.0, glow_intensity),
        ),
        (UiButtonVariant::Ghost, UiInteractionState::Pressed) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.18))),
            BorderColor::all(Color::NONE),
            glow_box_shadow(colors.glow, 0.1, 10.0, glow_intensity),
        ),
        (UiButtonVariant::Ghost, UiInteractionState::Selected) => (
            BackgroundColor(color(with_alpha(colors.primary, 0.14))),
            BorderColor::all(Color::NONE),
            glow_box_shadow(colors.glow, 0.08, 9.0, glow_intensity),
        ),
        (UiButtonVariant::Ghost, UiInteractionState::Focused) => (
            BackgroundColor(color(with_alpha(colors.secondary, 0.22))),
            BorderColor::all(Color::NONE),
            glow_box_shadow(colors.glow_muted, 0.07, 9.0, glow_intensity),
        ),
    }
}

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
            glow_box_shadow(colors.glow, 0.18, 16.0, glow_intensity),
        )
    } else {
        (
            BackgroundColor(color(with_alpha(colors.input, 0.88))),
            BorderColor::all(color(with_alpha(colors.border, 0.75))),
            glow_box_shadow(colors.glow_muted, 0.08, 11.0, glow_intensity),
        )
    }
}

fn glow_alpha(base_alpha: f32, glow_intensity: f32) -> f32 {
    (base_alpha * glow_intensity.max(0.0)).clamp(0.0, 1.0)
}

fn glow_box_shadow(
    color_token: bevy::color::Oklcha,
    base_alpha: f32,
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
        Val::Px(0.0),
        Val::Px(blur_px),
    )
}

fn no_box_shadow() -> BoxShadow {
    BoxShadow::new(
        Color::NONE,
        Val::Px(0.0),
        Val::Px(0.0),
        Val::Px(0.0),
        Val::Px(0.0),
    )
}

#[derive(Clone, Copy)]
enum HUDCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

fn spawn_corner(
    parent: &mut ChildSpawnerCommands,
    corner: HUDCorner,
    size_px: f32,
    stroke_px: f32,
    color: Color,
) {
    let mut node = Node {
        position_type: PositionType::Absolute,
        width: Val::Px(size_px),
        height: Val::Px(size_px),
        ..default()
    };
    match corner {
        HUDCorner::TopLeft => {
            node.left = Val::Px(-1.0);
            node.top = Val::Px(-1.0);
            node.border = UiRect::new(
                Val::Px(stroke_px),
                Val::Px(0.0),
                Val::Px(stroke_px),
                Val::Px(0.0),
            );
        }
        HUDCorner::TopRight => {
            node.right = Val::Px(-1.0);
            node.top = Val::Px(-1.0);
            node.border = UiRect::new(
                Val::Px(0.0),
                Val::Px(stroke_px),
                Val::Px(stroke_px),
                Val::Px(0.0),
            );
        }
        HUDCorner::BottomLeft => {
            node.left = Val::Px(-1.0);
            node.bottom = Val::Px(-1.0);
            node.border = UiRect::new(
                Val::Px(stroke_px),
                Val::Px(0.0),
                Val::Px(0.0),
                Val::Px(stroke_px),
            );
        }
        HUDCorner::BottomRight => {
            node.right = Val::Px(-1.0);
            node.bottom = Val::Px(-1.0);
            node.border = UiRect::new(
                Val::Px(0.0),
                Val::Px(stroke_px),
                Val::Px(0.0),
                Val::Px(stroke_px),
            );
        }
    }

    parent.spawn((node, BorderColor::all(color), FocusPolicy::Pass));
}

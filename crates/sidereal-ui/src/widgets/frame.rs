use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::theme::{UiSemanticTone, UiTheme, color, with_alpha};

use super::scanline::spawn_scanline_overlay;
use super::shadow::glow_alpha;

const HUD_FRAME_CORNER_SIZE_PX: f32 = 18.0;
const HUD_FRAME_CORNER_STROKE_PX: f32 = 2.0;
const HUD_FRAME_TITLE_LEFT_PX: f32 = 14.0;
const HUD_FRAME_TITLE_TOP_PX: f32 = -13.0;
const HUD_FRAME_SCANLINE_INSET_PX: f32 = 2.0;
const HUD_FRAME_SCANLINE_STRIDE_PX: f32 = 4.0;
const HUD_FRAME_SCANLINE_THICKNESS_PX: usize = 2;

#[derive(Clone, Copy)]
struct HudFrameChromeStyle {
    accent_color: Color,
    scanline_color: Color,
    scanline_edge_color: Color,
    title_bg: Color,
    title_text: Color,
}

pub fn spawn_hud_frame_chrome(
    parent: &mut ChildSpawnerCommands,
    images: &mut Assets<Image>,
    theme: UiTheme,
    title: Option<&str>,
    title_font: &Handle<Font>,
    glow_intensity: f32,
) {
    spawn_hud_frame_chrome_with_accent(
        parent,
        images,
        theme,
        title,
        title_font,
        glow_intensity,
        color(with_alpha(theme.colors.primary, 0.72)),
    );
}

pub fn spawn_hud_frame_chrome_with_accent(
    parent: &mut ChildSpawnerCommands,
    images: &mut Assets<Image>,
    theme: UiTheme,
    title: Option<&str>,
    title_font: &Handle<Font>,
    glow_intensity: f32,
    accent_color: Color,
) {
    let corner_color = accent_color.with_alpha(0.72);
    let title_bg = color(with_alpha(theme.colors.background, 0.98));
    let title_text = accent_color.with_alpha(0.86);

    spawn_hud_corner_frame(
        parent,
        corner_color,
        HUD_FRAME_CORNER_SIZE_PX,
        HUD_FRAME_CORNER_STROKE_PX,
    );
    spawn_scanline_overlay(
        parent,
        images,
        accent_color.with_alpha(0.003),
        accent_color.with_alpha(0.003),
        HUD_FRAME_SCANLINE_INSET_PX,
        HUD_FRAME_SCANLINE_STRIDE_PX,
        HUD_FRAME_SCANLINE_THICKNESS_PX,
    );

    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(22.0),
            right: Val::Px(22.0),
            height: Val::Px(1.0),
            ..default()
        },
        BackgroundColor(accent_color.with_alpha(glow_alpha(0.18, glow_intensity))),
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
        BackgroundColor(accent_color.with_alpha(glow_alpha(0.12, glow_intensity))),
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

pub fn spawn_hud_frame_chrome_with_tone(
    parent: &mut ChildSpawnerCommands,
    images: &mut Assets<Image>,
    theme: UiTheme,
    title: Option<&str>,
    title_font: &Handle<Font>,
    glow_intensity: f32,
    tone: UiSemanticTone,
) {
    let accent_color = tone.accent_color(theme);
    let chrome_color = tone.chrome_color(theme);
    spawn_hud_frame_chrome_inner(
        parent,
        images,
        title,
        title_font,
        glow_intensity,
        HudFrameChromeStyle {
            accent_color: chrome_color,
            scanline_color: chrome_color.with_alpha(0.024),
            scanline_edge_color: chrome_color.with_alpha(0.018),
            title_bg: match tone {
                UiSemanticTone::Info => color(with_alpha(theme.colors.background, 0.98)),
                UiSemanticTone::Success | UiSemanticTone::Warning | UiSemanticTone::Danger => {
                    accent_color.with_alpha(0.96)
                }
            },
            title_text: chrome_color.with_alpha(0.92),
        },
    );
}

fn spawn_hud_frame_chrome_inner(
    parent: &mut ChildSpawnerCommands,
    images: &mut Assets<Image>,
    title: Option<&str>,
    title_font: &Handle<Font>,
    glow_intensity: f32,
    style: HudFrameChromeStyle,
) {
    let corner_color = style.accent_color.with_alpha(0.72);

    spawn_hud_corner_frame(
        parent,
        corner_color,
        HUD_FRAME_CORNER_SIZE_PX,
        HUD_FRAME_CORNER_STROKE_PX,
    );
    spawn_scanline_overlay(
        parent,
        images,
        style.scanline_color,
        style.scanline_edge_color,
        HUD_FRAME_SCANLINE_INSET_PX,
        HUD_FRAME_SCANLINE_STRIDE_PX,
        HUD_FRAME_SCANLINE_THICKNESS_PX,
    );

    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(22.0),
            right: Val::Px(22.0),
            height: Val::Px(1.0),
            ..default()
        },
        BackgroundColor(
            style
                .accent_color
                .with_alpha(glow_alpha(0.18, glow_intensity)),
        ),
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
        BackgroundColor(
            style
                .accent_color
                .with_alpha(glow_alpha(0.12, glow_intensity)),
        ),
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
                BackgroundColor(style.title_bg),
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
                    TextColor(style.title_text),
                    FocusPolicy::Pass,
                ));
            });
    }
}

#[derive(Clone, Copy)]
enum HudCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

fn spawn_corner(
    parent: &mut ChildSpawnerCommands,
    corner: HudCorner,
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
        HudCorner::TopLeft => {
            node.left = Val::Px(-1.0);
            node.top = Val::Px(-1.0);
            node.border = UiRect::new(
                Val::Px(stroke_px),
                Val::Px(0.0),
                Val::Px(stroke_px),
                Val::Px(0.0),
            );
        }
        HudCorner::TopRight => {
            node.right = Val::Px(-1.0);
            node.top = Val::Px(-1.0);
            node.border = UiRect::new(
                Val::Px(0.0),
                Val::Px(stroke_px),
                Val::Px(stroke_px),
                Val::Px(0.0),
            );
        }
        HudCorner::BottomLeft => {
            node.left = Val::Px(-1.0);
            node.bottom = Val::Px(-1.0);
            node.border = UiRect::new(
                Val::Px(stroke_px),
                Val::Px(0.0),
                Val::Px(0.0),
                Val::Px(stroke_px),
            );
        }
        HudCorner::BottomRight => {
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

pub fn spawn_hud_corner_frame(
    parent: &mut ChildSpawnerCommands,
    color: Color,
    size_px: f32,
    stroke_px: f32,
) {
    spawn_corner(parent, HudCorner::TopLeft, size_px, stroke_px, color);
    spawn_corner(parent, HudCorner::TopRight, size_px, stroke_px, color);
    spawn_corner(parent, HudCorner::BottomLeft, size_px, stroke_px, color);
    spawn_corner(parent, HudCorner::BottomRight, size_px, stroke_px, color);
}

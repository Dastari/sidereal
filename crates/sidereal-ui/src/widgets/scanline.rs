use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

pub fn spawn_scanline_overlay(
    parent: &mut ChildSpawnerCommands,
    primary_line_color: Color,
    secondary_line_color: Color,
    line_count: usize,
    inset_px: f32,
) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(inset_px),
                right: Val::Px(inset_px),
                bottom: Val::Px(inset_px),
                left: Val::Px(inset_px),
                padding: UiRect::axes(Val::Px(0.0), Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(Color::NONE),
            FocusPolicy::Pass,
        ))
        .with_children(|scanlines| {
            for idx in 0..line_count {
                scanlines.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(if idx % 2 == 0 {
                        primary_line_color
                    } else {
                        secondary_line_color
                    }),
                    FocusPolicy::Pass,
                ));
            }
        });
}

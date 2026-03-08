//! Dedicated replication/session bind loading screen.

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;

use super::app_state::{ClientAppState, ClientSession};
use super::resources::EmbeddedFonts;

#[derive(Component)]
pub(super) struct WorldLoadingRoot;

#[derive(Component)]
pub(super) struct WorldLoadingStatusText;

#[derive(Component)]
pub(super) struct WorldLoadingHintText;

pub(super) fn setup_world_loading_screen(
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.98)),
            WorldLoadingRoot,
            DespawnOnExit(ClientAppState::WorldLoading),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(560.0),
                    padding: UiRect::all(Val::Px(24.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(14.0),
                    ..default()
                },
                Transform::default(),
                GlobalTransform::default(),
                BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.95)),
                BorderColor::all(Color::srgba(0.2, 0.3, 0.45, 0.8)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Entering World"),
                    TextFont {
                        font: fonts.bold.clone(),
                        font_size: 34.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                panel.spawn((
                    Text::new("Connecting to replication and waiting for your player entity."),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.84, 0.9, 1.0, 0.95)),
                    WorldLoadingHintText,
                ));
                panel.spawn((
                    Text::new(""),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.72, 0.81, 0.93, 0.95)),
                    WorldLoadingStatusText,
                ));
            });
        });
}

pub(super) fn update_world_loading_screen(
    session: Res<'_, ClientSession>,
    mut status_query: Query<'_, '_, &'_ mut Text, With<WorldLoadingStatusText>>,
) {
    let Ok(mut status_text) = status_query.single_mut() else {
        return;
    };
    status_text.0 = if session.status.trim().is_empty() {
        "Waiting for replication session bind...".to_string()
    } else {
        session.status.clone()
    };
}

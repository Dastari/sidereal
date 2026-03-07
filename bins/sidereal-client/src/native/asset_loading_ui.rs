//! Dedicated pre-world asset loading screen.

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;

use super::app_state::{ClientAppState, ClientSession};
use super::assets::LocalAssetManager;
use super::resources::EmbeddedFonts;

#[derive(Component)]
pub(super) struct AssetLoadingRoot;

#[derive(Component)]
pub(super) struct AssetLoadingText;

#[derive(Component)]
pub(super) struct AssetLoadingStatusText;

#[derive(Component)]
pub(super) struct AssetLoadingBarFill;

pub(super) fn setup_asset_loading_screen(
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
            BackgroundColor(Color::srgba(0.03, 0.05, 0.1, 0.96)),
            AssetLoadingRoot,
            DespawnOnExit(ClientAppState::AssetLoading),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(520.0),
                    padding: UiRect::all(Val::Px(24.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.1, 0.16, 0.9)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Preparing Assets"),
                    TextFont {
                        font: fonts.bold.clone(),
                        font_size: 34.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                panel.spawn((
                    Text::new("Waiting for bootstrap manifest..."),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.84, 0.9, 1.0, 0.95)),
                    AssetLoadingText,
                ));
                panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(18.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.12, 0.16, 0.24, 0.9)),
                        BorderColor::all(Color::srgba(0.8, 0.9, 1.0, 0.85)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.35, 0.84, 1.0)),
                            AssetLoadingBarFill,
                        ));
                    });
                panel.spawn((
                    Text::new(""),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.72, 0.81, 0.93, 0.95)),
                    AssetLoadingStatusText,
                ));
            });
        });
}

pub(super) fn update_asset_loading_screen(
    asset_manager: Res<'_, LocalAssetManager>,
    session: Res<'_, ClientSession>,
    mut loading_text_query: Query<
        '_,
        '_,
        &'_ mut Text,
        (With<AssetLoadingText>, Without<AssetLoadingStatusText>),
    >,
    mut bar_query: Query<'_, '_, &'_ mut Node, With<AssetLoadingBarFill>>,
    mut status_query: Query<
        '_,
        '_,
        &'_ mut Text,
        (With<AssetLoadingStatusText>, Without<AssetLoadingText>),
    >,
) {
    let Ok(mut loading_text) = loading_text_query.single_mut() else {
        return;
    };
    let Ok(mut bar_node) = bar_query.single_mut() else {
        return;
    };
    let Ok(mut status_text) = status_query.single_mut() else {
        return;
    };
    let pct = (asset_manager.bootstrap_progress() * 100.0)
        .round()
        .clamp(0.0, 100.0);
    bar_node.width = Val::Percent(pct);
    loading_text.0 = if asset_manager.bootstrap_manifest_seen {
        format!("Loading required assets... {}%", pct as i32)
    } else {
        "Waiting for bootstrap manifest...".to_string()
    };
    status_text.0 = session.status.clone();
}

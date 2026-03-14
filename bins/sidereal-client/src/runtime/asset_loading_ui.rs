//! Dedicated pre-world asset loading screen.

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use sidereal_ui::layout;
use sidereal_ui::theme::{ActiveUiTheme, UiVisualSettings, theme_definition};
use sidereal_ui::typography::text_font;
use sidereal_ui::widgets::{panel_surface, spawn_hud_frame_chrome};

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
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    let (panel_bg, panel_border, panel_shadow) = panel_surface(theme, glow_intensity);
    commands
        .spawn((
            layout::fullscreen_centered_root(),
            BackgroundColor(theme.colors.background_color()),
            AssetLoadingRoot,
            DespawnOnExit(ClientAppState::AssetLoading),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    ..layout::panel(
                        Val::Px(520.0),
                        theme.metrics.panel_padding_px,
                        12.0,
                        theme.metrics.panel_radius_px,
                        theme.metrics.panel_border_px,
                    )
                },
                panel_bg,
                panel_border,
                panel_shadow,
            ))
            .with_children(|panel| {
                spawn_hud_frame_chrome(
                    panel,
                    theme,
                    Some("Asset Bootstrap"),
                    &fonts.mono,
                    glow_intensity,
                );

                panel.spawn((
                    Text::new("Preparing Assets"),
                    text_font(fonts.display.clone(), 34.0),
                    TextColor(theme.colors.foreground_color()),
                ));
                panel.spawn((
                    Text::new("Waiting for bootstrap manifest..."),
                    text_font(fonts.regular.clone(), 18.0),
                    TextColor(theme.colors.muted_foreground_color()),
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
                        BackgroundColor(theme.colors.input_color().with_alpha(0.9)),
                        BorderColor::all(theme.colors.border_color()),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(theme.colors.primary_color()),
                            AssetLoadingBarFill,
                        ));
                    });
                panel.spawn((
                    Text::new(""),
                    text_font(fonts.mono.clone(), 14.0),
                    TextColor(theme.colors.muted_foreground_color()),
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

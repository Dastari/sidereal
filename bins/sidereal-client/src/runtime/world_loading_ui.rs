//! Dedicated replication/session bind loading screen.

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use sidereal_ui::layout;
use sidereal_ui::theme::{ActiveUiTheme, UiVisualSettings, theme_definition};
use sidereal_ui::typography::text_font;
use sidereal_ui::widgets::{panel_surface, spawn_hud_frame_chrome};

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
    mut images: ResMut<'_, Assets<Image>>,
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
            WorldLoadingRoot,
            DespawnOnExit(ClientAppState::WorldLoading),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    ..layout::panel(
                        Val::Px(560.0),
                        theme.metrics.panel_padding_px,
                        theme.metrics.row_gap_px,
                        theme.metrics.panel_radius_px,
                        theme.metrics.panel_border_px,
                    )
                },
                Transform::default(),
                GlobalTransform::default(),
                panel_bg,
                panel_border,
                panel_shadow,
            ))
            .with_children(|panel| {
                spawn_hud_frame_chrome(
                    panel,
                    &mut images,
                    theme,
                    Some("World Bootstrap"),
                    &fonts.mono,
                    glow_intensity,
                );

                panel.spawn((
                    Text::new("Entering World"),
                    text_font(fonts.display.clone(), 34.0),
                    TextColor(theme.colors.foreground_color()),
                ));
                panel.spawn((
                    Text::new("Connecting to replication and waiting for your player entity."),
                    text_font(fonts.regular.clone(), 18.0),
                    TextColor(theme.colors.muted_foreground_color()),
                    WorldLoadingHintText,
                ));
                panel.spawn((
                    Text::new(""),
                    text_font(fonts.mono.clone(), 14.0),
                    TextColor(theme.colors.muted_foreground_color()),
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

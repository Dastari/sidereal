//! In-world Escape menu (Quit / Disconnect / Settings placeholder).

use bevy::app::AppExit;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::log::info;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};
use sidereal_ui::layout;
use sidereal_ui::theme::{ActiveUiTheme, UiVisualSettings, theme_definition};
use sidereal_ui::typography::text_font;
use sidereal_ui::widgets::{
    UiButtonVariant, UiInteractionState, button_surface, panel_surface, spawn_hud_frame_chrome,
};

use super::app_state::ClientAppState;
use super::dev_console::{DevConsoleState, is_console_open};
use super::ecs_util::queue_despawn_if_exists_force;
use super::resources::{DisconnectRequest, PauseMenuState};

#[derive(Component)]
pub(super) struct PauseMenuRoot;

#[derive(Component, Clone, Copy)]
enum PauseMenuAction {
    ToggleFullscreen,
    Disconnect,
    Quit,
    Settings,
}

#[derive(Component)]
pub(super) struct PauseMenuButton(PauseMenuAction);

pub(super) fn toggle_pause_menu_system(
    keys: Res<'_, ButtonInput<KeyCode>>,
    mut key_events: MessageReader<'_, '_, KeyboardInput>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    mut pause_menu_state: ResMut<'_, PauseMenuState>,
) {
    if is_console_open(dev_console_state.as_deref()) {
        return;
    }
    let escape_event_pressed = key_events.read().any(|event| {
        event.state == ButtonState::Pressed && matches!(event.logical_key, Key::Escape)
    });
    if keys.just_pressed(KeyCode::Escape) || escape_event_pressed {
        pause_menu_state.open = !pause_menu_state.open;
    }
}

pub(super) fn sync_pause_menu_ui_system(
    mut commands: Commands<'_, '_>,
    mut images: ResMut<'_, Assets<Image>>,
    fonts: Res<'_, super::EmbeddedFonts>,
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    pause_menu_state: Res<'_, PauseMenuState>,
    existing: Query<'_, '_, Entity, With<PauseMenuRoot>>,
) {
    if pause_menu_state.open {
        if !existing.is_empty() {
            return;
        }
        let theme = theme_definition(active_theme.0);
        let glow_intensity = visual_settings.glow_intensity();
        let (panel_bg, panel_border, panel_shadow) = panel_surface(theme, glow_intensity);
        commands
            .spawn((
                layout::fullscreen_centered_root(),
                Transform::default(),
                GlobalTransform::default(),
                BackgroundColor(theme.colors.overlay_color()),
                ZIndex(900),
                PauseMenuRoot,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .with_children(|root| {
                root.spawn((
                    Node {
                        max_width: percent(90.0),
                        ..layout::panel(
                            px(420.0),
                            theme.metrics.panel_padding_px,
                            12.0,
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
                        Some("Pause Menu"),
                        &fonts.mono,
                        glow_intensity,
                    );
                    panel.spawn((
                        Text::new("Menu"),
                        text_font(fonts.display.clone(), 28.0),
                        TextColor(theme.colors.foreground_color()),
                    ));
                    spawn_menu_button(
                        panel,
                        &fonts.mono_bold,
                        theme,
                        glow_intensity,
                        "Toggle Fullscreen",
                        PauseMenuAction::ToggleFullscreen,
                    );
                    spawn_menu_button(
                        panel,
                        &fonts.mono_bold,
                        theme,
                        glow_intensity,
                        "Disconnect",
                        PauseMenuAction::Disconnect,
                    );
                    spawn_menu_button(
                        panel,
                        &fonts.mono_bold,
                        theme,
                        glow_intensity,
                        "Settings",
                        PauseMenuAction::Settings,
                    );
                    spawn_menu_button(
                        panel,
                        &fonts.mono_bold,
                        theme,
                        glow_intensity,
                        "Quit",
                        PauseMenuAction::Quit,
                    );
                });
            });
        return;
    }

    for entity in &existing {
        queue_despawn_if_exists_force(&mut commands, entity);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn handle_pause_menu_interactions_system(
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            &PauseMenuButton,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut BoxShadow,
        ),
        Changed<Interaction>,
    >,
    mut pause_menu_state: ResMut<'_, PauseMenuState>,
    mut disconnect_request: ResMut<'_, DisconnectRequest>,
    mut app_exit: MessageWriter<'_, AppExit>,
    mut primary_window: Query<'_, '_, &'_ mut Window, With<PrimaryWindow>>,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    for (interaction, button, mut bg, mut border, mut shadow) in &mut interactions {
        match *interaction {
            Interaction::Pressed => match button.0 {
                PauseMenuAction::ToggleFullscreen => {
                    if let Ok(mut window) = primary_window.single_mut() {
                        window.mode = match window.mode {
                            WindowMode::Windowed => {
                                WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                            }
                            WindowMode::BorderlessFullscreen(_) | WindowMode::Fullscreen(_, _) => {
                                WindowMode::Windowed
                            }
                        };
                    }
                }
                PauseMenuAction::Disconnect => {
                    disconnect_request.0 = true;
                    pause_menu_state.open = false;
                }
                PauseMenuAction::Quit => {
                    pause_menu_state.open = false;
                    app_exit.write(AppExit::Success);
                }
                PauseMenuAction::Settings => {
                    info!("pause menu settings selected (not implemented)");
                }
            },
            Interaction::Hovered | Interaction::None => {}
        }
        let state = match *interaction {
            Interaction::Pressed => UiInteractionState::Pressed,
            Interaction::Hovered => UiInteractionState::Hovered,
            Interaction::None => UiInteractionState::Idle,
        };
        let (next_bg, next_border, next_shadow) =
            button_surface(theme, UiButtonVariant::Outline, state, glow_intensity);
        *bg = next_bg;
        *border = next_border;
        *shadow = next_shadow;
    }
}

fn spawn_menu_button(
    parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands<'_>,
    font: &Handle<Font>,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
    label: &str,
    action: PauseMenuAction,
) {
    let (bg, border, shadow) = button_surface(
        theme,
        UiButtonVariant::Outline,
        UiInteractionState::Idle,
        glow_intensity,
    );
    parent
        .spawn((
            Button,
            PauseMenuButton(action),
            layout::button(
                percent(100.0),
                44.0,
                theme.metrics.input_radius_px,
                theme.metrics.control_border_px,
            ),
            Transform::default(),
            GlobalTransform::default(),
            bg,
            border,
            shadow,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label.to_ascii_uppercase()),
                text_font(font.clone(), 17.0),
                TextColor(theme.colors.panel_foreground_color()),
            ));
        });
}

//! In-world Escape menu (Quit / Disconnect / Settings placeholder).

use bevy::app::AppExit;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::log::info;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};

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
    fonts: Res<'_, super::EmbeddedFonts>,
    pause_menu_state: Res<'_, PauseMenuState>,
    existing: Query<'_, '_, Entity, With<PauseMenuRoot>>,
) {
    if pause_menu_state.open {
        if !existing.is_empty() {
            return;
        }
        let font_bold = fonts.bold.clone();
        let font_regular = fonts.regular.clone();
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.0),
                    height: percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Transform::default(),
                GlobalTransform::default(),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                ZIndex(900),
                PauseMenuRoot,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .with_children(|root| {
                root.spawn((
                    Node {
                        width: px(420.0),
                        max_width: percent(90.0),
                        padding: UiRect::all(px(28.0)),
                        border: UiRect::all(px(2.0)),
                        border_radius: BorderRadius::all(px(12.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(12.0),
                        ..default()
                    },
                    Transform::default(),
                    GlobalTransform::default(),
                    BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.96)),
                    BorderColor::all(Color::srgba(0.3, 0.5, 0.7, 0.8)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Menu"),
                        TextFont {
                            font: font_bold,
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.6, 0.8, 1.0)),
                    ));
                    spawn_menu_button(
                        panel,
                        &font_regular,
                        "Toggle Fullscreen",
                        PauseMenuAction::ToggleFullscreen,
                    );
                    spawn_menu_button(
                        panel,
                        &font_regular,
                        "Disconnect",
                        PauseMenuAction::Disconnect,
                    );
                    spawn_menu_button(panel, &font_regular, "Settings", PauseMenuAction::Settings);
                    spawn_menu_button(panel, &font_regular, "Quit", PauseMenuAction::Quit);
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
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            &PauseMenuButton,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Changed<Interaction>,
    >,
    mut pause_menu_state: ResMut<'_, PauseMenuState>,
    mut disconnect_request: ResMut<'_, DisconnectRequest>,
    mut app_exit: MessageWriter<'_, AppExit>,
    mut primary_window: Query<'_, '_, &'_ mut Window, With<PrimaryWindow>>,
) {
    for (interaction, button, mut bg, mut border) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                match button.0 {
                    PauseMenuAction::ToggleFullscreen => {
                        if let Ok(mut window) = primary_window.single_mut() {
                            window.mode = match window.mode {
                                WindowMode::Windowed => {
                                    WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                                }
                                WindowMode::BorderlessFullscreen(_)
                                | WindowMode::Fullscreen(_, _) => WindowMode::Windowed,
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
                }
                *bg = BackgroundColor(Color::srgba(0.2, 0.25, 0.35, 0.9));
                *border = BorderColor::all(Color::srgba(0.4, 0.5, 0.65, 1.0));
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(Color::srgba(0.2, 0.25, 0.35, 0.9));
                *border = BorderColor::all(Color::srgba(0.4, 0.5, 0.65, 1.0));
            }
            Interaction::None => {
                *bg = BackgroundColor(Color::srgba(0.15, 0.2, 0.3, 0.9));
                *border = BorderColor::all(Color::srgba(0.3, 0.4, 0.55, 0.9));
            }
        }
    }
}

fn spawn_menu_button(
    parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands<'_>,
    font_regular: &Handle<Font>,
    label: &str,
    action: PauseMenuAction,
) {
    parent
        .spawn((
            Button,
            PauseMenuButton(action),
            Node {
                width: percent(100.0),
                height: px(44.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(6.0)),
                ..default()
            },
            Transform::default(),
            GlobalTransform::default(),
            BackgroundColor(Color::srgba(0.15, 0.2, 0.3, 0.9)),
            BorderColor::all(Color::srgba(0.3, 0.4, 0.55, 0.9)),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: font_regular.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.92, 1.0)),
            ));
        });
}

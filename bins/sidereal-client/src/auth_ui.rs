use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;

use crate::{
    AssetRootPath, AuthAction, ClientAppState, ClientSession, FocusField, active_field_mut,
    dialog_ui, is_printable_char, mask, submit_auth_request,
};

#[derive(Component)]
struct AuthUiRoot;

#[derive(Component)]
struct AuthUiBackdrop;

#[derive(Component)]
struct AuthUiStatusText;

#[derive(Component)]
struct AuthUiFlowTitle;

#[derive(Component)]
struct AuthUiSubmitLabel;

#[derive(Component)]
struct AuthUiFieldContainer {
    field: FocusField,
}

#[derive(Component)]
struct AuthUiInputBox {
    field: FocusField,
}

#[derive(Component)]
struct AuthUiInputText {
    field: FocusField,
    is_password: bool,
}

#[derive(Component)]
struct AuthUiCursor {
    field: FocusField,
}

#[derive(Component)]
struct AuthUiButton(AuthButtonKind);

#[derive(Clone, Copy)]
enum AuthButtonKind {
    Submit,
    SwitchFlow(AuthAction),
    Focus(FocusField),
}

#[derive(Resource)]
struct CursorBlink {
    timer: Timer,
    visible: bool,
}

impl Default for CursorBlink {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            visible: true,
        }
    }
}

pub fn register_auth_ui(app: &mut App) {
    app.init_resource::<CursorBlink>();
    app.add_systems(OnEnter(ClientAppState::Auth), setup_auth_screen);
    app.add_systems(
        Update,
        (
            animate_auth_background,
            tick_cursor_blink,
            handle_auth_keyboard_input,
            handle_auth_button_interactions,
            update_auth_text,
            update_auth_field_layout,
            update_auth_field_content,
        )
            .run_if(in_state(ClientAppState::Auth)),
    );
}

fn setup_auth_screen(mut commands: Commands<'_, '_>, fonts: Res<'_, crate::EmbeddedFonts>) {
    let font_bold = fonts.bold.clone();
    let font_regular = fonts.regular.clone();

    commands.spawn((Camera2d, DespawnOnExit(ClientAppState::Auth)));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            AuthUiRoot,
            DespawnOnExit(ClientAppState::Auth),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.03, 0.04, 0.08)),
                AuthUiBackdrop,
            ));

            root.spawn((
                Node {
                    width: Val::Px(540.0),
                    padding: UiRect::all(Val::Px(30.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(14.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.92)),
                BorderColor::all(Color::srgba(0.2, 0.3, 0.45, 0.8)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("SIDEREAL"),
                    TextFont {
                        font: font_bold.clone(),
                        font_size: 42.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.92, 1.0)),
                ));

                panel.spawn((
                    Text::new("Login"),
                    TextFont {
                        font: font_regular.clone(),
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.76, 0.82, 0.92, 0.95)),
                    AuthUiFlowTitle,
                ));

                spawn_input_field(panel, &font_regular, "Email", FocusField::Email, false);
                spawn_input_field(panel, &font_regular, "Password", FocusField::Password, true);
                spawn_input_field(
                    panel,
                    &font_regular,
                    "Reset Token",
                    FocusField::ResetToken,
                    false,
                );
                spawn_input_field(
                    panel,
                    &font_regular,
                    "New Password",
                    FocusField::NewPassword,
                    true,
                );

                panel
                    .spawn((
                        Button,
                        AuthUiButton(AuthButtonKind::Submit),
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(46.0),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.2, 0.46, 0.85)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Login"),
                            TextFont {
                                font: font_bold.clone(),
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            AuthUiSubmitLabel,
                        ));
                    });

                panel
                    .spawn((Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },))
                    .with_children(|row| {
                        spawn_flow_button(row, &font_regular, "Login", AuthAction::Login);
                        spawn_flow_button(row, &font_regular, "Register", AuthAction::Register);
                        spawn_flow_button(
                            row,
                            &font_regular,
                            "Forgot Request",
                            AuthAction::ForgotRequest,
                        );
                        spawn_flow_button(
                            row,
                            &font_regular,
                            "Forgot Confirm",
                            AuthAction::ForgotConfirm,
                        );
                    });

                panel.spawn((
                    Text::new(""),
                    TextFont {
                        font: font_regular,
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.72, 0.84, 0.75, 0.95)),
                    AuthUiStatusText,
                ));
            });
        });
}

fn spawn_input_field(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: &str,
    field: FocusField,
    is_password: bool,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            AuthUiFieldContainer { field },
        ))
        .with_children(|container| {
            container.spawn((
                Text::new(label),
                TextFont {
                    font: font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgba(0.72, 0.79, 0.88, 0.95)),
            ));

            container
                .spawn((
                    Button,
                    AuthUiInputBox { field },
                    AuthUiButton(AuthButtonKind::Focus(field)),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(42.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                        justify_content: JustifyContent::FlexStart,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(7.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.09, 0.11, 0.16, 0.95)),
                    BorderColor::all(Color::srgba(0.24, 0.28, 0.35, 0.9)),
                ))
                .with_children(|input_box| {
                    input_box.spawn((
                        Text::new(""),
                        TextFont {
                            font: font.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.93, 0.98)),
                        AuthUiInputText { field, is_password },
                    ));

                    input_box.spawn((
                        Text::new("|"),
                        TextFont {
                            font: font.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.93, 0.98)),
                        AuthUiCursor { field },
                        Visibility::Hidden,
                    ));
                });
        });
}

fn spawn_flow_button(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: &str,
    action: AuthAction,
) {
    parent
        .spawn((
            Button,
            AuthUiButton(AuthButtonKind::SwitchFlow(action)),
            Node {
                height: Val::Px(34.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.18, 0.2, 0.26, 0.85)),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: font.clone(),
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgba(0.83, 0.89, 0.95, 0.95)),
            ));
        });
}

fn animate_auth_background(
    time: Res<'_, Time>,
    mut bg_query: Query<'_, '_, &mut BackgroundColor, With<AuthUiBackdrop>>,
) {
    let t = time.elapsed_secs();
    let pulse = 0.03 + 0.015 * (t * 0.5).sin().abs();
    for mut color in &mut bg_query {
        *color = BackgroundColor(Color::srgb(pulse, pulse * 1.2, pulse * 1.8));
    }
}

fn tick_cursor_blink(time: Res<'_, Time>, mut blink: ResMut<'_, CursorBlink>) {
    blink.timer.tick(time.delta());
    if blink.timer.just_finished() {
        blink.visible = !blink.visible;
    }
}

fn handle_auth_keyboard_input(
    mut keyboard_input_reader: MessageReader<'_, '_, KeyboardInput>,
    keys: Res<'_, ButtonInput<KeyCode>>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
    mut session: ResMut<'_, ClientSession>,
    mut dialog_queue: ResMut<'_, dialog_ui::DialogQueue>,
    asset_root: Res<'_, AssetRootPath>,
) {
    let mut submit = false;
    for event in keyboard_input_reader.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }

        match &event.logical_key {
            Key::F1 => {
                session.selected_action = AuthAction::Login;
                session.focus = FocusField::Email;
                session.ui_dirty = true;
            }
            Key::F2 => {
                session.selected_action = AuthAction::Register;
                session.focus = FocusField::Email;
                session.ui_dirty = true;
            }
            Key::F3 => {
                session.selected_action = AuthAction::ForgotRequest;
                session.focus = FocusField::Email;
                session.ui_dirty = true;
            }
            Key::F4 => {
                session.selected_action = AuthAction::ForgotConfirm;
                session.focus = FocusField::ResetToken;
                session.ui_dirty = true;
            }
            Key::Tab => {
                session.focus = next_focus_field(session.selected_action, session.focus);
                session.ui_dirty = true;
            }
            Key::Enter => {
                submit = true;
            }
            Key::Backspace => {
                active_field_mut(&mut session).pop();
                session.ui_dirty = true;
            }
            _ => {
                if let Some(inserted_text) = &event.text
                    && inserted_text.chars().all(is_printable_char)
                {
                    active_field_mut(&mut session).push_str(inserted_text);
                    session.ui_dirty = true;
                }
            }
        }
    }

    if keys.just_pressed(KeyCode::Enter) {
        submit = true;
    }

    if submit {
        submit_auth_request(
            &mut session,
            &mut next_state,
            &mut dialog_queue,
            &asset_root,
        );
    }
}

fn handle_auth_button_interactions(
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            &AuthUiButton,
            &mut BackgroundColor,
            Option<&AuthUiInputBox>,
        ),
        Changed<Interaction>,
    >,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
    mut session: ResMut<'_, ClientSession>,
    mut dialog_queue: ResMut<'_, dialog_ui::DialogQueue>,
    asset_root: Res<'_, AssetRootPath>,
) {
    for (interaction, button, mut bg, input_box) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                if let Some(input) = input_box {
                    session.focus = input.field;
                    session.ui_dirty = true;
                    *bg = BackgroundColor(Color::srgba(0.12, 0.15, 0.21, 0.98));
                    continue;
                }

                match button.0 {
                    AuthButtonKind::Submit => {
                        *bg = BackgroundColor(Color::srgb(0.16, 0.38, 0.74));
                        submit_auth_request(
                            &mut session,
                            &mut next_state,
                            &mut dialog_queue,
                            &asset_root,
                        );
                    }
                    AuthButtonKind::SwitchFlow(action) => {
                        session.selected_action = action;
                        session.focus = first_focus_field(action);
                        session.ui_dirty = true;
                        *bg = BackgroundColor(Color::srgba(0.25, 0.28, 0.36, 0.9));
                    }
                    AuthButtonKind::Focus(field) => {
                        session.focus = field;
                        session.ui_dirty = true;
                        *bg = BackgroundColor(Color::srgba(0.12, 0.15, 0.21, 0.98));
                    }
                }
            }
            Interaction::Hovered => {
                if input_box.is_some() {
                    *bg = BackgroundColor(Color::srgba(0.11, 0.13, 0.2, 0.96));
                } else {
                    *bg = match button.0 {
                        AuthButtonKind::Submit => BackgroundColor(Color::srgb(0.24, 0.5, 0.9)),
                        AuthButtonKind::SwitchFlow(_) => {
                            BackgroundColor(Color::srgba(0.22, 0.25, 0.32, 0.88))
                        }
                        AuthButtonKind::Focus(_) => {
                            BackgroundColor(Color::srgba(0.11, 0.13, 0.2, 0.96))
                        }
                    };
                }
            }
            Interaction::None => {
                if input_box.is_some() {
                    *bg = BackgroundColor(Color::srgba(0.09, 0.11, 0.16, 0.95));
                } else {
                    *bg = match button.0 {
                        AuthButtonKind::Submit => BackgroundColor(Color::srgb(0.2, 0.46, 0.85)),
                        AuthButtonKind::SwitchFlow(_) => {
                            BackgroundColor(Color::srgba(0.18, 0.2, 0.26, 0.85))
                        }
                        AuthButtonKind::Focus(_) => {
                            BackgroundColor(Color::srgba(0.09, 0.11, 0.16, 0.95))
                        }
                    };
                }
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_auth_text(
    session: Res<'_, ClientSession>,
    mut text_sets: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, (&mut Text, &mut TextColor), With<AuthUiStatusText>>,
            Query<'_, '_, &mut Text, With<AuthUiFlowTitle>>,
            Query<'_, '_, &mut Text, With<AuthUiSubmitLabel>>,
        ),
    >,
) {
    let flow_title = flow_title(session.selected_action);

    for mut text in &mut text_sets.p1() {
        text.0 = flow_title.to_string();
    }

    let submit_label = submit_label(session.selected_action);
    for mut text in &mut text_sets.p2() {
        text.0 = submit_label.to_string();
    }

    for (mut text, mut color) in &mut text_sets.p0() {
        text.0 = session.status.clone();
        *color =
            if session.status.starts_with("Request failed") || session.status.contains("failed") {
                TextColor(Color::srgb(0.92, 0.46, 0.46))
            } else {
                TextColor(Color::srgba(0.72, 0.84, 0.75, 0.95))
            };
    }
}

fn update_auth_field_layout(
    session: Res<'_, ClientSession>,
    mut field_containers: Query<'_, '_, (&AuthUiFieldContainer, &mut Visibility)>,
    mut input_boxes: Query<'_, '_, (&AuthUiInputBox, &mut BorderColor)>,
) {
    for (container, mut visibility) in &mut field_containers {
        *visibility = if is_field_visible(session.selected_action, container.field) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (input_box, mut border) in &mut input_boxes {
        let focused = session.focus == input_box.field
            && is_field_visible(session.selected_action, input_box.field);
        *border = if focused {
            BorderColor::all(Color::srgb(0.25, 0.54, 0.92))
        } else {
            BorderColor::all(Color::srgba(0.24, 0.28, 0.35, 0.9))
        };
    }
}

fn update_auth_field_content(
    session: Res<'_, ClientSession>,
    blink: Res<'_, CursorBlink>,
    mut input_text_query: Query<'_, '_, (&AuthUiInputText, &mut Text)>,
    mut cursor_query: Query<'_, '_, (&AuthUiCursor, &mut Visibility)>,
) {
    for (input, mut text) in &mut input_text_query {
        let value = match input.field {
            FocusField::Email => session.email.as_str(),
            FocusField::Password => session.password.as_str(),
            FocusField::ResetToken => session.reset_token.as_str(),
            FocusField::NewPassword => session.new_password.as_str(),
        };

        text.0 = if input.is_password {
            mask(value)
        } else {
            value.to_string()
        };
    }

    for (cursor, mut visibility) in &mut cursor_query {
        let visible = blink.visible
            && session.focus == cursor.field
            && is_field_visible(session.selected_action, cursor.field);
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn flow_title(action: AuthAction) -> &'static str {
    match action {
        AuthAction::Login => "Login",
        AuthAction::Register => "Register",
        AuthAction::ForgotRequest => "Request Password Reset",
        AuthAction::ForgotConfirm => "Confirm Password Reset",
    }
}

fn submit_label(action: AuthAction) -> &'static str {
    match action {
        AuthAction::Login => "Login",
        AuthAction::Register => "Create Account",
        AuthAction::ForgotRequest => "Request Reset Token",
        AuthAction::ForgotConfirm => "Set New Password",
    }
}

fn is_field_visible(action: AuthAction, field: FocusField) -> bool {
    match action {
        AuthAction::Login | AuthAction::Register => {
            matches!(field, FocusField::Email | FocusField::Password)
        }
        AuthAction::ForgotRequest => matches!(field, FocusField::Email),
        AuthAction::ForgotConfirm => {
            matches!(field, FocusField::ResetToken | FocusField::NewPassword)
        }
    }
}

fn first_focus_field(action: AuthAction) -> FocusField {
    match action {
        AuthAction::Login | AuthAction::Register | AuthAction::ForgotRequest => FocusField::Email,
        AuthAction::ForgotConfirm => FocusField::ResetToken,
    }
}

fn next_focus_field(action: AuthAction, current: FocusField) -> FocusField {
    match action {
        AuthAction::Login | AuthAction::Register => match current {
            FocusField::Email => FocusField::Password,
            _ => FocusField::Email,
        },
        AuthAction::ForgotRequest => FocusField::Email,
        AuthAction::ForgotConfirm => match current {
            FocusField::ResetToken => FocusField::NewPassword,
            _ => FocusField::ResetToken,
        },
    }
}

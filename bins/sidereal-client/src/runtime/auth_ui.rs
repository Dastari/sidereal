use bevy::app::AppExit;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::log::info;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use sidereal_ui::layout;
use sidereal_ui::theme::{ActiveUiTheme, UiVisualSettings, theme_definition};
use sidereal_ui::typography::text_font;
use sidereal_ui::widgets::{
    UiButtonVariant, UiInteractionState, button_surface, input_surface, panel_surface,
    spawn_hud_frame_chrome,
};

use super::dev_console::DevConsoleState;
use super::resources::GatewayHttpAdapter;
use super::{
    AuthAction, ClientAppState, ClientSession, EmbeddedFonts, FocusField, submit_auth_request,
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
    Quit,
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
            sync_auth_button_visuals,
            update_auth_text,
            update_auth_field_layout,
            update_auth_field_content,
        )
            .run_if(in_state(ClientAppState::Auth)),
    );
}

fn setup_auth_screen(
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
) {
    info!("client auth UI setup: spawning auth screen");
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    let (panel_bg, panel_border, panel_shadow) = panel_surface(theme, glow_intensity);
    let (submit_bg, submit_border, submit_shadow) = button_surface(
        theme,
        UiButtonVariant::Primary,
        UiInteractionState::Idle,
        glow_intensity,
    );
    let (quit_bg, quit_border, quit_shadow) = button_surface(
        theme,
        UiButtonVariant::Outline,
        UiInteractionState::Idle,
        glow_intensity,
    );

    commands
        .spawn((
            layout::fullscreen_centered_root(),
            Transform::default(),
            GlobalTransform::default(),
            AuthUiRoot,
            DespawnOnExit(ClientAppState::Auth),
        ))
        .with_children(|root| {
            root.spawn((
                layout::fullscreen_backdrop(),
                Transform::default(),
                GlobalTransform::default(),
                BackgroundColor(theme.colors.background_color()),
                AuthUiBackdrop,
            ));

            root.spawn((
                layout::panel(
                    Val::Px(560.0),
                    theme.metrics.panel_padding_px,
                    theme.metrics.row_gap_px,
                    theme.metrics.panel_radius_px,
                    theme.metrics.panel_border_px,
                ),
                Transform::default(),
                GlobalTransform::default(),
                panel_bg,
                panel_border,
                panel_shadow,
            ))
            .with_children(|panel| {
                spawn_hud_frame_chrome(
                    panel,
                    theme,
                    Some("Auth Terminal"),
                    &fonts.mono.clone(),
                    glow_intensity,
                );

                panel.spawn((
                    Text::new("SIDEREAL"),
                    text_font(fonts.display.clone(), 42.0),
                    TextColor(theme.colors.foreground_color()),
                ));

                panel.spawn((
                    Text::new("Login"),
                    text_font(fonts.mono.clone(), 12.0),
                    TextColor(theme.colors.muted_foreground_color()),
                    AuthUiFlowTitle,
                ));

                spawn_input_field(
                    panel,
                    &fonts,
                    theme,
                    glow_intensity,
                    "Email",
                    FocusField::Email,
                    false,
                );
                spawn_input_field(
                    panel,
                    &fonts,
                    theme,
                    glow_intensity,
                    "Password",
                    FocusField::Password,
                    true,
                );
                spawn_input_field(
                    panel,
                    &fonts,
                    theme,
                    glow_intensity,
                    "Reset Token",
                    FocusField::ResetToken,
                    false,
                );
                spawn_input_field(
                    panel,
                    &fonts,
                    theme,
                    glow_intensity,
                    "New Password",
                    FocusField::NewPassword,
                    true,
                );

                panel
                    .spawn((
                        Button,
                        AuthUiButton(AuthButtonKind::Submit),
                        layout::button(
                            Val::Percent(100.0),
                            48.0,
                            theme.metrics.control_radius_px,
                            theme.metrics.control_border_px,
                        ),
                        submit_bg,
                        submit_border,
                        submit_shadow,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("LOGIN"),
                            text_font(fonts.mono_bold.clone(), 18.0),
                            TextColor(theme.colors.primary_foreground_color()),
                            AuthUiSubmitLabel,
                        ));
                    });

                panel
                    .spawn((
                        layout::grid(2, 34.0, 8.0),
                        Transform::default(),
                        GlobalTransform::default(),
                    ))
                    .with_children(|row| {
                        spawn_flow_button(
                            row,
                            &fonts,
                            theme,
                            glow_intensity,
                            "Login",
                            AuthAction::Login,
                        );
                        spawn_flow_button(
                            row,
                            &fonts,
                            theme,
                            glow_intensity,
                            "Register",
                            AuthAction::Register,
                        );
                        spawn_flow_button(
                            row,
                            &fonts,
                            theme,
                            glow_intensity,
                            "Forgot Request",
                            AuthAction::ForgotRequest,
                        );
                        spawn_flow_button(
                            row,
                            &fonts,
                            theme,
                            glow_intensity,
                            "Forgot Confirm",
                            AuthAction::ForgotConfirm,
                        );
                    });
                panel
                    .spawn((
                        Button,
                        AuthUiButton(AuthButtonKind::Quit),
                        Node {
                            align_self: AlignSelf::FlexEnd,
                            ..layout::button(
                                Val::Px(140.0),
                                38.0,
                                theme.metrics.control_radius_px,
                                theme.metrics.control_border_px,
                            )
                        },
                        Transform::default(),
                        GlobalTransform::default(),
                        quit_bg,
                        quit_border,
                        quit_shadow,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("QUIT"),
                            text_font(fonts.mono_bold.clone(), 16.0),
                            TextColor(theme.colors.panel_foreground_color()),
                        ));
                    });

                panel.spawn((
                    Text::new(""),
                    text_font(fonts.mono.clone(), 12.0),
                    TextColor(theme.colors.success_color()),
                    AuthUiStatusText,
                ));
            });
        });
}

fn spawn_input_field(
    parent: &mut ChildSpawnerCommands,
    fonts: &EmbeddedFonts,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
    label: &str,
    field: FocusField,
    is_password: bool,
) {
    let (input_bg, input_border, input_shadow) = input_surface(theme, false, glow_intensity);
    parent
        .spawn((
            layout::vertical_stack(6.0),
            Transform::default(),
            GlobalTransform::default(),
            AuthUiFieldContainer { field },
        ))
        .with_children(|container| {
            container.spawn((
                Text::new(label.to_ascii_uppercase()),
                text_font(fonts.bold.clone(), 11.0),
                TextColor(theme.colors.muted_foreground_color()),
            ));

            container
                .spawn((
                    Button,
                    AuthUiInputBox { field },
                    AuthUiButton(AuthButtonKind::Focus(field)),
                    layout::input_box(
                        44.0,
                        theme.metrics.control_radius_px,
                        theme.metrics.control_border_px,
                    ),
                    Transform::default(),
                    GlobalTransform::default(),
                    input_bg,
                    input_border,
                    input_shadow,
                ))
                .with_children(|input_box| {
                    input_box.spawn((
                        Text::new(""),
                        text_font(fonts.bold.clone(), 16.0),
                        TextColor(theme.colors.panel_foreground_color()),
                        AuthUiInputText { field, is_password },
                    ));

                    input_box.spawn((
                        Text::new("|"),
                        text_font(fonts.mono.clone(), 16.0),
                        TextColor(theme.colors.glow_color()),
                        AuthUiCursor { field },
                        Visibility::Hidden,
                    ));
                });
        });
}

fn spawn_flow_button(
    parent: &mut ChildSpawnerCommands,
    fonts: &EmbeddedFonts,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
    label: &str,
    action: AuthAction,
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
            AuthUiButton(AuthButtonKind::SwitchFlow(action)),
            layout::button(
                Val::Percent(100.0),
                34.0,
                theme.metrics.control_radius_px,
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
                text_font(fonts.mono_bold.clone(), 15.0),
                TextColor(theme.colors.panel_foreground_color()),
            ));
        });
}

fn animate_auth_background(
    time: Res<'_, Time>,
    active_theme: Res<'_, ActiveUiTheme>,
    mut bg_query: Query<'_, '_, &mut BackgroundColor, With<AuthUiBackdrop>>,
) {
    let theme = theme_definition(active_theme.0);
    let t = time.elapsed_secs();
    let pulse = 0.75 + 0.25 * (t * 0.5).sin().abs();
    for mut color in &mut bg_query {
        let base = theme.colors.background;
        *color = BackgroundColor(Color::from(base.with_lightness(base.lightness * pulse)));
    }
}

fn tick_cursor_blink(time: Res<'_, Time>, mut blink: ResMut<'_, CursorBlink>) {
    blink.timer.tick(time.delta());
    if blink.timer.just_finished() {
        blink.visible = !blink.visible;
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_auth_keyboard_input(
    mut keyboard_input_reader: MessageReader<'_, '_, KeyboardInput>,
    keys: Res<'_, ButtonInput<KeyCode>>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    mut session: ResMut<'_, ClientSession>,
    mut request_state: ResMut<'_, super::auth_net::GatewayRequestState>,
    gateway_http: Res<'_, GatewayHttpAdapter>,
) {
    if super::dev_console::is_console_open(dev_console_state.as_deref()) {
        return;
    }
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
        submit_auth_request(&mut session, request_state.as_mut(), *gateway_http);
    }
}

fn handle_auth_button_interactions(
    mut interactions: Query<
        '_,
        '_,
        (&Interaction, &AuthUiButton, Option<&AuthUiInputBox>),
        Changed<Interaction>,
    >,
    mut session: ResMut<'_, ClientSession>,
    mut request_state: ResMut<'_, super::auth_net::GatewayRequestState>,
    gateway_http: Res<'_, GatewayHttpAdapter>,
    mut app_exit: MessageWriter<'_, AppExit>,
) {
    for (interaction, button, input_box) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                if let Some(input) = input_box {
                    session.focus = input.field;
                    session.ui_dirty = true;
                    continue;
                }

                match button.0 {
                    AuthButtonKind::Submit => {
                        submit_auth_request(&mut session, request_state.as_mut(), *gateway_http);
                    }
                    AuthButtonKind::SwitchFlow(action) => {
                        session.selected_action = action;
                        session.focus = first_focus_field(action);
                        session.ui_dirty = true;
                    }
                    AuthButtonKind::Focus(field) => {
                        session.focus = field;
                        session.ui_dirty = true;
                    }
                    AuthButtonKind::Quit => {
                        app_exit.write(AppExit::Success);
                    }
                }
            }
            Interaction::Hovered | Interaction::None => {}
        }
    }
}

#[allow(clippy::type_complexity)]
fn sync_auth_button_visuals(
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    session: Res<'_, ClientSession>,
    mut query: Query<
        '_,
        '_,
        (
            &Interaction,
            &AuthUiButton,
            Option<&AuthUiInputBox>,
            &mut BackgroundColor,
            Option<&mut BorderColor>,
            Option<&mut BoxShadow>,
        ),
    >,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    for (interaction, button, input_box, mut bg, border, shadow) in &mut query {
        let state = match *interaction {
            Interaction::Pressed => UiInteractionState::Pressed,
            Interaction::Hovered => UiInteractionState::Hovered,
            Interaction::None => match button.0 {
                AuthButtonKind::SwitchFlow(action) if action == session.selected_action => {
                    UiInteractionState::Selected
                }
                AuthButtonKind::Focus(field)
                    if field == session.focus
                        && is_field_visible(session.selected_action, field) =>
                {
                    UiInteractionState::Focused
                }
                _ => UiInteractionState::Idle,
            },
        };

        let (next_bg, next_border, next_shadow) = if input_box.is_some() {
            input_surface(
                theme,
                matches!(
                    state,
                    UiInteractionState::Focused | UiInteractionState::Pressed
                ),
                glow_intensity,
            )
        } else {
            let variant = match button.0 {
                AuthButtonKind::Submit => UiButtonVariant::Primary,
                AuthButtonKind::SwitchFlow(_) => UiButtonVariant::Outline,
                AuthButtonKind::Focus(_) => UiButtonVariant::Outline,
                AuthButtonKind::Quit => UiButtonVariant::Outline,
            };
            button_surface(theme, variant, state, glow_intensity)
        };
        *bg = next_bg;
        if let Some(mut border) = border {
            *border = next_border;
        }
        if let Some(mut shadow) = shadow {
            *shadow = next_shadow;
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_auth_text(
    session: Res<'_, ClientSession>,
    active_theme: Res<'_, ActiveUiTheme>,
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
    let theme = theme_definition(active_theme.0);
    let flow_title = flow_title(session.selected_action);

    for mut text in &mut text_sets.p1() {
        text.0 = flow_title.to_string();
    }

    let submit_label = submit_label(session.selected_action);
    for mut text in &mut text_sets.p2() {
        text.0 = submit_label.to_ascii_uppercase();
    }

    for (mut text, mut color) in &mut text_sets.p0() {
        text.0 = session.status.clone();
        *color =
            if session.status.starts_with("Request failed") || session.status.contains("failed") {
                TextColor(theme.colors.destructive_color())
            } else {
                TextColor(theme.colors.success_color())
            };
    }
}

fn update_auth_field_layout(
    session: Res<'_, ClientSession>,
    mut field_containers: Query<'_, '_, (&AuthUiFieldContainer, &mut Visibility)>,
) {
    for (container, mut visibility) in &mut field_containers {
        *visibility = if is_field_visible(session.selected_action, container.field) {
            Visibility::Visible
        } else {
            Visibility::Hidden
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

fn active_field_mut(session: &mut ClientSession) -> &mut String {
    match session.focus {
        FocusField::Email => &mut session.email,
        FocusField::Password => &mut session.password,
        FocusField::ResetToken => &mut session.reset_token,
        FocusField::NewPassword => &mut session.new_password,
    }
}

fn mask(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    "*".repeat(value.chars().count())
}

fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);
    !is_in_private_use_area && !chr.is_ascii_control()
}

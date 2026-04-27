use bevy::app::AppExit;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::log::{info, warn};
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

const TOTP_CODE_LENGTH: usize = 6;

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
struct AuthUiTotpCodeInput;

#[derive(Component)]
struct AuthUiTotpDigitBox {
    field: FocusField,
    index: usize,
}

#[derive(Component)]
struct AuthUiTotpDigitText {
    index: usize,
}

#[derive(Component)]
struct AuthUiTotpDigitCursor {
    index: usize,
}

#[derive(Component)]
struct AuthUiButton(AuthButtonKind);

#[derive(Clone, Copy)]
enum AuthButtonKind {
    Submit,
    Focus(FocusField),
    FocusTotpDigit(usize),
    ForgotPasswordLink,
    Quit,
}

#[derive(Resource)]
struct CursorBlink {
    timer: Timer,
    visible: bool,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
struct TotpInputCursor {
    index: usize,
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
    app.init_resource::<TotpInputCursor>();
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
    mut images: ResMut<'_, Assets<Image>>,
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
                    Val::Px(420.0),
                    16.0,
                    10.0,
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
                    &mut images,
                    theme,
                    Some("Auth Terminal"),
                    &fonts.mono.clone(),
                    glow_intensity,
                );

                panel.spawn((
                    Text::new("SIDEREAL"),
                    text_font(fonts.display.clone(), 30.0),
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
                spawn_totp_code_input(
                    panel,
                    &fonts,
                    theme,
                    glow_intensity,
                    "Authenticator Code",
                    FocusField::TotpCode,
                );
                panel
                    .spawn((
                        Button,
                        AuthUiButton(AuthButtonKind::Submit),
                        layout::button(
                            Val::Percent(100.0),
                            42.0,
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
                        Button,
                        AuthUiButton(AuthButtonKind::ForgotPasswordLink),
                        Node {
                            align_self: AlignSelf::FlexEnd,
                            width: Val::Px(170.0),
                            height: Val::Px(24.0),
                            justify_content: JustifyContent::FlexEnd,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        Transform::default(),
                        GlobalTransform::default(),
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                    ))
                    .with_children(|link| {
                        link.spawn((
                            Text::new("Forgot Password?"),
                            text_font(fonts.mono_bold.clone(), 13.0),
                            TextColor(theme.colors.primary_color()),
                        ));
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

fn spawn_totp_code_input(
    parent: &mut ChildSpawnerCommands,
    fonts: &EmbeddedFonts,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
    label: &str,
    field: FocusField,
) {
    parent
        .spawn((
            layout::vertical_stack(6.0),
            Transform::default(),
            GlobalTransform::default(),
            AuthUiFieldContainer { field },
            AuthUiTotpCodeInput,
        ))
        .with_children(|container| {
            container.spawn((
                Text::new(label.to_ascii_uppercase()),
                text_font(fonts.bold.clone(), 11.0),
                TextColor(theme.colors.muted_foreground_color()),
            ));

            container
                .spawn((
                    Node {
                        display: Display::Grid,
                        width: Val::Percent(100.0),
                        height: Val::Px(48.0),
                        grid_template_columns: RepeatedGridTrack::flex(
                            TOTP_CODE_LENGTH as u16,
                            1.0,
                        ),
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    Transform::default(),
                    GlobalTransform::default(),
                ))
                .with_children(|digits| {
                    for index in 0..TOTP_CODE_LENGTH {
                        let (input_bg, input_border, input_shadow) =
                            input_surface(theme, false, glow_intensity);
                        let mut digit_entity = digits.spawn_empty();
                        digit_entity.insert(Button);
                        digit_entity.insert(AuthUiButton(AuthButtonKind::FocusTotpDigit(index)));
                        digit_entity.insert(AuthUiTotpDigitBox { field, index });
                        digit_entity.insert(Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(48.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(theme.metrics.control_border_px)),
                            border_radius: BorderRadius::all(Val::Px(
                                theme.metrics.control_radius_px,
                            )),
                            ..default()
                        });
                        digit_entity.insert(Transform::default());
                        digit_entity.insert(GlobalTransform::default());
                        digit_entity.insert(input_bg);
                        digit_entity.insert(input_border);
                        digit_entity.insert(input_shadow);
                        digit_entity.with_children(|digit| {
                            digit.spawn((
                                Text::new(""),
                                text_font(fonts.mono_bold.clone(), 22.0),
                                TextColor(theme.colors.panel_foreground_color()),
                                AuthUiTotpDigitText { index },
                            ));

                            digit.spawn((
                                Text::new("|"),
                                text_font(fonts.mono.clone(), 18.0),
                                TextColor(theme.colors.glow_color()),
                                AuthUiTotpDigitCursor { index },
                                Visibility::Hidden,
                            ));
                        });
                    }
                });
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
    mut totp_cursor: ResMut<'_, TotpInputCursor>,
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
                session.totp_challenge_id = None;
                session.totp_code.clear();
                totp_cursor.index = 0;
                session.ui_dirty = true;
            }
            Key::Tab => {
                session.focus = next_focus_field(&session, session.focus);
                if session.focus == FocusField::TotpCode {
                    totp_cursor.index = next_totp_cursor_index(&session.totp_code);
                }
                session.ui_dirty = true;
            }
            Key::Enter => {
                submit = true;
            }
            Key::Backspace => {
                if session.focus == FocusField::TotpCode {
                    handle_totp_backspace(&mut session.totp_code, &mut totp_cursor);
                } else {
                    active_field_mut(&mut session).pop();
                }
                session.ui_dirty = true;
            }
            Key::ArrowLeft if session.focus == FocusField::TotpCode => {
                totp_cursor.index = totp_cursor.index.saturating_sub(1);
                session.ui_dirty = true;
            }
            Key::ArrowRight if session.focus == FocusField::TotpCode => {
                totp_cursor.index = (totp_cursor.index.saturating_add(1)).min(TOTP_CODE_LENGTH - 1);
                session.ui_dirty = true;
            }
            _ => {
                if let Some(inserted_text) = &event.text
                    && inserted_text.chars().all(is_printable_char)
                {
                    if session.focus == FocusField::TotpCode {
                        insert_totp_digits(&mut session.totp_code, &mut totp_cursor, inserted_text);
                    } else {
                        active_field_mut(&mut session).push_str(inserted_text);
                    }
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

#[allow(clippy::type_complexity)]
fn handle_auth_button_interactions(
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            &AuthUiButton,
            Option<&AuthUiInputBox>,
            Option<&AuthUiTotpDigitBox>,
        ),
        Changed<Interaction>,
    >,
    mut session: ResMut<'_, ClientSession>,
    mut totp_cursor: ResMut<'_, TotpInputCursor>,
    mut request_state: ResMut<'_, super::auth_net::GatewayRequestState>,
    gateway_http: Res<'_, GatewayHttpAdapter>,
    mut app_exit: MessageWriter<'_, AppExit>,
) {
    for (interaction, button, input_box, totp_digit) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                if let Some(input) = input_box {
                    session.focus = input.field;
                    session.ui_dirty = true;
                    continue;
                }
                if let Some(input) = totp_digit {
                    session.focus = input.field;
                    totp_cursor.index = input.index.min(TOTP_CODE_LENGTH - 1);
                    session.ui_dirty = true;
                    continue;
                }

                match button.0 {
                    AuthButtonKind::Submit => {
                        submit_auth_request(&mut session, request_state.as_mut(), *gateway_http);
                    }
                    AuthButtonKind::Focus(field) => {
                        session.focus = field;
                        session.ui_dirty = true;
                    }
                    AuthButtonKind::FocusTotpDigit(index) => {
                        session.focus = FocusField::TotpCode;
                        totp_cursor.index = index.min(TOTP_CODE_LENGTH - 1);
                        session.ui_dirty = true;
                    }
                    AuthButtonKind::ForgotPasswordLink => {
                        let url = forgot_password_url();
                        match open_external_url(&url) {
                            Ok(()) => {
                                session.status =
                                    "Opened password reset in your browser.".to_string();
                            }
                            Err(err) => {
                                warn!("failed to open password reset URL: {err}");
                                session.status =
                                    format!("Open this URL to reset your password: {url}");
                            }
                        }
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
    totp_cursor: Res<'_, TotpInputCursor>,
    mut query: Query<
        '_,
        '_,
        (
            &Interaction,
            &AuthUiButton,
            Option<&AuthUiInputBox>,
            Option<&AuthUiTotpDigitBox>,
            &mut BackgroundColor,
            Option<&mut BorderColor>,
            Option<&mut BoxShadow>,
        ),
    >,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    for (interaction, button, input_box, totp_digit, mut bg, border, shadow) in &mut query {
        if matches!(button.0, AuthButtonKind::ForgotPasswordLink) {
            *bg = match *interaction {
                Interaction::Hovered | Interaction::Pressed => {
                    BackgroundColor(theme.colors.primary_color().with_alpha(0.08))
                }
                Interaction::None => BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            };
            continue;
        }

        let state = match *interaction {
            Interaction::Pressed => UiInteractionState::Pressed,
            Interaction::Hovered => UiInteractionState::Hovered,
            Interaction::None => match button.0 {
                AuthButtonKind::Focus(field)
                    if field == session.focus && is_field_visible(&session, field) =>
                {
                    UiInteractionState::Focused
                }
                AuthButtonKind::FocusTotpDigit(index)
                    if session.focus == FocusField::TotpCode
                        && is_field_visible(&session, FocusField::TotpCode)
                        && index == totp_cursor.index =>
                {
                    UiInteractionState::Focused
                }
                _ => UiInteractionState::Idle,
            },
        };

        let (next_bg, next_border, next_shadow) = if input_box.is_some() || totp_digit.is_some() {
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
                AuthButtonKind::Focus(_) => UiButtonVariant::Outline,
                AuthButtonKind::FocusTotpDigit(_) => UiButtonVariant::Outline,
                AuthButtonKind::ForgotPasswordLink => UiButtonVariant::Outline,
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
    let flow_title = flow_title(&session);

    for mut text in &mut text_sets.p1() {
        text.0 = flow_title.to_string();
    }

    let submit_label = submit_label(&session);
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
        *visibility = if is_field_visible(&session, container.field) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn update_auth_field_content(
    session: Res<'_, ClientSession>,
    blink: Res<'_, CursorBlink>,
    totp_cursor: Res<'_, TotpInputCursor>,
    mut input_text_query: Query<'_, '_, (&AuthUiInputText, &mut Text)>,
    mut totp_text_query: Query<'_, '_, (&AuthUiTotpDigitText, &mut Text)>,
    mut cursor_query: Query<'_, '_, (&AuthUiCursor, &mut Visibility)>,
    mut totp_cursor_query: Query<'_, '_, (&AuthUiTotpDigitCursor, &mut Visibility)>,
) {
    for (input, mut text) in &mut input_text_query {
        let value = match input.field {
            FocusField::Email => session.email.as_str(),
            FocusField::Password => session.password.as_str(),
            FocusField::TotpCode => session.totp_code.as_str(),
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
            && is_field_visible(&session, cursor.field);
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    let totp_digits = normalize_totp_code(&session.totp_code);
    for (digit, mut text) in &mut totp_text_query {
        text.0 = totp_digits
            .chars()
            .nth(digit.index)
            .map(|value| value.to_string())
            .unwrap_or_default();
    }

    for (cursor, mut visibility) in &mut totp_cursor_query {
        let visible = blink.visible
            && session.focus == FocusField::TotpCode
            && is_field_visible(&session, FocusField::TotpCode)
            && cursor.index == totp_cursor.index;
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn normalize_totp_code(raw: &str) -> String {
    raw.chars()
        .filter(|value| value.is_ascii_digit())
        .take(TOTP_CODE_LENGTH)
        .collect()
}

fn next_totp_cursor_index(code: &str) -> usize {
    normalize_totp_code(code)
        .len()
        .min(TOTP_CODE_LENGTH.saturating_sub(1))
}

fn insert_totp_digits(code: &mut String, cursor: &mut TotpInputCursor, raw: &str) {
    let inserted = normalize_totp_code(raw);
    if inserted.is_empty() {
        return;
    }

    let mut digits: Vec<char> = normalize_totp_code(code).chars().collect();
    digits.resize(TOTP_CODE_LENGTH, '\0');
    let mut index = cursor.index.min(TOTP_CODE_LENGTH - 1);
    for digit in inserted.chars() {
        digits[index] = digit;
        if index >= TOTP_CODE_LENGTH - 1 {
            break;
        }
        index += 1;
    }
    *code = digits.into_iter().filter(|digit| *digit != '\0').collect();
    cursor.index = index.min(TOTP_CODE_LENGTH - 1);
}

fn handle_totp_backspace(code: &mut String, cursor: &mut TotpInputCursor) {
    let mut digits: Vec<char> = normalize_totp_code(code).chars().collect();
    if digits.is_empty() {
        cursor.index = 0;
        return;
    }

    let active_index = cursor.index.min(TOTP_CODE_LENGTH - 1);
    let remove_index = if active_index < digits.len() {
        active_index
    } else {
        digits.len().saturating_sub(1)
    };
    digits.remove(remove_index);
    *code = digits.into_iter().collect();
    cursor.index = remove_index.saturating_sub(usize::from(remove_index > 0));
}

fn flow_title(session: &ClientSession) -> &'static str {
    if session.totp_challenge_id.is_some() && session.selected_action == AuthAction::Login {
        return "Authenticator Required";
    }
    match session.selected_action {
        AuthAction::Login => "Login",
    }
}

fn submit_label(session: &ClientSession) -> &'static str {
    if session.totp_challenge_id.is_some() && session.selected_action == AuthAction::Login {
        return "Verify Code";
    }
    match session.selected_action {
        AuthAction::Login => "Login",
    }
}

fn is_field_visible(session: &ClientSession, field: FocusField) -> bool {
    let totp_required =
        session.selected_action == AuthAction::Login && session.totp_challenge_id.is_some();
    match session.selected_action {
        AuthAction::Login if totp_required => matches!(field, FocusField::TotpCode),
        AuthAction::Login => matches!(field, FocusField::Email | FocusField::Password),
    }
}

fn next_focus_field(session: &ClientSession, current: FocusField) -> FocusField {
    if session.selected_action == AuthAction::Login && session.totp_challenge_id.is_some() {
        return FocusField::TotpCode;
    }
    match session.selected_action {
        AuthAction::Login => match current {
            FocusField::Email => FocusField::Password,
            _ => FocusField::Email,
        },
    }
}

fn active_field_mut(session: &mut ClientSession) -> &mut String {
    match session.focus {
        FocusField::Email => &mut session.email,
        FocusField::Password => &mut session.password,
        FocusField::TotpCode => &mut session.totp_code,
    }
}

fn forgot_password_url() -> String {
    let base = dashboard_base_url();
    format!("{}/forgot-password", base.trim_end_matches('/'))
}

#[cfg(not(target_arch = "wasm32"))]
fn dashboard_base_url() -> String {
    std::env::var("SIDEREAL_DASHBOARD_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}

#[cfg(target_arch = "wasm32")]
fn dashboard_base_url() -> String {
    "/".to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn open_external_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url);
        command
    };

    command.spawn().map(|_| ()).map_err(|err| err.to_string())
}

#[cfg(target_arch = "wasm32")]
fn open_external_url(url: &str) -> Result<(), String> {
    web_sys::window()
        .ok_or_else(|| "browser window is unavailable".to_string())?
        .open_with_url(url)
        .map(|_| ())
        .map_err(|err| format!("{err:?}"))
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

use bevy::app::AppExit;
use bevy::asset::RenderAssetUsages;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::state::state_scoped::DespawnOnExit;
use bevy::ui::{ComputedNode, FocusPolicy, RelativeCursorPosition};
use sidereal_ui::layout;
use sidereal_ui::theme::{
    ActiveUiTheme, UiSemanticTone, UiThemeId, UiVisualSettings, theme_definition,
};
use sidereal_ui::typography::text_font;
use sidereal_ui::widgets::{
    TextInputDelete, TextInputKind, TextInputMovement, TextInputState, UiButtonVariant,
    UiInteractionState, button_surface, input_surface, panel_surface, panel_surface_with_tone,
    spawn_hud_frame_chrome, spawn_hud_frame_chrome_with_tone,
};
use std::collections::{HashMap, HashSet};

use super::dev_console::DevConsoleState;
use super::resources::GatewayHttpAdapter;
use super::{
    AuthAction, ClientAppState, ClientSession, EmbeddedFonts, FocusField, submit_auth_request,
};

const TOTP_CODE_LENGTH: usize = 6;
const AUTH_INPUT_ICON_PX: f32 = 18.0;
const AUTH_INPUT_ICON_RASTER_PX: u32 = 48;
const AUTH_INPUT_CARET_HEIGHT_PX: f32 = 22.0;
const AUTH_INPUT_CARET_WIDTH_PX: f32 = 1.25;

#[derive(Component)]
struct AuthUiRoot;

#[derive(Component)]
struct AuthUiBackdrop;

#[derive(Component)]
struct AuthUiStatusText {
    tone: UiSemanticTone,
}

#[derive(Component)]
struct AuthUiStatusFrame {
    tone: UiSemanticTone,
}

#[derive(Component)]
struct AuthUiStatusTitle {
    tone: UiSemanticTone,
}

#[derive(Component)]
struct AuthUiStatusIconSlot {
    tone: UiSemanticTone,
}

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
    segment: AuthInputTextSegment,
    kind: TextInputKind,
}

#[derive(Component)]
struct AuthUiCursor {
    field: FocusField,
    edge: AuthInputCursorEdge,
}

#[derive(Component)]
struct AuthUiSelectionBox {
    field: FocusField,
}

#[derive(Component)]
struct AuthUiSelectionText {
    field: FocusField,
}

#[derive(Component)]
struct AuthUiInputTextSlot {
    field: FocusField,
}

#[derive(Component)]
struct AuthUiSvgIconAnchor {
    role: AuthUiSvgIconRole,
}

#[derive(Component)]
struct AuthUiSvgIcon {
    anchor: Entity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum AuthUiSvgIconRole {
    Alert,
    Email,
    Password,
    PasswordVisibilityToggle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum AuthUiSvgIconKind {
    CircleAlert,
    Email,
    Password,
    Eye,
    EyeOff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum AuthUiSvgIconColor {
    Primary,
    SemanticForeground(UiSemanticTone),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct AuthUiSvgIconCacheKey {
    kind: AuthUiSvgIconKind,
    color: AuthUiSvgIconColor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthInputTextSegment {
    BeforeSelection,
    AfterSelection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthInputCursorEdge {
    SelectionStart,
    SelectionEnd,
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
    TogglePasswordVisibility,
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

#[derive(Resource, Debug, Default)]
struct AuthReusableInputState {
    email: TextInputState,
    password: TextInputState,
    clipboard: String,
}

#[derive(Resource, Debug, Default)]
struct AuthInputPointerState {
    dragging: Option<FocusField>,
    last_click_field: Option<FocusField>,
    last_click_time_s: f64,
    click_count: u8,
}

#[derive(Resource, Debug, Default)]
struct AuthPasswordDisplayState {
    reveal_password: bool,
}

#[derive(Default)]
struct AuthSvgIconHandleCache {
    theme_id: Option<UiThemeId>,
    handles_by_key: HashMap<AuthUiSvgIconCacheKey, Handle<Image>>,
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
    app.init_resource::<AuthReusableInputState>();
    app.init_resource::<AuthInputPointerState>();
    app.init_resource::<AuthPasswordDisplayState>();
    app.add_systems(OnEnter(ClientAppState::Auth), setup_auth_screen);
    app.add_systems(
        Update,
        (
            animate_auth_background,
            tick_cursor_blink,
            handle_auth_keyboard_input,
            handle_auth_input_pointer,
            handle_auth_button_interactions,
            sync_auth_svg_icon_adornments,
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
    session: Res<'_, ClientSession>,
    mut input_state: ResMut<'_, AuthReusableInputState>,
) {
    info!("client auth UI setup: spawning auth screen");
    input_state.email.set_text(session.email.clone());
    input_state.password.set_text(session.password.clone());
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

                spawn_status_frame(panel, &mut images, &fonts, theme, glow_intensity);

                spawn_input_field(
                    panel,
                    &fonts,
                    theme,
                    glow_intensity,
                    "Email",
                    FocusField::Email,
                    TextInputKind::Text,
                    AuthUiSvgIconRole::Email,
                    None,
                );
                spawn_input_field(
                    panel,
                    &fonts,
                    theme,
                    glow_intensity,
                    "Password",
                    FocusField::Password,
                    TextInputKind::password(),
                    AuthUiSvgIconRole::Password,
                    Some(AuthUiSvgIconRole::PasswordVisibilityToggle),
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
                            theme.metrics.input_radius_px,
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
                                theme.metrics.input_radius_px,
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
            });
        });
}

fn spawn_status_frame(
    parent: &mut ChildSpawnerCommands,
    images: &mut Assets<Image>,
    fonts: &EmbeddedFonts,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
) {
    for tone in [
        UiSemanticTone::Danger,
        UiSemanticTone::Warning,
        UiSemanticTone::Info,
    ] {
        let (status_bg, status_border, status_shadow) =
            panel_surface_with_tone(theme, glow_intensity, tone);
        let status_text_color = tone.foreground_color(theme);
        let mut frame = parent.spawn((
            Node {
                display: Display::None,
                width: Val::Percent(100.0),
                min_height: Val::Px(58.0),
                padding: UiRect::all(Val::Px(12.0)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                border: UiRect::all(Val::Px(theme.metrics.control_border_px)),
                border_radius: BorderRadius::all(Val::Px(theme.metrics.control_radius_px.max(4.0))),
                ..default()
            },
            Transform::default(),
            GlobalTransform::default(),
            status_bg,
            status_border,
            status_shadow,
            AuthUiStatusFrame { tone },
        ));

        frame.with_children(|status| {
            spawn_hud_frame_chrome_with_tone(
                status,
                images,
                theme,
                None,
                &fonts.mono,
                glow_intensity,
                tone,
            );

            status.spawn((
                Node {
                    width: Val::Px(28.0),
                    ..layout::input_adornment()
                },
                Transform::default(),
                GlobalTransform::default(),
                AuthUiSvgIconAnchor {
                    role: AuthUiSvgIconRole::Alert,
                },
                AuthUiStatusIconSlot { tone },
            ));

            status
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                    Transform::default(),
                    GlobalTransform::default(),
                ))
                .with_children(|copy| {
                    copy.spawn((
                        Text::new(""),
                        text_font(fonts.mono_bold.clone(), 11.0),
                        TextColor(status_text_color),
                        AuthUiStatusTitle { tone },
                    ));
                    copy.spawn((
                        Text::new(""),
                        text_font(fonts.mono.clone(), 12.0),
                        TextColor(status_text_color),
                        AuthUiStatusText { tone },
                    ));
                });
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_input_field(
    parent: &mut ChildSpawnerCommands,
    fonts: &EmbeddedFonts,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
    label: &str,
    field: FocusField,
    kind: TextInputKind,
    start_icon: AuthUiSvgIconRole,
    end_icon: Option<AuthUiSvgIconRole>,
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
                    layout::input_box_with_adornments(
                        44.0,
                        theme.metrics.input_radius_px,
                        theme.metrics.control_border_px,
                        true,
                        end_icon.is_some(),
                    ),
                    Transform::default(),
                    GlobalTransform::default(),
                    RelativeCursorPosition::default(),
                    input_bg,
                    input_border,
                    input_shadow,
                ))
                .with_children(|input_box| {
                    spawn_input_svg_adornment(input_box, start_icon, false);

                    input_box
                        .spawn((
                            layout::input_text_slot(),
                            Transform::default(),
                            GlobalTransform::default(),
                            RelativeCursorPosition::default(),
                            AuthUiInputTextSlot { field },
                        ))
                        .with_children(|slot| {
                            slot.spawn((
                                Text::new(""),
                                text_font(fonts.bold.clone(), 16.0),
                                TextColor(theme.colors.panel_foreground_color()),
                                AuthUiInputText {
                                    field,
                                    segment: AuthInputTextSegment::BeforeSelection,
                                    kind,
                                },
                            ));

                            slot.spawn((
                                Node {
                                    width: Val::Px(AUTH_INPUT_CARET_WIDTH_PX),
                                    height: Val::Px(AUTH_INPUT_CARET_HEIGHT_PX),
                                    flex_shrink: 0.0,
                                    display: Display::None,
                                    ..default()
                                },
                                Transform::default(),
                                GlobalTransform::default(),
                                BackgroundColor(theme.colors.glow_color()),
                                AuthUiCursor {
                                    field,
                                    edge: AuthInputCursorEdge::SelectionStart,
                                },
                            ));

                            slot.spawn((
                                Node {
                                    display: Display::None,
                                    min_width: Val::Px(0.0),
                                    padding: UiRect::axes(Val::Px(2.0), Val::Px(1.0)),
                                    border_radius: BorderRadius::all(Val::Px(3.0)),
                                    ..default()
                                },
                                Transform::default(),
                                GlobalTransform::default(),
                                BackgroundColor(theme.colors.primary_color().with_alpha(0.28)),
                                AuthUiSelectionBox { field },
                            ))
                            .with_children(|selection| {
                                selection.spawn((
                                    Text::new(""),
                                    text_font(fonts.bold.clone(), 16.0),
                                    TextColor(theme.colors.panel_foreground_color()),
                                    AuthUiSelectionText { field },
                                ));
                            });

                            slot.spawn((
                                Node {
                                    width: Val::Px(AUTH_INPUT_CARET_WIDTH_PX),
                                    height: Val::Px(AUTH_INPUT_CARET_HEIGHT_PX),
                                    flex_shrink: 0.0,
                                    display: Display::None,
                                    ..default()
                                },
                                Transform::default(),
                                GlobalTransform::default(),
                                BackgroundColor(theme.colors.glow_color()),
                                AuthUiCursor {
                                    field,
                                    edge: AuthInputCursorEdge::SelectionEnd,
                                },
                            ));

                            slot.spawn((
                                Text::new(""),
                                text_font(fonts.bold.clone(), 16.0),
                                TextColor(theme.colors.panel_foreground_color()),
                                AuthUiInputText {
                                    field,
                                    segment: AuthInputTextSegment::AfterSelection,
                                    kind,
                                },
                            ));
                        });

                    if let Some(end_icon) = end_icon {
                        spawn_input_svg_adornment(input_box, end_icon, true);
                    }
                });
        });
}

fn spawn_input_svg_adornment(
    parent: &mut ChildSpawnerCommands,
    role: AuthUiSvgIconRole,
    interactive: bool,
) {
    let mut entity = parent.spawn((
        Node {
            width: Val::Px(24.0),
            ..layout::input_adornment()
        },
        Transform::default(),
        GlobalTransform::default(),
        AuthUiSvgIconAnchor { role },
    ));
    if interactive {
        entity.insert((
            Button,
            AuthUiButton(AuthButtonKind::TogglePasswordVisibility),
            RelativeCursorPosition::default(),
        ));
    }
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
                                theme.metrics.input_radius_px,
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
    mut input_state: ResMut<'_, AuthReusableInputState>,
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
                let email_end = input_state.email.text.len();
                input_state.email.set_cursor(email_end);
                totp_cursor.index = 0;
                session.ui_dirty = true;
            }
            Key::Tab => {
                if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                    session.focus = previous_focus_field(&session, session.focus);
                } else {
                    session.focus = next_focus_field(&session, session.focus);
                }
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
                    let delete = if command_modifier(&keys) && cfg!(target_os = "macos") {
                        TextInputDelete::ToStart
                    } else if word_modifier(&keys) {
                        TextInputDelete::PreviousWord
                    } else {
                        TextInputDelete::PreviousGrapheme
                    };
                    if let Some(input) = active_text_input_mut(&mut input_state, session.focus) {
                        input.delete(delete);
                    }
                }
                session.ui_dirty = true;
            }
            Key::Delete => {
                if let Some(input) = active_text_input_mut(&mut input_state, session.focus) {
                    let delete = if word_modifier(&keys) {
                        TextInputDelete::NextWord
                    } else {
                        TextInputDelete::NextGrapheme
                    };
                    input.delete(delete);
                    sync_session_text_inputs(&mut session, &input_state);
                    session.ui_dirty = true;
                }
            }
            Key::ArrowLeft if session.focus == FocusField::TotpCode => {
                totp_cursor.index = totp_cursor.index.saturating_sub(1);
                session.ui_dirty = true;
            }
            Key::ArrowRight if session.focus == FocusField::TotpCode => {
                totp_cursor.index = (totp_cursor.index.saturating_add(1)).min(TOTP_CODE_LENGTH - 1);
                session.ui_dirty = true;
            }
            Key::ArrowLeft
            | Key::ArrowRight
            | Key::Home
            | Key::End
            | Key::ArrowUp
            | Key::ArrowDown
                if session.focus != FocusField::TotpCode =>
            {
                if let Some(input) = active_text_input_mut(&mut input_state, session.focus) {
                    let extend =
                        keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                    let movement = match &event.logical_key {
                        Key::ArrowLeft if word_modifier(&keys) => TextInputMovement::PreviousWord,
                        Key::ArrowLeft => TextInputMovement::PreviousGrapheme,
                        Key::ArrowRight if word_modifier(&keys) => TextInputMovement::NextWord,
                        Key::ArrowRight => TextInputMovement::NextGrapheme,
                        Key::Home | Key::ArrowUp => TextInputMovement::Start,
                        Key::End | Key::ArrowDown => TextInputMovement::End,
                        _ => TextInputMovement::End,
                    };
                    input.move_cursor(movement, extend);
                    session.ui_dirty = true;
                }
            }
            Key::Character(_)
                if session.focus != FocusField::TotpCode && primary_modifier(&keys) =>
            {
                handle_text_input_shortcut(event.key_code, &keys, &mut input_state, &mut session);
            }
            Key::Copy if session.focus != FocusField::TotpCode => {
                copy_active_selection(&mut input_state, session.focus);
            }
            Key::Cut if session.focus != FocusField::TotpCode => {
                cut_active_selection(&mut input_state, session.focus);
                sync_session_text_inputs(&mut session, &input_state);
                session.ui_dirty = true;
            }
            Key::Paste if session.focus != FocusField::TotpCode => {
                paste_into_active_input(&mut input_state, session.focus);
                sync_session_text_inputs(&mut session, &input_state);
                session.ui_dirty = true;
            }
            Key::Undo if session.focus != FocusField::TotpCode => {
                if let Some(input) = active_text_input_mut(&mut input_state, session.focus) {
                    input.undo();
                }
                sync_session_text_inputs(&mut session, &input_state);
                session.ui_dirty = true;
            }
            Key::Redo if session.focus != FocusField::TotpCode => {
                if let Some(input) = active_text_input_mut(&mut input_state, session.focus) {
                    input.redo();
                }
                sync_session_text_inputs(&mut session, &input_state);
                session.ui_dirty = true;
            }
            Key::Insert if session.focus != FocusField::TotpCode => {
                if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                    paste_into_active_input(&mut input_state, session.focus);
                    sync_session_text_inputs(&mut session, &input_state);
                    session.ui_dirty = true;
                } else if control_modifier(&keys) {
                    copy_active_selection(&mut input_state, session.focus);
                }
            }
            _ => {
                if let Some(inserted_text) = &event.text
                    && inserted_text.chars().all(is_printable_char)
                    && !control_modifier(&keys)
                    && !command_modifier(&keys)
                {
                    if session.focus == FocusField::TotpCode {
                        insert_totp_digits(&mut session.totp_code, &mut totp_cursor, inserted_text);
                    } else {
                        if let Some(input) = active_text_input_mut(&mut input_state, session.focus)
                        {
                            input.insert_text(inserted_text);
                        }
                        sync_session_text_inputs(&mut session, &input_state);
                    }
                    session.ui_dirty = true;
                }
            }
        }
        if session.focus != FocusField::TotpCode {
            sync_session_text_inputs(&mut session, &input_state);
        }
    }

    if keys.just_pressed(KeyCode::Enter) {
        submit = true;
    }

    if submit {
        submit_auth_request(&mut session, request_state.as_mut(), *gateway_http);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_auth_input_pointer(
    time: Res<'_, Time>,
    mouse: Res<'_, ButtonInput<MouseButton>>,
    keys: Res<'_, ButtonInput<KeyCode>>,
    mut pointer_state: ResMut<'_, AuthInputPointerState>,
    mut session: ResMut<'_, ClientSession>,
    mut input_state: ResMut<'_, AuthReusableInputState>,
    text_slots: Query<'_, '_, (&AuthUiInputTextSlot, &RelativeCursorPosition, &ComputedNode)>,
    input_text_nodes: Query<'_, '_, (&AuthUiInputText, &ComputedNode)>,
    selection_text_nodes: Query<'_, '_, (&AuthUiSelectionText, &ComputedNode)>,
) {
    if mouse.just_released(MouseButton::Left) {
        pointer_state.dragging = None;
    }

    if mouse.just_pressed(MouseButton::Left) {
        for (text_slot, cursor_position, slot_node) in &text_slots {
            if !cursor_position.cursor_over {
                continue;
            }

            let fraction = pointer_text_fraction(
                text_slot.field,
                cursor_position,
                slot_node,
                &input_text_nodes,
                &selection_text_nodes,
            );
            session.focus = text_slot.field;
            let extend = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
            if let Some(input) = active_text_input_mut(&mut input_state, text_slot.field) {
                input.set_cursor_from_fraction(fraction, extend);
                let now = time.elapsed_secs_f64();
                let same_field = pointer_state.last_click_field == Some(text_slot.field);
                if same_field && now - pointer_state.last_click_time_s <= 0.35 {
                    pointer_state.click_count = pointer_state.click_count.saturating_add(1);
                } else {
                    pointer_state.click_count = 1;
                }
                pointer_state.last_click_field = Some(text_slot.field);
                pointer_state.last_click_time_s = now;
                if pointer_state.click_count == 2 {
                    input.select_word_at_cursor();
                } else if pointer_state.click_count >= 3 {
                    input.select_all();
                    pointer_state.click_count = 0;
                }
            }
            pointer_state.dragging = Some(text_slot.field);
            sync_session_text_inputs(&mut session, &input_state);
            session.ui_dirty = true;
            return;
        }
    }

    if mouse.pressed(MouseButton::Left)
        && let Some(dragging_field) = pointer_state.dragging
    {
        for (text_slot, cursor_position, slot_node) in &text_slots {
            if text_slot.field != dragging_field || !cursor_position.cursor_over {
                continue;
            }
            let fraction = pointer_text_fraction(
                text_slot.field,
                cursor_position,
                slot_node,
                &input_text_nodes,
                &selection_text_nodes,
            );
            if let Some(input) = active_text_input_mut(&mut input_state, dragging_field) {
                input.set_cursor_from_fraction(fraction, true);
            }
            sync_session_text_inputs(&mut session, &input_state);
            session.ui_dirty = true;
            return;
        }
    }
}

fn pointer_text_fraction(
    field: FocusField,
    cursor_position: &RelativeCursorPosition,
    slot_node: &ComputedNode,
    input_text_nodes: &Query<'_, '_, (&AuthUiInputText, &ComputedNode)>,
    selection_text_nodes: &Query<'_, '_, (&AuthUiSelectionText, &ComputedNode)>,
) -> f32 {
    let pointer_fraction = cursor_position
        .normalized
        .map(|position| position.x + 0.5)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    let pointer_x = pointer_fraction * slot_node.size().x.max(0.0);
    let text_width = input_text_nodes
        .iter()
        .filter(|(input, _)| input.field == field)
        .map(|(_, node)| node.size().x)
        .chain(
            selection_text_nodes
                .iter()
                .filter(|(selection, _)| selection.field == field)
                .map(|(_, node)| node.size().x),
        )
        .sum::<f32>();

    if text_width <= f32::EPSILON {
        return 0.0;
    }
    (pointer_x / text_width).clamp(0.0, 1.0)
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
    mut password_display: ResMut<'_, AuthPasswordDisplayState>,
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
                    AuthButtonKind::TogglePasswordVisibility => {
                        session.focus = FocusField::Password;
                        password_display.reveal_password = !password_display.reveal_password;
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

#[allow(clippy::too_many_arguments)]
fn sync_auth_svg_icon_adornments(
    mut commands: Commands<'_, '_>,
    active_theme: Res<'_, ActiveUiTheme>,
    password_display: Res<'_, AuthPasswordDisplayState>,
    mut images: ResMut<'_, Assets<Image>>,
    mut icon_cache: Local<'_, AuthSvgIconHandleCache>,
    anchors: Query<'_, '_, (Entity, &AuthUiSvgIconAnchor, Option<&AuthUiStatusIconSlot>)>,
    mut icons: Query<'_, '_, (Entity, &AuthUiSvgIcon, &mut ImageNode)>,
) {
    let theme = theme_definition(active_theme.0);
    if icon_cache.theme_id != Some(active_theme.0) {
        icon_cache.handles_by_key.clear();
        icon_cache.theme_id = Some(active_theme.0);
    }

    let existing_anchors = icons
        .iter()
        .map(|(_, icon, _)| icon.anchor)
        .collect::<HashSet<_>>();
    for (anchor, icon_anchor, status_icon) in &anchors {
        if existing_anchors.contains(&anchor) {
            continue;
        }
        let kind = auth_svg_icon_kind(icon_anchor.role, password_display.reveal_password);
        let icon_color = auth_svg_icon_color(icon_anchor.role, status_icon, theme);
        let cache_key = AuthUiSvgIconCacheKey {
            kind,
            color: auth_svg_icon_color_key(icon_anchor.role, status_icon),
        };
        let Some(handle) =
            auth_svg_icon_handle(cache_key, icon_color, &mut icon_cache, &mut images)
        else {
            continue;
        };
        commands.entity(anchor).with_children(|slot| {
            slot.spawn((
                Node {
                    width: Val::Px(AUTH_INPUT_ICON_PX),
                    height: Val::Px(AUTH_INPUT_ICON_PX),
                    flex_shrink: 0.0,
                    ..default()
                },
                ImageNode::new(handle),
                FocusPolicy::Pass,
                AuthUiSvgIcon { anchor },
            ));
        });
    }

    for (entity, icon, mut image_node) in &mut icons {
        let Ok((_, anchor, status_icon)) = anchors.get(icon.anchor) else {
            commands.entity(entity).despawn();
            continue;
        };
        let kind = auth_svg_icon_kind(anchor.role, password_display.reveal_password);
        let icon_color = auth_svg_icon_color(anchor.role, status_icon, theme);
        let cache_key = AuthUiSvgIconCacheKey {
            kind,
            color: auth_svg_icon_color_key(anchor.role, status_icon),
        };
        let Some(handle) =
            auth_svg_icon_handle(cache_key, icon_color, &mut icon_cache, &mut images)
        else {
            continue;
        };
        image_node.image = handle;
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
        if matches!(
            button.0,
            AuthButtonKind::ForgotPasswordLink | AuthButtonKind::TogglePasswordVisibility
        ) {
            *bg = match *interaction {
                Interaction::Hovered | Interaction::Pressed => {
                    BackgroundColor(theme.colors.primary_color().with_alpha(0.08))
                }
                Interaction::None => BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            };
            if let Some(mut border) = border {
                *border = BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.0));
            }
            if let Some(mut shadow) = shadow {
                *shadow = BoxShadow::default();
            }
            continue;
        }

        let is_focused_control = match button.0 {
            AuthButtonKind::Focus(field) => {
                field == session.focus && is_field_visible(&session, field)
            }
            AuthButtonKind::TogglePasswordVisibility => {
                session.focus == FocusField::Password
                    && is_field_visible(&session, FocusField::Password)
            }
            AuthButtonKind::FocusTotpDigit(index) => {
                session.focus == FocusField::TotpCode
                    && is_field_visible(&session, FocusField::TotpCode)
                    && index == totp_cursor.index
            }
            AuthButtonKind::Submit | AuthButtonKind::ForgotPasswordLink | AuthButtonKind::Quit => {
                false
            }
        };
        let is_input_surface = input_box.is_some() || totp_digit.is_some();
        let state = match *interaction {
            Interaction::Pressed => UiInteractionState::Pressed,
            Interaction::Hovered if is_input_surface && is_focused_control => {
                UiInteractionState::Focused
            }
            Interaction::Hovered => UiInteractionState::Hovered,
            Interaction::None if is_focused_control => UiInteractionState::Focused,
            Interaction::None => UiInteractionState::Idle,
        };

        let (next_bg, next_border, next_shadow) = if is_input_surface {
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
                AuthButtonKind::TogglePasswordVisibility => UiButtonVariant::Outline,
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
            Query<'_, '_, (&AuthUiStatusText, &mut Text, &mut TextColor)>,
            Query<'_, '_, &mut Text, With<AuthUiFlowTitle>>,
            Query<'_, '_, &mut Text, With<AuthUiSubmitLabel>>,
            Query<'_, '_, (&AuthUiStatusFrame, &mut Node), With<AuthUiStatusFrame>>,
            Query<'_, '_, (&AuthUiStatusTitle, &mut Text, &mut TextColor)>,
            Query<'_, '_, (&AuthUiStatusIconSlot, &mut Node)>,
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

    let status = session.status.trim();
    let status_tone = auth_status_tone(status);

    for (frame, mut node) in &mut text_sets.p3() {
        node.display = if status_tone == Some(frame.tone) {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (icon, mut node) in &mut text_sets.p5() {
        node.display = if status_tone == Some(icon.tone)
            && matches!(icon.tone, UiSemanticTone::Danger | UiSemanticTone::Warning)
        {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (title, mut text, mut color) in &mut text_sets.p4() {
        text.0 = auth_status_title(status, title.tone).to_string();
        *color = TextColor(title.tone.foreground_color(theme));
    }

    for (status_text, mut text, mut color) in &mut text_sets.p0() {
        text.0 = status.to_string();
        *color = TextColor(status_text.tone.foreground_color(theme));
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

#[allow(clippy::type_complexity)]
fn update_auth_field_content(
    session: Res<'_, ClientSession>,
    input_state: Res<'_, AuthReusableInputState>,
    password_display: Res<'_, AuthPasswordDisplayState>,
    blink: Res<'_, CursorBlink>,
    totp_cursor: Res<'_, TotpInputCursor>,
    mut field_queries: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, (&AuthUiInputText, &mut Text)>,
            Query<'_, '_, (&AuthUiTotpDigitText, &mut Text)>,
            Query<'_, '_, (&AuthUiCursor, &mut Node)>,
            Query<'_, '_, (&AuthUiTotpDigitCursor, &mut Visibility)>,
            Query<'_, '_, (&AuthUiSelectionText, &mut Text)>,
            Query<'_, '_, (&AuthUiSelectionBox, &mut Node)>,
        ),
    >,
) {
    for (input, mut text) in &mut field_queries.p0() {
        let Some(state) = active_text_input(&input_state, input.field) else {
            continue;
        };
        let segments = state.display_segments(display_kind_for_field(
            input.field,
            input.kind,
            &password_display,
        ));
        text.0 = match input.segment {
            AuthInputTextSegment::BeforeSelection => segments.before_selection,
            AuthInputTextSegment::AfterSelection => segments.after_selection,
        };
    }

    for (cursor, mut node) in &mut field_queries.p2() {
        let Some(state) = active_text_input(&input_state, cursor.field) else {
            node.display = Display::None;
            continue;
        };
        let segments = state.display_segments(display_kind_for_field(
            cursor.field,
            input_kind(cursor.field),
            &password_display,
        ));
        let edge_visible = match cursor.edge {
            AuthInputCursorEdge::SelectionStart => segments.caret_at_selection_start,
            AuthInputCursorEdge::SelectionEnd => !segments.caret_at_selection_start,
        };
        let visible = edge_visible
            && blink.visible
            && session.focus == cursor.field
            && is_field_visible(&session, cursor.field);
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (selection, mut text) in &mut field_queries.p4() {
        let Some(state) = active_text_input(&input_state, selection.field) else {
            continue;
        };
        text.0 = state
            .display_segments(display_kind_for_field(
                selection.field,
                input_kind(selection.field),
                &password_display,
            ))
            .selected;
    }

    for (selection, mut node) in &mut field_queries.p5() {
        let visible = active_text_input(&input_state, selection.field).is_some_and(|state| {
            state.has_selection()
                && session.focus == selection.field
                && is_field_visible(&session, selection.field)
        });
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }

    let totp_digits = normalize_totp_code(&session.totp_code);
    for (digit, mut text) in &mut field_queries.p1() {
        text.0 = totp_digits
            .chars()
            .nth(digit.index)
            .map(|value| value.to_string())
            .unwrap_or_default();
    }

    for (cursor, mut visibility) in &mut field_queries.p3() {
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

fn auth_status_visible(status: &str) -> bool {
    !status.is_empty() && status != "Ready. Enter your gateway account credentials."
}

fn auth_status_tone(status: &str) -> Option<UiSemanticTone> {
    if !auth_status_visible(status) {
        return None;
    }
    let normalized = status.to_ascii_lowercase();
    if normalized.contains("failed")
        || normalized.contains("invalid")
        || normalized.contains("rejected")
        || normalized.contains("missing")
        || normalized.contains("no access token")
    {
        Some(UiSemanticTone::Danger)
    } else if normalized.contains("authenticator") || normalized.contains("code required") {
        Some(UiSemanticTone::Warning)
    } else {
        Some(UiSemanticTone::Info)
    }
}

fn auth_status_title(status: &str, tone: UiSemanticTone) -> &'static str {
    match tone {
        UiSemanticTone::Danger => "Authentication failed",
        UiSemanticTone::Warning if status.to_ascii_lowercase().contains("authenticator") => {
            "Authenticator required"
        }
        UiSemanticTone::Warning => "Action required",
        UiSemanticTone::Success => "Success",
        UiSemanticTone::Info => "Gateway status",
    }
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

fn previous_focus_field(session: &ClientSession, current: FocusField) -> FocusField {
    if session.selected_action == AuthAction::Login && session.totp_challenge_id.is_some() {
        return FocusField::TotpCode;
    }
    match session.selected_action {
        AuthAction::Login => match current {
            FocusField::Password => FocusField::Email,
            _ => FocusField::Password,
        },
    }
}

fn input_kind(field: FocusField) -> TextInputKind {
    match field {
        FocusField::Password => TextInputKind::password(),
        FocusField::Email | FocusField::TotpCode => TextInputKind::Text,
    }
}

fn display_kind_for_field(
    field: FocusField,
    base_kind: TextInputKind,
    password_display: &AuthPasswordDisplayState,
) -> TextInputKind {
    if field == FocusField::Password && password_display.reveal_password {
        TextInputKind::Text
    } else {
        base_kind
    }
}

fn auth_svg_icon_kind(role: AuthUiSvgIconRole, reveal_password: bool) -> AuthUiSvgIconKind {
    match role {
        AuthUiSvgIconRole::Alert => AuthUiSvgIconKind::CircleAlert,
        AuthUiSvgIconRole::Email => AuthUiSvgIconKind::Email,
        AuthUiSvgIconRole::Password => AuthUiSvgIconKind::Password,
        AuthUiSvgIconRole::PasswordVisibilityToggle if reveal_password => AuthUiSvgIconKind::EyeOff,
        AuthUiSvgIconRole::PasswordVisibilityToggle => AuthUiSvgIconKind::Eye,
    }
}

fn auth_svg_icon_color_key(
    role: AuthUiSvgIconRole,
    status_icon: Option<&AuthUiStatusIconSlot>,
) -> AuthUiSvgIconColor {
    match role {
        AuthUiSvgIconRole::Alert => AuthUiSvgIconColor::SemanticForeground(
            status_icon
                .map(|icon| icon.tone)
                .unwrap_or(UiSemanticTone::Danger),
        ),
        AuthUiSvgIconRole::Email
        | AuthUiSvgIconRole::Password
        | AuthUiSvgIconRole::PasswordVisibilityToggle => AuthUiSvgIconColor::Primary,
    }
}

fn auth_svg_icon_color(
    role: AuthUiSvgIconRole,
    status_icon: Option<&AuthUiStatusIconSlot>,
    theme: sidereal_ui::theme::UiTheme,
) -> Color {
    match auth_svg_icon_color_key(role, status_icon) {
        AuthUiSvgIconColor::Primary => theme.colors.primary_color(),
        AuthUiSvgIconColor::SemanticForeground(tone) => tone.foreground_color(theme),
    }
}

fn auth_svg_icon_handle(
    key: AuthUiSvgIconCacheKey,
    color: Color,
    cache: &mut AuthSvgIconHandleCache,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    if let Some(handle) = cache.handles_by_key.get(&key) {
        return Some(handle.clone());
    }

    let (bytes, _) = auth_svg_icon_bytes(key.kind);
    let image = auth_svg_icon_image(bytes, color)?;
    let handle = images.add(image);
    cache.handles_by_key.insert(key, handle.clone());
    Some(handle)
}

fn auth_svg_icon_bytes(kind: AuthUiSvgIconKind) -> (&'static [u8], &'static str) {
    match kind {
        AuthUiSvgIconKind::CircleAlert => (
            include_bytes!("../../../../data/icons/circle-alert.svg"),
            "embedded-auth-circle-alert.svg",
        ),
        AuthUiSvgIconKind::Email => (
            include_bytes!("../../../../data/icons/email.svg"),
            "embedded-auth-email.svg",
        ),
        AuthUiSvgIconKind::Password => (
            include_bytes!("../../../../data/icons/password.svg"),
            "embedded-auth-password.svg",
        ),
        AuthUiSvgIconKind::Eye => (
            include_bytes!("../../../../data/icons/eye.svg"),
            "embedded-auth-eye.svg",
        ),
        AuthUiSvgIconKind::EyeOff => (
            include_bytes!("../../../../data/icons/eye-off.svg"),
            "embedded-auth-eye-off.svg",
        ),
    }
}

fn auth_svg_icon_image(bytes: &[u8], color: Color) -> Option<Image> {
    let source = std::str::from_utf8(bytes)
        .ok()?
        .replace("currentColor", &color_to_svg_hex(color));
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(source.as_bytes(), &options).ok()?;
    let size = tree.size();
    let natural_width = size.width().max(1.0);
    let natural_height = size.height().max(1.0);
    let scale = AUTH_INPUT_ICON_RASTER_PX as f32 / natural_width.max(natural_height);
    let width = (natural_width * scale).ceil().max(1.0) as u32;
    let height = (natural_height * scale).ceil().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut data = pixmap.data().to_vec();
    demultiply_rgba(&mut data);
    Some(Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    ))
}

fn color_to_svg_hex(color: Color) -> String {
    let srgba = color.to_srgba();
    let r = (srgba.red.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (srgba.green.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (srgba.blue.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn demultiply_rgba(data: &mut [u8]) {
    for pixel in data.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        if alpha == 0 || alpha == 255 {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * 255) / alpha).min(255) as u8;
        }
    }
}

fn active_text_input_mut(
    input_state: &mut AuthReusableInputState,
    field: FocusField,
) -> Option<&mut TextInputState> {
    match field {
        FocusField::Email => Some(&mut input_state.email),
        FocusField::Password => Some(&mut input_state.password),
        FocusField::TotpCode => None,
    }
}

fn active_text_input(
    input_state: &AuthReusableInputState,
    field: FocusField,
) -> Option<&TextInputState> {
    match field {
        FocusField::Email => Some(&input_state.email),
        FocusField::Password => Some(&input_state.password),
        FocusField::TotpCode => None,
    }
}

fn sync_session_text_inputs(session: &mut ClientSession, input_state: &AuthReusableInputState) {
    session.email.clone_from(&input_state.email.text);
    session.password.clone_from(&input_state.password.text);
}

fn control_modifier(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)
}

fn command_modifier(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight)
}

fn primary_modifier(keys: &ButtonInput<KeyCode>) -> bool {
    control_modifier(keys) || command_modifier(keys)
}

fn word_modifier(keys: &ButtonInput<KeyCode>) -> bool {
    control_modifier(keys)
        || (cfg!(target_os = "macos")
            && (keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight)))
}

fn handle_text_input_shortcut(
    key_code: KeyCode,
    keys: &ButtonInput<KeyCode>,
    input_state: &mut AuthReusableInputState,
    session: &mut ClientSession,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    match key_code {
        KeyCode::KeyA => {
            if let Some(input) = active_text_input_mut(input_state, session.focus) {
                input.select_all();
                session.ui_dirty = true;
            }
        }
        KeyCode::KeyC => {
            copy_active_selection(input_state, session.focus);
        }
        KeyCode::KeyX => {
            cut_active_selection(input_state, session.focus);
            sync_session_text_inputs(session, input_state);
            session.ui_dirty = true;
        }
        KeyCode::KeyV => {
            paste_into_active_input(input_state, session.focus);
            sync_session_text_inputs(session, input_state);
            session.ui_dirty = true;
        }
        KeyCode::KeyZ if shift => {
            if let Some(input) = active_text_input_mut(input_state, session.focus) {
                input.redo();
            }
            sync_session_text_inputs(session, input_state);
            session.ui_dirty = true;
        }
        KeyCode::KeyZ => {
            if let Some(input) = active_text_input_mut(input_state, session.focus) {
                input.undo();
            }
            sync_session_text_inputs(session, input_state);
            session.ui_dirty = true;
        }
        KeyCode::KeyY => {
            if let Some(input) = active_text_input_mut(input_state, session.focus) {
                input.redo();
            }
            sync_session_text_inputs(session, input_state);
            session.ui_dirty = true;
        }
        _ => {}
    }
}

fn copy_active_selection(input_state: &mut AuthReusableInputState, field: FocusField) {
    if let Some(selected) =
        active_text_input(input_state, field).and_then(TextInputState::copy_selection)
    {
        write_system_clipboard(&selected);
        input_state.clipboard = selected;
    }
}

fn cut_active_selection(input_state: &mut AuthReusableInputState, field: FocusField) {
    if let Some(input) = active_text_input_mut(input_state, field)
        && let Some(selected) = input.cut_selection()
    {
        write_system_clipboard(&selected);
        input_state.clipboard = selected;
    }
}

fn paste_into_active_input(input_state: &mut AuthReusableInputState, field: FocusField) {
    let clipboard = read_system_clipboard().unwrap_or_else(|| input_state.clipboard.clone());
    if clipboard.is_empty() {
        return;
    }
    if let Some(input) = active_text_input_mut(input_state, field) {
        input.insert_text(&clipboard);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_system_clipboard() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard.get_text().ok()
}

#[cfg(target_arch = "wasm32")]
fn read_system_clipboard() -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn write_system_clipboard(value: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(value.to_string());
    }
}

#[cfg(target_arch = "wasm32")]
fn write_system_clipboard(_value: &str) {}

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

fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);
    !is_in_private_use_area && !chr.is_ascii_control()
}

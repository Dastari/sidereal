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


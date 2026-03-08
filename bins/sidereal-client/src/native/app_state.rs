//! App state enums and session-facing resources.

use bevy::prelude::*;
use sidereal_core::gateway_dtos::ReplicationTransportConfig;

use super::resources::HeadlessTransportMode;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[states(scoped_entities)]
pub(crate) enum ClientAppState {
    #[default]
    Auth,
    CharacterSelect,
    WorldLoading,
    AssetLoading,
    InWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthAction {
    Login,
    Register,
    ForgotRequest,
    ForgotConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusField {
    Email,
    Password,
    ResetToken,
    NewPassword,
}

#[derive(Debug, Resource)]
pub(crate) struct ClientSession {
    pub gateway_url: String,
    pub selected_action: AuthAction,
    pub focus: FocusField,
    pub email: String,
    pub password: String,
    pub reset_token: String,
    pub new_password: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub account_id: Option<String>,
    pub player_entity_id: Option<String>,
    pub replication_transport: ReplicationTransportConfig,
    pub status: String,
    pub ui_dirty: bool,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct CharacterSelectionState {
    pub characters: Vec<String>,
    pub selected_player_entity_id: Option<String>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct SessionReadyState {
    pub ready_player_entity_id: Option<String>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct LocalPlayerViewState {
    pub controlled_entity_id: Option<String>,
    pub desired_controlled_entity_id: Option<String>,
    pub selected_entity_id: Option<String>,
    pub detached_free_camera: bool,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct FreeCameraState {
    pub position_xy: Vec2,
    pub initialized: bool,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct OwnedEntitiesPanelState {
    pub last_entity_ids: Vec<String>,
    pub last_selected_id: Option<String>,
    pub last_detached_mode: bool,
}

pub(crate) fn is_active_world_state(
    app_state: &Option<Res<'_, State<ClientAppState>>>,
    headless_mode: &HeadlessTransportMode,
) -> bool {
    app_state.as_ref().is_some_and(|state| {
        matches!(
            state.get(),
            ClientAppState::InWorld | ClientAppState::WorldLoading | ClientAppState::AssetLoading
        )
    }) || headless_mode.0
}

impl Default for ClientSession {
    fn default() -> Self {
        Self {
            gateway_url: std::env::var("GATEWAY_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string()),
            selected_action: AuthAction::Login,
            focus: FocusField::Email,
            email: "pilot@example.com".to_string(),
            password: "very-strong-password".to_string(),
            reset_token: String::new(),
            new_password: "new-very-strong-password".to_string(),
            access_token: None,
            refresh_token: None,
            account_id: None,
            player_entity_id: None,
            replication_transport: ReplicationTransportConfig::default(),
            status: "Ready. F1 Login, F2 Register, F3 Forgot Request, F4 Forgot Confirm."
                .to_string(),
            ui_dirty: true,
        }
    }
}

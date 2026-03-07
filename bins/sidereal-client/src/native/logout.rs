//! Logout: disconnect client and return to auth state.
//!
//! Flow: request (UI menu or window close) sets PendingDisconnectNotify; a separate system
//! sends ClientDisconnectNotifyMessage on ControlChannel, waits one frame, then triggers
//! Disconnect; logout_cleanup clears state and transitions to Auth. The one-frame delay
//! avoids dropping the notify in same-frame disconnect races.

use bevy::prelude::*;
use bevy::window::WindowCloseRequested;
use lightyear::prelude::MessageSender;
use lightyear::prelude::client::{Client, Connected, Disconnect, RawClient};
use sidereal_net::{ClientDisconnectNotifyMessage, ControlChannel};

use super::app_state::*;
use super::assets::LocalAssetManager;
use super::auth_net::AssetBootstrapRequestState;
use super::ecs_util::queue_despawn_if_exists;
use super::resources::{
    BootstrapWatchdogState, ClientAuthSyncState, ClientControlRequestState, ClientInputAckTracker,
    DisconnectRequest, LogoutCleanupRequested, PauseMenuState, PendingDisconnectNotify,
    PendingDisconnectNotifySent,
};

/// Requests logout on window close (native only). Sets PendingDisconnectNotify so the
/// notify is sent before the app exits. Runs in the same states as Escape logout.
#[cfg(not(target_arch = "wasm32"))]
pub fn request_logout_on_window_close_system(
    session: Res<'_, ClientSession>,
    mut pending: ResMut<'_, PendingDisconnectNotify>,
    mut pending_sent: ResMut<'_, PendingDisconnectNotifySent>,
    mut close_reader: MessageReader<'_, '_, WindowCloseRequested>,
) {
    if pending.0.is_some() {
        return;
    }
    let Some(player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    if close_reader.read().next().is_some() {
        pending.0 = Some(player_entity_id.clone());
        pending_sent.0 = false;
    }
}

/// Requests logout: sets PendingDisconnectNotify so the notify is sent before Disconnect.
/// Triggered by explicit UI actions (not direct Escape key handling).
pub fn request_logout_system(
    session: Res<'_, ClientSession>,
    mut disconnect_request: ResMut<'_, DisconnectRequest>,
    mut pending: ResMut<'_, PendingDisconnectNotify>,
    mut pending_sent: ResMut<'_, PendingDisconnectNotifySent>,
) {
    if pending.0.is_some() {
        return;
    }
    if !disconnect_request.0 {
        return;
    }
    disconnect_request.0 = false;
    let Some(player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    pending.0 = Some(player_entity_id.clone());
    pending_sent.0 = false;
}

/// Sends ClientDisconnectNotifyMessage on ControlChannel and triggers Disconnect, then
/// requests cleanup. Kept in a separate system so we can use MessageSender without
/// exceeding system arity in the cleanup system.
pub fn send_disconnect_notify_and_trigger_system(
    mut pending: ResMut<'_, PendingDisconnectNotify>,
    mut pending_sent: ResMut<'_, PendingDisconnectNotifySent>,
    mut cleanup_requested: ResMut<'_, LogoutCleanupRequested>,
    mut senders: Query<
        '_,
        '_,
        &mut MessageSender<ClientDisconnectNotifyMessage>,
        (With<Client>, With<Connected>),
    >,
    client_entities: Query<'_, '_, Entity, With<RawClient>>,
    mut commands: Commands<'_, '_>,
) {
    let Some(player_entity_id) = pending.0.as_ref().cloned() else {
        return;
    };

    if !pending_sent.0 {
        let msg = ClientDisconnectNotifyMessage {
            player_entity_id: player_entity_id.clone(),
        };
        for mut sender in &mut senders {
            sender.send::<ControlChannel>(msg.clone());
        }
        pending_sent.0 = true;
        return;
    }

    pending.0 = None;
    pending_sent.0 = false;
    for entity in &client_entities {
        commands.trigger(Disconnect { entity });
    }
    cleanup_requested.0 = true;
}

#[allow(clippy::too_many_arguments)]
pub fn logout_cleanup_system(
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
    mut session: ResMut<'_, ClientSession>,
    mut remote_registry: ResMut<'_, super::resources::RemoteEntityRegistry>,
    mut entity_registry: ResMut<'_, sidereal_runtime_sync::RuntimeEntityHierarchy>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut auth_state: ResMut<'_, ClientAuthSyncState>,
    mut control_request_state: ResMut<'_, ClientControlRequestState>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut character_selection: ResMut<'_, CharacterSelectionState>,
    mut session_ready: ResMut<'_, SessionReadyState>,
    mut free_camera: ResMut<'_, FreeCameraState>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    mut ack_tracker: ResMut<'_, ClientInputAckTracker>,
    mut cleanup_requested: ResMut<'_, LogoutCleanupRequested>,
    mut pending: ResMut<'_, PendingDisconnectNotify>,
    mut pending_sent: ResMut<'_, PendingDisconnectNotifySent>,
) {
    if !cleanup_requested.0 {
        return;
    }
    cleanup_requested.0 = false;
    next_state.set(ClientAppState::Auth);
    session.account_id = None;
    session.player_entity_id = None;
    session.access_token = None;
    session.refresh_token = None;
    session.status = "Logged out. Back on auth screen.".to_string();
    session.ui_dirty = true;
    remote_registry.by_entity_id.clear();
    entity_registry.by_entity_id.clear();
    entity_registry.pending_children_by_parent_id.clear();
    asset_manager.bootstrap_manifest_seen = false;
    asset_manager.bootstrap_phase_complete = false;
    asset_manager.bootstrap_total_bytes = 0;
    asset_manager.bootstrap_ready_bytes = 0;
    auth_state.sent_for_client_entities.clear();
    auth_state.last_sent_at_s_by_client_entity.clear();
    auth_state.last_player_entity_id = None;
    *control_request_state = ClientControlRequestState::default();
    *player_view_state = LocalPlayerViewState::default();
    *character_selection = CharacterSelectionState::default();
    *session_ready = SessionReadyState::default();
    *free_camera = FreeCameraState::default();
    *watchdog = BootstrapWatchdogState::default();
    *ack_tracker = ClientInputAckTracker::default();
    *pending = PendingDisconnectNotify::default();
    *pending_sent = PendingDisconnectNotifySent::default();
}

pub fn reset_logout_ui_flags_system(
    cleanup_requested: Res<'_, LogoutCleanupRequested>,
    mut disconnect_request: ResMut<'_, DisconnectRequest>,
    mut pause_menu_state: ResMut<'_, PauseMenuState>,
) {
    if !cleanup_requested.0 {
        return;
    }
    *disconnect_request = DisconnectRequest::default();
    *pause_menu_state = PauseMenuState::default();
}

pub fn reset_asset_bootstrap_state_system(
    cleanup_requested: Res<'_, LogoutCleanupRequested>,
    mut asset_bootstrap_state: ResMut<'_, AssetBootstrapRequestState>,
) {
    if cleanup_requested.0 {
        *asset_bootstrap_state = AssetBootstrapRequestState::default();
    }
}

pub fn purge_stale_world_and_transport_on_enter_auth_system(
    mut commands: Commands<'_, '_>,
    raw_clients: Query<'_, '_, Entity, With<RawClient>>,
) {
    for entity in &raw_clients {
        queue_despawn_if_exists(&mut commands, entity);
    }
}

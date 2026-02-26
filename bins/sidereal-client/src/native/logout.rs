//! Logout: disconnect client and return to auth state.

use bevy::prelude::*;
use lightyear::prelude::client::{Disconnect, RawClient};

use super::resources::{
    BootstrapWatchdogState, ClientAuthSyncState, ClientControlRequestState, ClientInputAckTracker,
    LocalAssetManager,
};
use super::state::*;

#[allow(clippy::too_many_arguments)]
pub fn logout_to_auth_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    mut commands: Commands<'_, '_>,
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
    client_entities: Query<'_, '_, Entity, With<RawClient>>,
) {
    if !input.just_pressed(KeyCode::Escape) {
        return;
    }
    // TODO: send ClientDisconnectNotifyMessage before Disconnect so server Unlinks immediately.
    // Client has 16+ system params here; adding MessageSender hits Bevy IntoSystem arity limit.
    for entity in &client_entities {
        commands.trigger(Disconnect { entity });
    }
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
    asset_manager.pending_assets.clear();
    asset_manager.requested_asset_ids.clear();
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
}

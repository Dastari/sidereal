use bevy::prelude::*;

use crate::runtime::app_state::ClientAppState;
use crate::runtime::logout;
use crate::runtime::resources::LogoutCleanupRequested;

pub(super) fn add_logout_systems(app: &mut App) {
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(
        PreUpdate,
        (
            logout::request_logout_system.run_if(in_state(ClientAppState::InWorld)),
            logout::request_logout_system.run_if(in_state(ClientAppState::WorldLoading)),
            logout::request_logout_system.run_if(in_state(ClientAppState::AssetLoading)),
            logout::request_logout_system.run_if(in_state(ClientAppState::CharacterSelect)),
            logout::request_logout_on_window_close_system.run_if(in_state(ClientAppState::InWorld)),
            logout::request_logout_on_window_close_system
                .run_if(in_state(ClientAppState::WorldLoading)),
            logout::request_logout_on_window_close_system
                .run_if(in_state(ClientAppState::AssetLoading)),
            logout::request_logout_on_window_close_system
                .run_if(in_state(ClientAppState::CharacterSelect)),
            logout::send_disconnect_notify_and_trigger_system,
            logout::reset_asset_bootstrap_state_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::reset_asset_hot_reload_state_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::reset_logout_ui_flags_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::logout_cleanup_system.run_if(resource_equals(LogoutCleanupRequested(true))),
        )
            .chain(),
    );

    #[cfg(target_arch = "wasm32")]
    app.add_systems(
        PreUpdate,
        (
            logout::request_logout_system.run_if(in_state(ClientAppState::InWorld)),
            logout::request_logout_system.run_if(in_state(ClientAppState::WorldLoading)),
            logout::request_logout_system.run_if(in_state(ClientAppState::AssetLoading)),
            logout::request_logout_system.run_if(in_state(ClientAppState::CharacterSelect)),
            logout::send_disconnect_notify_and_trigger_system,
            logout::reset_asset_bootstrap_state_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::reset_asset_hot_reload_state_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::reset_logout_ui_flags_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::logout_cleanup_system.run_if(resource_equals(LogoutCleanupRequested(true))),
        )
            .chain(),
    );
}

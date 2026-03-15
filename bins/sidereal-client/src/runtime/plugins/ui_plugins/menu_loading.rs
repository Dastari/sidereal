use bevy::prelude::*;

use crate::runtime::app_state::ClientAppState;
use crate::runtime::camera::gate_menu_camera_system;
use crate::runtime::{
    asset_loading_ui, audio, auth_net, scene, startup_assets, startup_loading_ui, world_loading_ui,
};

pub(super) fn add_audio_state_systems(app: &mut App) {
    app.add_systems(
        Update,
        audio::ensure_menu_music_system.run_if(in_state(ClientAppState::StartupLoading)),
    );
    app.add_systems(
        Update,
        audio::ensure_menu_music_system.run_if(in_state(ClientAppState::Auth)),
    );
    app.add_systems(
        Update,
        audio::ensure_menu_music_system.run_if(in_state(ClientAppState::WorldLoading)),
    );
    app.add_systems(
        Update,
        audio::ensure_world_music_system.run_if(in_state(ClientAppState::InWorld)),
    );
}

pub(super) fn add_menu_and_loading_ui_systems(app: &mut App) {
    app.add_systems(Update, scene::handle_character_select_buttons);
    app.add_systems(
        Update,
        scene::sync_character_select_button_visuals
            .run_if(in_state(ClientAppState::CharacterSelect)),
    );
    app.add_systems(Update, startup_assets::poll_startup_asset_request_results);
    app.add_systems(Update, auth_net::poll_gateway_request_results);
    app.add_systems(Update, auth_net::poll_asset_bootstrap_request_results);
    app.add_systems(Update, auth_net::trigger_asset_catalog_refresh_requests);
    app.add_systems(
        Update,
        gate_menu_camera_system.run_if(not(in_state(ClientAppState::InWorld))),
    );
    app.add_systems(
        Update,
        startup_loading_ui::update_startup_loading_screen
            .run_if(in_state(ClientAppState::StartupLoading)),
    );
    app.add_systems(
        Update,
        world_loading_ui::update_world_loading_screen
            .run_if(in_state(ClientAppState::WorldLoading)),
    );
    app.add_systems(
        Update,
        asset_loading_ui::update_asset_loading_screen
            .run_if(in_state(ClientAppState::AssetLoading)),
    );
}

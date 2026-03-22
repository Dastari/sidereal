use bevy::log::info;
use bevy::prelude::*;

use crate::runtime::app_state::ClientAppState;
use crate::runtime::{
    asset_loading_ui, auth_net, auth_ui, bootstrap, dialog_ui, logout, replication, scene,
    scene_world, startup_assets, startup_loading_ui, transport, world_loading_ui,
};

pub(crate) struct ClientBootstrapPlugin {
    pub(crate) headless: bool,
}

fn configure_non_headless_bootstrap(app: &mut App) {
    scene::insert_embedded_fonts(app);
    auth_net::init_gateway_request_state(app);
    startup_assets::init_startup_asset_request_state(app);
    app.init_state::<ClientAppState>();
    app.add_systems(
        OnEnter(ClientAppState::StartupLoading),
        (
            startup_loading_ui::setup_startup_loading_screen,
            startup_assets::submit_startup_asset_request_system,
        )
            .chain(),
    );
    app.add_systems(
        OnEnter(ClientAppState::Auth),
        logout::purge_stale_world_and_transport_on_enter_auth_system,
    );
    auth_ui::register_auth_ui(app);
    dialog_ui::register_dialog_ui(app);
    app.add_systems(Startup, scene::spawn_ui_overlay_camera);
    app.add_systems(
        OnEnter(ClientAppState::CharacterSelect),
        scene::setup_character_select_screen,
    );
    app.add_systems(
        OnEnter(ClientAppState::WorldLoading),
        (
            transport::ensure_lightyear_client_system,
            bootstrap::reset_bootstrap_watchdog_on_enter_world_loading,
            world_loading_ui::setup_world_loading_screen,
        )
            .chain(),
    );
    app.add_systems(
        OnEnter(ClientAppState::AssetLoading),
        asset_loading_ui::setup_asset_loading_screen,
    );
    app.add_systems(
        OnEnter(ClientAppState::InWorld),
        (
            transport::ensure_lightyear_client_system,
            scene_world::spawn_world_scene,
            bootstrap::reset_bootstrap_watchdog_on_enter_in_world,
        )
            .chain(),
    );
}

impl Plugin for ClientBootstrapPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(replication::ensure_parent_spatial_components_on_children_added);
        if self.headless {
            app.add_systems(Startup, auth_net::configure_headless_session_from_env);
            app.add_systems(Startup, transport::start_lightyear_client_transport);
            app.add_systems(Startup, || {
                info!("sidereal-client headless transport mode");
            });
            return;
        }
        configure_non_headless_bootstrap(app);
    }
}

pub(crate) struct ClientTransportPlugin {
    pub(crate) headless: bool,
}

fn add_headless_transport_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            auth_net::apply_headless_account_switch_system,
            transport::ensure_client_transport_channels,
            transport::adapt_client_timeline_tuning_for_window_focus,
            transport::handle_unexpected_server_disconnect_system,
            auth_net::send_lightyear_auth_messages,
            auth_net::receive_lightyear_session_ready_messages,
            auth_net::submit_asset_bootstrap_after_session_ready,
        )
            .chain(),
    );
}

fn add_windowed_transport_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            transport::ensure_client_transport_channels,
            transport::adapt_client_timeline_tuning_for_window_focus,
            transport::handle_unexpected_server_disconnect_system,
            auth_net::send_lightyear_auth_messages,
            auth_net::receive_lightyear_session_ready_messages,
            auth_net::submit_asset_bootstrap_after_session_ready,
            auth_net::receive_lightyear_session_denied_messages,
            auth_net::watch_session_ready_timeout_system,
        )
            .chain(),
    );
}

impl Plugin for ClientTransportPlugin {
    fn build(&self, app: &mut App) {
        if self.headless {
            add_headless_transport_systems(app);
        } else {
            add_windowed_transport_systems(app);
        }
    }
}

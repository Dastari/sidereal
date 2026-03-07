#![cfg(feature = "lightyear_protocol")]

use bevy::prelude::App;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::server::ServerPlugins;
use sidereal_game::EntityAction;
use sidereal_net::{
    PlayerInput, register_lightyear_client_protocol, register_lightyear_server_protocol,
};

#[test]
fn lightyear_server_protocol_registration_registers_messages() {
    let mut app = App::new();
    app.add_plugins(ServerPlugins::default());
    register_lightyear_server_protocol(&mut app);
}

#[test]
fn lightyear_client_protocol_registration_registers_messages() {
    let mut app = App::new();
    app.add_plugins(ClientPlugins::default());
    register_lightyear_client_protocol(&mut app);
}

#[test]
fn player_input_matches_axis_mapping() {
    let player_input = PlayerInput::from_axis_inputs(1.0, -1.0, false, false, false);
    assert_eq!(
        player_input.actions,
        vec![
            EntityAction::Forward,
            EntityAction::Right,
            EntityAction::AfterburnerOff
        ]
    );
}

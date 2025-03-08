use bevy::prelude::*;
use bevy_renet::netcode::*;
use bevy_renet::renet::*;
use bevy_renet::*;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime};

pub struct NetworkClientPlugin;

pub const SERVER_ADDR: &str = "127.0.0.1:5000";

impl Plugin for NetworkClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenetClientPlugin);

        let client = RenetClient::new(ConnectionConfig::default());
        app.insert_resource(client);

        // Setup the transport layer
        app.add_plugins(NetcodeClientPlugin);

        let authentication = ClientAuthentication::Unsecure {
            server_addr: SERVER_ADDR.parse().unwrap(),
            client_id: 0,
            user_data: None,
            protocol_id: 0,
        };
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut transport =
            NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

        app.insert_resource(transport);

        app.add_systems(Update, (send_message_system, receive_message_system));
    }
}

// Systems

fn send_message_system(mut client: ResMut<RenetClient>) {
    // Send a text message to the server
    client.send_message(DefaultChannel::ReliableOrdered, "server message");
}

fn receive_message_system(mut client: ResMut<RenetClient>) {
    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        // Handle received message
    }
}

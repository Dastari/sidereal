use crate::ecs::systems::network::{receive_client_message_system, NetworkMessageEvent};
use bevy::prelude::*;
use bevy_renet::netcode::*;
use bevy_renet::renet::*;
use bevy_renet::*;
use std::net::UdpSocket;
use std::time::SystemTime;

pub struct NetworkClientPlugin;

pub const SERVER_ADDR: &str = "127.0.0.1:5000";

impl Plugin for NetworkClientPlugin {
    fn build(&self, app: &mut App) {
        let client = RenetClient::new(ConnectionConfig::default());
        app.add_plugins(RenetClientPlugin);
        app.add_event::<NetworkMessageEvent>();
        app.insert_resource(client);

        // Setup the transport layer
        app.add_plugins(NetcodeClientPlugin);

        let client_id = uuid::Uuid::new_v4();
        let client_id_str = client_id.to_string();

        let authentication = ClientAuthentication::Unsecure {
            server_addr: SERVER_ADDR.parse().unwrap(),
            client_id: client_id.as_u128() as u64,
            user_data: Some({
                let mut user_data = [0; NETCODE_USER_DATA_BYTES];

                // Copy the UUID string bytes into user_data
                let uuid_bytes = client_id_str.as_bytes();
                user_data[..uuid_bytes.len()].copy_from_slice(uuid_bytes);
                user_data
            }),
            protocol_id: 0,
        };
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

        app.insert_resource(transport);
        app.add_systems(Update, receive_client_message_system);
    }
}

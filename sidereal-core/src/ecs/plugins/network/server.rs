use bevy::prelude::*;
use bevy_renet::netcode::*;
use bevy_renet::renet::*;
use bevy_renet::*;
use std::net::UdpSocket;
use std::time::SystemTime;
use crate::ecs::systems::network::{NetworkMessageEvent, send_message_system, receive_server_message_system, handle_server_events};

pub struct NetworkServerPlugin;

impl Plugin for NetworkServerPlugin {
    fn build(&self, app: &mut App) {    
        let server = RenetServer::new(ConnectionConfig::default());

        app.add_systems(Update, handle_server_events);

        app.add_plugins((RenetServerPlugin, NetcodeServerPlugin));
        app.add_event::<NetworkMessageEvent>();
        app.insert_resource(server);

        let server_addr = "127.0.0.1:5000".parse().unwrap();
        let socket = UdpSocket::bind(server_addr).unwrap();
        let server_config = ServerConfig {
            current_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap(),
            max_clients: 64,
            protocol_id: 0,
            public_addresses: vec![server_addr],
            authentication: ServerAuthentication::Unsecure,
        };

        // Extract values before moving server_config
        let protocol_id = server_config.protocol_id;
        let max_clients = server_config.max_clients;

        match NetcodeServerTransport::new(server_config, socket) {
            Ok(transport) => {  
                info!("Server transport created and listening on {}", server_addr);
                info!(" -- Protocol ID: {}", protocol_id);
                info!(" -- Max Clients: {}", max_clients);
                app.insert_resource(transport);
            }
            Err(e) => {
                eprintln!("Failed to create server transport: {:?}", e);
            }
        }
        app.add_systems(
            Update,
            (
                send_message_system,
                receive_server_message_system
            ),
        );
    }
}





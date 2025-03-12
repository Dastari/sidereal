use bevy::prelude::*;
use bevy_renet::renet::*;
use bincode::config;

use super::{NetworkMessage, NetworkMessageEvent};

pub fn receive_server_message_system(
    mut server: ResMut<RenetServer>,
    mut network_message_events: EventWriter<NetworkMessageEvent>,
) {
    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, DefaultChannel::ReliableOrdered)
        {
            match bincode::decode_from_slice::<NetworkMessage, _>(&message, config::standard()) {
                Ok((network_message, _)) => {
                    network_message_events.send(NetworkMessageEvent {
                        client_id,
                        message: network_message,
                    });
                }
                Err(e) => {
                    eprintln!("Failed to decode message: {:?}", e);
                }
            }
        }
    }
}

pub fn receive_client_message_system(
    mut client: ResMut<RenetClient>,
    mut network_message_events: EventWriter<NetworkMessageEvent>,
) {
    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        match bincode::decode_from_slice::<NetworkMessage, _>(&message, config::standard()) {
            Ok((network_message, _)) => {
                network_message_events.send(NetworkMessageEvent {
                    client_id: 0, // Client doesn't need to track its own ID
                    message: network_message,
                });
            }
            Err(e) => {
                eprintln!("Failed to decode message: {:?}", e);
            }
        }
    }
}

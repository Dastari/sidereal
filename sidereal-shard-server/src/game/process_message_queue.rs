use bevy::prelude::*;
use bevy_renet::renet::*;
use sidereal_core::ecs::systems::network::{NetworkMessage, NetworkMessageEvent};

pub fn process_message_queue(
    mut client: ResMut<RenetClient>,
    mut network_message_events: EventReader<NetworkMessageEvent>,
) {
    for event in network_message_events.read() {
        match &event.message {
            NetworkMessage::Ping => {
                println!("Processing: Received Ping from {}", event.client_id);
                let message =
                    bincode::encode_to_vec(&NetworkMessage::Pong, bincode::config::standard())
                        .unwrap();
                client.send_message(DefaultChannel::ReliableOrdered, message);
            }
            NetworkMessage::Pong => {
                println!("Processing: Received Pong from {}", event.client_id);
            }

            _ => {}
        }
    }
}

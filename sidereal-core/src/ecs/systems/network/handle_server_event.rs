use crate::ecs::systems::network::{NetworkMessage, NetworkMessageEvent};
use bevy::prelude::*;
use bevy_renet::renet::*;

// System to detect connections
pub fn handle_server_events(
    mut server_events: EventReader<ServerEvent>,
    mut network_events: EventWriter<NetworkMessageEvent>,
) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!("Shard Server {client_id} connected");
                network_events.send(NetworkMessageEvent {
                    client_id: *client_id,
                    message: NetworkMessage::ShardConnected,
                });
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                network_events.send(NetworkMessageEvent {
                    client_id: *client_id,
                    message: NetworkMessage::ShardDisconnected,
                });
                warn!("Shard Server {client_id} disconnected: {reason}");
            }
        }
    }
}

// Use an exclusive system for serialization and sending
// Exclusive systems can safely access the entire world and all resources
// pub fn process_connections_exclusive(
//     world: &mut World,
// ) {
//     // Import the trait
//     use crate::plugins::EntitySerializer;

//     // Extract client IDs first
//     let client_ids = {
//         let mut client_events = world.resource_mut::<Events<ClientConnectedEvent>>();
//         let mut reader = client_events.get_reader();
//         reader.read(&client_events)
//             .map(|event| event.client_id)
//             .collect::<Vec<_>>()
//     };

//     // Exit early if no events
//     if client_ids.is_empty() {
//         return;
//     }

//     // Extract entity IDs next
//     let entity_ids = world.query_filtered::<Entity, With<Id>>().iter(world).collect::<Vec<_>>();

//     // Serialize entities
//     let serialized_entities = {
//         let mut entities = Vec::new();
//         for entity in entity_ids {
//             if let Ok(serialized) = world.serialize_entity(entity) {
//                 entities.push(serialized);
//             }
//         }
//         entities
//     };

//     // Create the message once
//     let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
//     let state_message = NetworkMessage::EntityUpdates {
//         updated_entities: serialized_entities,
//         timestamp,
//     };
//     let message = bincode::encode_to_vec(state_message, bincode::config::standard()).unwrap_or_else(|e| {
//         error!("Failed to encode message: {:?}", e);
//         Vec::new()
//     });

//     // Send if we have a valid message
//     if !message.is_empty() {
//         println!("Sending message to {} clients", client_ids.len());
//         let mut server = world.resource_mut::<RenetServer>();
//         for client_id in client_ids {
//             server.send_message(client_id, DefaultChannel::ReliableOrdered, message.clone());
//         }
//     }
// }

use bevy::prelude::*;
use bevy_renet::renet::*;
use crate::{ecs::systems::network::NetworkMessage, plugins::SerializedEntity};
use crate::ecs::components::id::Id;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::plugins::EntitySerializer;

// Define events
#[derive(Event)]
pub struct ClientConnectedEvent {
    pub client_id: ClientId,
}

// System to detect connections
pub fn detect_connections(
    mut server_events: EventReader<ServerEvent>, 
    mut connection_events: EventWriter<ClientConnectedEvent>
) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!("Client {client_id} connected");
                connection_events.send(ClientConnectedEvent { client_id: *client_id });
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!("Client {client_id} disconnected: {reason}");
            }
        }
    }
}

// Use an exclusive system for serialization and sending
// Exclusive systems can safely access the entire world and all resources
pub fn process_connections_exclusive(
    world: &mut World,
) {
    // Import the trait
    use crate::plugins::EntitySerializer;
    
    // Extract client IDs first
    let client_ids = {
        let mut client_events = world.resource_mut::<Events<ClientConnectedEvent>>();
        let mut reader = client_events.get_reader();
        reader.read(&client_events)
            .map(|event| event.client_id)
            .collect::<Vec<_>>()
    };
    
    // Exit early if no events
    if client_ids.is_empty() {
        return;
    }
    
    // Extract entity IDs next
    let entity_ids = world.query_filtered::<Entity, With<Id>>().iter(world).collect::<Vec<_>>();
    
    // Serialize entities
    let serialized_entities = {
        let mut entities = Vec::new();
        for entity in entity_ids {
            if let Ok(serialized) = world.serialize_entity(entity) {
                entities.push(serialized);
            }
        }
        entities
    };
    
    // Create the message once
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
    let state_message = NetworkMessage::EntityUpdates {
        updated_entities: serialized_entities,
        timestamp,
    };
    let message = bincode::encode_to_vec(state_message, bincode::config::standard()).unwrap_or_else(|e| {
        error!("Failed to encode message: {:?}", e);
        Vec::new()
    });
    
    // Send if we have a valid message
    if !message.is_empty() {
        let mut server = world.resource_mut::<RenetServer>();
        for client_id in client_ids {
            server.send_message(client_id, DefaultChannel::ReliableOrdered, message.clone());
        }
    }
}

// Register systems
pub fn register_network_systems(app: &mut App) {
    app.add_event::<ClientConnectedEvent>();
    app.add_systems(Update, detect_connections);
    // Use the exclusive system approach which completely avoids conflicts
    app.add_systems(Update, process_connections_exclusive.after(detect_connections));
}
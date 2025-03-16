use crate::ecs::systems::network::{NetworkMessage, NetworkMessageEvent};
use crate::plugins::{SerializedEntity, EntitySerializer};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct PendingEntityUpdates {
    pub entities: Vec<SerializedEntity>,
    pub timestamp: f64,
}

#[derive(Event)]
pub struct ProcessEntitiesEvent;

pub fn receive_entity_updates(
    mut network_events: EventReader<NetworkMessageEvent>,
    mut pending_updates: ResMut<PendingEntityUpdates>,
    mut process_event: EventWriter<ProcessEntitiesEvent>,
) {
    for event in network_events.read() {
        if let NetworkMessage::EntityUpdates {
            updated_entities,
            timestamp,
        } = &event.message
        {
            info!(
                "Received {} entity updates from replication server at timestamp {}",
                updated_entities.len(),
                timestamp,
            );
            let _json =
                serde_json::to_string_pretty(&updated_entities).expect("Failed to convert to JSON");
            println!("JSON Length:{}", _json.len());

            pending_updates.entities = updated_entities.clone();
            pending_updates.timestamp = *timestamp;
            process_event.send(ProcessEntitiesEvent);
        }
    }
}

pub fn process_entity_updates(world: &mut World) {
    // Extract all the data we need upfront
    let pending_entities = {
        let mut pending_updates = match world.get_resource_mut::<PendingEntityUpdates>() {
            Some(updates) => updates,
            None => return,
        };
        if pending_updates.entities.is_empty() {
            return;
        }
        pending_updates.entities.drain(..).collect::<Vec<_>>()
    };

    info!(
        "Processing {} pending entity updates",
        pending_entities.len()
    );
    let deserialized_entities = pending_entities
        .into_iter()
        .filter_map(
            |serialized_entity| match world.deserialize_entity(&serialized_entity) {
                Ok(entity) => Some(entity),
                Err(e) => {
                    error!("Failed to deserialize entity: {}", e);
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    info!(
        "Successfully processed {} entity updates",
        deserialized_entities.len()
    );
}

pub struct EntityUpdatePlugin;

impl Plugin for EntityUpdatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingEntityUpdates>()
            .add_event::<ProcessEntitiesEvent>();
        app.add_systems(Update, receive_entity_updates);
        app.add_systems(Update, process_entity_updates);
    }
}

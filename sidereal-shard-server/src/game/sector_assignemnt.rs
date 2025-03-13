use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_renet::renet::*;
use sidereal_core::ecs::components::InSector;
use sidereal_core::ecs::systems::network::{NetworkMessage, NetworkMessageEvent};
use sidereal_core::ecs::systems::sectors::SectorCoord;
use sidereal_core::plugins::SerializedEntity;
use sidereal_core::EntitySerializer;
use std::collections::HashSet;
use std::time::Duration;

#[derive(Resource)]
pub struct ShardSectorAssignments {
    pub assigned_sectors: HashSet<SectorCoord>,
}

impl Default for ShardSectorAssignments {
    fn default() -> Self {
        Self {
            assigned_sectors: HashSet::new(),
        }
    }
}

#[derive(Resource, Default)]
pub struct PendingEntityUpdates {
    pub entities: Vec<SerializedEntity>,
    pub timestamp: f64,
}

#[derive(Event)]
pub struct ProcessEntitiesEvent;

pub fn handle_sector_assignments(
    mut client: ResMut<RenetClient>,
    mut assignments: ResMut<ShardSectorAssignments>,
    mut network_events: EventReader<NetworkMessageEvent>,
) {
    for event in network_events.read() {
        match &event.message {
            NetworkMessage::AssignSectors { sectors } => {
                info!("Received {} sector assignments", sectors.len());
                for sector in sectors {
                    assignments.assigned_sectors.insert(*sector);
                }
                let confirm_message = bincode::encode_to_vec(
                    &NetworkMessage::SectorAssignmentConfirm {
                        sectors: sectors.clone(),
                    },
                    bincode::config::standard(),
                )
                .unwrap();
                client.send_message(DefaultChannel::ReliableOrdered, confirm_message);
            }
            NetworkMessage::RevokeSectors { sectors } => {
                info!("Server revoked {} sector assignments", sectors.len());
                for sector in sectors {
                    assignments.assigned_sectors.remove(sector);
                }
            }
            _ => {}
        }
    }
}

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
            let _json = serde_json::to_string_pretty(&updated_entities).expect("Failed to convert to JSON");
            println!("{}", _json);

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

    let assigned_sectors = {
        match world.get_resource::<ShardSectorAssignments>() {
            Some(assignments) => assignments.assigned_sectors.clone(),
            None => return,
        }
    };

    info!(
        "Processing {} pending entity updates",
        pending_entities.len()
    );

    // Process entities in two phases
    let mut entity_count = 0;
    let mut to_despawn = Vec::new();

    // Phase 1: Deserialize entities
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

    // Phase 2: Check sectors and mark for despawning if needed
    for entity in &deserialized_entities {
        let entity_has_valid_sector = world.get_entity(*entity)
            .map(|entity_ref| {
                if let Some(in_sector) = entity_ref.get::<InSector>() {
                    if assigned_sectors.contains(&in_sector.0) {
                        info!("Deserialized entity {} for assigned sector", entity.index());
                        true
                    } else {
                        warn!(
                            "Deserialized entity {} for unassigned sector {:?}, marking for despawn",
                            entity.index(),
                            in_sector.0
                        );
                        false
                    }
                } else {
                    info!("Deserialized entity {} without InSector", entity.index());
                    true
                }
            })
            .unwrap_or_else(|_| {
                warn!("Deserialized entity {} no longer exists", entity.index());
                false
            });

        if entity_has_valid_sector {
            entity_count += 1;
        } else {
            to_despawn.push(*entity);
        }
    }

    // Phase 3: Despawn entities that need to be removed
    for entity in to_despawn {
        world.despawn(entity);
    }

    info!("Successfully processed {} entity updates", entity_count);
}

pub fn report_shard_load(mut client: ResMut<RenetClient>, _time: Res<Time>) {
    let load_factor = 0.5;
    let load_message = bincode::encode_to_vec(
        &NetworkMessage::SectorLoadReport { load_factor },
        bincode::config::standard(),
    )
    .unwrap();
    client.send_message(DefaultChannel::ReliableOrdered, load_message);
}

pub struct SectorAssignmentPlugin;

impl Plugin for SectorAssignmentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShardSectorAssignments>()
            .init_resource::<PendingEntityUpdates>()
            .add_event::<ProcessEntitiesEvent>();
        app.add_systems(Update, handle_sector_assignments);
        app.add_systems(Update, receive_entity_updates);
        app.add_systems(
            Update,
            report_shard_load.run_if(on_timer(Duration::from_secs(30))),
        );
        app.add_systems(Update, process_entity_updates);
    }
}

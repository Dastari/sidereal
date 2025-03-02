use bevy::math::{IVec2, Vec2};
use bevy::prelude::*;
use bevy_replicon::core::ClientId;
use bevy_replicon::prelude::ConnectedClients;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::netcode::{
    NetcodeServerTransport, ServerAuthentication, ServerSetupConfig,
};
use bevy_replicon_renet2::renet2::ServerEvent;
use bevy_replicon_renet2::renet2::{self, RenetServer};
use bevy_replicon_renet2::RenetChannelsExt;
use bevy_replicon_renet2::RepliconRenetServerPlugin;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::scene::SceneState;
use sidereal_core::ecs::plugins::replication::network::{NetworkConfig, RepliconServerPlugin};

/// Plugin for handling all replication tasks
pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        info!("Building replication plugin");

        app.insert_resource(NetworkConfig {
            server_address: "127.0.0.1".to_string(),
            port: 5000,
            protocol_id: 1,
            max_clients: 10,
        });
        app.add_plugins(RepliconServerPlugin);

        // Register replication events
        app.add_event::<ReplicationEvent>()
            .add_event::<ShardServerConnectionEvent>()
            .add_event::<ClusterAssignmentEvent>()
            .add_event::<EntityTransferEvent>();

        // Add systems for messaging
        app.add_systems(Update, (process_client_messages,));

        // Add systems for the Ready state
        app.add_systems(
            Update,
            (process_entity_transfer_requests, handle_replication_events)
                .run_if(in_state(SceneState::Ready)),
        );

        // Get the resource once before using it
        let network_config = app.world_mut().get_resource::<NetworkConfig>().unwrap();
        info!(
            "Replication server started at {}:{}",
            network_config.server_address, network_config.port
        );
    }
}

/// Resource to track clients that have already received a welcome message
#[derive(Resource, Default)]
struct WelcomedClients(HashSet<ClientId>);

#[derive(Event)]
pub enum ShardServerConnectionEvent {
    Connected { client_id: u64, shard_id: Uuid },
    Disconnected { client_id: u64, shard_id: Uuid },
    Authenticated { client_id: u64, shard_id: Uuid },
}

// Events for cluster assignment
#[derive(Event)]
pub struct ClusterAssignmentEvent {
    pub shard_id: Uuid,
    pub client_id: u64,
    pub clusters: Vec<ClusterAssignment>,
}

#[derive(Clone)]
pub struct ClusterAssignment {
    pub id: Uuid,
    pub base_coordinates: IVec2,
    pub size: IVec2,
}

// Events for entity transfer between shards
#[derive(Event)]
pub enum EntityTransferEvent {
    Request {
        entity_id: Entity,
        source_shard_id: Uuid,
        destination_shard_id: Uuid,
        position: Vec2,
        velocity: Vec2,
    },
    Acknowledge {
        entity_id: Entity,
        destination_shard_id: Uuid,
        transfer_time: f64,
    },
}

/// Events for entity replication
#[derive(Event)]
pub enum ReplicationEvent {
    EntityUpdated { entity: Entity, cluster_id: Uuid },
    EntityCreated { entity: Entity, cluster_id: Uuid },
    EntityDeleted { entity: Entity, cluster_id: Uuid },
}

/// Process client messages
fn process_client_messages(
    mut server: Option<ResMut<RenetServer>>,
    connected_clients: Res<bevy_replicon::prelude::ConnectedClients>,
) {
    if let Some(mut server) = server {
        for client in connected_clients.iter() {
            let client_id = client.id().get();

            // Check for messages on channel 0
            while let Some(message) = server.receive_message(client_id, 0) {
                if let Ok(msg_str) = std::str::from_utf8(&message) {
                    debug!("Message from client {}: {}", client_id, msg_str);

                    // Send acknowledgment
                    let ack_msg = format!(
                        "{{\"type\":\"ack\",\"time\":{}}}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs_f64()
                    );

                    if server.can_send_message(client_id, 0, ack_msg.len()) {
                        server.send_message(client_id, 0, ack_msg.into_bytes());
                    }
                } else {
                    debug!(
                        "Binary message from client {}: {} bytes",
                        client_id,
                        message.len()
                    );
                }
            }
        }
    }
}

/// Handle replication events
fn handle_replication_events(mut events: EventReader<ReplicationEvent>) {
    for event in events.read() {
        match event {
            ReplicationEvent::EntityUpdated { entity, cluster_id } => {
                debug!("Entity {:?} updated in cluster {}", entity, cluster_id);
            }
            ReplicationEvent::EntityCreated { entity, cluster_id } => {
                debug!("Entity {:?} created in cluster {}", entity, cluster_id);
            }
            ReplicationEvent::EntityDeleted { entity, cluster_id } => {
                debug!("Entity {:?} deleted in cluster {}", entity, cluster_id);
            }
        }
    }
}

/// Process entity transfer requests
fn process_entity_transfer_requests(mut events: EventReader<EntityTransferEvent>) {
    for event in events.read() {
        match event {
            EntityTransferEvent::Request {
                entity_id,
                source_shard_id,
                destination_shard_id,
                ..
            } => {
                debug!(
                    "Entity transfer request: Entity {:?} from shard {} to shard {}",
                    entity_id, source_shard_id, destination_shard_id
                );
            }
            EntityTransferEvent::Acknowledge {
                entity_id,
                destination_shard_id,
                transfer_time,
            } => {
                debug!(
                    "Entity transfer acknowledged: Entity {:?} to shard {} at time {}",
                    entity_id, destination_shard_id, transfer_time
                );
            }
        }
    }
}

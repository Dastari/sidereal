use bevy::prelude::*;
use tracing::{info, warn, error};
use bevy_replicon::prelude::ClientId;
use bevy::math::Vec2;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::collections::HashMap;

// Message types for direct shard-to-shard communication
#[derive(Serialize, Deserialize, Debug)]
pub enum P2PMessage {
    Heartbeat {
        timestamp: f64,
        shard_id: Uuid,
    },
    EntityUpdate {
        timestamp: f64,
        shard_id: Uuid,
        entities: Vec<ShadowEntityData>,
    },
    EntityAck {
        timestamp: f64,
        shard_id: Uuid,
        entity_ids: Vec<Entity>,
    },
}

// Shadow entity data for P2P communication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShadowEntityData {
    pub entity_id: Entity,
    pub position: Vec2,
    pub velocity: Vec2,
    pub components: String, // Serialized components
}

// Direction of a boundary between shards
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum BoundaryDirection {
    North,
    East,
    South,
    West,
}

impl BoundaryDirection {
    pub fn opposite(&self) -> Self {
        match self {
            BoundaryDirection::North => BoundaryDirection::South,
            BoundaryDirection::East => BoundaryDirection::West,
            BoundaryDirection::South => BoundaryDirection::North,
            BoundaryDirection::West => BoundaryDirection::East,
        }
    }
}

// Status of a P2P connection
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Failed,
}

// Connection to another shard
pub struct P2PConnection {
    pub shard_id: Uuid,
    pub client_id: ClientId,
    pub boundary_direction: BoundaryDirection,
    pub last_activity: f64,
    pub status: ConnectionStatus,
}

// Resource to track P2P connections
#[derive(Resource, Default)]
pub struct ShardP2PConnections {
    pub connections: HashMap<Uuid, P2PConnection>,
}

// Events for P2P connection management
#[derive(Event)]
pub enum P2PConnectionEvent {
    Connect {
        shard_id: Uuid,
        host: String,
        port: u16,
        boundary_direction: BoundaryDirection,
    },
    Disconnect {
        shard_id: Uuid,
    },
}

// Events for shadow entity updates
#[derive(Event)]
pub struct ShadowEntitySendEvent {
    pub shard_id: Uuid,
    pub entities: Vec<ShadowEntityData>,
}

#[derive(Event)]
pub struct ShadowEntityReceiveEvent {
    pub shard_id: Uuid,
    pub entities: Vec<ShadowEntityData>,
}

// Plugin for P2P communication
pub struct ShardP2PPlugin;

impl Plugin for ShardP2PPlugin {
    fn build(&self, app: &mut App) {
        info!("Building shard P2P communication plugin");
        
        // Register resources
        app.init_resource::<ShardP2PConnections>();
        
        // Register events
        app.add_event::<P2PConnectionEvent>()
           .add_event::<ShadowEntitySendEvent>()
           .add_event::<ShadowEntityReceiveEvent>();
        
        // Add systems
        app.add_systems(Update, (
            handle_p2p_connection_events,
            process_p2p_connections,
            send_heartbeats.run_if(|res: Option<Res<ShardP2PConnections>>| res.is_some()),
            handle_connection_timeouts,
        ));
    }
}

// System to handle P2P connection events
fn handle_p2p_connection_events(
    mut events: EventReader<P2PConnectionEvent>,
    mut p2p_connections: ResMut<ShardP2PConnections>,
    time: Res<Time>,
) {
    for event in events.read() {
        match event {
            P2PConnectionEvent::Connect { shard_id, host, port, boundary_direction } => {
                // Skip if we already have a connection to this shard
                if p2p_connections.connections.contains_key(shard_id) {
                    continue;
                }
                
                info!("Initiating P2P connection to shard {} at {}:{}", shard_id, host, port);
                
                // In a real implementation, this would establish a connection
                // For now, we'll just simulate it
                let client_id = ClientId::new(rand::random::<u64>());
                
                // Add the connection
                p2p_connections.connections.insert(
                    *shard_id,
                    P2PConnection {
                        shard_id: *shard_id,
                        client_id,
                        boundary_direction: *boundary_direction,
                        last_activity: time.elapsed_secs_f64(),
                        status: ConnectionStatus::Connecting,
                    }
                );
                
                info!("P2P connection to shard {} initiated", shard_id);
            },
            P2PConnectionEvent::Disconnect { shard_id } => {
                if let Some(connection) = p2p_connections.connections.remove(shard_id) {
                    info!("Disconnected P2P connection to shard {}", shard_id);
                }
            }
        }
    }
}

// System to process P2P connections
fn process_p2p_connections(
    mut p2p_connections: ResMut<ShardP2PConnections>,
    time: Res<Time>,
) {
    // In a real implementation, this would process connection status
    // For now, we'll just simulate connections becoming active
    for (shard_id, connection) in p2p_connections.connections.iter_mut() {
        if connection.status == ConnectionStatus::Connecting {
            // Simulate connection becoming active after a short delay
            if time.elapsed_secs_f64() - connection.last_activity > 1.0 {
                info!("P2P connection to shard {} is now active", shard_id);
                connection.status = ConnectionStatus::Connected;
                connection.last_activity = time.elapsed_secs_f64();
            }
        }
    }
}

// System to send heartbeats to connected shards
fn send_heartbeats(
    mut p2p_connections: ResMut<ShardP2PConnections>,
    time: Res<Time>,
) {
    // In a real implementation, this would send heartbeats
    // For now, we'll just update the last activity time
    for (_, connection) in p2p_connections.connections.iter_mut() {
        if connection.status == ConnectionStatus::Connected {
            // Only send heartbeats every 5 seconds
            if time.elapsed_secs_f64() - connection.last_activity > 5.0 {
                info!("Sending heartbeat to shard {}", connection.shard_id);
                connection.last_activity = time.elapsed_secs_f64();
            }
        }
    }
}

// System to handle connection timeouts
fn handle_connection_timeouts(
    mut p2p_connections: ResMut<ShardP2PConnections>,
    time: Res<Time>,
) {
    // In a real implementation, this would detect timeouts
    // For now, we'll just simulate it
    let current_time = time.elapsed_secs_f64();
    
    // Collect shards to disconnect
    let timed_out_shards: Vec<Uuid> = p2p_connections.connections.iter()
        .filter(|(_, conn)| {
            conn.status == ConnectionStatus::Connected && 
            current_time - conn.last_activity > 15.0
        })
        .map(|(shard_id, _)| *shard_id)
        .collect();
    
    // Remove timed out connections
    for shard_id in timed_out_shards {
        if let Some(_) = p2p_connections.connections.remove(&shard_id) {
            info!("P2P connection to shard {} timed out", shard_id);
        }
    }
} 
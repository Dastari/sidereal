use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use std::fmt::Display;

/// The status of a replication client.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplicationClientStatus {
    /// The client is connected to the server.
    Connected { client_id: Option<u64> },
    /// The client is connecting to the server.
    Connecting,
    /// The client is disconnected from the server.
    Disconnected,
}

/// An entity's replication state
#[derive(Debug, Clone, PartialEq)]
pub enum EntityState {
    /// The entity is being created
    Creating,
    /// The entity is being updated
    Updating,
    /// The entity is being despawned
    Despawning,
}

/// The type of entity update
#[derive(Debug, Clone, PartialEq)]
pub enum EntityUpdateType {
    /// Full entity update with all components
    Full,
    /// Partial update with only changed components
    Partial(Vec<ComponentId>),
    /// Position-only update for spatial tracking
    Position,
}

impl Display for EntityUpdateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityUpdateType::Full => write!(f, "Full"),
            EntityUpdateType::Partial(components) => {
                write!(f, "Partial({} components)", components.len())
            }
            EntityUpdateType::Position => write!(f, "Position"),
        }
    }
}

/// Network configuration for replication
#[derive(Resource)]
pub struct NetworkConfig {
    /// Server address
    pub server_address: String,
    /// Server port
    pub port: u16,
    /// Network protocol ID
    pub protocol_id: u64,
    /// Maximum clients that can connect
    pub max_clients: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            server_address: "127.0.0.1".to_string(),
            port: 5000,
            protocol_id: 0,
            max_clients: 64,
        }
    }
}

use bevy::prelude::*;

// Re-export the core implementations to maintain compatibility with existing code
pub use sidereal_core::ecs::plugins::replication::client::ReplicationClient;
pub use sidereal_core::ecs::plugins::replication::common::{
    EntityUpdateType, ReplicationClientStatus, EntityState
}; 
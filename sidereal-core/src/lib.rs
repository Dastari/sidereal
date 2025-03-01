// Sidereal Core Library
//
// This is the main entry point for the sidereal-core crate.

pub mod ecs;

// Re-export the replication modules for easier access
pub use ecs::plugins::replication::{
    client::ReplicationClientPlugin,
    common::{
        ReplicationClientStatus, EntityUpdateType, ClientStreamEvent,
        EntityState, get_backoff_time, MAX_CONNECTION_ATTEMPTS,
    },
    network::{ConnectionConfig, RepliconSetup},
};

/// Initialize the core library
pub fn init() {
    println!("Sidereal Core initialized");
} 
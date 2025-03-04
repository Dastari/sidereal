//! Replication module for network synchronization
//! 
//! This module contains the plugins and components for network replication
//! using bevy_replicon and renet2 for transport.

pub mod client;
pub mod server;
pub mod common;
pub mod config;

pub use client::RepliconRenetClientPlugin;
pub use server::RepliconRenetServerPlugin;
pub use common::{EntityState, EntityUpdateType, NetworkConfig, ReplicationClientStatus};
pub use config::ReplicationConfig;

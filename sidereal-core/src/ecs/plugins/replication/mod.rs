//! Replication module for network synchronization
//!
//! This module contains the plugins and components for network replication
//! using bevy_replicon and renet2 for transport.

pub mod client;
pub mod common;
pub mod config;
pub mod server;

pub use client::RepliconRenetClientPlugin;
pub use common::{EntityState, EntityUpdateType, NetworkConfig, ReplicationClientStatus};
pub use config::ReplicationConfig;
pub use server::RepliconRenetServerPlugin;

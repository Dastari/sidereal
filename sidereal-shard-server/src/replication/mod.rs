pub mod client;
pub mod p2p;
pub mod plugin;

pub use plugin::ReplicationPlugin;
pub use client::ReplicationClient;
// systems.rs contains mock implementation code, removing it
// mod systems; 
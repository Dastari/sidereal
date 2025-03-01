use bevy::prelude::*;
use dotenv::dotenv;
use std::env;
use std::net::SocketAddr;
use tracing::info;
use uuid::Uuid;

/// Plugin for initializing and managing server configuration
pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        info!("Building config plugin");
        
        // Load environment variables
        dotenv().ok();
        
        // Create shard configuration from environment
        let shard_config = ShardConfig::from_env();
        let physics_config = PhysicsConfig::default();
        
        app.insert_resource(shard_config)
           .insert_resource(physics_config);

        // Make sure the app initializes the state
        app.init_state::<ShardState>();
    }
}

/// State enum for the shard server
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ShardState {
    #[default]
    Starting,
    Connecting,
    Ready,
    Stopping,
    Error,
}

/// Configuration for the shard server
#[derive(Resource)]
pub struct ShardConfig {
    pub shard_id: Uuid,
    pub replication_server_address: String,
    pub replication_server_port: u16,
    pub local_address: SocketAddr,
    pub entity_capacity: usize,
    pub network_protocol_id: u64,
}

impl ShardConfig {
    pub fn from_env() -> Self {
        // Generate a new UUID for this shard if not provided
        let shard_id = env::var("SHARD_ID")
            .map(|id| Uuid::parse_str(&id).expect("Invalid SHARD_ID format"))
            .unwrap_or_else(|_| {
                let id = Uuid::new_v4();
                info!("No SHARD_ID provided, generated new ID: {}", id);
                id
            });
        
        // Get the replication server address or use default
        let replication_server_address = env::var("REPLICATION_SERVER_ADDRESS")
            .unwrap_or_else(|_| {
                info!("Using default replication server address: 127.0.0.1");
                "127.0.0.1".to_string()
            });
        
        // Get the replication server port or use default
        let replication_server_port = env::var("REPLICATION_SERVER_PORT")
            .map(|port| port.parse::<u16>().expect("Invalid REPLICATION_SERVER_PORT"))
            .unwrap_or_else(|_| {
                info!("Using default replication server port: 5000");
                5000
            });
        
        // Get network protocol ID or use default
        let network_protocol_id = env::var("NETWORK_PROTOCOL_ID")
            .map(|id| id.parse::<u64>().expect("Invalid NETWORK_PROTOCOL_ID"))
            .unwrap_or_else(|_| {
                info!("Using default network protocol ID: 0");
                0
            });
        
        // Get local server address and port
        let local_ip = env::var("SHARD_SERVER_IP")
            .unwrap_or_else(|_| {
                info!("Using default shard server IP: 127.0.0.1");
                "127.0.0.1".to_string()
            });
        
        let local_port = env::var("SHARD_SERVER_PORT")
            .map(|port| port.parse::<u16>().expect("Invalid SHARD_SERVER_PORT"))
            .unwrap_or_else(|_| {
                info!("Using default shard server port: 7777");
                7777
            });
        
        let local_address = format!("{}:{}", local_ip, local_port)
            .parse()
            .expect("Invalid local address format");
        
        // Get entity capacity or use default
        let entity_capacity = env::var("ENTITY_CAPACITY")
            .map(|cap| cap.parse::<usize>().expect("Invalid ENTITY_CAPACITY"))
            .unwrap_or_else(|_| {
                info!("Using default entity capacity: 5000");
                5000
            });
        
        Self {
            shard_id,
            replication_server_address,
            replication_server_port,
            local_address,
            entity_capacity,
            network_protocol_id,
        }
    }
}

/// Physics configuration for the server
#[derive(Resource)]
pub struct PhysicsConfig {
    pub physics_fps: f32,
    pub physics_substeps: usize,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        // Read from environment variables or use defaults
        let physics_fps = env::var("PHYSICS_FPS")
            .map(|fps| fps.parse::<f32>().expect("Invalid PHYSICS_FPS"))
            .unwrap_or(30.0);
        
        let physics_substeps = env::var("PHYSICS_SUBSTEPS")
            .map(|steps| steps.parse::<usize>().expect("Invalid PHYSICS_SUBSTEPS"))
            .unwrap_or(1);
        
        info!("Physics configuration: FPS={}, Substeps={}", physics_fps, physics_substeps);
        
        Self {
            physics_fps,
            physics_substeps,
        }
    }
} 
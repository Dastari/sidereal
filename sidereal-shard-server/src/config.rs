use bevy::prelude::*;
use dotenv::dotenv;
use std::env;
use std::net::SocketAddr;
use tracing::info;
use uuid::Uuid;
use sidereal_core::ecs::plugins::replication::network::ConnectionConfig as CoreConnectionConfig;

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
        
        // Create and add the core connection config
        let connection_config = shard_config.to_connection_config();
        
        app.insert_resource(shard_config)
           .insert_resource(physics_config)
           .insert_resource(connection_config);

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
#[derive(Resource, Clone)]
pub struct ShardConfig {
    /// Unique identifier for this shard
    pub shard_id: Uuid,
    /// Maximum number of entities this shard can handle
    pub entity_capacity: usize,
    /// Local address for the shard server to bind to
    pub local_address: SocketAddr,
    /// Client ID to use when connecting to the replication server
    pub client_id: u64,
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

        // Get client ID or generate one based on shard_id
        let client_id = env::var("CLIENT_ID")
            .map(|id| id.parse::<u64>().expect("Invalid CLIENT_ID"))
            .unwrap_or_else(|_| {
                // Use the last 8 bytes of the UUID as a u64 client ID
                let id_bytes = shard_id.as_bytes();
                let mut client_id_bytes = [0u8; 8];
                client_id_bytes.copy_from_slice(&id_bytes[8..16]);
                let client_id = u64::from_ne_bytes(client_id_bytes);
                info!("Using generated client ID: {} (from shard ID)", client_id);
                client_id
            });
        
        Self {
            shard_id,
            local_address,
            entity_capacity,
            client_id,
        }
    }

    /// Convert this shard config to a core ConnectionConfig
    pub fn to_connection_config(&self) -> CoreConnectionConfig {
        // Get replication server address from environment or use default
        let server_address = env::var("REPLICATION_SERVER_ADDRESS")
            .unwrap_or_else(|_| {
                info!("Using default replication server address: 127.0.0.1");
                "127.0.0.1".to_string()
            });
        
        // Get replication server port from environment or use default
        let port = env::var("REPLICATION_SERVER_PORT")
            .map(|port| port.parse::<u16>().expect("Invalid REPLICATION_SERVER_PORT"))
            .unwrap_or_else(|_| {
                info!("Using default replication server port: 5000");
                5000
            });
        
        // Get protocol ID from environment or use default
        let protocol_id = env::var("NETWORK_PROTOCOL_ID")
            .map(|id| id.parse::<u64>().expect("Invalid NETWORK_PROTOCOL_ID"))
            .unwrap_or_else(|_| {
                info!("Using default network protocol ID: 0");
                0
            });
        
        info!("Creating connection config to replication server at {}:{} with protocol ID {}", 
            server_address, port, protocol_id);
        
        CoreConnectionConfig {
            server_address,
            port,
            protocol_id,
            // This isn't relevant for clients, but set to 1 to satisfy the API
            max_clients: 1,
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
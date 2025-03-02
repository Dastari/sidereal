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
        
        Self {
            shard_id,
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
mod game;

use avian2d::prelude::*;
use bevy::hierarchy::HierarchyPlugin;
use bevy::log::*;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
use bevy_state::app::StatesPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    renet2::RenetClient,
    RepliconRenetPlugins
};

use sidereal::net::{ShardConfig, NetworkConfig, DEFAULT_PROTOCOL_ID, BiDirectionalReplicationSetupPlugin};
use sidereal::net::{NetworkStats, ClientNetworkPlugin};
use sidereal::ecs::plugins::SiderealPlugin;

use tracing::{info, Level};
use std::env;
use std::net::SocketAddr;

/// System to log when new entities are received from the replication server
fn log_received_entities(
    query: Query<Entity, (With<Replicated>, Added<Replicated>)>,
) {
    let count = query.iter().count();
    if count > 0 {
        info!("Received {} new replicated entities from replication server", count);
        
        for entity in query.iter() {
            info!("Received replicated entity: {:?}", entity);
        }
    }
}

/// System to show connection stat

fn main() {
    // Set environment variables first
    #[cfg(debug_assertions)]
    {
        std::env::set_var("RUST_LOG", "info,renetcode2=trace,renet2=debug,bevy_replicon=debug");
    }
    
    // Then initialize logging once
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(Level::DEBUG) // Allow debug logs
        .init();
    
    info!("Starting Sidereal Shard Server");

    // Get shard ID from command line, default to 1
    let args: Vec<String> = env::args().collect();
    let shard_id = if args.len() > 1 {
        args[1].parse::<u64>().unwrap_or(1)
    } else {
        1
    };

    info!("Initializing shard server with ID: {}", shard_id);

    // Configure shard server with default network configuration and dynamic port
    let mut config = ShardConfig::default();
    config.bind_addr = "127.0.0.1:0".parse().unwrap(); // Use port 0 for dynamic port assignment
    config.replication_server_addr = "127.0.0.1:5000".parse().unwrap();
    config.shard_id = shard_id;
    config.protocol_id = DEFAULT_PROTOCOL_ID;
    
    info!("Shard configuration: {:?}", config);
    
    // Initialize the Bevy app with minimal plugins for headless operation
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins((
            TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .add_plugins((
            HierarchyPlugin,
            // RemotePlugin::default(),
            // RemoteHttpPlugin::default()
            //     .with_header("Access-Control-Allow-Origin", "http://localhost:3000")
            //     .with_header(
            //         "Access-Control-Allow-Headers",
            //         "content-type, authorization",
            //     )
            //     .with_header(
            //         "Access-Control-Allow-Methods",
            //         "GET, POST, PUT, DELETE, OPTIONS",
            //     ),
            StatesPlugin::default(),
            PhysicsPlugins::default(),
            // Add bi-directional networking - RepliconPlugins must come before RepliconRenetPlugins
            RepliconPlugins,
            RepliconRenetPlugins,
            // Add the core plugin
            SiderealPlugin,
            // Set up bi-directional replication
            BiDirectionalReplicationSetupPlugin {
                shard_config: Some(config),
                replication_server_config: None,
                known_shard_addresses: Vec::new(),
            },
        ))
        // Add client network plugin separately
        .add_plugins(ClientNetworkPlugin)
        // Add systems to monitor replication
        .add_systems(Update, (
            log_received_entities
        ))
        .run();
}

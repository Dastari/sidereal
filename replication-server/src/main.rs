mod database;
mod game;

use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
use bevy_state::app::StatesPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::RepliconRenetPlugins;

use game::SceneLoaderPlugin;
use sidereal::net::{ReplicationServerConfig, BiDirectionalReplicationSetupPlugin};
use sidereal::net::{NetworkStats, ServerNetworkPlugin};
use sidereal::ecs::plugins::SiderealPlugin;

use tracing::{info, Level};
use std::net::SocketAddr;

fn main() {
    // Initialize tracing
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Sidereal Replication Server");

    // Enable debug tracing for netcode to see raw packet details
    #[cfg(debug_assertions)]
    {
        use std::env;
        env::set_var("RUST_LOG", "info,renetcode2=trace,renet2=debug");

        // Initialize tracing if not already done
        if tracing_subscriber::fmt::Subscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init()
            .is_ok()
        {
            println!("Enhanced logging enabled for netcode debugging");
        }
    }

    // Configure replication server
    let config = ReplicationServerConfig {
        bind_addr: "0.0.0.0:5000".parse().unwrap(),
        protocol_id: 7,
    };
    
    // Known shard addresses - in a real application, this might come from config
    let shard_addresses = vec![
        SocketAddr::new("127.0.0.1".parse().unwrap(), 5001)
    ];
    
    // Initialize the Bevy app with minimal plugins for headless operation
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins((
            RepliconPlugins,
            RepliconRenetPlugins,
            HierarchyPlugin,
            TransformPlugin,
            StatesPlugin::default(),
            RemotePlugin::default(),
            RemoteHttpPlugin::default(),
            // Add the replication core plugin
            SiderealPlugin,
            // Add server-specific networking
            ServerNetworkPlugin,
            // Setup bi-directional replication
            BiDirectionalReplicationSetupPlugin {
                replication_server_config: Some(config),
                shard_config: None,
                known_shard_addresses: shard_addresses,
            },
            // Add scene loader
            SceneLoaderPlugin,
        ))
        .run();
}

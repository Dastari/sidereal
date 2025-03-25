mod database;
mod game;

use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::RepliconRenetPlugins;
use bevy_state::app::StatesPlugin;

use game::SceneLoaderPlugin;
use sidereal::ecs::plugins::SiderealPlugin;
use sidereal::net::{
    BiDirectionalReplicationSetupPlugin, ReplicationServerConfig, ServerNetworkPlugin,
    DEFAULT_PROTOCOL_ID,
};

use tracing::{info, Level};

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

    // Configure replication server with default network configuration
    let mut config = ReplicationServerConfig::default();
    config.bind_addr = "0.0.0.0:5000".parse().unwrap();
    config.protocol_id = DEFAULT_PROTOCOL_ID;

    info!("Replication server configuration: {:?}", config);
    info!("Waiting for shard servers to connect - their addresses will be discovered dynamically");

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
            SiderealPlugin,
            ServerNetworkPlugin,
            BiDirectionalReplicationSetupPlugin {
                replication_server_config: Some(config),
                shard_config: None,
                known_shard_addresses: Vec::new(), // No longer needed
            },
            // Add scene loader
            SceneLoaderPlugin,
        ))
        .run();
}

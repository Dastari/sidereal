mod database;
mod game;

use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    renet2::RenetServer,
    RepliconRenetPlugins
};
use bevy_state::app::StatesPlugin;

use game::SceneLoaderPlugin;
use game::scene_loader::SceneState;
use sidereal::ecs::plugins::SiderealPlugin;
use sidereal::net::{
    BiDirectionalReplicationSetupPlugin, ReplicationServerConfig, ServerNetworkPlugin,
    DEFAULT_PROTOCOL_ID,
};

use tracing::{info, Level, debug};

/// System that runs when scene loading is complete and marks all entities for replication
fn mark_entities_for_replication(
    mut commands: Commands,
    query: Query<Entity, (Without<Replicated>, With<Transform>)>,
    mut next_state: ResMut<NextState<SceneState>>,
) {
    let count = query.iter().count();
    if count > 0 {
        info!("Marking {} entities for replication", count);
        
        for entity in query.iter() {
            commands.entity(entity).insert(Replicated);
        }
        
        info!("All entities marked for replication");
    }
}

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
        // Add system to mark entities for replication when scene loading is complete
        .add_systems(OnEnter(SceneState::Completed), mark_entities_for_replication)
        // Add debug system to show replication status
        .add_systems(Update, debug_replication)
        .run();
}

/// Debug system to monitor replication status
fn debug_replication(
    server: Option<Res<RenetServer>>,
    query: Query<Entity, With<Replicated>>,
) {
    if let Some(server) = server {
        let client_count = server.connected_clients();
        if client_count > 0 {
            debug!("Server has {} connected clients and {} replicated entities", 
                client_count, query.iter().count());
        }
    }
}

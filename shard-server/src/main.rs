mod game;
use avian2d::prelude::*;
use bevy::asset::AssetPlugin;
use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bevy::transform::TransformPlugin;
use bevy_remote::RemotePlugin;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_state::app::StatesPlugin;
use sidereal::ecs::plugins::SiderealPlugin;
use sidereal::net::config::DEFAULT_PROTOCOL_ID;
use sidereal::net::utils::ClientNetworkPlugin;
use sidereal::net::{ShardConfig, ReplicationTopologyPlugin, shard_communication::REPLICATION_SERVER_SHARD_PORT};
use std::env;
use std::time::Duration;
use uuid::Uuid;

use tracing::{Level, info, warn, error};

fn main() {
    #[cfg(debug_assertions)]
    unsafe {
        std::env::set_var(
            "RUST_LOG",
            "info,bevy_app=info,bevy_ecs=info,renetcode2=info,renet2=info,sidereal=debug",
        );
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(Level::DEBUG)
        .init();

    let args: Vec<String> = env::args().collect();

    let shard_id = args
        .get(1)
        .map_or(Ok(1), |arg| arg.parse::<u64>())
        .unwrap_or_else(|e| {
            warn!("Failed to parse shard ID arg: {}. Defaulting to 1.", e);
            1
        });

    info!("Initializing Sidereal Shard Server with ID: {}", shard_id);

    let shard_config = ShardConfig {
        shard_id: Uuid::new_v4(),
        protocol_id: DEFAULT_PROTOCOL_ID,
        replication_server_addr: format!("127.0.0.1:{}", REPLICATION_SERVER_SHARD_PORT)
            .parse()
            .expect("Failed to parse replication server address"),
        bind_addr: "127.0.0.1:0".parse().expect("Failed to parse bind address"),
    };

    info!("Initial shard configuration: {:?}", shard_config);

    App::new()
        .add_plugins(
            MinimalPlugins
                .set(bevy::app::ScheduleRunnerPlugin::run_loop(
                    Duration::from_secs_f64(1.0 / 60.0),
                ))
                .build(),
        )
        .add_plugins((
            TransformPlugin,
            AssetPlugin::default(),
            ScenePlugin,
            HierarchyPlugin,
            PhysicsPlugins::default(),
            StatesPlugin::default(),
            RemotePlugin::default(),
            RemoteHttpPlugin::default()
                .with_header("Access-Control-Allow-Origin", "http://localhost:3000")
                .with_header(
                    "Access-Control-Allow-Headers",
                    "content-type, authorization",
                )
                .with_header(
                    "Access-Control-Allow-Methods",
                    "GET, POST, PUT, DELETE, OPTIONS",
                ),
        ))
        .add_plugins((
            // Use SiderealPlugin with Replicon disabled
            SiderealPlugin::without_replicon(),
            ClientNetworkPlugin,
            // Directly add the ShardClientPlugin which handles communications with the replication server
            ReplicationTopologyPlugin {
                shard_config: Some(shard_config.clone()),
                replication_server_config: None,
            },
        ))
        .insert_resource(shard_config)
        .add_systems(Update, log_status)
        .run();
}

// Simplified logging function
fn log_status(time: Res<Time>) {
    // Log every minute
    let seconds = time.elapsed().as_secs_f64() as u64;
    if seconds % 60 == 0 && seconds > 0 {
        info!("Shard server running for {} seconds", seconds);
    }
}

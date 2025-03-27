mod game;
use avian2d::prelude::*;
use bevy::asset::AssetPlugin;
use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bevy::transform::TransformPlugin;
use bevy_remote::RemotePlugin;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_replicon::prelude::*;
use bevy_state::app::StatesPlugin;
use sidereal::ecs::plugins::SiderealPlugin;
use sidereal::net::config::{DEFAULT_PROTOCOL_ID, DEFAULT_REPLICATION_PORT};
use sidereal::net::{ClientNetworkPlugin, ReplicationTopologyPlugin, ShardConfig};
use std::env;
use std::time::Duration;
use uuid::Uuid;

use tracing::{Level, info, warn};

fn main() {
    #[cfg(debug_assertions)]
    unsafe {
        // TODO: Audit that the environment access only happens in single-threaded code.
        std::env::set_var(
            "RUST_LOG",
            "info,bevy_app=info,bevy_ecs=info,renetcode2=info,renet2=info,bevy_replicon=warn,sidereal=warn",
        );
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(Level::INFO)
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
        replication_server_addr: format!("127.0.0.1:{}", DEFAULT_REPLICATION_PORT)
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
            RepliconPlugins,
            ClientNetworkPlugin,
            ReplicationTopologyPlugin {
                shard_config: Some(shard_config),
                replication_server_config: None,
            },
        ))
        .add_plugins(SiderealPlugin)
        .add_systems(Update, log_received_entities)
        .run();
}

fn log_received_entities(query: Query<Entity, Added<Replicated>>) {
    let count = query.iter().count();
    if count > 0 {
        info!("Shard received {} new entities from server", count);
    }
}

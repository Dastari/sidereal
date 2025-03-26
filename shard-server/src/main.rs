mod game; 
use avian2d::prelude::*;
use bevy::asset::AssetPlugin;
use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bevy::transform::TransformPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2::ServerEvent;
use bevy_replicon_renet2::RepliconRenetPlugins;
use bevy_state::app::StatesPlugin;
use sidereal::ecs::plugins::SiderealPlugin;
use sidereal::net::config::{DEFAULT_PROTOCOL_ID, DEFAULT_REPLICATION_PORT};
use sidereal::net::{ClientNetworkPlugin, ReplicationTopologyPlugin, ShardConfig};
use std::env;
use std::time::Duration;

use tracing::{debug, info, trace, warn, Level};

fn main() {
    #[cfg(debug_assertions)]
    {
        std::env::set_var(
            "RUST_LOG",
            "info,bevy_app=info,bevy_ecs=info,renetcode2=info,renet2=info,bevy_replicon=debug,sidereal=debug",
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
        shard_id,
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
        .add_systems(
            Update,
            (
                log_received_entities,
                log_shard_replicated_entities,
            ),
        )
        .run();
}

fn log_received_entities(
    query: Query<Entity, Added<Replicated>>,
) {
    let count = query.iter().count();
    if count > 0 {
        debug!(
            "Shard received {} new entities from server",
            count
        );
    }
}

fn log_shard_replicated_entities(
    query: Query<Entity, With<Replicated>>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    let current_time = time.elapsed_secs();
    let log_interval = 10.0; // Log count less frequently

    if current_time - *last_log_time > log_interval {
        *last_log_time = current_time;
        let count = query.iter().count();
        if count > 0 {
            debug!(
                "Shard currently managing {} replicated entities (received from server)",
                count
            );
        } else {
            debug!("Shard has no entities marked as Replicated.");
        }
    }
}

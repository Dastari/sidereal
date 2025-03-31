mod database;
mod game;

use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_state::app::StatesPlugin;
use game::SceneLoaderPlugin;
use std::time::Duration;

use game::scene_loader::SceneState;
use sidereal::ecs::components::Object;
use sidereal::ecs::plugins::SiderealPlugin;
use sidereal::net::config::{DEFAULT_PROTOCOL_ID, DEFAULT_REPLICATION_PORT};
use sidereal::net::shard_communication::{ConnectedShards, REPLICATION_SERVER_SHARD_PORT};
use sidereal::net::{ReplicationServerConfig, ReplicationTopologyPlugin, ServerNetworkPlugin};

use tracing::{Level, debug, info};

fn main() {
    #[cfg(debug_assertions)]
    unsafe {
        // TODO: Audit that the environment access only happens in single-threaded code.
        std::env::set_var(
            "RUST_LOG",
            "info,bevy_app=info,bevy_ecs=info,renetcode2=info,renet2=info,bevy_replicon=warn",
        );
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(Level::DEBUG)
        .init();

    info!("Starting Sidereal Replication Server");

    let replication_bind_addr = format!("0.0.0.0:{}", DEFAULT_REPLICATION_PORT)
        .parse()
        .expect("Failed to parse default replication server address");

    let replication_config = ReplicationServerConfig {
        bind_addr: replication_bind_addr,
        protocol_id: DEFAULT_PROTOCOL_ID,
    };

    info!("Replication server configuration: {:?}", replication_config);
    info!("Shard server port: {}", REPLICATION_SERVER_SHARD_PORT);

    App::new()
        .add_plugins(
            MinimalPlugins
                .set(bevy::app::ScheduleRunnerPlugin::run_loop(
                    Duration::from_secs_f64(1.0 / 60.0),
                ))
                .build(),
        )
        .add_plugins((HierarchyPlugin, TransformPlugin, StatesPlugin))
        .add_plugins((
            RepliconPlugins,
            ServerNetworkPlugin,
            ReplicationTopologyPlugin {
                replication_server_config: Some(replication_config),
                shard_config: None,
            },
        ))
        .add_plugins((SiderealPlugin::default().with_replicon(true), SceneLoaderPlugin))
        .add_systems(
            OnEnter(SceneState::Completed),
            mark_entities_for_replication,
        )
        .add_systems(Update, (log_marked_entities, log_shard_connections))
        .run();
}

fn mark_entities_for_replication(
    mut commands: Commands,
    query: Query<Entity, (With<Object>, Without<Replicated>)>,
) {
    let mut count = 0;
    for entity in query.iter() {
        commands.entity(entity).insert(Replicated);
        count += 1;
    }

    if count > 0 {
        info!("Marked {} loaded scene entities for replication", count);
    }
}

fn log_marked_entities(query: Query<(Entity, Option<&Name>), Added<Replicated>>) {
    for (entity, name) in query.iter() {
        if let Some(name) = name {
            debug!(
                "Entity '{:}' ({:?}) marked as Replicated on server",
                name, entity
            );
        } else {
            debug!(
                "Entity {:?} marked as Replicated on server (no name)",
                entity
            );
        }
    }
}

// System to monitor shard connections
fn log_shard_connections(
    shards: Option<Res<ConnectedShards>>,
    time: Res<Time>,
    mut last_log: Local<f64>,
) {
    // Log every 60 seconds
    let current_time = time.elapsed().as_secs_f64();
    if current_time - *last_log > 60.0 {
        *last_log = current_time;

        if let Some(shards) = shards {
            let count = shards.shards.len();
            if count > 0 {
                info!(
                    "Replication server is managing {} connected shard servers",
                    count
                );

                for (client_id, shard) in &shards.shards {
                    info!(
                        "Shard {} (client_id: {}) managing {} sectors",
                        shard.shard_id,
                        client_id,
                        shard.sectors.len()
                    );
                }
            } else {
                debug!("No shard servers connected to replication server");
            }
        } else {
            debug!("Shard tracking system not initialized");
        }
    }
}

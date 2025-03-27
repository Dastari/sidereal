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
use sidereal::net::{ReplicationServerConfig, ReplicationTopologyPlugin, ServerNetworkPlugin};

use tracing::{debug, info, Level};

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

    info!("Starting Sidereal Replication Server");

    let replication_bind_addr = format!("0.0.0.0:{}", DEFAULT_REPLICATION_PORT)
        .parse()
        .expect("Failed to parse default replication server address");

    let replication_config = ReplicationServerConfig {
        bind_addr: replication_bind_addr,
        protocol_id: DEFAULT_PROTOCOL_ID,
    };

    info!("Replication server configuration: {:?}", replication_config);

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
        .add_plugins((SiderealPlugin, SceneLoaderPlugin))
        .add_systems(
            OnEnter(SceneState::Completed),
            mark_entities_for_replication,
        )
        .add_systems(Update, log_marked_entities)
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

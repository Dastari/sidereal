mod database;
mod game;

use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{renet2::ServerEvent, RepliconRenetPlugins};
use bevy_state::app::StatesPlugin;
use std::time::Duration;

use game::scene_loader::SceneState;
use game::SceneLoaderPlugin;
use sidereal::net::{
    BiDirectionalReplicationSetupPlugin, ReplicationServerConfig, ServerNetworkPlugin,
    DEFAULT_PROTOCOL_ID,
};
use sidereal::{ecs::plugins::SiderealPlugin, Object};

use tracing::{info, Level};

fn main() {
    // Set environment variables first
    #[cfg(debug_assertions)]
    {
        std::env::set_var(
            "RUST_LOG",
            "info,renetcode2=trace,renet2=debug,bevy_replicon=debug",
        );
    }
    // Then initialize logging once
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(Level::DEBUG) // Allow debug logs
        .init();

    info!("Starting Sidereal Replication Server");

    // Configure replication server with default network configuration
    let mut config = ReplicationServerConfig::default();
    // Explicitly use IPv4 address
    config.bind_addr = "127.0.0.1:5000".parse().unwrap();
    config.protocol_id = DEFAULT_PROTOCOL_ID;

    info!("Replication server configuration: {:?}", config);

    App::new()
        .add_plugins(MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin {
            run_mode: bevy::app::RunMode::Loop {
                wait: Some(Duration::from_secs_f64(1.0 / 30.0)),
            },
        }))
        .add_plugins((
            RepliconPlugins,
            RepliconRenetPlugins,
            HierarchyPlugin,
            TransformPlugin,
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
            StatesPlugin::default(),
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

        .add_systems(
            OnEnter(SceneState::Completed),
            mark_entities_for_replication,
        )
        .add_systems(Update, mark_clients_for_replication)
        .add_systems(Update, log_received_entities)
        .add_systems(Update, monitor_server_connection_events)
        .run();
}

fn mark_entities_for_replication(
    mut commands: Commands,
    query: Query<Entity, (Without<Replicated>, With<Object>)>,
    mut _next_state: ResMut<NextState<SceneState>>,
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

fn mark_clients_for_replication(
    mut commands: Commands,
    query: Query<Entity, (With<ConnectedClient>, Without<ReplicatedClient>)>,
) {
    for entity in query.iter() {
        info!("Marking client {:?} for replication", entity);
        commands.entity(entity).insert(ReplicatedClient);
    }
}

fn log_received_entities(
    query: Query<(Entity, Option<&Name>), (With<Replicated>, Added<Replicated>)>,
) {
    let count = query.iter().count();
    if count > 0 {
        info!(
            "Replication server received {} new replicated entities",
            count
        );

        for (entity, name) in query.iter() {
            if let Some(name) = name {
                info!("Received replicated entity: {:}", name);
            } else {
                info!("Received replicated entity: {:} (no name)", entity);
            }
        }
    }
}

fn monitor_server_connection_events(mut server_events: EventReader<ServerEvent>) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!("ServerEvent: Client {} connected", client_id);
            }
            ServerEvent::ClientDisconnected { client_id, .. } => {
                info!("ServerEvent: Client {} disconnected", client_id);
            }
        }
    }
}

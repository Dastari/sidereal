mod game;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::asset::AssetPlugin;
use bevy::hierarchy::HierarchyPlugin;
use bevy::scene::ScenePlugin;
use bevy::transform::TransformPlugin;
// No longer need manual Bevy component imports for replication
// use bevy::hierarchy::{Children, Parent};
// use bevy::transform::{Transform, GlobalTransform};
// use bevy::render::view::{InheritedVisibility, ViewVisibility, Visibility};
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
// Replicon networking:
use bevy_replicon::prelude::*;
// We only need ServerSet for ordering if we keep specific systems
use bevy_replicon::server::ServerSet;
// Don't need ServerPlugin manually anymore
// use bevy_replicon::server::ServerPlugin;
// Don't need shared/client manually anymore
// use bevy_replicon::shared::RepliconSharedPlugin;
// use bevy_replicon::client::ClientPlugin;
use bevy_replicon_renet2::renet2::ServerEvent;
use bevy_replicon_renet2::RepliconRenetPlugins;
// State management:
use bevy_state::app::StatesPlugin;
// Sidereal components/logic:
use sidereal::components::Object;
use sidereal::ecs::plugins::SiderealPlugin;
// Sidereal networking config/plugins:
use sidereal::net::config::{DEFAULT_PROTOCOL_ID, DEFAULT_REPLICATION_PORT};
use sidereal::net::{
    BiDirectionalReplicationSetupPlugin, ClientNetworkPlugin, ShardConfig,
};
// Standard library:
use std::env;
// Removed unused IpAddr import
use std::time::Duration;
// Logging:
use tracing::{debug, info, trace, warn, Level};

// Removed configure_replicon_server function

fn main() {
    // --- Logging Setup ---
    #[cfg(debug_assertions)]
    {
        std::env::set_var(
            "RUST_LOG",
            "info,renetcode2=trace,renet2=debug,bevy_replicon=debug,sidereal=debug",
        );
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(Level::DEBUG)
        .init();

    info!("Starting Sidereal Shard Server");

    // --- Configuration ---
    let args: Vec<String> = env::args().collect();
    let shard_id = args.get(1).map_or(1, |arg| arg.parse::<u64>().unwrap_or(1));

    info!("Initializing shard server with ID: {}", shard_id);

    let shard_config = ShardConfig {
        shard_id,
        protocol_id: DEFAULT_PROTOCOL_ID,
        replication_server_addr: format!("127.0.0.1:{}", DEFAULT_REPLICATION_PORT)
            .parse()
            .expect("Failed to parse replication server address"),
        bind_addr: "127.0.0.1:0".parse().expect("Failed to parse bind address"),
    };

    info!("Initial shard configuration: {:?}", shard_config);

    // --- App Setup ---
    App::new()
        .add_plugins(MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin {
            run_mode: bevy::app::RunMode::Loop {
                wait: Some(Duration::from_secs_f64(1.0 / 60.0)),
            },
        }))
        .add_plugins((
            TransformPlugin,
            AssetPlugin::default(),
            ScenePlugin,
            HierarchyPlugin,
        ))
        .add_plugins((
            RemotePlugin::default(),
            RemoteHttpPlugin::default()
                .with_header("Access-Control-Allow-Origin", "http://localhost:3000")
                .with_header("Access-Control-Allow-Headers", "content-type, authorization")
                .with_header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS"),
            StatesPlugin::default(),
            PhysicsPlugins::default(),
        ))
        // --- *** Use Default RepliconPlugins Group Again *** ---
        .add_plugins((RepliconPlugins, RepliconRenetPlugins))
        // --- Sidereal and Custom Plugins/Systems ---
        .add_plugins(SiderealPlugin) // Ensure SiderealPlugin also calls .replicate for its components
        .add_plugins((
            ClientNetworkPlugin, // For the shard's client connection stats
            BiDirectionalReplicationSetupPlugin {
                shard_config: Some(shard_config),
                replication_server_config: None,
            },
        ))
        // Removed manual component registrations
        // Removed Startup system configure_replicon_server
        .add_systems(
            Update,
            (
                log_received_entities,
                mark_shard_entities_for_replication,
                monitor_shard_server_connections,
                log_shard_replicated_entities,
                // System to verify ReplicatedClient is added (optional debug)
                verify_replication_server_marked,
                // Removed original mark_clients_for_replication system
            ),
        )
        .run();
}

// --- Systems ---

/// Debug: Logs entities received from the replication server.
fn log_received_entities(query: Query<Entity, Added<Replicated>>) {
    let count = query.iter().count();
    if count > 0 {
        debug!("Shard received {} new replicated entities", count);
    }
}

// REMOVED mark_clients_for_replication system

/// Optional Debug: Verifies that the replication server's connection entity gets ReplicatedClient.
fn verify_replication_server_marked(
    // Query for the specific entity we expect to be the replication server connection
    // It should have ConnectedClient and ReplicatedClient (added automatically)
    query: Query<Entity, (With<ConnectedClient>, With<ReplicatedClient>, Added<ReplicatedClient>)>,
    server: Option<Res<bevy_replicon_renet2::renet2::RenetServer>>, // To map Entity -> Client ID
    mut logged_once: Local<bool>, // Log only once
) {
    if *logged_once {
        return;
    }
    if let Some(server) = server {
        for entity in query.iter() {
            // Try to confirm it's the replication server based on client ID mapping (if available)
            // Note: This mapping might not be immediately available via RenetServer,
            // but we can check the ID range if we could get the ID.
            // For now, just log that *some* client got marked.
            // We rely on monitor_shard_server_connections to see the actual ID.
            info!("Debug Verify: Entity {:?} was marked with ReplicatedClient (likely replication server)", entity);
            *logged_once = true; // Prevent spamming this log
            break; // Only need to see it once
        }
    }
}


/// Core: Marks shard-owned `Object` entities to be replicated upwards.
fn mark_shard_entities_for_replication(
    mut commands: Commands,
    query: Query<Entity, (With<Object>, Without<Replicated>)>,
    named_query: Query<&Name, With<Object>>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    let current_time = time.elapsed_secs();
    let log_interval = 5.0;
    let should_log = current_time - *last_log_time > log_interval;

    let mut count = 0;
    for entity in query.iter() {
        count += 1;
        commands.entity(entity).insert(Replicated);
        if should_log {
            let entity_name = named_query
                .get(entity)
                .map(|name| name.as_str())
                .unwrap_or("unnamed");
            debug!(
                "Marking shard entity for replication: {:?} ({})",
                entity, entity_name
            );
        }
    }

    if count > 0 && should_log {
        *last_log_time = current_time;
        info!(
            "Marked {} shard entities for replication this interval",
            count
        );
    } else if should_log {
        *last_log_time = current_time;
    }
}

/// Debug: Monitors connections *to* this shard's server component.
fn monitor_shard_server_connections(
    mut server_events: EventReader<ServerEvent>,
    server: Option<Res<bevy_replicon_renet2::renet2::RenetServer>>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    let current_time = time.elapsed_secs();
    let log_interval = 5.0;
    let should_log_list = current_time - *last_log_time > log_interval;

    if should_log_list {
        *last_log_time = current_time;
        if let Some(server) = server.as_ref() {
            let client_count = server.connected_clients();
            if client_count > 0 {
                debug!("SHARD SERVER: {} client(s) connected", client_count);
                for client_id in server.clients_id() {
                     if client_id >= 10000 { // Assuming REPL_CLIENT_ID_OFFSET is 10000
                        debug!(
                            "SHARD SERVER: Replication server connected (ID: {})",
                            client_id
                        );
                    } else {
                        debug!("SHARD SERVER: Other client connected (ID: {})", client_id);
                    }
                }
            } else {
                debug!("SHARD SERVER: No clients connected.");
            }
        }
    }

    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                 if *client_id >= 10000 { // Assuming REPL_CLIENT_ID_OFFSET is 10000
                    info!(
                        "SHARD SERVER: Replication server CONNECTED (ID: {})",
                        client_id
                    );
                } else {
                    info!("SHARD SERVER: Other client CONNECTED (ID: {})", client_id);
                }
            }
            ServerEvent::ClientDisconnected { client_id, .. } => {
                 if *client_id >= 10000 { // Assuming REPL_CLIENT_ID_OFFSET is 10000
                    warn!(
                        "SHARD SERVER: Replication server DISCONNECTED (ID: {})",
                        client_id
                    );
                } else {
                    info!("SHARD SERVER: Other client DISCONNECTED (ID: {})", client_id);
                }
            }
        }
    }
}

/// Debug: Periodically logs all entities marked with `Replicated` on this shard.
fn log_shard_replicated_entities(
    query: Query<Entity, With<Replicated>>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    let current_time = time.elapsed_secs();
    let log_interval = 10.0;

    if current_time - *last_log_time > log_interval {
        *last_log_time = current_time;
        let count = query.iter().count();
        if count > 0 {
            debug!("Shard currently managing {} replicated entities", count);
        } else {
            debug!("Shard has no entities marked for replication.");
        }
    }
}
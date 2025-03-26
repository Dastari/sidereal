mod database; // Assuming these are used by SceneLoaderPlugin or SiderealPlugin
mod game;

use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
use bevy_replicon::prelude::*;
// Removed ServerEvent import as monitor_server_connection_events is removed
// use bevy_replicon_renet2::{renet2::ServerEvent, RepliconRenetPlugins};
use bevy_replicon_renet2::RepliconRenetPlugins;
use bevy_state::app::StatesPlugin;
use std::time::Duration;

use game::scene_loader::SceneState;
use game::SceneLoaderPlugin;
// Import constants directly from config (assuming optimization kept them there)
use sidereal::net::config::{DEFAULT_PROTOCOL_ID, DEFAULT_REPLICATION_PORT};
use sidereal::net::{
    BiDirectionalReplicationSetupPlugin, ReplicationServerConfig, ServerNetworkPlugin,
};
use sidereal::{ecs::plugins::SiderealPlugin, Object}; // Assuming Object is used by mark_entities_for_replication

use tracing::{info, Level}; // Removed unused `debug` if not needed elsewhere

fn main() {
    // --- Logging Setup (Looks Good) ---
    #[cfg(debug_assertions)]
    {
        std::env::set_var(
            "RUST_LOG",
            "info,renetcode2=trace,renet2=debug,bevy_replicon=debug,sidereal=debug", // Added sidereal=debug
        );
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(Level::DEBUG)
        .init();

    info!("Starting Sidereal Replication Server");

    // --- Configuration ---
    // Use default port, but listen on all interfaces (0.0.0.0)
    let replication_bind_addr = format!("0.0.0.0:{}", DEFAULT_REPLICATION_PORT)
        .parse()
        .expect("Failed to parse default replication server address");

    let replication_config = ReplicationServerConfig {
        bind_addr: replication_bind_addr,
        protocol_id: DEFAULT_PROTOCOL_ID,
        // network_config field removed in previous optimization
    };

    info!("Replication server configuration: {:?}", replication_config);

    // --- App Setup ---
    App::new()
        // MinimalPlugins for headless server
        .add_plugins(MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin {
            run_mode: bevy::app::RunMode::Loop {
                // Target ~30hz tick rate
                wait: Some(Duration::from_secs_f64(1.0 / 30.0)),
            },
        }))
        // Core Bevy & External Plugins
        .add_plugins((
            HierarchyPlugin,         // Needed for parent/child relationships
            TransformPlugin,         // Needed for spatial components if scene uses them
            StatesPlugin,            // For SceneLoaderPlugin state machine
            // RemotePlugin::default(), // For bevy-remote debugging
            // RemoteHttpPlugin::default() // For bevy-remote HTTP endpoint
            //     // CORS Headers for bevy-remote frontend
            //     .with_header("Access-Control-Allow-Origin", "http://localhost:3000")
            //     .with_header(
            //         "Access-Control-Allow-Headers",
            //         "content-type, authorization",
            //     )
            //     .with_header(
            //         "Access-Control-Allow-Methods",
            //         "GET, POST, PUT, DELETE, OPTIONS",
            //     ),
        ))
        // Replicon Networking Plugins
        .add_plugins((RepliconPlugins, RepliconRenetPlugins))
        // Sidereal ECS/Game Logic Plugins
        .add_plugins((SiderealPlugin, SceneLoaderPlugin))
        // Custom Networking Plugins
        .add_plugins((
            // Adds server stats updates and logging
            ServerNetworkPlugin,
            // Configures Replicon for the replication server role,
            // handles shard connections, and adds mark_clients_as_replicated system
            BiDirectionalReplicationSetupPlugin {
                replication_server_config: Some(replication_config), // Enable server role
                shard_config: None,                                  // Disable shard role
                                                                     // known_shard_addresses field removed previously
            },
        ))
        // Custom Systems
        .add_systems(
            OnEnter(SceneState::Completed),
            mark_entities_for_replication, // Mark loaded scene entities for replication
        )
        // Removed mark_clients_for_replication (handled by BiDirectional plugin)
        // Debug system (Optional: consider removing or cfg-gating for production)
        .add_systems(Update, log_received_entities)
        // Removed monitor_server_connection_events (logging handled elsewhere)
        .run();
}

/// Marks entities loaded by the scene loader with `Replicated` component.
fn mark_entities_for_replication(
    mut commands: Commands,
    // Query for scene objects that haven't been marked yet
    query: Query<Entity, (With<Object>, Without<Replicated>)>,
    // Removed unused _next_state argument
) {
    let mut count = 0;
    for entity in query.iter() {
        commands.entity(entity).insert(Replicated);
        count += 1;
    }

    if count > 0 {
        info!("Marked {} loaded scene entities for replication", count);
    }
    // No need for "All entities marked" log, the count serves the purpose.
}

// Removed mark_clients_for_replication system (redundant)

/// Debug system to log entities when they gain the `Replicated` component on the server.
/// Might be verbose if many entities are marked at once.
fn log_received_entities(
    // Query for entities that just had `Replicated` added this frame
    query: Query<(Entity, Option<&Name>), Added<Replicated>>,
) {
    let mut count = 0;
    for (entity, name) in query.iter() {
        count += 1;
        if let Some(name) = name {
            debug!("Entity '{:?}' ({:?}) marked as Replicated", name, entity);
        } else {
            debug!("Entity {:?} marked as Replicated (no name)", entity);
        }
    }
    // Log summary only if needed and maybe at trace level?
    // if count > 0 {
    //     trace!("{} entities gained Replicated component this frame", count);
    // }
}

// Removed monitor_server_connection_events system (redundant)

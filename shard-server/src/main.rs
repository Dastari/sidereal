mod game;

use avian2d::prelude::*;
use bevy::hierarchy::HierarchyPlugin;
use bevy::log::*;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    RepliconRenetPlugins,
    renet2::ServerEvent,
};
use bevy_state::app::StatesPlugin;
use sidereal::components::Object;
use sidereal::ecs::plugins::SiderealPlugin;
use sidereal::net::{
    BiDirectionalReplicationSetupPlugin, ClientNetworkPlugin, ShardConfig, DEFAULT_PROTOCOL_ID,
};
use std::env;
use std::time::Duration;

use tracing::{info, Level};

fn main() {
    #[cfg(debug_assertions)]
    {
        std::env::set_var(
            "RUST_LOG",
            "info,renetcode2=trace,renet2=debug,bevy_replicon=debug",
        );
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(Level::DEBUG) // Allow debug logs
        .init();

    info!("Starting Sidereal Shard Server");

    // Get shard ID from command line, default to 1
    let args: Vec<String> = env::args().collect();
    let shard_id = if args.len() > 1 {
        args[1].parse::<u64>().unwrap_or(1)
    } else {
        1
    };

    info!("Initializing shard server with ID: {}", shard_id);

    // Configure shard server with default network configuration and dynamic port
    let mut config = ShardConfig::default();
    config.bind_addr = "127.0.0.1:0".parse().unwrap(); // Use port 0 for dynamic port assignment
    config.replication_server_addr = "127.0.0.1:5000".parse().unwrap();
    config.shard_id = shard_id;
    config.protocol_id = DEFAULT_PROTOCOL_ID;

    info!("Shard configuration: {:?}", config);

    App::new()
        .add_plugins(MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin {
            run_mode: bevy::app::RunMode::Loop {
                wait: Some(Duration::from_secs_f64(1.0 / 60.0)),
            },
        }))
        .add_plugins((
            TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .add_plugins((
            HierarchyPlugin,
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
            PhysicsPlugins::default(),
            // Configure Replicon for full server authority replication
            RepliconPlugins,
            RepliconRenetPlugins,
            SiderealPlugin,
            BiDirectionalReplicationSetupPlugin {
                shard_config: Some(config),
                replication_server_config: None,
                known_shard_addresses: Vec::new(),
            },
            ClientNetworkPlugin,
        ))
        .add_systems(
            Update,
            (
                log_received_entities, 
                mark_shard_entities_for_replication,
                monitor_shard_server_connections,
                log_shard_replicated_entities,
                mark_clients_for_replication,
            ),
        )
        .run();
}

fn log_received_entities(query: Query<Entity, (With<Replicated>, Added<Replicated>)>) {
    let count = query.iter().count();
    if count > 0 {
        info!(
            "Received {} new replicated entities from replication server",
            count
        );

        for entity in query.iter() {
            info!("Received replicated entity: {:?}", entity);
        }
    }
}

fn mark_clients_for_replication(
    mut commands: Commands,
    query: Query<Entity, ( Without<ReplicatedClient>)>,
) {
    for entity in query.iter() {
        info!("Marking client {:?} for replication", entity);
        commands.entity(entity).insert(ReplicatedClient);
    }
}


fn mark_shard_entities_for_replication(
    mut commands: Commands,
    query: Query<Entity, (Without<Replicated>, With<Object>)>,
    already_replicated: Query<Entity, (With<Replicated>, With<Object>)>,
    named_query: Query<&Name, With<Object>>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    // Only log once per second to avoid spam
    let current_time = time.elapsed().as_secs_f32();
    let should_log = current_time - *last_log_time > 1.0;
    
    if should_log {
        *last_log_time = current_time;
    }
    
    let count = query.iter().count();
    let already_marked = already_replicated.iter().count();
    
    // Only log if we should log based on time
    if should_log {
        debug!(
            "Replication status: {} entities need replication marking, {} already marked",
            count,
            already_marked
        );
    }

    if count > 0 {
        info!(
            "Marking {} shard entities for replication to replication server",
            count
        );

        for entity in query.iter() {
            // Try to get the name for better logging
            let entity_name = named_query.get(entity).map(|name| name.as_str()).unwrap_or("unnamed");
            
            info!("Marking shard entity for replication: {:?} ({})", entity, entity_name);
            commands.entity(entity).insert(Replicated);
        }
        
        info!("All shard entities marked for replication");
    }
}

fn monitor_shard_server_connections(
    mut server_events: EventReader<ServerEvent>,
    server: Option<Res<bevy_replicon_renet2::renet2::RenetServer>>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    // Use a consistent time-based approach for logging (once per second)
    let current_time = time.elapsed().as_secs_f32();
    let should_log = current_time - *last_log_time > 1.0;
    
    if should_log {
        *last_log_time = current_time;
    }

    // Only log connected clients periodically
    if let Some(server) = server.as_ref() {
        if server.connected_clients() > 0 && should_log {
            debug!("SHARD SERVER: Has {} connected clients", server.connected_clients());
            
            // Log details of each client
            for client_id in server.clients_id() {
                debug!("SHARD SERVER: Client {} is connected", client_id);
                // Check if this is the replication server (10000+ ID range)
                if client_id >= 10000 {
                    info!("SHARD SERVER: Replication server connected with ID {}", client_id);
                }
            }
        }
    }
    
    // Always log connection events as they're important and not frequent
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                // Always log new connections
                if *client_id >= 10000 {
                    info!("SHARD SERVER: Replication server connected with reverse connection (ID: {})", client_id);
                } else {
                    info!("SHARD SERVER: Regular client connected with ID {}", client_id);
                }
            }
            ServerEvent::ClientDisconnected { client_id, .. } => {
                // Always log disconnections
                if *client_id >= 10000 {
                    warn!("SHARD SERVER: Replication server disconnected from reverse connection (ID: {})", client_id);
                } else {
                    info!("SHARD SERVER: Regular client disconnected with ID {}", client_id);
                }
            }
        }
    }
}

// Add a new system to track all entities with the Replicated flag on the shard
fn log_shard_replicated_entities(
    query: Query<(Entity, Option<&Name>), With<Replicated>>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    // Only log once every 5 seconds to avoid spam
    let current_time = time.elapsed().as_secs_f32();
    if current_time - *last_log_time < 5.0 {
        return;
    }
    
    *last_log_time = current_time;
    
    let count = query.iter().count();
    if count > 0 {
        info!(
            "Shard server currently has {} entities marked for replication",
            count
        );
        
        // Log details about each entity
        for (entity, name) in query.iter() {
            if let Some(name) = name {
                debug!("Shard replicated entity: {} ({:?})", name, entity);
            } else {
                debug!("Shard replicated entity: {:?} (no name)", entity);
            }
        }
    } else {
        info!("Shard server has no entities marked for replication");
    }
}

// Add a new system to spawn a test entity on the shard server


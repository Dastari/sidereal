use super::config::{ReplicationServerConfig, ShardConfig};
use super::connection::{init_client, init_server};
use bevy::prelude::*;
use bevy_replicon::prelude::{ConnectedClient, ReplicatedClient};
use bevy_replicon_renet2::RepliconRenetPlugins;
use bevy_replicon_renet2::renet2::ServerEvent;
use std::{
    collections::HashMap,
    error::Error,
    net::{IpAddr, SocketAddr},
};
use tracing::{error, info, warn};

pub const REPLICATION_SERVER_DEFAULT_PORT: u16 = 5000;
pub const SHARD_CLIENT_ID_OFFSET: u64 = 20000;

/// Stores information about shards connected TO the replication server.
#[derive(Resource, Default)]
pub struct ConnectedShards {
    /// Maps the client ID (u64) connected TO the replication server
    /// to the logical shard ID (u64).
    pub client_to_shard: HashMap<u64, u64>,
}

/// Sets up the appropriate networking role (Replication Server or Shard Client/Server).
pub struct ReplicationTopologyPlugin {
    pub shard_config: Option<ShardConfig>,
    pub replication_server_config: Option<ReplicationServerConfig>,
}

impl Default for ReplicationTopologyPlugin {
    fn default() -> Self {
        Self {
            shard_config: None,
            replication_server_config: None,
        }
    }
}

impl Plugin for ReplicationTopologyPlugin {
    fn build(&self, app: &mut App) {
        let is_shard = self.shard_config.is_some();
        let is_replication_server = self.replication_server_config.is_some();

        if is_shard && is_replication_server {
            panic!("Cannot be both a Shard Server and a Replication Server in the same instance.");
        }

        app.add_plugins(RepliconRenetPlugins);

        if is_replication_server {
            app.init_resource::<ConnectedShards>().add_systems(
                Update,
                (handle_shard_connections, mark_clients_as_replicated),
            );
        } else if is_shard {
            // Shard-specific update systems for this plugin (if any) would go here.
        }

        if let Some(shard_config) = self.shard_config.clone() {
            app.add_systems(Startup, move |mut commands: Commands| {
                match init_shard(&mut commands, &shard_config) {
                    Ok(_) => {
                        info!(
                            shard_id = shard_config.shard_id.to_string(),
                            "Shard initialized successfully"
                        )
                    }
                    Err(e) => {
                        error!(
                            shard_id = shard_config.shard_id.to_string(),
                            "Failed to initialize shard: {}", e
                        )
                    }
                }
            });
        }

        if let Some(replication_server_config) = self.replication_server_config.clone() {
            let port = if replication_server_config.bind_addr.port() == 0 {
                REPLICATION_SERVER_DEFAULT_PORT
            } else {
                replication_server_config.bind_addr.port()
            };
            let bind_ip: IpAddr = "0.0.0.0".parse().expect("Failed to parse 0.0.0.0");
            let final_bind_addr = SocketAddr::new(bind_ip, port);
            let config_with_defaults = ReplicationServerConfig {
                bind_addr: final_bind_addr,
                ..replication_server_config
            };

            app.add_systems(
                Startup,
                move |mut commands: Commands| match init_replication_server(
                    &mut commands,
                    &config_with_defaults,
                ) {
                    Ok(_) => info!("Replication server initialized successfully"),
                    Err(e) => error!("Failed to initialize replication server: {}", e),
                },
            );
        }
    }
}

/// Initialize a shard: starts a server for game clients and a client for the replication server.
pub fn init_shard(commands: &mut Commands, config: &ShardConfig) -> Result<(), Box<dyn Error>> {
    let repl_server_addr = if config.replication_server_addr.port() == 0 {
        warn!(
            shard_id = config.shard_id.to_string(),
            "Replication server address port is 0 in config, using default port {}.",
            REPLICATION_SERVER_DEFAULT_PORT
        );
        SocketAddr::new(
            config.replication_server_addr.ip(),
            REPLICATION_SERVER_DEFAULT_PORT,
        )
    } else {
        config.replication_server_addr
    };
    let client_id_for_replication = SHARD_CLIENT_ID_OFFSET;

    info!(
        shard_id = config.shard_id.to_string(),
        client_id = client_id_for_replication,
        target_addr = %repl_server_addr,
        protocol_id = config.protocol_id,
        "Initializing shard client component (connecting to replication server)..."
    );
    init_client(
        commands,
        repl_server_addr,
        config.protocol_id,
        client_id_for_replication,
    )?;
    info!(
        shard_id = config.shard_id.to_string(),
        client_id = client_id_for_replication,
        "Shard client component initialized."
    );

    let final_config = ShardConfig {
        replication_server_addr: repl_server_addr,
        ..config.clone()
    };
    commands.insert_resource(final_config);

    Ok(())
}

pub fn init_replication_server(
    commands: &mut Commands,
    config: &ReplicationServerConfig,
) -> Result<(), Box<dyn Error>> {
    info!(
        addr = %config.bind_addr,
        protocol_id = config.protocol_id,
        "Initializing replication server (listening for shards)..."
    );
    init_server(commands, config.bind_addr.port(), Some(config.protocol_id))?;
    commands.insert_resource(config.clone());
    Ok(())
}

/// Replication Server: Handles connection/disconnection events from Shard Servers.
pub fn handle_shard_connections(
    mut server_events: EventReader<ServerEvent>,
    mut connected_shards: ResMut<ConnectedShards>,
) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                if *client_id >= SHARD_CLIENT_ID_OFFSET {
                    let shard_id = *client_id - SHARD_CLIENT_ID_OFFSET;
                    info!(client_id, shard_id, "Shard connected to replication server");
                    if connected_shards.client_to_shard.contains_key(client_id) {
                        warn!(client_id, shard_id, "Duplicate connection event ignored.");
                        continue;
                    }
                    connected_shards
                        .client_to_shard
                        .insert(*client_id, shard_id);
                } else {
                    info!(client_id, "Regular client connected to replication server");
                }
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                if let Some(shard_id) = connected_shards.client_to_shard.remove(client_id) {
                    info!(
                        client_id,
                        shard_id,
                        ?reason,
                        "Shard disconnected from replication server"
                    );
                } else {
                    info!(
                        client_id,
                        ?reason,
                        "Regular client disconnected from replication server"
                    );
                }
            }
        }
    }
}

/// Replication Server: Marks newly connected client entities to receive replicated data.
pub fn mark_clients_as_replicated(
    mut commands: Commands,
    newly_connected_clients: Query<Entity, (Added<ConnectedClient>, Without<ReplicatedClient>)>,
) {
    for entity in newly_connected_clients.iter() {
        info!(
            ?entity,
            "Marking newly connected client entity for replication."
        );
        commands.entity(entity).insert(ReplicatedClient);
    }
}

use super::config::{ReplicationServerConfig, ShardConfig};
use super::connection::{init_client, init_server};
use super::shard_communication::{init_shard_client, REPLICATION_SERVER_SHARD_PORT};
use bevy::prelude::*;
#[cfg(feature = "replicon")]
use bevy_replicon::prelude::{ConnectedClient, ReplicatedClient};
#[cfg(feature = "replicon")]
use bevy_replicon_renet2::RepliconRenetPlugins;
use renet2::ServerEvent;
use uuid::Uuid;
use std::{
    error::Error,
    net::{IpAddr, SocketAddr},
};
use tracing::{error, info, warn};

pub const REPLICATION_SERVER_DEFAULT_PORT: u16 = 5000;

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

        // Add Replicon only if the feature is enabled and we're a replication server
        #[cfg(feature = "replicon")]
        {
            app.add_plugins(RepliconRenetPlugins);
            
            if is_replication_server {
                // Add system to mark clients for replication
                app.add_systems(Update, mark_clients_as_replicated);
            }
        }

        if is_shard {
            // Add the shard client plugin for direct renet2 communication with replication server
            app.add_plugins(super::shard_communication::ShardClientPlugin);
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

        if let Some(replication_config) = self.replication_server_config.clone() {
            // Clone the config before using it in both closures
            let config1 = replication_config.clone();
            app.add_systems(Startup, move |mut commands: Commands| {
                match init_replication_server(&mut commands, &config1) {
                    Ok(_) => info!("Replication server initialized successfully"),
                    Err(e) => error!("Failed to initialize replication server: {}", e),
                }
            });
            
            // Use a different clone for the second closure
            let config2 = replication_config.clone();
            // Initialize the shard server component on the replication server
            app.add_systems(Startup, move |mut commands: Commands| {
                if let Err(e) = super::shard_communication::init_shard_server(
                    &mut commands,
                    REPLICATION_SERVER_SHARD_PORT,
                    config2.protocol_id,
                ) {
                    error!("Failed to initialize shard server component: {}", e);
                }
            });
            
            // Add the plugin for handling shard server events (connections, messages)
            app.add_plugins(super::shard_communication::ShardServerPlugin);
        }
    }
}

/// Initialize a shard: starts a server for game clients and a client for the replication server.
pub fn init_shard(commands: &mut Commands, config: &ShardConfig) -> Result<(), Box<dyn Error>> {
    let repl_server_addr = if config.replication_server_addr.port() == 0 {
        warn!(
            shard_id = config.shard_id.to_string(),
            "Replication server address port is 0 in config, using default port {}.",
            REPLICATION_SERVER_SHARD_PORT
        );
        SocketAddr::new(
            config.replication_server_addr.ip(),
            REPLICATION_SERVER_SHARD_PORT,
        )
    } else {
        config.replication_server_addr
    };

    info!(
        shard_id = config.shard_id.to_string(),
        "Initializing shard client (connecting to replication server)..."
    );
    
    // Use the new shard_communication init function
    init_shard_client(
        commands,
        repl_server_addr,
        config.protocol_id,
        config.shard_id,
    )?;
    
    info!(
        shard_id = config.shard_id.to_string(),
        "Shard client component initialized."
    );

    let final_config = ShardConfig {
        replication_server_addr: repl_server_addr,
        ..config.clone()
    };
    commands.insert_resource(final_config);
    
    // We don't need to add the ShardClientPlugin here - it should be added
    // in the plugin's build method based on shard_config.is_some()

    Ok(())
}

pub fn init_replication_server(
    commands: &mut Commands,
    config: &ReplicationServerConfig,
) -> Result<(), Box<dyn Error>> {
    info!(
        addr = %config.bind_addr,
        protocol_id = config.protocol_id,
        "Initializing replication server for game clients..."
    );
    init_server(commands, config.bind_addr.port(), Some(config.protocol_id))?;
    commands.insert_resource(config.clone());
    Ok(())
}

/// Mark clients as replicated when they connect to enable client-server replication
#[cfg(feature = "replicon")]
fn mark_clients_as_replicated(
    mut commands: Commands,
    clients: Query<Entity, (With<ConnectedClient>, Without<ReplicatedClient>)>,
) {
    for client in clients.iter() {
        commands.entity(client).insert(ReplicatedClient);
        info!("Marked client {:?} for replication", client);
    }
}

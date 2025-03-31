use super::config::{ReplicationServerConfig, ShardConfig};
use super::connection::init_server;
use super::shard_communication::{
    REPLICATION_SERVER_SHARD_PORT, ShardClientPlugin, ShardServerPlugin, init_shard_client,
    init_shard_server,
};
use bevy::prelude::*;
use bevy_renet2::netcode::NetcodeClientPlugin;
use bevy_renet2::prelude::RenetClientPlugin;
use bevy_replicon::prelude::{ConnectedClient, ReplicatedClient};
use bevy_replicon_renet2::RepliconRenetPlugins;
use std::{error::Error, net::SocketAddr};
use tracing::{info, warn};

pub const REPLICATION_SERVER_DEFAULT_PORT: u16 = 5000;

// === New Separate Plugins ===

/// Plugin to configure and run the application as a Shard Server.
pub struct ShardPlugin {
    pub config: ShardConfig,
}

impl Plugin for ShardPlugin {
    fn build(&self, app: &mut App) {
        app
            // Add core plugins for shard client connection to replication server
            .add_plugins(RenetClientPlugin)
            .add_plugins(NetcodeClientPlugin)
            .add_plugins(ShardClientPlugin) // Our custom logic
            // Insert config and add initialization system
            .insert_resource(self.config.clone())
            .add_systems(Startup, init_shard_system);
    }
}

/// Plugin to configure and run the application as a Replication Server.
pub struct ReplicationServerPlugin {
    pub config: ReplicationServerConfig,
}

impl Plugin for ReplicationServerPlugin {
    fn build(&self, app: &mut App) {
        // Add Replicon plugins for game client connections
        #[cfg(feature = "replicon")]
        {
            app.add_plugins(RepliconRenetPlugins)
                .add_systems(Update, mark_clients_as_replicated);
        }

        app
            // Add custom plugin for handling shard connections
            .add_plugins(ShardServerPlugin)
            // Insert config and add initialization systems
            .insert_resource(self.config.clone())
            .add_systems(Startup, init_replication_server_system)
            .add_systems(Startup, init_shard_server_system);
    }
}

// === Helper Systems ===

// System to initialize shard client components
fn init_shard_system(mut commands: Commands, config: Res<ShardConfig>) {
    init_shard(&mut commands, &config).expect("Failed to initialize shard client connection");
    info!(
        shard_id = config.shard_id.to_string(),
        "Shard networking initialized successfully"
    );
}

// System to initialize replication server components for game clients
fn init_replication_server_system(mut commands: Commands, config: Res<ReplicationServerConfig>) {
    init_replication_server(&mut commands, &config)
        .expect("Failed to initialize replication server (for game clients)");
    info!("Replication server (client-facing) initialized successfully");
}

// System to initialize shard listener components on the replication server
fn init_shard_server_system(mut commands: Commands, config: Res<ReplicationServerConfig>) {
    match init_shard_server(REPLICATION_SERVER_SHARD_PORT, config.protocol_id) {
        Ok(listener) => {
            commands.insert_resource(listener);
            info!("Shard listener component initialized and inserted as resource.");
        }
        Err(e) => {
            // Use expect here as well, as the replication server cannot function without the shard listener
            panic!("Failed to initialize shard server component: {}", e);
        }
    }
}

// === Old Combined Plugin (To be deleted) ===

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
    clients: Query<Entity, Added<ConnectedClient>>,
) {
    for client in clients.iter() {
        commands.entity(client).insert(ReplicatedClient);
        info!("Marked client {:?} for replication", client);
    }
}

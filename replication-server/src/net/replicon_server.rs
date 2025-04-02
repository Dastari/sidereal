use bevy_replicon::{prelude::Replicated, RepliconPlugins};
use sidereal::{net::config::{create_connection_config, DEFAULT_PROTOCOL_ID}, Object, RepliconServerConfig}; 
use bevy::prelude::*;
use bevy_replicon_renet2::{
    RepliconRenetPlugins,
    netcode::{
         NativeSocket, NetcodeServerTransport,
        ServerAuthentication, ServerSetupConfig,
    },
    renet2::{ConnectionConfig, RenetServer},
};
use std::{
    error::Error,
    net::{SocketAddr, UdpSocket},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{info, warn};

use crate::game::SceneState;

pub struct RepliconServerPlugin {
    config: RepliconServerConfig,
    replication_enabled: bool,
}

impl Default for RepliconServerPlugin {
    fn default() -> Self {
        Self {
            config: RepliconServerConfig::default(),
            replication_enabled: true,
        }
    }
}

impl RepliconServerPlugin {
    pub fn with_config(config: RepliconServerConfig) -> Self {
        Self {
            config,
            replication_enabled: true,
        }
    }

    /// Set whether to enable entity replication
    pub fn with_replication(mut self, enabled: bool) -> Self {
        self.replication_enabled = enabled;
        self
    }
}

impl Plugin for RepliconServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((RepliconPlugins, RepliconRenetPlugins))
            .insert_resource(self.config.clone());

        if self.replication_enabled {
            app.add_systems(OnEnter(SceneState::Completed), mark_entities_for_replication)
                .add_systems(Update, log_marked_entities);
        }
        app.add_systems(Startup, init_replicon_server_system);
        info!("Replicon Renet2 networking plugins added.");
    }
}

pub fn default_connection_config() -> ConnectionConfig {
    create_connection_config() 
}

fn init_replicon_server_system(mut commands: Commands, config: Res<RepliconServerConfig>) {
    match init_server(&mut commands, config.bind_addr.port(), Some(config.protocol_id)) {
        Ok(_) => info!("Replicon server initialized successfully"),
        Err(e) => warn!("Failed to initialize replicon server: {}", e),
    }
}

pub fn init_server(
    commands: &mut Commands,
    server_port: u16,
    protocol_id: Option<u64>,
) -> Result<(), Box<dyn Error>> {
    let listen_addr = SocketAddr::new("0.0.0.0".parse().unwrap(), server_port);
    let final_protocol_id = protocol_id.unwrap_or(DEFAULT_PROTOCOL_ID);

    info!(
        "Initializing replicon server on {} with protocol ID {}",
        listen_addr, final_protocol_id
    );

    let socket = UdpSocket::bind(listen_addr)?;
    socket.set_nonblocking(true)?;
    let native_socket = NativeSocket::new(socket)?;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;

    let max_clients = 64;
    let server_config = ServerSetupConfig {
        current_time,
        max_clients,
        protocol_id: final_protocol_id,
        socket_addresses: vec![vec![listen_addr]],
        authentication: ServerAuthentication::Unsecure,
    };

    let transport = NetcodeServerTransport::new(server_config, native_socket)?;
    let server = RenetServer::new(default_connection_config());

    commands.insert_resource(server);
    commands.insert_resource(transport);
    info!("Server resources initialized with {} max clients", max_clients);

    Ok(())
}

fn mark_entities_for_replication(
    mut commands: Commands,
    query: Query<Entity, (With<Object>, Without<Replicated>)>,
) {
    let count = query.iter().count();
    if count > 0 {
        for entity in query.iter() {
            commands.entity(entity).insert(Replicated);
        }
        info!("Marked {} loaded scene entities for replication", count);
    }
}

fn log_marked_entities(query: Query<(Entity, Option<&Name>), Added<Replicated>>) {
    for (entity, name) in query.iter() {
        debug!(
            "Entity '{:}' ({:?}) marked as Replicated on server",
            name.unwrap_or(&Name::new("unnamed")),
            entity
        );
    }
}

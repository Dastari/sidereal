use super::config::{ReplicationServerConfig, ShardConfig};
use super::connection::{init_client, init_server, default_connection_config}; // Ensure default_connection_config is accessible
use super::DEFAULT_PROTOCOL_ID;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    // Removed NetcodeServerTransport
    netcode::{ClientAuthentication, NativeSocket, NetcodeClientTransport},
    // Removed RenetServer
    renet2::{RenetClient, ServerEvent},
};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tracing::{debug, error, info, warn};

// --- Constants (remain the same) ---
pub const REPLICATION_SERVER_DEFAULT_PORT: u16 = 5000;
pub const SHARD_PORT_OFFSET: u16 = 5000;
pub const SHARD_CLIENT_ID_OFFSET: u64 = 20000;
pub const REPL_CLIENT_ID_OFFSET: u64 = 10000;

// --- Resources (remain the same) ---
#[derive(Resource, Default)]
pub struct ShardClients {
    pub clients: HashMap<u64, RenetClient>,
}

#[derive(Resource, Default)]
pub struct ShardTransports {
    pub transports: HashMap<u64, NetcodeClientTransport>,
}

#[derive(Resource, Default)]
pub struct ConnectedShards {
    pub client_to_shard: HashMap<u64, u64>,
    pub shard_addresses: HashMap<u64, SocketAddr>,
    pub reverse_connected: HashSet<u64>,
}

// --- Plugin (remains the same) ---
pub struct BiDirectionalReplicationSetupPlugin {
    pub shard_config: Option<ShardConfig>,
    pub replication_server_config: Option<ReplicationServerConfig>,
}

impl Default for BiDirectionalReplicationSetupPlugin {
    fn default() -> Self {
        Self {
            shard_config: None,
            replication_server_config: None,
        }
    }
}

impl Plugin for BiDirectionalReplicationSetupPlugin {
    fn build(&self, app: &mut App) {
        let is_shard = self.shard_config.is_some();
        let is_replication_server = self.replication_server_config.is_some();

        if is_shard && is_replication_server {
            app.add_systems(Update, (update_shard_clients, update_shard_transports));
        } else if is_shard {
            // Nothing extra needed here for shard-only ticks
        } else if is_replication_server {
            app.init_resource::<ShardClients>()
                .init_resource::<ShardTransports>()
                .init_resource::<ConnectedShards>()
                .add_systems(
                    Update,
                    (
                        handle_shard_connections,
                        update_shard_clients,
                        update_shard_transports,
                        cleanup_disconnected_shards.after(update_shard_transports),
                        monitor_shard_connections,
                        mark_clients_as_replicated,
                    ),
                );
        }

        // --- Startup Initialization ---
        if let Some(shard_config) = self.shard_config.clone() {
            app.add_systems(Startup, move |mut commands: Commands| {
                // Pass config by value or clone if necessary for 'move' closure
                match init_shard_server(&mut commands, &shard_config) {
                    Ok(_) => info!(shard_id = shard_config.shard_id, "Shard server initialized successfully"),
                    Err(e) => error!(shard_id = shard_config.shard_id, "Failed to initialize shard server: {}", e),
                }
            });
        }

        if let Some(replication_server_config) = self.replication_server_config.clone() {
             let config_with_defaults = ReplicationServerConfig {
                 bind_addr: SocketAddr::new(
                     replication_server_config.bind_addr.ip(),
                     replication_server_config.bind_addr.port()
                 ),
                 ..replication_server_config
             };
            app.add_systems(Startup, move |mut commands: Commands| {
                 match init_replication_server(&mut commands, &config_with_defaults) {
                    Ok(_) => info!("Replication server initialized successfully"),
                    Err(e) => error!("Failed to initialize replication server: {}", e),
                 }
            });
        }
    }
}


// --- Initialization Functions ---

/// Initialize a shard server: runs a server and connects as a client to the replication server.
pub fn init_shard_server(
    commands: &mut Commands,
    config: &ShardConfig,
) -> Result<(), Box<dyn Error>> {
    let bind_port = SHARD_PORT_OFFSET + config.shard_id as u16;
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), bind_port);

    info!(
        shard_id = config.shard_id,
        addr = %bind_addr,
        protocol_id = config.protocol_id,
        "Initializing shard server..."
    );

    init_server(commands, bind_addr.port(), Some(config.protocol_id))?;

    // Determine the replication server address. Use config value if provided and valid, else default.
    // Assuming `replication_server_addr: SocketAddr` in ShardConfig. Check if port is 0 as sentinel for default.
    let repl_server_addr = if config.replication_server_addr.port() == 0 {
         warn!(shard_id = config.shard_id, "Replication server address port is 0, using default.");
         SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), REPLICATION_SERVER_DEFAULT_PORT)
    } else {
         config.replication_server_addr
    };

    let client_id = SHARD_CLIENT_ID_OFFSET + config.shard_id;

    info!(
        shard_id = config.shard_id,
        client_id = client_id,
        target_addr = %repl_server_addr,
        protocol_id = config.protocol_id,
        "Shard connecting to replication server..."
    );

    init_client(
        commands,
        repl_server_addr,
        config.protocol_id,
        client_id,
    )?;

    // Store effective config
    let final_config = ShardConfig {
        bind_addr,
        replication_server_addr: repl_server_addr, // Store the actual address used (not Option)
        ..config.clone()
    };
    commands.insert_resource(final_config);

    Ok(())
}

/// Initialize the replication server. (Remains the same)
pub fn init_replication_server(
    commands: &mut Commands,
    config: &ReplicationServerConfig,
) -> Result<(), Box<dyn Error>> {
    let port = if config.bind_addr.port() == 0 {
        REPLICATION_SERVER_DEFAULT_PORT
    } else {
        config.bind_addr.port()
    };
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);

    info!(
        addr = %bind_addr,
        protocol_id = config.protocol_id,
        "Initializing replication server..."
    );

    init_server(commands, bind_addr.port(), Some(config.protocol_id))?;

    let final_config = ReplicationServerConfig {
        bind_addr,
        ..config.clone()
    };
    commands.insert_resource(final_config);

    Ok(())
}

// --- Update Systems ---

/// Replication Server: Detects shard connections and initiates reverse connections.
pub fn handle_shard_connections(
    // Removed `mut commands: Commands` as it was unused
    mut server_events: EventReader<ServerEvent>,
    config: Option<Res<ReplicationServerConfig>>,
    mut connected_shards: ResMut<ConnectedShards>,
    mut shard_clients: ResMut<ShardClients>,
    mut shard_transports: ResMut<ShardTransports>,
) {
    let replication_protocol_id = config.map(|c| c.protocol_id).unwrap_or(DEFAULT_PROTOCOL_ID);

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

                    connected_shards.client_to_shard.insert(*client_id, shard_id);

                    let shard_port = SHARD_PORT_OFFSET + shard_id as u16;
                    let shard_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), shard_port);
                    info!(shard_id, addr = %shard_addr, "Calculated shard server address");
                    connected_shards.shard_addresses.insert(shard_id, shard_addr);

                    if !connected_shards.reverse_connected.contains(&shard_id) {
                        info!(shard_id, addr = %shard_addr, "Attempting reverse connection...");
                        match connect_to_shard(
                            shard_id,
                            shard_addr,
                            replication_protocol_id,
                            &mut shard_clients,
                            &mut shard_transports,
                        ) {
                            Ok(_) => {
                                info!(shard_id, "Reverse connection setup initiated.");
                                connected_shards.reverse_connected.insert(shard_id);
                            }
                            Err(e) => {
                                error!(shard_id, addr = %shard_addr, "Reverse connection failed: {}", e);
                                connected_shards.shard_addresses.remove(&shard_id);
                                connected_shards.client_to_shard.remove(client_id);
                            }
                        }
                    } else {
                        info!(shard_id, "Reverse connection already established or pending.");
                    }
                } else {
                    info!(client_id, "Regular client connected to replication server");
                }
            }
            ServerEvent::ClientDisconnected { client_id, .. } => {
                if let Some(shard_id) = connected_shards.client_to_shard.remove(client_id) {
                    info!(client_id, shard_id, "Shard disconnected from replication server");
                    if connected_shards.reverse_connected.remove(&shard_id) {
                         info!(shard_id, "Removed shard from reverse connection tracking.");
                    }
                    connected_shards.shard_addresses.remove(&shard_id);
                } else {
                    info!(client_id, "Regular client disconnected from replication server");
                }
            }
        }
    }
}

/// Replication Server: Establishes a client connection *to* a specific shard server.
fn connect_to_shard(
    shard_id: u64,
    shard_addr: SocketAddr,
    protocol_id: u64,
    shard_clients: &mut ShardClients,
    shard_transports: &mut ShardTransports,
) -> Result<(), Box<dyn Error>> {
    let replication_client_id = REPL_CLIENT_ID_OFFSET + shard_id;

    info!(
        shard_id,
        target_addr = %shard_addr,
        client_id = replication_client_id,
        protocol_id,
        "Setting up reverse connection client..."
    );

    if shard_clients.clients.contains_key(&shard_id) || shard_transports.transports.contains_key(&shard_id) {
        warn!(shard_id, "Reverse connection client/transport already exists. Skipping.");
        return Ok(());
    }

    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))?;
    socket.set_nonblocking(true)?;
    let local_addr = socket.local_addr()?;
    info!(client_id=replication_client_id, local_addr=%local_addr, "Reverse connection socket bound");

    let native_socket = NativeSocket::new(socket)?;

    let authentication = ClientAuthentication::Unsecure {
        client_id: replication_client_id,
        protocol_id,
        server_addr: shard_addr,
        user_data: None,
        socket_id: 0,
    };

    let transport = NetcodeClientTransport::new(current_time, authentication, native_socket)?;

    // Use default connection config function (assuming it's accessible)
    let connection_config = default_connection_config();
    let client = RenetClient::new(connection_config, false);

    shard_clients.clients.insert(shard_id, client);
    shard_transports.transports.insert(shard_id, transport);

    info!(shard_id, client_id = replication_client_id, "Reverse connection client and transport stored.");
    Ok(())
}

/// Replication Server: Ticks the RenetClient for each reverse connection.
pub fn update_shard_clients(mut clients: ResMut<ShardClients>) {
    let delta = Duration::from_secs_f32(1.0 / 60.0);
    // Use _shard_id as it's not needed in the loop body
    for (_shard_id, client) in clients.clients.iter_mut() {
        client.update(delta);
    }
}

/// Replication Server: Updates the NetcodeClientTransport for each reverse connection.
pub fn update_shard_transports(
    time: Res<Time>,
    mut transports: ResMut<ShardTransports>,
    mut clients: ResMut<ShardClients>,
    mut last_log_time: Local<f32>,
) {
    // Corrected: Use elapsed_secs()
    let current_time = time.elapsed_secs();
    let log_throttle_secs = 5.0;
    let should_log_detail = current_time - *last_log_time > log_throttle_secs;

    if should_log_detail {
        *last_log_time = current_time;
        if !transports.transports.is_empty() {
             debug!("Updating {} reverse connection transports...", transports.transports.len());
        }
    }

    let mut disconnected_shards = HashSet::new();

    for (shard_id, transport) in transports.transports.iter_mut() {
        if let Some(client) = clients.clients.get_mut(shard_id) {
             if client.is_connected() || client.is_connecting() {
                if let Err(e) = transport.update(time.delta(), client) {
                    error!(shard_id = *shard_id, "Reverse transport update error: {}", e);
                    disconnected_shards.insert(*shard_id);
                } else {
                    if should_log_detail && client.is_connected() {
                        debug!(shard_id = *shard_id, "Reverse transport updated successfully.");
                    }
                }
            }
            else if should_log_detail {
                 debug!(shard_id = *shard_id, "Reverse client not active, skipping transport update.");
            }
        } else {
            error!(shard_id = *shard_id, "Reverse transport exists, but no matching client found!");
            disconnected_shards.insert(*shard_id);
        }
    }
    if should_log_detail && !disconnected_shards.is_empty() {
        warn!(?disconnected_shards, "Encountered errors or inconsistencies during reverse transport updates.");
    }
}


/// Replication Server: Removes client/transport resources for disconnected reverse connections. (Remains the same)
pub fn cleanup_disconnected_shards(
    mut clients: ResMut<ShardClients>,
    mut transports: ResMut<ShardTransports>,
    mut connected_shards: ResMut<ConnectedShards>,
) {
    let mut shards_to_remove = HashSet::new();

    for (shard_id, client) in clients.clients.iter() {
        if !client.is_connected() && !client.is_connecting() {
            shards_to_remove.insert(*shard_id);
        }
    }

    for shard_id in transports.transports.keys() {
        if !clients.clients.contains_key(shard_id) {
            shards_to_remove.insert(*shard_id);
        }
    }

    if !shards_to_remove.is_empty() {
        info!(?shards_to_remove, "Cleaning up disconnected/orphaned reverse connections...");
        for shard_id in shards_to_remove {
            clients.clients.remove(&shard_id);
            transports.transports.remove(&shard_id);
            connected_shards.reverse_connected.remove(&shard_id);
        }
    }
}


/// Replication Server: Logs the status of reverse connections periodically. (Optional Debugging)
pub fn monitor_shard_connections(
    clients: Res<ShardClients>,
    connected_shards: Res<ConnectedShards>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    // Corrected: Use elapsed_secs()
    let current_time = time.elapsed_secs();
    let log_interval = 10.0;

    if current_time - *last_log_time > log_interval {
        *last_log_time = current_time;

        let total_clients = clients.clients.len();
        let connected_count = clients
            .clients
            .values()
            .filter(|c| c.is_connected())
            .count();

        if total_clients > 0 {
            info!(
                total = total_clients,
                connected = connected_count,
                "Reverse Connection Status:"
            );
             for (shard_id, client) in &clients.clients {
                 let status = if client.is_connected() { "Connected" }
                     else if client.is_connecting() { "Connecting" }
                     else { "Disconnected" };
                 let addr = connected_shards.shard_addresses.get(shard_id)
                    .map_or_else(|| "N/A".to_string(), |a| a.to_string());
                debug!(shard_id = *shard_id, %addr, status, "Shard Reverse Connection");
             }
        } else {
             debug!("No active reverse shard connections to monitor.");
        }
        debug!("Forward connections (Shards connected TO us): {:?}", connected_shards.client_to_shard);
    }
}

/// Replication Server: Marks newly connected clients to receive replicated data. (Remains the same)
pub fn mark_clients_as_replicated(
    mut commands: Commands,
    newly_connected_clients: Query<Entity, (With<ConnectedClient>, Without<ReplicatedClient>)>,
) {
    for entity in newly_connected_clients.iter() {
        info!(?entity, "Marking newly connected client to receive replicated data.");
        commands.entity(entity).insert(ReplicatedClient);
    }
}
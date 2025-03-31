// sidereal/src/net/shard_communication.rs

use bevy::prelude::*;
use bevy_renet2::netcode::{
    ClientAuthentication, ServerAuthentication, ServerSetupConfig,
    NetcodeClientTransport, NetcodeServerTransport, NativeSocket
};
use renet2::{
    RenetClient, RenetServer, ServerEvent, ChannelConfig, SendType
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    net::{SocketAddr, UdpSocket},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{error, info, debug};
use uuid::Uuid;
use bincode;


// --- Constants ---
pub const REPLICATION_SERVER_SHARD_PORT: u16 = 5001; // Different port from client connections
pub const SHARD_CHANNEL_UNRELIABLE: u8 = 0;
pub const SHARD_CHANNEL_RELIABLE: u8 = 1;

// --- Message Types ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ShardToReplicationMessage {
    EntityUpdates(Vec<EntityUpdate>),
    SpawnRequest {
        entity_type: String,
        position: (f32, f32),
    },
    DespawnNotification(Uuid),
    IdentifyShard {
        shard_id: Uuid,
        sectors: Vec<(i32, i32)>,
    }, // Used when a shard connects
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ReplicationToShardMessage {
    InitializeSector {
        sector_id: (i32, i32),
        entities: Vec<EntityInitData>,
    },
    EntityAdded(EntityData),
    EntityRemoved(Uuid),
    PlayerCommand {
        player_id: Uuid,
        command_type: String,
        data: Vec<u8>,
    },
    AssignSectors {
        sectors: Vec<(i32, i32)>,
    }, // Assign sectors to a shard
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityUpdate {
    pub id: Uuid,
    pub position: (f32, f32), // x, y position
    pub velocity: (f32, f32), // x, y velocity
    pub rotation: f32,        // z-axis rotation
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityData {
    pub id: Uuid,
    pub entity_type: String,
    pub position: (f32, f32),
    pub velocity: (f32, f32),
    pub rotation: f32,
    pub attributes: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityInitData {
    pub id: Uuid,
    pub entity_type: String,
    pub position: (f32, f32),
    pub attributes: HashMap<String, String>,
}

// --- Shard Tracking ---
#[derive(Resource, Default)]
pub struct ConnectedShards {
    // Maps client_id to shard info
    pub shards: HashMap<u64, ShardInfo>,
}

#[derive(Debug, Clone)]
pub struct ShardInfo {
    pub shard_id: Uuid,
    pub sectors: HashSet<(i32, i32)>,
    pub connected_at: std::time::SystemTime,
}

// --- Shard Resources ---
#[derive(Resource, Default, Debug)]
pub struct AssignedSectors {
    pub sectors: HashSet<(i32, i32)>,
    pub dirty: bool, // Set to true when sectors have changed
}

// --- Resource for Manual Shard Server Management ---
#[derive(Resource)]
pub struct ShardListener {
    pub server: RenetServer,
    pub transport: NetcodeServerTransport,
}

/// Initialize a shard client that connects to the replication server
pub fn init_shard_client(
    commands: &mut Commands,
    server_addr: SocketAddr,
    protocol_id: u64,
    shard_id: Uuid,
) -> Result<(), Box<dyn Error>> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let client_id = shard_id.as_u128() as u64;

    // Create a connection config with channels
    let connection_config = renet2::ConnectionConfig {
        server_channels_config: vec![
            ChannelConfig {
                channel_id: SHARD_CHANNEL_UNRELIABLE,
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::Unreliable,
            },
            ChannelConfig {
                channel_id: SHARD_CHANNEL_RELIABLE,
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: std::time::Duration::from_millis(200),
                },
            },
        ],
        client_channels_config: vec![
            ChannelConfig {
                channel_id: SHARD_CHANNEL_UNRELIABLE,
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::Unreliable,
            },
            ChannelConfig {
                channel_id: SHARD_CHANNEL_RELIABLE,
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: std::time::Duration::from_millis(200),
                },
            },
        ],
        available_bytes_per_tick: 1024 * 1024,
    };

    // Create RenetClient with the connection config
    let client = RenetClient::new(connection_config, false);

    // Create client transport
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id,
        server_addr,
        user_data: None,
        socket_id: 0, // Add the missing socket_id field
    };

    let socket = NativeSocket::new(socket)?;
    let transport = NetcodeClientTransport::new(current_time, authentication, socket)?;

    // Insert resources into ECS
    commands.insert_resource(client);
    commands.insert_resource(transport);

    Ok(())
}

/// Initialize a replication server that shards connect to
pub fn init_shard_server(
    port: u16,
    protocol_id: u64,
) -> Result<ShardListener, Box<dyn Error>> {
    let server_addr = SocketAddr::new("0.0.0.0".parse()?, port);
    let socket = UdpSocket::bind(server_addr)?;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;

    // Create a connection config with channels
    let connection_config = renet2::ConnectionConfig {
        server_channels_config: vec![
            ChannelConfig {
                channel_id: SHARD_CHANNEL_UNRELIABLE,
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::Unreliable,
            },
            ChannelConfig {
                channel_id: SHARD_CHANNEL_RELIABLE,
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: std::time::Duration::from_millis(200),
                },
            },
        ],
        client_channels_config: vec![
            ChannelConfig {
                channel_id: SHARD_CHANNEL_UNRELIABLE,
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::Unreliable,
            },
            ChannelConfig {
                channel_id: SHARD_CHANNEL_RELIABLE,
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: std::time::Duration::from_millis(200),
                },
            },
        ],
        available_bytes_per_tick: 1024 * 1024,
    };

    // Create server with the connection config
    let server = RenetServer::new(connection_config);

    // Create server transport with proper config
    let setup_config = ServerSetupConfig {
        current_time,
        max_clients: 32,
        protocol_id,
        socket_addresses: vec![vec![server_addr]],
        authentication: ServerAuthentication::Unsecure,
    };

    let socket = NativeSocket::new(socket)?;
    let transport = NetcodeServerTransport::new(setup_config, socket)?;

    // Return the ShardListener containing the server and transport
    Ok(ShardListener {
        server,
        transport,
    })
}

// --- Plugins ---
pub struct ShardServerPlugin;
pub struct ShardClientPlugin;

impl Plugin for ShardServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConnectedShards>()
            .add_systems(Update, manual_shard_server_update.run_if(resource_exists::<ShardListener>))
            .add_systems(Update, (handle_server_events, log_shard_stats));
    }
}

impl Plugin for ShardClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssignedSectors>()
            .add_systems(
                Update,
                (
                    log_connection_status,
                    send_shard_identification.run_if(resource_exists::<RenetClient>),
                    receive_replication_messages.run_if(resource_exists::<RenetClient>),
                    handle_sector_assignments,
                ),
            );
    }
}

// --- Manual Update System ---
fn manual_shard_server_update(
    mut listener: ResMut<ShardListener>,
    time: Res<Time>,
) {
    // Destructure listener to get independent mutable borrows of server and transport
    let ShardListener { server, transport } = listener.as_mut();

    server.update(time.delta());
    // Pass the independent mutable borrow of server to the transport update
    if let Err(e) = transport.update(time.delta(), server) {
        error!("Shard transport update error: {:?}", e);
    }
}

/// Handle server events (client connections, disconnections)
fn handle_server_events(
    mut listener: ResMut<ShardListener>, // Use the ShardListener resource
    mut connected_shards: ResMut<ConnectedShards>,
) {
    let server = &mut listener.server; // Get mutable ref to server

    // Process RenetServer Events & Messages
    while let Some(event) = server.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                // Connection confirmed by RenetServer, now wait for IdentifyShard message
                info!(client_id = %client_id, "Shard client connected (RenetServer), awaiting identification");
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                // Authoritative disconnect: remove from our tracking
                if let Some(shard) = connected_shards.shards.remove(&client_id) {
                    info!(
                        client_id = %client_id,
                        shard_id = %shard.shard_id,
                        reason = ?reason,
                        "Shard disconnected from replication server"
                    );
                    // TODO: Handle sector reassignment
                } else {
                     // This case might happen if the client disconnected before identifying
                     info!(
                        client_id = %client_id,
                        reason = ?reason,
                        "Unidentified client disconnected from shard server"
                    );
                }
            }
        }
    }

    // Process messages from identified clients
    for client_id in server.clients_id() { // Iterate connected clients according to RenetServer
        while let Some(message) = server.receive_message(client_id, SHARD_CHANNEL_RELIABLE) {
            match bincode::serde::decode_from_slice::<ShardToReplicationMessage, _>(&message, bincode::config::standard()).map(|(v, _)| v) { // Use bincode::serde::decode_from_slice
                Ok(ShardToReplicationMessage::IdentifyShard { shard_id, sectors }) => {
                    info!(
                        client_id = %client_id,
                        shard_id = %shard_id,
                        "Shard connected and identified"
                    );

                    // Store or update shard information
                    let shard_info = ShardInfo {
                        shard_id,
                        sectors: sectors.clone().into_iter().collect(),
                        connected_at: std::time::SystemTime::now(),
                    };

                    connected_shards.shards.insert(client_id, shard_info);

                    // Send acknowledgment or sector assignment if needed
                    // This is where we could assign sectors based on load balancing
                    if sectors.is_empty() {
                        // Assign some initial sectors (just for example)
                        let initial_sectors: HashSet<(i32, i32)> =
                            [(0, 0), (0, 1), (1, 0), (1, 1)].iter().cloned().collect();
                        let assign_message = ReplicationToShardMessage::AssignSectors {
                            sectors: initial_sectors.iter().cloned().collect(),
                        };

                        if let Ok(bytes) = bincode::serde::encode_to_vec(&assign_message, bincode::config::standard()) {
                            server.send_message(client_id, SHARD_CHANNEL_RELIABLE, bytes);
                            info!(
                                client_id = %client_id,
                                shard_id = %shard_id,
                                sectors = ?initial_sectors,
                                "Assigned initial sectors to shard"
                            );

                            // Update stored sectors
                            if let Some(shard) = connected_shards.shards.get_mut(&client_id) {
                                shard.sectors.extend(initial_sectors);
                            }
                        }
                    }
                }
                Ok(message) => {
                    info!(
                        client_id = %client_id,
                        message_type = ?std::any::type_name_of_val(&message),
                        "Received message from shard"
                    );
                }
                Err(e) => {
                    error!(
                        client_id = %client_id,
                        error = %e,
                        "Failed to deserialize message from shard"
                    );
                }
            }
        }
    }
}

/// Process newly assigned sectors
fn handle_sector_assignments(assigned_sectors: Res<AssignedSectors>, mut commands: Commands) {
    if !assigned_sectors.dirty {
        return;
    }

    // Here you would:
    // 1. Load/unload map chunks based on sector changes
    // 2. Spawn entities for new sectors
    // 3. Handle entity transfers between shards

    if !assigned_sectors.sectors.is_empty() {
        info!(
            sectors = ?assigned_sectors.sectors,
            "Shard is now responsible for {} sectors",
            assigned_sectors.sectors.len()
        );
    }

    // Mark as processed
    commands.insert_resource(AssignedSectors {
        sectors: assigned_sectors.sectors.clone(),
        dirty: false,
    });
}

/// System to log connection status periodically
fn log_connection_status(
    client: Option<Res<RenetClient>>,
    time: Res<Time>,
    mut last_log: Local<f64>,
) {
    // Only log every 5 seconds
    let current_time = time.elapsed().as_secs_f64();
    if current_time - *last_log < 5.0 {
        return;
    }
    
    *last_log = current_time;
    
    if let Some(client) = client {
        if client.is_connected() {
            info!("Connected to replication server");
        } else {
            info!("Not connected to replication server");
        }
    } else {
        debug!("RenetClient not available");
    }
}

/// System to receive messages from the replication server
fn receive_replication_messages(
    mut client: ResMut<RenetClient>,
    mut assigned_sectors: ResMut<AssignedSectors>,
) {
    if !client.is_connected() {
        return;
    }

    // Handle unreliable messages (entity updates, etc.)
    while let Some(message) = client.receive_message(SHARD_CHANNEL_UNRELIABLE) {
        match bincode::serde::decode_from_slice::<ReplicationToShardMessage, _>(&message, bincode::config::standard()).map(|(v, _)| v) {
            Ok(repl_msg) => {
                match repl_msg {
                    ReplicationToShardMessage::InitializeSector {
                        sector_id,
                        entities,
                    } => {
                        info!(
                            "Received sector initialization: {:?} with {} entities",
                            sector_id,
                            entities.len()
                        );
                        // Here you would spawn entities in your ECS world for simulation
                    }
                    ReplicationToShardMessage::EntityAdded(entity_data) => {
                        info!("Received new entity: {}", entity_data.id);
                        // Add a new entity to the simulation
                    }
                    ReplicationToShardMessage::EntityRemoved(entity_id) => {
                        info!("Received entity removal: {}", entity_id);
                        // Remove an entity from the simulation
                    }
                    ReplicationToShardMessage::AssignSectors { sectors } => {
                        let sector_set: HashSet<(i32, i32)> = sectors.into_iter().collect();
                        info!("Received sector assignment: {:?}", sector_set);

                        // Store the assigned sectors and mark as dirty for processing
                        assigned_sectors.sectors = sector_set;
                        assigned_sectors.dirty = true;
                    }
                    ReplicationToShardMessage::PlayerCommand {
                        player_id,
                        command_type,
                        data: _,
                    } => {
                        info!(
                            "Received player command: {} of type {}",
                            player_id, command_type
                        );
                        // Handle player command
                    }
                }
            }
            Err(e) => error!("Failed to deserialize message: {:?}", e),
        }
    }

    // Handle reliable messages (commands, etc.)
    while let Some(message) = client.receive_message(SHARD_CHANNEL_RELIABLE) {
        match bincode::serde::decode_from_slice::<ReplicationToShardMessage, _>(&message, bincode::config::standard()).map(|(v, _)| v) {
            Ok(repl_msg) => {
                match repl_msg {
                    ReplicationToShardMessage::AssignSectors { sectors } => {
                        let sector_set: HashSet<(i32, i32)> = sectors.into_iter().collect();
                        info!(
                            "Shard server assigned {} sectors by replication server (reliable channel)",
                            sector_set.len()
                        );

                        // Store the assigned sectors and mark as dirty for processing
                        assigned_sectors.sectors = sector_set;
                        assigned_sectors.dirty = true;
                    }
                    _ => {
                        info!("Received reliable message from replication server");
                    }
                }
            }
            Err(e) => error!("Failed to deserialize reliable message: {:?}", e),
        }
    }
}

/// Send entity updates from a shard to the replication server
fn send_entity_updates_to_replication(mut client: ResMut<RenetClient>) {
    if !client.is_connected() {
        return;
    }

    // Sample code to collect entity updates - replace with your actual logic
    let updates = Vec::new();

    // Simulate collecting some entity updates
    // for (id, transform, velocity) in query.iter() {
    //     updates.push(EntityUpdate {
    //         id: id.0,
    //         position: (transform.translation.x, transform.translation.y),
    //         velocity: (velocity.0.x, velocity.0.y),
    //         rotation: transform.rotation.z,
    //     });
    // }

    if !updates.is_empty() {
        let message = ShardToReplicationMessage::EntityUpdates(updates.clone());
        match bincode::serde::encode_to_vec(&message, bincode::config::standard()) {
            Ok(bytes) => {
                client.send_message(SHARD_CHANNEL_UNRELIABLE, bytes);
                info!(
                    "Sent {} entity updates to replication server",
                    updates.len()
                );
            }
            Err(e) => error!("Failed to serialize entity updates: {:?}", e),
        }
    }
}

/// Send shard identification to replication server on connection
fn send_shard_identification(
    mut client: ResMut<RenetClient>,
    config: Res<super::config::ShardConfig>,
    mut sent: Local<bool>,
) {
    if !client.is_connected() {
        *sent = false;
        return;
    }

    // Only send identification once when we connect
    if !*sent {
        info!(
            shard_id = %config.shard_id,
            "Sending shard identification to replication server"
        );

        // Initial sectors are empty - replication server will assign them
        let message = ShardToReplicationMessage::IdentifyShard {
            shard_id: config.shard_id,
            sectors: Vec::new(),
        };

        match bincode::serde::encode_to_vec(&message, bincode::config::standard()) {
            Ok(bytes) => {
                client.send_message(SHARD_CHANNEL_RELIABLE, bytes);
                *sent = true;
                info!("Shard identification sent successfully");
            }
            Err(e) => error!("Failed to serialize shard identification: {:?}", e),
        }
    }
}

/// Periodically log stats about connected shards
fn log_shard_stats(
    connected_shards: Res<ConnectedShards>,
    time: Res<Time>,
    mut last_log: Local<f64>,
) {
    // Log every 30 seconds
    let current_time = time.elapsed().as_secs_f64();
    if current_time - *last_log > 30.0 {
        *last_log = current_time;

        if connected_shards.shards.is_empty() {
            info!("No shard servers currently connected to replication server");
            return;
        }

        info!("===== SHARD SERVER STATUS =====");
        info!("Connected shard servers: {}", connected_shards.shards.len());

        // Count sectors by shard
        let mut total_sectors = 0;
        for (client_id, shard) in &connected_shards.shards {
            let sector_count = shard.sectors.len();
            total_sectors += sector_count;

            let uptime = match shard.connected_at.elapsed() {
                Ok(duration) => {
                    let hours = duration.as_secs() / 3600;
                    let minutes = (duration.as_secs() % 3600) / 60;
                    let seconds = duration.as_secs() % 60;
                    format!("{}h {}m {}s", hours, minutes, seconds)
                }
                Err(_) => "unknown".to_string(),
            };

            info!(
                shard_id = %shard.shard_id,
                client_id = %client_id,
                sectors = %sector_count,
                uptime = %uptime,
                "Shard server status"
            );
        }

        info!("Total managed sectors: {}", total_sectors);
        info!("===============================");
    }
}

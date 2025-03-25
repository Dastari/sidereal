# Sidereal Replication System Design

## Overview

The Sidereal engine uses a distributed server architecture with a central replication server and multiple shard servers. This document outlines the design and implementation of this system using `bevy_replicon` and `bevy_replicon_renet2`.

## Libraries

### bevy_replicon

Bevy Replicon is a server-authoritative networking crate for the Bevy game engine that provides:

- Automatic world replication
- Remote events and triggers
- Control over client visibility of entities and events
- Abstraction to support singleplayer, client, dedicated server, and listen server configurations
- No built-in I/O, can be used with any messaging library

### bevy_replicon_renet2

This library provides integration between bevy_replicon and bevy_renet2, which handles the network transport layer. It includes:

- Automatic management of channels
- Integration with Renet Client/Server
- Netcode transport support
- Default configurations for common networking patterns

## Architecture

### System Components

Our architecture consists of three main components:

1. **Replication Server**: Central hub that manages entity replication between shards
2. **Shard Servers**: Individual game servers that handle specific regions
3. **Bidirectional Communication**: Two-way connections between replication and shard servers

### Deployment Patterns

| Pattern         | Description                                                          |
| --------------- | -------------------------------------------------------------------- |
| Multiple Shards | Multiple shard servers connect to a single replication server        |
| Single Shard    | Single shard server connects to replication server (simpler testing) |
| Combined        | A single process runs both a shard server and replication server     |

## Implementation

### 1. Plugin Organization

```rust
// Core plugins for replication
app.add_plugins((
    // IMPORTANT: RepliconPlugins must come BEFORE RepliconRenetPlugins
    RepliconPlugins,         // Core replication functionality
    RepliconRenetPlugins,    // Network transport integration
))

// Our custom plugins
app.add_plugins((
    // For replication server
    ServerNetworkPlugin,      // Server-specific networking

    // For shard server
    ClientNetworkPlugin,      // Client-specific networking

    // Configuration
    BiDirectionalReplicationSetupPlugin {
        replication_server_config: config, // If this is a replication server
        shard_config: config,              // If this is a shard server
    },
))
```

### 2. Resource Management

To avoid borrow checker issues, we split our connection resources:

```rust
// For storing shard client connections
#[derive(Resource, Default)]
pub struct ShardClients {
    pub clients: Vec<(u64, RenetClient)>,
}

// For storing shard transport connections
#[derive(Resource, Default)]
pub struct ShardTransports {
    pub transports: Vec<(u64, NetcodeClientTransport)>,
}

// Connection Tracking
#[derive(Resource, Default)]
pub struct ConnectedShards {
    // Map client IDs to shard IDs
    pub client_to_shard: HashMap<u64, u64>,
    // Map shard IDs to their addresses
    pub shard_addresses: HashMap<u64, SocketAddr>,
    // Track shards with reverse connections
    pub reverse_connected: Vec<u64>,
}
```

### 3. Connection Configuration

To ensure consistency, create stable connection configurations:

```rust
// Create a stable connection configuration with explicit channels
pub fn to_stable_connection_config() -> ConnectionConfig {
    // Create explicit channels matching Replicon's expectations
    let mut server_channels = Vec::new();
    let mut client_channels = Vec::new();

    // Channel 0: Reliable ordered for entities
    let reliable_ordered = ChannelConfig {
        channel_id: 0,
        max_memory_usage_bytes: 5 * 1024 * 1024, // 5MB
        send_type: SendType::ReliableOrdered {
            resend_time: Duration::from_millis(300)
        },
    };
    server_channels.push(reliable_ordered.clone());
    client_channels.push(reliable_ordered);

    // Channel 1: Unreliable for frequent updates
    let unreliable = ChannelConfig {
        channel_id: 1,
        max_memory_usage_bytes: 5 * 1024 * 1024, // 5MB
        send_type: SendType::Unreliable,
    };
    server_channels.push(unreliable.clone());
    client_channels.push(unreliable);

    // Channel 2: Reliable unordered for events
    let reliable_unordered = ChannelConfig {
        channel_id: 2,
        max_memory_usage_bytes: 5 * 1024 * 1024, // 5MB
        send_type: SendType::ReliableUnordered {
           resend_time: Duration::from_millis(300)
        },
    };
    server_channels.push(reliable_unordered.clone());
    client_channels.push(reliable_unordered);

    // Ensure consistent channel IDs on both sides
    ConnectionConfig::from_channels(server_channels, client_channels)
}
```

### 4. Server Initialization

Initialize servers with explicit configuration:

```rust
// Initialize a shard server with both server and client components
pub fn init_shard_server(
    commands: &mut Commands,
    config: &ShardConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use predictable port pattern for shard servers
    let bind_port = 5000 + config.shard_id as u16;
    let bind_addr = SocketAddr::new(
        "127.0.0.1".parse().unwrap(),
        bind_port
    );

    // Step 1: Initialize the shard as a server
    let socket = UdpSocket::bind(bind_addr)?;
    socket.set_nonblocking(true)?;

    let native_socket = NativeSocket::new(socket)?;
    let connection_config = to_stable_connection_config();

    let server_config = ServerSetupConfig {
        current_time: SystemTime::now().duration_since(UNIX_EPOCH)?,
        max_clients: 64,
        protocol_id: config.protocol_id,
        socket_addresses: vec![vec![bind_addr]],
        authentication: ServerAuthentication::Unsecure,
    };

    let transport = NetcodeServerTransport::new(server_config, native_socket)?;
    let server = RenetServer::new(connection_config);

    commands.insert_resource(server);
    commands.insert_resource(transport);

    // Step 2: Initialize the shard as a client to the replication server
    // CRUCIAL: Use a different ID range (20000+) for client role
    let client_id = 20000 + config.shard_id;

    // Small delay to avoid socket binding race conditions
    std::thread::sleep(Duration::from_millis(100));

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;

    let native_socket = NativeSocket::new(socket)?;
    let connection_config = to_stable_connection_config();

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: config.protocol_id,
        server_addr: config.replication_server_addr,
        user_data: None, // No user data for simplicity
        socket_id: 0,
    };

    let transport = NetcodeClientTransport::new(
        SystemTime::now().duration_since(UNIX_EPOCH)?,
        authentication,
        native_socket,
    )?;

    // IMPORTANT: Use false for should_disconnect_disconnected_clients
    // This prevents premature disconnections
    let client = RenetClient::new(connection_config, false);

    commands.insert_resource(client);
    commands.insert_resource(transport);

    Ok(())
}
```

### 5. Transport Updates

We separate update systems to avoid conflicts:

```rust
// Update client transport with fixed timestep and reconnection support
pub fn update_client_transport(
    mut client: ResMut<RenetClient>,
    mut client_transport: ResMut<NetcodeClientTransport>,
    mut network_stats: ResMut<NetworkStats>,
    time: Res<Time>,
) {
    // Update connection status
    let was_connected = network_stats.is_connected_to_server;
    network_stats.is_connected_to_server = client.is_connected();

    // Use a fixed delta time to ensure consistent behavior
    let delta = Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep

    if let Err(e) = client_transport.update(delta, &mut client) {
        // Log at debug level to reduce spam
        debug!("Client transport update error: {:?}", e);
        network_stats.is_connected_to_server = false;
        // Don't call client.disconnect() - let automatic reconnection happen
    }
}

// Update shard transports in dedicated system
pub fn update_shard_transports(
    time: Res<Time>,
    mut shard_transports: ResMut<ShardTransports>,
    mut shard_clients: ResMut<ShardClients>,
) {
    // Use a fixed delta time to ensure consistent behavior
    let delta = Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep

    // Create a local set to track disconnected shards
    let mut disconnected = HashSet::new();

    // Process each transport separately to avoid ownership issues
    for i in 0..shard_transports.transports.len() {
        if let Some((shard_id, transport)) = shard_transports.transports.get_mut(i) {
            // Find the corresponding client
            if let Some(j) = shard_clients.clients.iter_mut()
                .position(|(id, _)| id == shard_id)
            {
                if let Some((_, client)) = shard_clients.clients.get_mut(j) {
                    if let Err(e) = transport.update(delta, client) {
                        debug!("Shard transport update error: {:?}", e);
                        disconnected.insert(*shard_id);
                    }
                }
            }
        }
    }

    // Cleanup disconnected shards in a separate system
}
```

### 6. Shard Connection Management

Safe connection handling:

```rust
// Handle connecting shards
pub fn handle_shard_connections(
    time: Res<Time>,
    mut commands: Commands,
    mut server_events: EventReader<ServerEvent>,
    server: Res<RenetServer>,
    config: Option<Res<ReplicationServerConfig>>,
    mut connected_shards: ResMut<ConnectedShards>,
    mut shard_clients: ResMut<ShardClients>,
    mut shard_transports: ResMut<ShardTransports>,
    transport: Res<NetcodeServerTransport>,
) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                // Check if this is a shard connecting (20000+ ID range)
                if *client_id >= 20000 {
                    let shard_id = *client_id - 20000;
                    info!("Shard {} connected with client ID {}", shard_id, client_id);

                    connected_shards.client_to_shard.insert(*client_id, shard_id);

                    // Get the client's address for reverse connection
                    if let Some(addr) = server.user_data(*client_id) {
                        let shard_addr = SocketAddr::new(
                            "127.0.0.1".parse().unwrap(),
                            5000 + shard_id as u16 // Predictable shard port
                        );

                        connected_shards.shard_addresses.insert(shard_id, shard_addr);

                        // Establish reverse connection with a different ID range (10000+)
                        if !connected_shards.reverse_connected.contains(&shard_id) {
                            // Create reverse connection with ID in 10000+ range
                            connect_to_shard(
                                &mut commands,
                                &config,
                                shard_id,
                                shard_addr,
                                DEFAULT_PROTOCOL_ID,
                                &mut shard_clients,
                                &mut shard_transports,
                                true,
                            ).unwrap_or_else(|e| {
                                error!("Failed to connect to shard {}: {}", shard_id, e);
                            });

                            connected_shards.reverse_connected.push(shard_id);
                        }
                    }
                }
            },
            ServerEvent::ClientDisconnected { client_id, .. } => {
                // Remove disconnected shards
                if let Some(shard_id) = connected_shards.client_to_shard.remove(client_id) {
                    info!("Shard {} disconnected", shard_id);

                    if let Some(index) = connected_shards.reverse_connected
                        .iter().position(|id| *id == shard_id)
                    {
                        connected_shards.reverse_connected.swap_remove(index);
                    }
                }
            }
        }
    }
}
```

### 7. Key Lessons Learned

1. **Client ID Ranges**: Use distinct ID ranges for different connection types:

   - 20000+ for shard → replication server connections
   - 10000+ for replication → shard connections
   - 1-1000 for regular game clients

2. **Connection Configuration**: Use identical connection configurations:

   - Ensure all channel IDs and parameters match exactly
   - Use explicit channel configuration for predictability

3. **Socket Binding**: Avoid race conditions:

   - Add small delays between socket operations
   - Use non-blocking sockets for all connections

4. **Avoid Double Transport Updates**:

   - Servers should only update their server transport
   - Clients should only update their client transport
   - Bi-directional connections need careful update ordering

5. **Client Reconnection**:

   - Use `RenetClient::new(config, false)` to allow automatic reconnection
   - Don't call `client.disconnect()` in error handlers

6. **Resource Organization**:
   - Split client and transport storage into separate resources
   - Use dedicated systems for each update operation

## Testing Strategy

1. **Single-process test**: Run replication server and shard server in the same process
2. **Multi-process test**: Run replication server and multiple shard servers as separate processes
3. **Entity migration**: Test entity migration between shards
4. **Connection resilience**: Test reconnection behavior when connections drop
5. **Startup order**: Test different startup order combinations (replication first, shards first)

## Implementation Checklist

1. [x] Update plugin organization to use library systems where possible
2. [x] Use stable connection configurations for consistency
3. [x] Implement client ID range separation
4. [x] Split client and transport resources to avoid borrow checker issues
5. [x] Implement robust error handling for connection issues
6. [x] Use fixed timesteps for transport updates
7. [x] Add appropriate logging to track connection state

## Conclusion

By implementing these patterns and best practices, we've created a robust bi-directional replication system that handles connection failures gracefully and avoids the subtle issues common in complex networking code.

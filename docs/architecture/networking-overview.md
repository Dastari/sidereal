[← Back to Documentation Index](../README.md) | [Game World Partitioning](./game-world.md)

# Sidereal Networking & Communication Systems

## Overview

This document outlines the networking and communication technologies used throughout the Sidereal project, describing how different components interact across the distributed architecture.

```
┌─────────────────┐     ┌────────────────┐     ┌─────────────────┐
│                 │     │                │     │                 │
│  Web Clients    │◄───►│  Auth Server   │◄───►│  Supabase DB    │
│  (React/Babylon)│     │                │     │                 │
│                 │     └────────────────┘     └─────────────────┘
└────────┬────────┘               ▲                     ▲
         │                        │                     │
         ▼                        │                     │
┌─────────────────┐               │                     │
│                 │               │                     │
│  Replication    │◄──────────────┘                     │
│  Server         │                                     │
│                 │◄─────────────────────────────────┐  │
└────────┬────────┘                                  │  │
         │                                           │  │
         ▼                                           │  │
┌─────────────────┐     ┌─────────────────┐          │  │
│                 │     │                 │          │  │
│  Shard Server   │◄───►│  Shard Server   │◄─────────┘  │
│  Instance 1     │     │  Instance N     │             │
│                 │     │                 │             │
└─────────────────┘     └─────────────────┘             │
         │                       │                      │
         └───────────────────────┼──────────────────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │                 │
                        │  Sidereal Core  │
                        │  (Shared Code)  │
                        │                 │
                        └─────────────────┘
```

## Key Technologies

The following network technologies and protocols are used throughout the Sidereal project:

| Technology                           | Purpose                                | Components Using It               |
| ------------------------------------ | -------------------------------------- | --------------------------------- |
| bevy_replicon & bevy_replicon_renet2 | Server-to-server entity replication    | Replication Server, Shard Servers |
| WebSockets                           | Real-time client communication         | Replication Server, Web Clients   |
| HTTP/REST                            | Database interaction, authentication   | Auth Server, Replication Server   |
| GraphQL                              | Complex universe queries               | Replication Server                |
| Crossbeam channels                   | Internal message passing               | All server components             |
| Serde                                | Data serialization                     | All components                    |
| Direct P2P (renet2)                  | Shard-to-shard boundary communications | Shard Servers                     |

## Communication Pathways

### 1. Replication Server ↔ Shard Servers

**Technologies:** `bevy_replicon`, `bevy_replicon_renet2`

Communication between the replication server (`sidereal-replication-server`) and shard servers (`sidereal-shard-server`) is handled using:

- **bevy_replicon**: Provides high-level replication abstractions for Bevy ECS
- **bevy_replicon_renet2**: Implements the network transport layer using renet2 (UDP-based protocol)

This pathway handles:

- Shard server registration and management
- Entity updates and state synchronization
- Sector boundary crossing coordination
- Load balancing instructions
- Empty sector timeout management
- Neighbor shard discovery and connection facilitation

### 2. Shard Server ↔ Shard Server

**Technologies:** `renet2` (direct), `bevy_replicon` (mediated)

Shard servers can communicate with each other in two ways:

- **Direct communication**: Using renet2 for low-latency updates about entities near boundaries
- **Replication-server mediated**: Using bevy_replicon when direct communication isn't established or fails

This pathway handles:

- Read-only shadow entity synchronization
- Boundary awareness updates
- Handover coordination for entity transitions

Sample implementation:

```rust
// Shadow entity synchronization system (direct mode)
fn sync_shadow_entities_direct(
    managed_clusters: Res<ManagedClusters>,
    boundary_entities: Query<(Entity, &SpatialPosition, &Velocity), With<NearBoundary>>,
    neighbor_connections: Res<NeighborConnections>,
    mut outgoing_buffer: ResMut<DirectMessageBuffer>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_seconds_f64();

    // Only send updates at configured frequency to avoid network saturation
    if current_time - managed_clusters.last_boundary_sync < CONFIG.boundary_sync_interval {
        return;
    }

    // Get entities near each boundary
    for (neighbor_id, connection) in neighbor_connections.active.iter() {
        let boundary_direction = connection.boundary_direction;
        let mut update_batch = BoundaryEntityBatch {
            timestamp: current_time,
            source_shard_id: managed_clusters.shard_id,
            entities: Vec::new(),
        };

        // Find entities near this specific boundary
        for (entity, position, velocity) in boundary_entities.iter() {
            if is_near_boundary(position, boundary_direction, &managed_clusters.config) {
                // Add to update batch
                update_batch.entities.push(ShadowEntityData {
                    id: entity,
                    position: position.position,
                    velocity: velocity.0,
                    components: serialize_relevant_components(entity),
                });
            }
        }

        // If we have entities to send, queue the message
        if !update_batch.entities.is_empty() {
            outgoing_buffer.enqueue_message(*neighbor_id, Message::BoundarySyncUpdate(update_batch));
        }
    }
}

// Handle shadow entity updates from neighboring shards
fn process_shadow_updates(
    mut commands: Commands,
    mut incoming_messages: ResMut<IncomingMessageQueue>,
    mut shadow_entities: ResMut<ShadowEntityRegistry>,
) {
    while let Some(message) = incoming_messages.dequeue() {
        match message {
            Message::BoundarySyncUpdate(batch) => {
                for entity_data in batch.entities {
                    // Check if we already have this shadow entity
                    if let Some(shadow_entity) = shadow_entities.get(&entity_data.id) {
                        // Update existing shadow entity
                        if let Ok(mut transform) = shadow_entity.transform.get_mut() {
                            transform.translation = entity_data.position.extend(0.0);
                        }
                        // Update other components...
                    } else {
                        // Create new shadow entity
                        let shadow = commands
                            .spawn()
                            .insert(ShadowEntity {
                                source_shard_id: batch.source_shard_id,
                                original_id: entity_data.id,
                                is_read_only: true,
                            })
                            .insert(Transform::from_translation(entity_data.position.extend(0.0)))
                            .insert(Velocity(entity_data.velocity))
                            // Add visual representation but no physics collider
                            .insert(VisualOnly)
                            .id();

                        shadow_entities.register(entity_data.id, shadow);
                    }
                }
            },
            // Other message types...
        }
    }
}
```

### 3. Replication Server ↔ Web Clients

**Technologies:** `WebSockets`, `GraphQL`

The replication server provides two interfaces for web clients:

- **WebSockets**: For real-time bidirectional communication, including:

  - Entity position updates
  - Player actions
  - Environment events
  - Chat messages

- **GraphQL API**: For complex universe queries such as:
  - Advanced filtering of universe entities
  - Map data retrieval
  - Player statistics

### 4. Auth Server ↔ Web Clients and Replication Server

**Technologies:** `HTTP/REST`, `JWT`

The authentication server (`sidereal-auth-server`) handles:

- User registration and login requests
- Token generation and verification
- Session management
- Connection authorization between clients and replication server

### 5. Replication Server ↔ Supabase Database

**Technologies:** `HTTP/REST` (via `reqwest`)

The replication server interacts with Supabase using REST API calls for:

- Persistence of universe state
- CRUD operations on game entities
- User data storage
- Sector information
- Empty sector data storage and retrieval

## Workspace-Specific Implementation Details

### sidereal-core

The core library provides shared functionality used by all server components, including:

- Common data structures for network messages
- Serialization helpers
- ECS components designed for network replication
- Boundary entity shadow implementation

```rust
// Example of shadow entity implementation in sidereal-core
#[derive(Component, Serialize, Deserialize)]
pub struct ShadowEntity {
    pub source_shard_id: Uuid,
    pub original_id: Entity,
    pub is_read_only: bool,
    pub last_updated: f64,
}

// Message types for boundary awareness
#[derive(Serialize, Deserialize)]
pub enum BoundaryMessage {
    EntityUpdate {
        entities: Vec<ShadowEntityData>,
        source_shard_id: Uuid,
        timestamp: f64,
    },
    AwarenessRequest {
        boundary_directions: Vec<BoundaryDirection>,
        requesting_shard_id: Uuid,
    },
    AwarenessResponse {
        entities: Vec<ShadowEntityData>,
        boundary_direction: BoundaryDirection,
        source_shard_id: Uuid,
    }
}
```

### sidereal-replication-server

Current implementation includes:

- Basic heartbeat system for status monitoring
- Database client for Supabase interaction
- Scene loading and management
- Empty sector timeout management

Future implementations will include:

- WebSocket server for client connections
- bevy_replicon integration for shard communication
- GraphQL endpoint for complex queries
- Event dispatcher for handling cross-shard entity events
- Neighbor shard discovery and connection facilitation

```rust
// Example of neighbor discovery coordination in replication server
fn coordinate_neighbor_discovery(
    shard_assignments: Res<ShardAssignments>,
    mut events: EventWriter<NeighborDiscoveryEvent>,
) {
    // Check for clusters that are adjacent but managed by different shards
    for (shard_id, clusters) in &shard_assignments.assignments {
        for cluster_coords in clusters {
            // Check all adjacent directions
            for direction in [Direction::North, Direction::East, Direction::South, Direction::West] {
                let neighbor_coords = get_adjacent_cluster_coords(*cluster_coords, direction);

                // See if this adjacent cluster is assigned to a different shard
                if let Some(neighbor_shard) = shard_assignments.find_shard_for_cluster(&neighbor_coords) {
                    if *shard_id != neighbor_shard {
                        // These shards need to communicate about their boundary
                        events.send(NeighborDiscoveryEvent {
                            shard_a: *shard_id,
                            shard_b: neighbor_shard,
                            shared_boundary: SharedBoundary {
                                direction_from_a_to_b: direction,
                                cluster_a: *cluster_coords,
                                cluster_b: neighbor_coords,
                            }
                        });
                    }
                }
            }
        }
    }
}
```

### sidereal-shard-server

Current implementation is minimal, but future versions will include:

- bevy_replicon client for connecting to replication server
- Physics simulation for assigned sector
- Entity management systems
- Performance monitoring and reporting
- Direct communication with neighboring shard servers
- Shadow entity management for boundary awareness

```rust
// Example of shard server direct connection setup
fn setup_neighbor_connections(
    mut commands: Commands,
    discovery_events: EventReader<NeighborDiscoveryEvent>,
    config: Res<NetworkConfig>,
) {
    for event in discovery_events.iter() {
        if event.shard_a == config.local_shard_id {
            // We need to connect to shard_b
            let neighbor_info = event.shard_b_info.clone();

            info!("Setting up connection to neighboring shard {}", neighbor_info.id);

            // Create connection configuration
            let connection_config = ConnectionConfig {
                neighbor_id: neighbor_info.id,
                host: neighbor_info.host.clone(),
                port: neighbor_info.port,
                shared_boundary: event.shared_boundary.clone(),
            };

            // Spawn connection task
            commands.spawn((
                NeighborConnection {
                    state: ConnectionState::Connecting,
                    config: connection_config,
                    retry_count: 0,
                    last_activity: 0.0,
                },
                DirectConnectionTask::new(),
            ));
        }
    }
}
```

### sidereal-auth-server

This component handles:

- User authentication
- Token generation and validation
- Session management

### Future Components

#### sidereal-web-client

A planned React/BabylonJS frontend that will implement:

- WebSocket client for real-time updates
- GraphQL queries for universe information
- JWT authentication flow
- Game UI

#### sidereal-metrics-server

A potential future component for:

- Performance monitoring
- Server health checks
- Resource utilization tracking
- Player analytics

## Data Flow Examples

### Player Movement

1. Player sends movement input via WebSocket to replication server
2. Replication server routes input to appropriate shard server
3. Shard server processes physics and updates entity state
4. Updated entity state is sent back to replication server via bevy_replicon
5. Replication server broadcasts update to relevant web clients via WebSockets
6. Periodically, replication server persists state to Supabase

### Cross-Shard Entity Movement

1. Entity approaches shard boundary
2. Source shard detects boundary crossing and prepares for transition
3. Source shard shares entity with destination shard as a shadow entity
4. Entity continues to move, with updates mirrored to shadow entity
5. When entity crosses boundary, source shard notifies replication server
6. Replication server coordinates handover to destination shard
7. Shadow entity is promoted to full entity in destination shard
8. Original entity is removed from source shard
9. Clients observing the entity see a seamless transition

### Shadow Entity Awareness

1. Shard server identifies entities near boundary with neighboring shard
2. If direct connection exists, shard sends entity data directly to neighbor
3. If no direct connection, data is sent via replication server
4. Receiving shard creates read-only shadow entities
5. Shadow entities are updated regularly but have no physics simulation
6. Client sees entities across boundary for seamless visual experience
7. When entities move away from boundary, shadow entities are cleaned up

## Security Considerations

- All WebSocket connections require authentication via JWT
- Server-to-server communication is secured by internal network configuration
- Direct shard-to-shard connections use secure authentication tokens
- Database access is managed through Supabase's security framework
- Rate limiting is implemented on all public-facing endpoints

## Future Enhancements

- WebRTC for client-to-client direct communication (P2P features)
- Protocol buffers for more efficient serialization
- QUIC protocol for improved web client connections
- Redis for distributed cache and pub/sub functionality
- Advanced networking mesh for optimized shard-to-shard communication
- Auto-scaling shard servers based on player density and cluster activity

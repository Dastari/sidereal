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
┌─────────────────────────────────────────┐             │
│                                         │             │
│  Replication Server                     │             │
│  ┌─────────────────────────────────┐    │             │
│  │ Sidereal Core (Shared Library)  │    │◄────────────┘
│  └─────────────────────────────────┘    │
│                                         │
└───────────────────┬─────────────────────┘
                    │
                    │
                    ▼
    ┌───────────────────────────────────┐
    │                                   │
    ▼                                   ▼
┌─────────────────────────────┐  ┌─────────────────────────────┐
│                             │  │                             │
│  Shard Server Instance 1    │◄►│  Shard Server Instance N    │
│  ┌─────────────────────┐    │  │  ┌─────────────────────┐    │
│  │ Sidereal Core       │    │  │  │ Sidereal Core       │    │
│  │ (Shared Library)    │    │  │  │ (Shared Library)    │    │
│  └─────────────────────┘    │  │  └─────────────────────┘    │
│                             │  │                             │
└─────────────────────────────┘  └─────────────────────────────┘
```

## Key Technologies

The following network technologies and protocols are used throughout the Sidereal project:

| Technology                           | Purpose                                | Components Using It               |
| ------------------------------------ | -------------------------------------- | --------------------------------- |
| bevy_replicon & bevy_replicon_renet2 | Server-to-server entity replication    | Replication Server, Shard Servers |
| WebSockets                           | Real-time client communication         | Replication Server, Web Clients   |
| HTTP/REST                            | Database interaction, authentication   | Auth Server, Replication Server   |
| GraphQL                              | Complex universe queries               | Replication Server                |
| Bevy EventWriter/EventReader         | Internal message passing               | All server components             |
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

Shard servers communicate with each other using a hybrid approach:

- **Direct P2P communication**: Using renet2 for low-latency updates about entities near boundaries
- **Replication-server mediated**: Using bevy_replicon when direct communication isn't established or fails

This pathway handles:

- Shadow entity synchronization
- Boundary awareness updates
- Handover coordination for entity transitions

Sample implementation:

```rust
// Shadow entity synchronization system (direct mode)
fn sync_shadow_entities_direct(
    managed_clusters: Res<ManagedClusters>,
    boundary_entities: Query<(Entity, &SpatialPosition, &Velocity), With<NearBoundary>>,
    neighbor_connections: Res<NeighborConnections>,
    mut outgoing_events: EventWriter<ShadowEntityUpdateEvent>,
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
                    velocity: velocity.linvel,
                    components: serialize_relevant_components(entity),
                });
            }
        }

        // If we have entities to send, dispatch the event
        if !update_batch.entities.is_empty() {
            outgoing_events.send(ShadowEntityUpdateEvent {
                neighbor_id: *neighbor_id,
                batch: update_batch
            });
        }
    }
}

// Handle shadow entity updates from neighboring shards
fn process_shadow_updates(
    mut commands: Commands,
    mut incoming_events: EventReader<IncomingShadowUpdateEvent>,
    mut shadow_entities: ResMut<ShadowEntityRegistry>,
    mut transform_query: Query<&mut Transform>,
    mut velocity_query: Query<&mut Velocity>,
) {
    for event in incoming_events.iter() {
        let batch = &event.batch;

        for entity_data in &batch.entities {
            // Check if we already have this shadow entity
            if let Some(shadow_entity) = shadow_entities.get(&entity_data.id) {
                // Update existing shadow entity
                if let Ok(mut transform) = transform_query.get_mut(shadow_entity.local_entity) {
                    transform.translation = Vec3::new(entity_data.position.x, entity_data.position.y, 0.0);
                }

                if let Ok(mut velocity) = velocity_query.get_mut(shadow_entity.local_entity) {
                    velocity.linvel = entity_data.velocity;
                }

                // Update any other components that need updating
                update_shadow_entity_components(shadow_entity.local_entity, &entity_data.components, &mut commands);

                // Update last refreshed time
                shadow_entities.update_timestamp(&entity_data.id, batch.timestamp);
            } else {
                // Create new shadow entity
                let local_entity = commands
                    .spawn((
                        ShadowEntity {
                            source_shard_id: batch.source_shard_id,
                            original_entity: entity_data.id,
                            is_read_only: true,
                        },
                        Transform::from_translation(Vec3::new(entity_data.position.x, entity_data.position.y, 0.0)),
                        Velocity {
                            linvel: entity_data.velocity,
                            angvel: 0.0
                        },
                        // Visual representation components but no physics collider
                        VisualOnly,
                    ))
                    .id();

                // Add any additional components from the serialized data
                add_shadow_entity_components(local_entity, &entity_data.components, &mut commands);

                // Register in the shadow entity registry
                shadow_entities.register(entity_data.id, local_entity, batch.timestamp);
            }
        }

        // Clean up shadow entities that were not refreshed
        shadow_entities.prune_outdated_shadows(batch.timestamp - CONFIG.shadow_entity_timeout, &mut commands);
    }
}

// Helper to add component data to shadow entities
fn add_shadow_entity_components(
    entity: Entity,
    component_data: &HashMap<String, serde_json::Value>,
    commands: &mut Commands
) {
    for (name, value) in component_data {
        match name.as_str() {
            "ShipVisual" => {
                if let Ok(visual) = serde_json::from_value::<ShipVisualData>(value.clone()) {
                    commands.entity(entity).insert(ShipVisual {
                        model_type: visual.model_type,
                        color: visual.color,
                        scale: visual.scale,
                    });
                }
            },
            "Name" => {
                if let Ok(name_value) = serde_json::from_value::<String>(value.clone()) {
                    commands.entity(entity).insert(Name::new(name_value));
                }
            },
            // Add other component types as needed
            _ => {}
        }
    }
}

// Update components on existing shadow entities
fn update_shadow_entity_components(
    entity: Entity,
    component_data: &HashMap<String, serde_json::Value>,
    commands: &mut Commands
) {
    // Similar to add_shadow_entity_components but handles updates to existing components
    // Implementation would check for component existence and update or insert as needed
}

// System to clean up shadow entities when they're no longer needed or valid
fn cleanup_shadow_entities(
    mut shadow_registry: ResMut<ShadowEntityRegistry>,
    mut commands: Commands,
    time: Res<Time>,
) {
    let current_time = time.elapsed_seconds_f64();
    shadow_registry.prune_outdated_shadows(current_time - CONFIG.shadow_entity_timeout, &mut commands);
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
- Shadow entity implementation

```rust
// Shadow entity implementation in sidereal-core
#[derive(Component, Serialize, Deserialize)]
pub struct ShadowEntity {
    pub source_shard_id: Uuid,
    pub original_entity: Entity,
    pub is_read_only: bool,
    pub last_updated: f64,
}

// Marker component for visual-only entities (no physics processing)
#[derive(Component)]
pub struct VisualOnly;

// Shadow entity registry for managing shadow entities
#[derive(Resource)]
pub struct ShadowEntityRegistry {
    // Maps original entity ID to local shadow entity
    entity_map: HashMap<Entity, ShadowEntityInfo>,
}

#[derive(Clone)]
pub struct ShadowEntityInfo {
    pub local_entity: Entity,
    pub source_shard_id: Uuid,
    pub last_updated: f64,
}

impl ShadowEntityRegistry {
    pub fn new() -> Self {
        Self {
            entity_map: HashMap::new(),
        }
    }

    pub fn register(&mut self, original_id: Entity, local_entity: Entity, timestamp: f64) {
        self.entity_map.insert(original_id, ShadowEntityInfo {
            local_entity,
            source_shard_id: Uuid::nil(), // Will be set from the ShadowEntity component
            last_updated: timestamp,
        });
    }

    pub fn get(&self, original_id: &Entity) -> Option<&ShadowEntityInfo> {
        self.entity_map.get(original_id)
    }

    pub fn update_timestamp(&mut self, original_id: &Entity, timestamp: f64) {
        if let Some(info) = self.entity_map.get_mut(original_id) {
            info.last_updated = timestamp;
        }
    }

    pub fn prune_outdated_shadows(&mut self, cutoff_time: f64, commands: &mut Commands) {
        self.entity_map.retain(|original_id, info| {
            let keep = info.last_updated >= cutoff_time;
            if !keep {
                // Shadow is too old, remove the entity
                commands.entity(info.local_entity).despawn();
            }
            keep
        });
    }
}

// Events for shadow entity communication
#[derive(Event)]
pub struct ShadowEntityUpdateEvent {
    pub neighbor_id: Uuid,
    pub batch: BoundaryEntityBatch,
}

#[derive(Event)]
pub struct IncomingShadowUpdateEvent {
    pub batch: BoundaryEntityBatch,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BoundaryEntityBatch {
    pub timestamp: f64,
    pub source_shard_id: Uuid,
    pub entities: Vec<ShadowEntityData>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ShadowEntityData {
    pub id: Entity,
    pub position: Vec2,
    pub velocity: Vec2,
    pub components: HashMap<String, serde_json::Value>,
}
```

### sidereal-replication-server

Current implementation includes:

- Basic heartbeat system for status monitoring
- Database client for Supabase interaction
- Scene loading and management
- Empty sector timeout management
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

The shard server implementation includes:

- bevy_replicon client for connecting to replication server
- Physics simulation for assigned sectors
- Entity management systems
- Performance monitoring and reporting
- Direct communication with neighboring shard servers
- Shadow entity management for boundary awareness

```rust
// Example of shard server direct connection setup
fn setup_neighbor_connections(
    mut commands: Commands,
    mut discovery_events: EventReader<NeighborDiscoveryEvent>,
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

// System to detect entities approaching shard boundaries
fn detect_boundary_entities(
    mut query: Query<(Entity, &SpatialPosition, &Velocity)>,
    universe_config: Res<UniverseConfig>,
    mut boundary_entities: ResMut<NearBoundaryEntities>,
) {
    boundary_entities.entities.clear();

    for (entity, position, velocity) in &mut query {
        // Calculate distance to nearest sector boundary
        let sector_size = universe_config.sector_size;
        let pos_in_sector = Vec2::new(
            position.position.x % sector_size,
            position.position.y % sector_size
        );

        // Find distance to each boundary
        let dist_to_left = pos_in_sector.x;
        let dist_to_right = sector_size - pos_in_sector.x;
        let dist_to_top = pos_in_sector.y;
        let dist_to_bottom = sector_size - pos_in_sector.y;

        // Define boundary awareness threshold based on velocity and a fixed minimum
        let threshold = (velocity.linvel.length() * 2.0).max(universe_config.transition_zone_width);

        // Check if entity is near any boundary
        if dist_to_left < threshold || dist_to_right < threshold ||
           dist_to_top < threshold || dist_to_bottom < threshold {
            // Determine which boundaries the entity is approaching
            let mut boundaries = Vec::new();

            if dist_to_left < threshold {
                boundaries.push(BoundaryDirection::West);
            }
            if dist_to_right < threshold {
                boundaries.push(BoundaryDirection::East);
            }
            if dist_to_top < threshold {
                boundaries.push(BoundaryDirection::North);
            }
            if dist_to_bottom < threshold {
                boundaries.push(BoundaryDirection::South);
            }

            // Add to boundary entities list
            boundary_entities.entities.push((entity, boundaries));
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

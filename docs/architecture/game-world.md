[← Back to Documentation Index](../README.md) | [Architecture Documentation](./networking-overview.md)

# Sidereal: Game World Partitioning System

## Overview

This document outlines the design and implementation of Sidereal's game world partitioning system, which enables the distribution of universe simulation across multiple shard servers while maintaining a seamless player experience. The system uses an Extended Dynamic Grid with Clustering approach, optimized for Sidereal's distributed architecture and built on the Bevy ECS framework.

## Core Design: Extended Dynamic Grid with Clustering

### Conceptual Framework

The universe in Sidereal is partitioned using a hierarchical system:

1. **Sectors**: Fixed-size square regions (e.g., 1000×1000 units) that form the basic unit of spatial division.
2. **Clusters**: Groups of adjacent sectors (e.g., 3×3 or 5×5) that are assigned as a unit to shard servers.
3. **Transition Zones**: Overlapping areas at cluster boundaries that facilitate seamless entity transfers.

This approach supports:

- An infinitely expandable universe
- Efficient distribution of computational load
- Optimal assignment of related entities to the same shard server
- Seamless transitions between areas managed by different shard servers

### Key Components Diagram

```
                  Universe
                      │
                      ▼
         ┌───────────────────────────┐
         │                           │
         ▼                           ▼
┌──────────────────┐        ┌──────────────────┐
│  Shard Server 1  │        │  Shard Server 2  │
│  ┌────────────┐  │        │  ┌────────────┐  │
│  │  Cluster A │  │        │  │  Cluster B │  │
│  │  ┌──┬──┐   │  │        │  │  ┌──┬──┐   │  │
│  │  │S1│S2│   │  │        │  │  │S7│S8│   │  │
│  │  ├──┼──┤   │  │        │  │  ├──┼──┤   │  │
│  │  │S3│S4│   │  │        │  │  │S9│S10│  │  │
│  │  └──┴──┘   │  │        │  │  └──┴──┘   │  │
│  └────────────┘  │        │  └────────────┘  │
│  ┌────────────┐  │        │  ┌────────────┐  │
│  │  Cluster C │  │        │  │  Cluster D │  │
│  │  ┌──┬──┐   │  │        │  │  ┌──┬──┐   │  │
│  │  │S5│S6│   │  │        │  │  │S11│S12│  │  │
│  │  └──┴──┘   │  │        │  │  └──┴──┘   │  │
│  └────────────┘  │        │  └────────────┘  │
└──────────────────┘        └──────────────────┘
     ▲                                 ▲
     │                                 │
     └─────────────┬───────────────────┘
                   │
                   ▼
         ┌───────────────────┐
         │Replication Server │
         │ (Coordinates      │
         │  assignments)     │
         └───────────────────┘
```

## Implementation Details

### 1. Data Structures

#### Core Components (Bevy ECS)

```rust
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::{HashMap, HashSet};

// Entity location and sector tracking
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct SpatialPosition {
    pub position: Vec2,       // Actual position in world space
    pub sector_coords: IVec2, // Current sector coordinates
    pub cluster_coords: IVec2, // Current cluster coordinates
}

// Velocity component for physics-based entities
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Velocity {
    pub linvel: Vec2,      // Linear velocity vector
    pub angvel: f32,       // Angular velocity in radians per second
}

// Marker component that requires SpatialPosition
#[derive(Component)]
pub struct SpatialTracked;

// Sector definition - contains entities in a spatial region
#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct Sector {
    pub coordinates: IVec2,
    pub entities: HashSet<Entity>,
    pub active: bool,
    pub last_updated: f64, // Time since startup
    pub last_entity_seen: f64, // Timestamp when the last entity was in this sector
    pub last_saved: f64, // Timestamp of last persistence to database
}

// Cluster definition - group of sectors managed by a single shard
#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct Cluster {
    pub id: Uuid,
    pub base_coordinates: IVec2,
    pub size: IVec2, // How many sectors in each dimension
    pub sectors: HashMap<IVec2, Sector>,
    pub assigned_shard: Option<Uuid>,
    pub entity_count: usize,
    pub transition_zone_width: f32, // Width of buffer around edges
}

// Resource for universe configuration
#[derive(Resource)]
pub struct UniverseConfig {
    pub sector_size: f32,
    pub cluster_dimensions: IVec2,
    pub transition_zone_width: f32,
    pub empty_sector_timeout_seconds: f64, // Time before an empty sector is considered inactive
    pub empty_sector_check_interval: f64, // How often to check for empty sectors
    pub min_boundary_awareness: f32, // Minimum distance to detect boundary approaches
    pub velocity_awareness_factor: f32, // Factor to adjust boundary awareness based on velocity
}

// Resource for tracking all active clusters
#[derive(Resource)]
pub struct UniverseState {
    pub active_clusters: HashMap<IVec2, Cluster>,
    pub shard_assignments: HashMap<Uuid, Vec<IVec2>>,
    pub entity_locations: HashMap<Entity, IVec2>, // Maps entities to their cluster
}
```

#### Network Messages (for bevy_replicon communication)

```rust
use bevy_replicon::prelude::*;

// Messages sent between replication and shard servers
#[derive(Serialize, Deserialize, Component)]
pub enum ClusterManagementMessage {
    AssignCluster {
        cluster_id: Uuid,
        base_coordinates: IVec2,
        size: IVec2,
    },
    ReleaseCluster {
        cluster_id: Uuid,
    },
    ClusterEntityUpdate {
        cluster_id: Uuid,
        entities: Vec<EntityData>,
    },
    EntityTransitionRequest {
        entity_id: Entity,
        source_cluster_id: Uuid,
        destination_cluster_id: Uuid,
        current_position: Vec2,
        velocity: Vec2,
    },
    EntityTransitionAcknowledge {
        entity_id: Entity,
        destination_cluster_id: Uuid,
        transfer_time: f64,
    },
}

// Entity data for replication
#[derive(Serialize, Deserialize, Clone)]
pub struct EntityData {
    pub id: Entity,
    pub position: Vec2,
    pub velocity: Vec2,  // Added velocity for proper physics prediction
    pub components: HashMap<String, serde_json::Value>,
}

// After the ClusterManagementMessage enum, add EntityTransitionMessage
#[derive(Event)]
pub enum EntityTransitionMessage {
    Request {
        entity_id: Entity,
        source_cluster_id: Uuid,
        destination_cluster_id: Uuid,
        current_position: Vec2,
        velocity: Vec2,
    },
    Acknowledge {
        entity_id: Entity,
        destination_cluster_id: Uuid,
        transfer_time: f64,
    },
}
```

### 2. System Implementation

#### Replication Server Systems

```rust
// Plugin for the replication server's universe management
pub struct UniverseManagerPlugin;

impl Plugin for UniverseManagerPlugin {
    fn build(&self, app: &mut App) {
        info!("Building universe manager plugin");

        // Register component types
        SpatialTracked::register_required_components(app);
        ShadowEntity::register_required_components(app);

        // Register events
        app.add_event::<ClusterManagementMessage>()
           .add_event::<EntityTransitionMessage>()
           .add_event::<EntityApproachingBoundary>();

        // Initialize resources
        app.init_resource::<UniverseConfig>()
           .init_resource::<UniverseState>()
           .init_resource::<ShardServerRegistry>();

        // Add core universe management systems
        app.add_systems(Update, (
            update_global_universe_state,
            update_entity_sector_coordinates,
            handle_cluster_assignment,
            process_entity_transition_requests,
            send_entity_transition_acknowledgments,
            manage_empty_sectors,
        ).chain());

        // Add initialization system
        app.add_systems(OnEnter(SceneState::Ready), initialize_universe_state);
    }
}

// Initialize universe configuration
fn initialize_universe_state(
    mut commands: Commands,
    time: Res<Time>,
) {
    info!("Initializing universe state");

    // Universe is already initialized with defaults via init_resource
    // But we can create initial clusters here if needed

    // For now, let's create a single 3x3 cluster at the origin
    let base_coordinates = IVec2::new(0, 0);
    let cluster_id = Uuid::new_v4();
    let current_time = time.elapsed_secs_f64();

    let mut sectors = HashMap::new();

    // Create a 3x3 grid of sectors
    for x in 0..3 {
        for y in 0..3 {
            let sector_coords = IVec2::new(x, y);

            sectors.insert(sector_coords, Sector {
                coordinates: sector_coords,
                entities: HashSet::new(),
                active: true,
                last_updated: current_time,
                last_entity_seen: current_time,
                last_saved: 0.0, // Not yet saved
            });
        }
    }

    // Create the cluster
    let cluster = Cluster {
        id: cluster_id,
        base_coordinates,
        size: IVec2::new(3, 3),
        sectors,
        assigned_shard: None, // Not yet assigned to a shard
        entity_count: 0,
        transition_zone_width: 50.0,
    };

    // Add the cluster to the universe state
    commands.insert_resource(UniverseConfig {
        sector_size: 1000.0,
        cluster_dimensions: IVec2::new(3, 3), // 3x3 sectors per cluster
        transition_zone_width: 50.0,
        empty_sector_timeout_seconds: 300.0, // 5 minutes before unloading empty sectors
        empty_sector_check_interval: 60.0, // Check once per minute
        min_boundary_awareness: 30.0, // Minimum distance for boundary detection
        velocity_awareness_factor: 2.0, // Velocity factor for boundary awareness
    });

    commands.insert_resource(UniverseState {
        active_clusters: [(base_coordinates, cluster)].into_iter().collect(),
        shard_assignments: HashMap::new(),
        entity_locations: HashMap::new(),
    });

    info!("Created initial cluster at {:?} with ID {}", base_coordinates, cluster_id);
}

// System to determine which cluster an entity belongs to
fn calculate_entity_cluster(
    position: Vec2,
    config: &UniverseConfig,
) -> IVec2 {
    let sector_x = (position.x / config.sector_size).floor() as i32;
    let sector_y = (position.y / config.sector_size).floor() as i32;

    let cluster_x = (sector_x as f32 / config.cluster_dimensions.x as f32).floor() as i32;
    let cluster_y = (sector_y as f32 / config.cluster_dimensions.y as f32).floor() as i32;

    IVec2::new(cluster_x, cluster_y)
}

// System to process entity transition requests
pub fn process_entity_transition_requests(
    mut universe_state: ResMut<UniverseState>,
    config: Res<UniverseConfig>,
    mut commands: Commands,
    time: Res<Time>,
    // Only read transition requests
    mut transition_requests: EventReader<EntityTransitionMessage>,
    // Use a command to store transition data for the next system
    mut transition_queue: Local<Vec<(Entity, Uuid, f64)>>,
) {
    for message in transition_requests.read() {
        if let EntityTransitionMessage::Request {
            entity_id,
            source_cluster_id,
            destination_cluster_id,
            current_position: _,
            velocity: _,
        } = message {
            // Handle the transition logic
            info!("Processing entity transition: {:?} from cluster {} to cluster {}",
                  entity_id, source_cluster_id, destination_cluster_id);

            // In a real implementation, this would involve:
            // 1. Coordinate between source and destination shards
            // 2. Update entity_locations mapping

            // Queue the acknowledgment for the next system
            transition_queue.push((*entity_id, *destination_cluster_id, time.elapsed_secs_f64()));

            // Update entity location in the universe state
            // For demonstration only - in a real implementation, we'd have a way to get
            // the cluster coordinates from the cluster ID

            // Since this is just a demo system, we don't need to actually update the entity location
            // The real logic would involve more sophisticated handling of the entity transition
        }
    }
}

// System to send entity transition acknowledgments
pub fn send_entity_transition_acknowledgments(
    mut transition_queue: Local<Vec<(Entity, Uuid, f64)>>,
    mut transition_acks: EventWriter<EntityTransitionMessage>,
) {
    // Send acknowledgments for all queued transitions
    for (entity_id, destination_cluster_id, transfer_time) in transition_queue.drain(..) {
        transition_acks.send(EntityTransitionMessage::Acknowledge {
            entity_id,
            destination_cluster_id,
            transfer_time,
        });
    }
}

// System to assign clusters to shard servers based on load
fn handle_cluster_assignment(
    mut state: ResMut<UniverseState>,
    // Network information about connected shard servers
    shard_info: Res<ShardServerRegistry>,
    // Network sender to send cluster assignments to shard servers
    mut cluster_assignment_sender: EventWriter<ClusterManagementMessage>,
) {
    // Algorithm to assign clusters to shards based on:
    // 1. Load balancing (entity count, update frequency)
    // 2. Spatial proximity (assign adjacent clusters to same shard where possible)
    // 3. Shard server capacity

    // Send assignment messages to relevant shard servers
}

// System to monitor and manage empty sectors
fn manage_empty_sectors(
    time: Res<Time>,
    config: Res<UniverseConfig>,
    mut state: ResMut<UniverseState>,
    mut last_check: Local<f64>,
    mut cluster_management_sender: EventWriter<ClusterManagementMessage>,
    database: Res<DatabaseClient>,
) {
    // Only check periodically to reduce overhead
    if time.elapsed_secs_f64() - *last_check < config.empty_sector_check_interval {
        return;
    }

    *last_check = time.elapsed_secs_f64();
    let current_time = time.elapsed_secs_f64();
    let mut clusters_to_release = Vec::new();
    let mut sectors_to_deactivate = Vec::new();

    // Check each cluster and its sectors
    for (cluster_coords, cluster) in &mut state.active_clusters {
        let mut all_sectors_empty = true;
        let mut all_sectors_timed_out = true;

        for (sector_coords, sector) in &mut cluster.sectors {
            if !sector.entities.is_empty() {
                // Sector has entities
                sector.last_entity_seen = current_time;
                all_sectors_empty = false;
                all_sectors_timed_out = false;
            } else if sector.active {
                // Sector is empty but still active
                let empty_duration = current_time - sector.last_entity_seen;

                if empty_duration > config.empty_sector_timeout_seconds {
                    // Mark sector for deactivation after timeout
                    sectors_to_deactivate.push((*cluster_coords, *sector_coords));
                } else {
                    all_sectors_timed_out = false;
                }
            }
        }

        // If all sectors in cluster are empty and timed out, consider releasing the cluster
        if all_sectors_empty && all_sectors_timed_out {
            clusters_to_release.push(*cluster_coords);
        }
    }

    // Deactivate sectors that have been empty for too long
    for (cluster_coords, sector_coords) in sectors_to_deactivate {
        if let Some(cluster) = state.active_clusters.get_mut(&cluster_coords) {
            if let Some(sector) = cluster.sectors.get_mut(&sector_coords) {
                // Save sector state to database before deactivating
                info!("Deactivating empty sector at {:?} in cluster {:?}", sector_coords, cluster_coords);
                sector.active = false;

                // Persist sector data to database asynchronously
                let sector_clone = sector.clone();
                task::spawn(async move {
                    database.save_sector_state(&sector_clone).await;
                });
            }
        }
    }

    // Process cluster releases when all sectors are inactive
    for cluster_coords in clusters_to_release {
        if let Some(cluster) = state.active_clusters.get(&cluster_coords) {
            if let Some(shard_id) = cluster.assigned_shard {
                // Persist inactive cluster data to database first
                info!("Releasing empty cluster at {:?} from shard {:?}", cluster_coords, shard_id);

                // Tell the shard server to release it
                cluster_management_sender.send(ClusterManagementMessage::ReleaseCluster {
                    cluster_id: cluster.id,
                });

                // Update the assignment tracking
                if let Some(assigned_clusters) = state.shard_assignments.get_mut(&shard_id) {
                    assigned_clusters.retain(|coords| *coords != cluster_coords);
                }
            }
        }
    }
}
```

#### Shard Server Systems

```rust
// Plugin for shard server's handling of assigned clusters
pub struct ShardClusterPlugin;

impl Plugin for ShardClusterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ManagedClusters>()
           .add_systems(Update, (
               process_cluster_assignments,
               update_entity_positions,
               detect_entity_transitions,
               run_physics_simulation,
               sync_entity_states
           ));
    }
}

// Resource to track clusters managed by this shard
#[derive(Resource)]
pub struct ManagedClusters {
    pub clusters: HashMap<Uuid, Cluster>,
    pub entity_map: HashMap<Entity, Uuid>, // Maps entities to their cluster
    pub transition_candidates: Vec<(Entity, Uuid)>, // Entities approaching boundaries
}

// System to receive and process cluster assignments
fn process_cluster_assignments(
    mut managed_clusters: ResMut<ManagedClusters>,
    // Network receiver for cluster management messages
    mut cluster_receiver: EventReader<ClusterManagementMessage>,
    mut commands: Commands,
    time: Res<Time>,
) {
    for message in cluster_receiver.iter() {
        match message {
            ClusterManagementMessage::AssignCluster { cluster_id, base_coordinates, size } => {
                // Initialize cluster data structures
                // Set up sectors within the cluster
                // Load entities from replication server
                let current_time = time.elapsed_secs_f64();

                // Create new sectors with proper initialization
                let mut sectors = HashMap::new();

                // Calculate sector coordinates within the cluster
                for x in 0..size.x {
                    for y in 0..size.y {
                        let sector_coords = IVec2::new(
                            base_coordinates.x + x,
                            base_coordinates.y + y
                        );

                        sectors.insert(sector_coords, Sector {
                            coordinates: sector_coords,
                            entities: HashSet::new(),
                            active: true,
                            last_updated: current_time,
                            last_entity_seen: current_time, // Initialize as current time
                            last_saved: 0.0, // Not yet saved
                        });
                    }
                }

                // Add the new cluster to managed clusters
                let new_cluster = Cluster {
                    id: *cluster_id,
                    base_coordinates: *base_coordinates,
                    size: *size,
                    sectors,
                    assigned_shard: Some(/* shard server ID */),
                    entity_count: 0,
                    transition_zone_width: 50.0, // Get from config
                };

                managed_clusters.clusters.insert(*cluster_id, new_cluster);
            },
            ClusterManagementMessage::ReleaseCluster { cluster_id } => {
                // Clean up cluster data
                // Return any remaining entities to replication server

                if let Some(cluster) = managed_clusters.clusters.remove(cluster_id) {
                    info!("Shard server releasing cluster: {:?}", cluster_id);

                    // Remove all entities in this cluster from entity map
                    managed_clusters.entity_map.retain(|_, cluster_id_value| *cluster_id_value != *cluster_id);

                    // Remove any transition candidates from this cluster
                    managed_clusters.transition_candidates.retain(|(_, cid)| *cid != *cluster_id);

                    // Any final cleanup or state saving should happen here
                },
            },
            // Handle other message types
            _ => {}
        }
    }
}

// System to detect entities approaching cluster boundaries
fn detect_entity_transitions(
    mut managed_clusters: ResMut<ManagedClusters>,
    query: Query<(Entity, &SpatialPosition, &Velocity)>,
    config: Res<UniverseConfig>,
    // Network sender for transition requests
    mut transition_sender: EventWriter<EntityTransitionRequest>,
) {
    for (entity, position, velocity) in query.iter() {
        if let Some(cluster_id) = managed_clusters.entity_map.get(&entity) {
            let cluster = &managed_clusters.clusters[cluster_id];

            // Check if entity is approaching a cluster boundary
            // Using position, velocity, and transition_zone_width

            if is_approaching_boundary(position, velocity, &config) {
                // Calculate destination cluster
                let dest_cluster_coords = calculate_destination_cluster(position, velocity, cluster);

                // Add to transition candidates list
                managed_clusters.transition_candidates.push((entity, *cluster_id));

                // Send transition request to replication server
                transition_sender.send(EntityTransitionRequest {
                    entity_id: entity,
                    source_cluster_id: *cluster_id,
                    destination_cluster_id: calculate_destination_cluster_id(dest_cluster_coords),
                    current_position: position.position,
                    velocity: velocity.0,
                });
            }
        }
    }
}

// Physics simulation system using Rapier
fn run_physics_simulation(
    mut rapier_context: ResMut<RapierContext>,
    managed_clusters: Res<ManagedClusters>,
    mut query: Query<(Entity, &mut Transform, &SpatialPosition, &RigidBody)>,
    time: Res<Time>,
) {
    // Run physics simulation for entities in managed clusters
    // Handle collisions, forces, etc.

    // Note: Actual implementation would integrate more deeply with bevy_rapier
}
```

### 3. Entity Transition Process

The critical process of transitioning entities between clusters (and potentially between shard servers) is handled as follows:

```rust
// Helper function to manage the entity transition process
fn handle_entity_transition(
    entity: Entity,
    source_cluster_id: Uuid,
    dest_cluster_id: Uuid,
    current_state: &EntityData,
    mut source_cluster: &mut Cluster,
    mut dest_cluster: &mut Cluster,
) -> TransitionResult {
    // 1. Preparation Phase
    // Create a "ghost" entity in the destination cluster
    let ghost_entity = create_ghost_entity(dest_cluster, current_state);

    // 2. Synchronization Phase
    // Both clusters update the entity for a short period
    // Source cluster has authority but sends updates to destination
    update_ghost_entity(ghost_entity, current_state);

    // 3. Handover Phase
    // At the appropriate moment, transfer authority
    transfer_authority(entity, ghost_entity, source_cluster, dest_cluster);

    // 4. Cleanup Phase
    // Remove the original entity from source cluster
    remove_entity_from_cluster(entity, source_cluster);

    TransitionResult::Success
}
```

### 4. Load Balancing Algorithm

The system dynamically balances load across shard servers:

```rust
// Load balancing function run periodically by the replication server
fn rebalance_clusters(
    state: &mut UniverseState,
    shard_info: &ShardServerRegistry,
) -> Vec<ClusterReassignment> {
    // Metrics for determining load:
    // 1. Entity count per cluster
    // 2. Update frequency
    // 3. Physics complexity

    // Step 1: Calculate current load for each shard
    let mut shard_loads = calculate_shard_loads(state, shard_info);

    // Step 2: Identify overloaded and underloaded shards
    let (overloaded, underloaded) = identify_load_imbalances(&shard_loads);

    // Step 3: For each overloaded shard, find suitable clusters to move
    let mut reassignments = Vec::new();

    for shard_id in overloaded {
        // Find optimal clusters to relocate
        // Prefer clusters with fewer transitions to other clusters
        // Consider spatial proximity
        let cluster_to_move = select_cluster_to_reassign(shard_id, state);

        // Find best destination shard
        let destination_shard = select_destination_shard(cluster_to_move, &underloaded, state);

        reassignments.push(ClusterReassignment {
            cluster_id: cluster_to_move,
            source_shard: shard_id,
            destination_shard,
        });
    }

    reassignments
}
```

## Integration with Sidereal Architecture

### Replication Server Role

The replication server serves as the orchestrator for the world partitioning system:

1. **Global State Management**:

   - Maintains the definitive mapping of universe clusters
   - Coordinates shard server assignments
   - Handles persistence of inactive clusters to Supabase
   - Manages empty sector timeouts and deactivation

2. **Client Connection Management**:

   - Routes client WebSocket connections to appropriate shard servers
   - Handles authentication and initial player placement
   - Provides WebSocket and GraphQL endpoints for client access
   - Facilitates real-time game state updates to connected clients

3. **Entity Lifecycle Management**:

   - Creates new entities and determines initial placement
   - Coordinates entity transitions between clusters
   - Handles persistence of entity state
   - Monitors and manages resource utilization through empty sector recycling
   - Facilitates neighbor shard discovery and connection establishment

4. **Database Integration**:
   - Manages periodic persistence of game state
   - Handles save/load operations for player data
   - Coordinates database interactions across the distributed system

### Shard Server Role

Shard servers are responsible for the active simulation of their assigned clusters:

1. **Physics and Gameplay**:

   - Runs Rapier physics simulation for all entities in assigned clusters
   - Processes game logic and AI behaviors
   - Handles interactions between entities

2. **Local State Management**:

   - Tracks all entities within assigned clusters
   - Monitors cluster boundaries for transitioning entities
   - Optimizes local performance

3. **Replication**:
   - Uses bevy_replicon to synchronize state with the replication server
   - Handles incremental updates to minimize network usage

### Integration with bevy_replicon and bevy_replicon_renet2

The system leverages bevy_replicon for efficient state synchronization:

```rust
// Setup for replication server
fn setup_replication_server(app: &mut App) {
    app.add_plugins(ReplicationPlugins)
       .add_plugins(ServerPlugin::default())
       .register_replication::<SpatialPosition>()
       .register_replication::<Velocity>()
       // Register other replicated components
       .add_systems(PreUpdate,
           receive_entity_updates.after(ServerSet::Receive)
       )
       .add_systems(PostUpdate,
           send_entity_updates.before(ServerSet::Send)
       );
}

// Setup for shard server
fn setup_shard_server(app: &mut App) {
    app.add_plugins(ReplicationPlugins)
       .add_plugins(ClientPlugin::default())
       .register_replication::<SpatialPosition>()
       .register_replication::<Velocity>()
       // Register other replicated components
       .add_systems(PreUpdate,
           receive_cluster_assignments.after(ClientSet::Receive)
       )
       .add_systems(PostUpdate,
           send_entity_transitions.before(ClientSet::Send)
       );

    // Register component requirements
    app.world().register_component_requirement::<SpatialTracked, SpatialPosition>();
}
```

// Note: Bevy 0.15 Compatibility
// This codebase follows Bevy 0.15 patterns for world access:
// - Use app.world() as a method call, not app.world as a property
// - Use time.elapsed_secs_f64() instead of time.elapsed_seconds_f64()
// - Follow proper system parameter ordering with Commands before Res/ResMut parameters
// - Use world_mut() for mutable world access in tests and startup code

## Performance Optimizations

### 1. Spatial Data Structure Optimizations

```rust
// Optimize sector entity lookup with spatial hashing
#[derive(Resource)]
pub struct SpatialHashGrid {
    pub cell_size: f32,
    pub cells: HashMap<IVec2, Vec<Entity>>,
}

impl SpatialHashGrid {
    // Insert an entity into the appropriate cell
    pub fn insert(&mut self, entity: Entity, position: Vec2) {
        let cell_x = (position.x / self.cell_size).floor() as i32;
        let cell_y = (position.y / self.cell_size).floor() as i32;
        let cell_coords = IVec2::new(cell_x, cell_y);

        self.cells.entry(cell_coords).or_default().push(entity);
    }

    // Query for entities within a radius
    pub fn query_radius(&self, center: Vec2, radius: f32) -> Vec<Entity> {
        // Implementation details
    }
}
```

### 2. Network Bandwidth Optimizations

```rust
// Implement delta compression for entity updates
fn compress_entity_update(
    current: &EntityData,
    previous: Option<&EntityData>,
) -> CompressedEntityData {
    match previous {
        Some(prev) => {
            // Only include changed components
            let mut changed_components = HashMap::new();

            for (name, value) in &current.components {
                if !prev.components.contains_key(name) || prev.components[name] != *value {
                    changed_components.insert(name.clone(), value.clone());
                }
            }

            CompressedEntityData {
                id: current.id,
                position: if (current.position - prev.position).length_squared() > 0.001 {
                    Some(current.position)
                } else {
                    None
                },
                changed_components,
            }
        },
        None => {
            // Include all components for new entity
            CompressedEntityData {
                id: current.id,
                position: Some(current.position),
                changed_components: current.components.clone(),
            }
        }
    }
}
```

### 3. Database Interaction Optimizations

```rust
// Efficiently persist and load universe state
impl UniverseState {
    // Save only active and recently modified sectors
    pub async fn persist_to_database(&self, database: &DatabaseClient) -> DatabaseResult<()> {
        let mut entities_to_update = Vec::new();

        for (coords, cluster) in &self.active_clusters {
            for (_, sector) in &cluster.sectors {
                // Only persist sectors modified since last save
                if sector.active && sector.last_updated > sector.last_saved {
                    for entity in &sector.entities {
                        if let Some(entity_data) = self.get_entity_data(*entity) {
                            entities_to_update.push(convert_to_entity_record(entity_data));
                        }
                    }
                }
            }
        }

        // Batch update to database
        if !entities_to_update.is_empty() {
            database.batch_update_entities(&entities_to_update).await?;
        }

        Ok(())
    }
}
```

## Implementation Roadmap

### Phase 1: Basic Grid Implementation

1. Implement core data structures for sectors and clusters
2. Create single-shard version with basic entity assignment
3. Implement simple database persistence

### Phase 2: Multi-Shard Distribution

1. Implement cluster assignment to different shard servers
2. Create basic entity transition between clusters
3. Establish communication protocol between replication and shard servers

### Phase 3: Advanced Features

1. Implement dynamic load balancing
2. Add buffer zones for seamless transitions
3. Optimize for entity clustering patterns
4. Implement advanced persistence strategies

## Future Improvements

### Cross-Shard Awareness for Neighboring Sectors

As the game world grows and player interactions become more complex, we will need to implement a more sophisticated approach to handling awareness across shard server boundaries. One key improvement will be implementing a "read-only" entity awareness system for neighboring sectors.

#### The Challenge

When two different shard servers manage adjacent clusters, entities on one shard may need to be aware of entities in the neighboring shard for several gameplay reasons:

1. **Visibility**: Players should be able to see entities in neighboring sectors even if those entities are managed by a different shard
2. **AI Decision Making**: NPCs near boundaries need awareness of potential targets or threats across the boundary
3. **Long-range Interactions**: Some game mechanics like projectiles, sensors, or area effects may need to span across boundaries
4. **Seamless Transitions**: Players approaching boundaries need to see a consistent world before and after transition

#### Proposed Solution: Cross-Shard Entity Shadowing

```rust
// Shadow entity representation for entities from neighboring shards
#[derive(Component)]
pub struct ShadowEntity {
    pub source_cluster_id: Uuid,
    pub source_shard_id: Uuid,
    pub original_entity: Entity,
    pub is_read_only: bool,
    pub last_updated: f64,
}

// Registry to manage shadow entities
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

// Message for sharing boundary entities between shards
#[derive(Serialize, Deserialize)]
pub enum BoundarySharingMessage {
    BoundaryEntitiesUpdate {
        source_shard_id: Uuid,
        source_cluster_id: Uuid,
        boundary_entities: Vec<ShadowEntityData>,
        boundary_type: BoundaryType,
    },
    BoundaryAcknowledgement {
        receiving_shard_id: Uuid,
        timestamp: f64,
    }
}

// Types of boundaries between clusters
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryType {
    North,
    East,
    South,
    West,
    Corner(CornerType),
}

// System to identify and share entities near boundaries
fn share_boundary_entities(
    managed_clusters: Res<ManagedClusters>,
    query: Query<(Entity, &SpatialPosition, &Velocity, Changed<SpatialPosition>)>,
    config: Res<UniverseConfig>,
    time: Res<Time>,
    mut boundary_sender: EventWriter<BoundarySharingMessage>,
) {
    // For each cluster, find entities near its boundaries
    for (cluster_id, cluster) in &managed_clusters.clusters {
        let boundary_entities = collect_entities_near_boundaries(cluster, &query, &config);

        // Group by boundary direction
        let boundary_groups = group_entities_by_boundary(boundary_entities, cluster);

        // For each group, prepare and send updates to the appropriate neighboring shard
        for (boundary_type, entities) in boundary_groups {
            // Determine target shard based on boundary direction
            if let Some(neighbor_shard_id) = find_neighbor_shard_for_boundary(
                cluster,
                boundary_type,
                &managed_clusters
            ) {
                boundary_sender.send(BoundarySharingMessage::BoundaryEntitiesUpdate {
                    source_shard_id: managed_clusters.shard_id,
                    source_cluster_id: *cluster_id,
                    boundary_entities: serialize_entities_for_sharing(entities, &query),
                    boundary_type,
                });
            }
        }
    }
}
```

#### Recommended Approach

For Sidereal, a **hybrid approach** is optimal:

```rust
// Implementation for the hybrid communication approach
fn setup_cross_shard_communication(app: &mut App) {
    // Direct communication for high-frequency, performance-critical updates
    app.add_systems(Update, send_direct_neighbor_updates.run_if(|state: Res<ShardState>| {
        state.has_direct_neighbor_connections()
    }));

    // Replication server mediated for setup, teardown, and less frequent updates
    app.add_systems(Update, process_neighbor_discovery);

    // Shadow entity management
    app.add_systems(Update, (
        detect_boundary_entities,
        sync_shadow_entities_direct,
        process_shadow_updates,
        cleanup_shadow_entities
    ));

    // Events for shadow entity communication
    app.add_event::<ShadowEntityUpdateEvent>();
    app.add_event::<IncomingShadowUpdateEvent>();

    // Fallback to replication server when direct communication fails
    app.add_systems(Update, handle_direct_communication_fallback);
}

// System to detect entities approaching sector boundaries
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

In this hybrid model:

1. The replication server is responsible for:

   - Informing shards about which other shards manage neighboring clusters
   - Providing connection details for direct communication
   - Handling communication when direct paths are unavailable
   - Managing security and authentication

2. Shard servers:
   - Establish direct connections with neighboring shards for high-frequency updates
   - Send boundary entity information directly to relevant neighbors
   - Use Bevy's event system for efficient communication
   - Fall back to replication server mediation when direct communication fails

This approach provides the best balance of performance and manageability while leveraging Bevy's event system for cleaner, more maintainable code.

## Conclusion

The Extended Dynamic Grid with Clustering approach provides Sidereal with a scalable and efficient world partitioning system that:

1. Supports the infinite universe requirements of the game
2. Efficiently distributes computational load across multiple shard servers
3. Provides seamless player experiences even when crossing between server boundaries
4. Integrates well with the existing Bevy ECS and networking architecture
5. Allows for future optimizations and enhancements

This system forms the foundation for all spatial interactions within the game, from basic movement to complex multi-entity physics simulations, and is designed to scale with the growth of the player base and universe complexity.

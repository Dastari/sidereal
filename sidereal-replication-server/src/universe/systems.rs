use crate::database::DatabaseClient;
use bevy::math::{IVec2, Vec2};
use bevy::prelude::*;
use sidereal_core::ecs::components::*;
use std::collections::{HashMap, HashSet};
use tracing::info;
use uuid::Uuid;

use super::plugin::ShardServerRegistry;

/// Network messages for cluster management (placeholder for actual network implementation)
#[derive(Event)]
#[allow(dead_code)]
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
}

#[derive(Event)]
#[allow(dead_code)]
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

/// Entity data for replication
#[derive(Clone)]
#[allow(dead_code)]
pub struct EntityData {
    pub id: Entity,
    pub position: Vec2,
    pub velocity: Vec2,
    pub components: HashMap<String, serde_json::Value>,
}

/// Initialize universe configuration and state
pub fn initialize_universe_state(mut commands: Commands, time: Res<Time>) {
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

            sectors.insert(
                sector_coords,
                Sector {
                    coordinates: sector_coords,
                    entities: HashSet::new(),
                    active: true,
                    last_updated: current_time,
                    last_entity_seen: current_time,
                    last_saved: 0.0, // Not yet saved
                },
            );
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

    // Add the cluster to the universe state - fixed resource access
    commands.insert_resource(UniverseState {
        active_clusters: [(base_coordinates, cluster)].into_iter().collect(),
        shard_assignments: HashMap::new(),
        entity_locations: HashMap::new(),
    });

    info!(
        "Created initial cluster at {:?} with ID {}",
        base_coordinates, cluster_id
    );
}

/// Update the universe state based on entity positions and other factors
pub fn update_global_universe_state(
    mut universe_state: ResMut<UniverseState>,
    _universe_config: Res<UniverseConfig>,
    query: Query<(Entity, &SpatialPosition)>,
    time: Res<Time>,
) {
    // Update entity counts in clusters
    for (_cluster_coords, cluster) in universe_state.active_clusters.iter_mut() {
        // Reset counts
        cluster.entity_count = 0;

        // Reset all sector entity sets
        for sector in cluster.sectors.values_mut() {
            sector.entities.clear();
        }
    }

    // Redistribute entities to their current sectors
    for (entity, position) in query.iter() {
        if let Some(cluster) = universe_state
            .active_clusters
            .get_mut(&position.cluster_coords)
        {
            // Update entity count for the cluster
            cluster.entity_count += 1;

            // Add entity to its sector
            if let Some(sector) = cluster.sectors.get_mut(&position.sector_coords) {
                sector.entities.insert(entity);
                sector.last_updated = time.elapsed_secs_f64();
                sector.last_entity_seen = time.elapsed_secs_f64();
            }

            // Update entity location mapping
            universe_state
                .entity_locations
                .insert(entity, position.cluster_coords);
        }
    }
}

/// Update entity sector coordinates based on position
pub fn update_entity_sector_coordinates(
    mut query: Query<(Entity, &Transform, &mut SpatialPosition)>,
    universe_config: Res<UniverseConfig>,
    mut boundary_events: EventWriter<EntityApproachingBoundary>,
) {
    for (entity, transform, mut spatial_pos) in query.iter_mut() {
        // Calculate sector coordinates from world position
        let position_2d = Vec2::new(transform.translation.x, transform.translation.y);

        let new_sector_x = (position_2d.x / universe_config.sector_size).floor() as i32;
        let new_sector_y = (position_2d.y / universe_config.sector_size).floor() as i32;
        let new_sector_coords = IVec2::new(new_sector_x, new_sector_y);

        // Calculate cluster coordinates
        let new_cluster_x =
            (new_sector_x as f32 / universe_config.cluster_dimensions.x as f32).floor() as i32;
        let new_cluster_y =
            (new_sector_y as f32 / universe_config.cluster_dimensions.y as f32).floor() as i32;
        let new_cluster_coords = IVec2::new(new_cluster_x, new_cluster_y);

        // Update position data
        spatial_pos.position = position_2d;

        // If sector has changed, update coordinates
        if new_sector_coords != spatial_pos.sector_coords {
            // Sector changed
            spatial_pos.sector_coords = new_sector_coords;

            // If cluster changed, update that too
            if new_cluster_coords != spatial_pos.cluster_coords {
                spatial_pos.cluster_coords = new_cluster_coords;
            }
        }

        // Check if entity is approaching a sector boundary
        if let Some(direction) = is_approaching_boundary(&spatial_pos, None, &universe_config) {
            // Calculate distance to boundary
            let sector_size = universe_config.sector_size;
            let pos_in_sector = Vec2::new(
                position_2d.x - (spatial_pos.sector_coords.x as f32 * sector_size),
                position_2d.y - (spatial_pos.sector_coords.y as f32 * sector_size),
            );

            let distance = match direction {
                BoundaryDirection::North => pos_in_sector.y,
                BoundaryDirection::East => sector_size - pos_in_sector.x,
                BoundaryDirection::South => sector_size - pos_in_sector.y,
                BoundaryDirection::West => pos_in_sector.x,
            };

            boundary_events.send(EntityApproachingBoundary {
                entity,
                direction,
                distance,
            });
        }
    }
}

/// System to handle cluster assignment to shard servers
pub fn handle_cluster_assignment(
    mut universe_state: ResMut<UniverseState>,
    shard_registry: Res<ShardServerRegistry>,
    // For demonstration, we'll use events instead of direct network
    mut assignment_events: EventWriter<ClusterManagementMessage>,
    time: Res<Time>,
) {
    // Only run this periodically, not every frame
    if time.elapsed().as_secs_f64() % 5.0 > 0.1 {
        return;
    }

    // Skip if no shards are available
    if shard_registry.active_shards.is_empty() {
        return;
    }

    // Fix borrow checker issues by collecting all the information we need upfront
    // This avoids multiple mutable borrows of universe_state
    struct ClusterInfo {
        coords: IVec2,
        id: Uuid,
        base_coordinates: IVec2,
        size: IVec2,
    }

    let unassigned_clusters: Vec<ClusterInfo> = universe_state
        .active_clusters
        .iter()
        .filter(|(_, cluster)| cluster.assigned_shard.is_none())
        .map(|(coords, cluster)| ClusterInfo {
            coords: *coords,
            id: cluster.id,
            base_coordinates: cluster.base_coordinates,
            size: cluster.size,
        })
        .collect();

    if unassigned_clusters.is_empty() {
        return;
    }

    info!("Found {} unassigned clusters", unassigned_clusters.len());

    // Process each unassigned cluster
    for (idx, cluster_info) in unassigned_clusters.iter().enumerate() {
        let shard_idx = idx % shard_registry.active_shards.len();
        let shard = &shard_registry.active_shards[shard_idx];

        // First, update the cluster assignment
        if let Some(cluster) = universe_state.active_clusters.get_mut(&cluster_info.coords) {
            cluster.assigned_shard = Some(shard.id);
        }

        // Then, update shard assignments
        universe_state
            .shard_assignments
            .entry(shard.id)
            .or_insert_with(Vec::new)
            .push(cluster_info.coords);

        // Finally, send assignment message
        assignment_events.send(ClusterManagementMessage::AssignCluster {
            cluster_id: cluster_info.id,
            base_coordinates: cluster_info.base_coordinates,
            size: cluster_info.size,
        });

        info!(
            "Assigned cluster {:?} (ID: {}) to shard {}",
            cluster_info.coords, cluster_info.id, shard.id
        );
    }
}

/// System to process entity transition requests
pub fn process_entity_transition_requests(
    _universe_state: ResMut<UniverseState>,
    _config: Res<UniverseConfig>,
    _commands: Commands,
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
        } = message
        {
            // Handle the transition logic
            info!(
                "Processing entity transition: {:?} from cluster {} to cluster {}",
                entity_id, source_cluster_id, destination_cluster_id
            );

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

/// System to send entity transition acknowledgments
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

/// System to monitor and manage empty sectors
pub fn manage_empty_sectors(
    time: Res<Time>,
    config: Res<UniverseConfig>,
    mut universe_state: ResMut<UniverseState>,
    mut last_check: Local<f64>,
    database: Option<Res<DatabaseClient>>,
    mut cluster_management_events: EventWriter<ClusterManagementMessage>,
) {
    let current_time = time.elapsed_secs_f64();

    // Only check periodically to reduce overhead
    if current_time - *last_check < config.empty_sector_check_interval {
        return;
    }

    *last_check = current_time;

    let mut clusters_to_release = Vec::new();
    let mut sectors_to_deactivate = Vec::new();

    // Check each cluster and its sectors
    for (cluster_coords, cluster) in &mut universe_state.active_clusters {
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
        if all_sectors_empty && all_sectors_timed_out && cluster.assigned_shard.is_some() {
            clusters_to_release.push((
                *cluster_coords,
                cluster.id,
                cluster.assigned_shard.unwrap(),
            ));
        }
    }

    // Deactivate sectors that have been empty for too long
    for (cluster_coords, sector_coords) in sectors_to_deactivate {
        if let Some(cluster) = universe_state.active_clusters.get_mut(&cluster_coords) {
            if let Some(sector) = cluster.sectors.get_mut(&sector_coords) {
                // Save sector state to database before deactivating
                info!(
                    "Deactivating empty sector at {:?} in cluster {:?}",
                    sector_coords, cluster_coords
                );
                sector.active = false;

                // Persist sector data to database if available
                if let Some(_db) = database.as_ref() {
                    // This would be an async database operation in a real implementation
                    // For now, just update the last_saved timestamp
                    sector.last_saved = current_time;
                }
            }
        }
    }

    // Process cluster releases when all sectors are inactive
    for (cluster_coords, cluster_id, shard_id) in clusters_to_release {
        // Persist inactive cluster data to database first
        info!(
            "Releasing empty cluster at {:?} (ID: {}) from shard {:?}",
            cluster_coords, cluster_id, shard_id
        );

        // Tell the shard server to release it
        cluster_management_events.send(ClusterManagementMessage::ReleaseCluster { cluster_id });

        // Update the assignment tracking
        if let Some(assigned_clusters) = universe_state.shard_assignments.get_mut(&shard_id) {
            assigned_clusters.retain(|coords| *coords != cluster_coords);
        }

        // Update the cluster's assigned_shard field
        if let Some(cluster) = universe_state.active_clusters.get_mut(&cluster_coords) {
            cluster.assigned_shard = None;
        }
    }
}

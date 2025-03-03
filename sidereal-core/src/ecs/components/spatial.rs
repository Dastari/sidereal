use bevy::math::{IVec2, Vec2};
use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy_rapier2d::prelude::Velocity;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Component, Serialize, Deserialize, Clone, Debug, Reflect, Default)]
pub struct Position(Vec2);

// Add these implementations to Position
impl Position {
    pub fn get(&self) -> Vec2 {
        self.0
    }
    
    pub fn set(&mut self, value: Vec2) {
        self.0 = value;
    }
    
    pub fn new(value: Vec2) -> Self {
        Position(value)
    }
}




#[derive(Component, Serialize, Deserialize, Clone, Debug, Reflect, Default)]
pub struct SectorCoords(IVec2);

// Add these implementations to SectorCoords
impl SectorCoords {
    pub fn get(&self) -> IVec2 {
        self.0
    }
    
    pub fn set(&mut self, value: IVec2) {
        self.0 = value;
    }

    pub fn new(value: IVec2) -> Self {
        SectorCoords(value)
    }
}
#[derive(Component, Serialize, Deserialize, Clone, Debug, Reflect, Default)]
pub struct ClusterCoords(IVec2);

// Add these implementations to ClusterCoords
impl ClusterCoords {
    pub fn get(&self) -> IVec2 {
        self.0
    }
    
    pub fn set(&mut self, value: IVec2) {
        self.0 = value;
    }

    pub fn new(value: IVec2) -> Self {
        ClusterCoords(value)
    }
}

/// Sector definition - contains entities in a spatial region
#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, Reflect)]
pub struct Sector {
    pub coordinates: IVec2,
    pub entities: HashSet<Entity>,
    pub active: bool,
    pub last_updated: f64,     // Time since startup
    pub last_entity_seen: f64, // Timestamp when the last entity was in this sector
    pub last_saved: f64,       // Timestamp of last persistence to database
}

/// Cluster definition - group of sectors managed by a single shard
#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, Reflect)]
pub struct Cluster {
    pub id: Uuid,
    pub base_coordinates: IVec2,
    pub size: IVec2, // How many sectors in each dimension
    pub sectors: HashMap<IVec2, Sector>,
    pub assigned_shard: Option<Uuid>,
    pub entity_count: usize,
    pub transition_zone_width: f32, // Width of buffer around edges
}

/// Resource for universe configuration
#[derive(Resource)]
pub struct UniverseConfig {
    pub sector_size: f32,
    pub cluster_dimensions: IVec2,
    pub transition_zone_width: f32,
    pub empty_sector_timeout_seconds: f64, // Time before an empty sector is considered inactive
    pub empty_sector_check_interval: f64,  // How often to check for empty sectors
    pub velocity_awareness_factor: f32, // Multiplier for velocity to determine transition zone size
    pub min_boundary_awareness: f32,    // Minimum distance to be considered near a boundary
}

impl Default for UniverseConfig {
    fn default() -> Self {
        Self {
            sector_size: 1000.0,
            cluster_dimensions: IVec2::new(3, 3), // 3x3 sectors per cluster
            transition_zone_width: 50.0,
            empty_sector_timeout_seconds: 300.0, // 5 minutes before unloading empty sectors
            empty_sector_check_interval: 60.0,   // Check once per minute
            velocity_awareness_factor: 2.0, // Multiply velocity by this factor for awareness zone
            min_boundary_awareness: 50.0,   // Minimum 50 units from boundary
        }
    }
}

/// Resource for tracking all active clusters in the universe
#[derive(Resource, Default)]
pub struct UniverseState {
    pub active_clusters: HashMap<IVec2, Cluster>,
    pub shard_assignments: HashMap<Uuid, Vec<IVec2>>,
    pub entity_locations: HashMap<Entity, IVec2>, // Maps entities to their cluster
}

/// Event for entity approaching boundary
#[derive(Event, Debug)]
pub struct EntityApproachingBoundary {
    pub entity: Entity,
    pub direction: BoundaryDirection,
    pub distance: f32,
}

/// Direction of sector boundary
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Reflect)]
pub enum BoundaryDirection {
    North,
    East,
    South,
    West,
}

impl Default for BoundaryDirection {
    fn default() -> Self {
        BoundaryDirection::North
    }
}


//// Marker indicating entity is visual-only (no physics)
#[derive(Component, Reflect, Default)]
pub struct VisualOnly;

/// Shadow entity representation for entities from neighboring shards
#[derive(Component, Reflect)]
#[require(Position, SectorCoords, ClusterCoords, Velocity, VisualOnly)]
pub struct ShadowEntity {
    pub source_cluster_id: Uuid,
    pub source_shard_id: Uuid,
    pub original_entity: Entity,
    pub is_read_only: bool,
    pub last_updated: f64,
}

// Only used for reflection registration
impl ShadowEntity {
    pub fn register_reflection(app: &mut App) {
        app.register_type::<ShadowEntity>()
           .register_type::<VisualOnly>();
    }
}

impl Default for ShadowEntity {
    fn default() -> Self {
        Self {
            source_cluster_id: Uuid::nil(),
            source_shard_id: Uuid::nil(),
            original_entity: Entity::from_raw(0),
            is_read_only: true,
            last_updated: 0.0,
        }
    }
}

/// Helper to calculate which cluster an entity belongs to
pub fn calculate_entity_cluster(position: Vec2, config: &UniverseConfig) -> IVec2 {
    let sector_x = (position.x / config.sector_size).floor() as i32;
    let sector_y = (position.y / config.sector_size).floor() as i32;

    let cluster_x = (sector_x as f32 / config.cluster_dimensions.x as f32).floor() as i32;
    let cluster_y = (sector_y as f32 / config.cluster_dimensions.y as f32).floor() as i32;

    IVec2::new(cluster_x, cluster_y)
}

/// Helper to check if entity is near boundary
pub fn is_approaching_boundary(
    position: &Position,
    sector_coords: &SectorCoords,
    velocity: Option<&Velocity>,
    config: &UniverseConfig,
) -> Option<BoundaryDirection> {
    // Calculate position within current sector
    let sector_size = config.sector_size;
    let pos_in_sector = Vec2::new(
        position.0.x - (sector_coords.0.x as f32 * sector_size),
        position.0.y - (sector_coords.0.y as f32 * sector_size),
    );

    // Calculate distances to each boundary
    let dist_to_left = pos_in_sector.x;
    let dist_to_right = sector_size - pos_in_sector.x;
    let dist_to_top = pos_in_sector.y;
    let dist_to_bottom = sector_size - pos_in_sector.y;

    // Determine boundary awareness threshold based on velocity and minimum distance
    let threshold = if let Some(vel) = velocity {
        // Fast-moving entities need a larger awareness zone
        (vel.linvel.length() * config.velocity_awareness_factor).max(config.min_boundary_awareness)
    } else {
        // Static or non-physics entities use the minimum threshold
        config.min_boundary_awareness
    };

    // Check which boundary (if any) is closest and within threshold
    let min_dist = dist_to_left
        .min(dist_to_right)
        .min(dist_to_top)
        .min(dist_to_bottom);

    if min_dist >= threshold {
        return None;
    }

    if min_dist == dist_to_left {
        Some(BoundaryDirection::West)
    } else if min_dist == dist_to_right {
        Some(BoundaryDirection::East)
    } else if min_dist == dist_to_top {
        Some(BoundaryDirection::North)
    } else {
        Some(BoundaryDirection::South)
    }
}

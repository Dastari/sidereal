use crate::ecs::components::spatial::{
    calculate_entity_cluster, ClusterCoords, Position, SectorCoords, UniverseConfig,
};
use crate::ecs::plugins::serialization::EntitySerializationExt;
use crate::ecs::systems::physics::n_body_gravity_system;
use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use std::collections::HashMap;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Register types
        app.register_serializable_component::<Transform>()
            .register_serializable_component::<GlobalTransform>();

        // Add Rapier physics
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());

        // Add gravitational system
        app.add_systems(FixedUpdate, n_body_gravity_system);

        // Add UniverseConfig
        app.insert_resource(UniverseConfig::default());

        app.add_systems(
            FixedUpdate,
            sync_transform_to_spatial_position.after(PhysicsSet::Writeback),
        );

        app.add_systems(
            FixedUpdate,
            sync_spatial_position_to_transform.before(PhysicsSet::StepSimulation),
        );

        // Store the previous state of positions to detect manual position changes
        app.init_resource::<PositionChangeTracker>();
        app.add_systems(PostUpdate, track_position_changes);
    }
}

// Add this system to synchronize Transform to spatial components
fn sync_transform_to_spatial_position(
    mut query: Query<(
        &Transform,
        &mut Position,
        &mut SectorCoords,
        &mut ClusterCoords,
    )>,
    universe_config: Res<UniverseConfig>,
) {
    for (transform, mut position, mut sector_coords, mut cluster_coords) in query.iter_mut() {
        // Update the position from Transform
        position.set(transform.translation.truncate());

        // Calculate sector coordinates from position
        let pos = position.get();
        let sector_x = (pos.x / universe_config.sector_size).floor() as i32;
        let sector_y = (pos.y / universe_config.sector_size).floor() as i32;
        sector_coords.set(IVec2::new(sector_x, sector_y));

        // Recalculate cluster coordinates based on the new position
        let new_cluster_coords = calculate_entity_cluster(pos, &universe_config);
        cluster_coords.set(new_cluster_coords);
    }
}

// Add this system to synchronize spatial position to Transform
fn sync_spatial_position_to_transform(
    tracker: Res<PositionChangeTracker>,
    mut query: Query<(Entity, &Position, &mut Transform)>,
) {
    // Only process entities that were manually changed
    for entity in tracker.manually_changed.iter() {
        if let Ok((_, position, mut transform)) = query.get_mut(*entity) {
            // Update the transform's translation from Position
            // Keep the z-coordinate unchanged
            let current_z = transform.translation.z;
            let pos = position.get();
            transform.translation = Vec3::new(pos.x, pos.y, current_z);
        }
    }
}

// Resource to track position changes
#[derive(Resource, Default)]
struct PositionChangeTracker {
    manually_changed: Vec<Entity>,
    previous_positions: HashMap<Entity, Vec2>,
}

// System to track which positions were changed manually vs by transform sync
fn track_position_changes(
    mut tracker: ResMut<PositionChangeTracker>,
    position_query: Query<(Entity, &Position), Changed<Position>>,
    transform_query: Query<Entity, Changed<Transform>>,
) {
    // Clear previous frame's changes
    tracker.manually_changed.clear();

    // Check each entity with a changed position
    for (entity, position) in position_query.iter() {
        let pos = position.get();

        // If the entity's transform didn't change OR
        // if the new position doesn't match what we'd expect from transform changes,
        // consider it a manual change
        if !transform_query.contains(entity) {
            if let Some(prev_pos) = tracker.previous_positions.get(&entity) {
                if *prev_pos != pos {
                    tracker.manually_changed.push(entity);
                }
            } else {
                // First time seeing this entity, assume it's a manual change
                tracker.manually_changed.push(entity);
            }
        }

        // Update the stored position
        tracker.previous_positions.insert(entity, pos);
    }
}

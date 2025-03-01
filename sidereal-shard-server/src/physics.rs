use bevy::prelude::*;
use bevy_rapier2d::prelude::{Velocity, RapierPhysicsPlugin, NoUserData};
use tracing::info;
use std::time::Duration;

use crate::config::{PhysicsConfig, ShardState};
use sidereal_core::ecs::components::spatial::{EntityApproachingBoundary, SpatialPosition};
// Import the core physics systems (but not the plugin that includes debug rendering)
use sidereal_core::ecs::systems::physics::n_body_gravity_system;

/// Plugin for shard-specific physics configuration and systems
pub struct ShardPhysicsPlugin;

impl Plugin for ShardPhysicsPlugin {
    fn build(&self, app: &mut App) {
        info!("Building shard physics plugin");
        
        // Add Rapier physics plugin directly (without debug rendering)
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
           // Add n-body gravity system from core
           .add_systems(FixedUpdate, n_body_gravity_system)
           // Add our shard-specific physics configurations
           .insert_resource(Time::<Fixed>::from_seconds(1.0 / 30.0))
           .add_systems(OnEnter(ShardState::Ready), configure_physics_timestep)
           // Add shard-specific physics systems
           .add_systems(
               FixedUpdate, 
               (
                   process_physics,
                   check_entity_boundaries,
               ).chain().run_if(in_state(ShardState::Ready))
           )
           // Register the boundary event
           .add_event::<EntityApproachingBoundary>();
    }
}

/// Configure the physics timestep based on the PhysicsConfig
fn configure_physics_timestep(
    physics_config: Res<PhysicsConfig>,
    mut time: ResMut<Time<Fixed>>,
) {
    let period = 1.0 / physics_config.physics_fps;
    info!("Setting physics timestep to {} seconds ({} FPS)", period, physics_config.physics_fps);
    
    time.set_timestep(Duration::from_secs_f32(period));
}

/// Process physics simulation
fn process_physics(
    time: Res<Time>,
    entities: Query<Entity>,
) {
    // Log every few seconds to avoid flooding logs
    if time.elapsed_secs() % 10.0 < time.delta_secs() {
        info!("Physics processing active - {} entities in simulation", entities.iter().count());
    }
}

/// Check for entities approaching sector boundaries
fn check_entity_boundaries(
    _query: Query<(Entity, &SpatialPosition, &Velocity)>,
    _entity_events: EventWriter<EntityApproachingBoundary>,
) {
    // This system will be implemented to use the is_approaching_boundary functionality from sidereal-core
    // For now this is a placeholder
} 
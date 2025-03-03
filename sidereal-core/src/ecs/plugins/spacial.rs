use crate::ecs::components::spatial::*;
use bevy::prelude::*;

pub struct SpatialPlugin;

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut App) {
        // Register all types for reflection
        app.register_type::<Position>()
            .register_type::<SectorCoords>()
            .register_type::<ClusterCoords>()
            .register_type::<BoundaryDirection>()
            .register_type::<VisualOnly>()
            .register_type::<ShadowEntity>();

        // Register events
        app.add_event::<EntityApproachingBoundary>();

        // Add spatial systems
        // app.add_systems(Update, ());
    }
}

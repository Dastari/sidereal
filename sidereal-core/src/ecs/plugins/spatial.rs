use crate::ecs::components::spatial::*;
use crate::ecs::plugins::serialization::EntitySerializationExt;
use bevy::prelude::*;

pub struct SpatialPlugin;

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut App) {
        // Register all types for reflection
        app.register_serializable_component::<Position>()
            .register_serializable_component::<SectorCoords>()
            .register_serializable_component::<ClusterCoords>()
            .register_serializable_component::<VisualOnly>()
            .register_serializable_component::<ShadowEntity>();

        // Register events
        app.add_event::<EntityApproachingBoundary>();

        // Add spatial systems
        // app.add_systems(Update, ());
    }
}

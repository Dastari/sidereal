use crate::ecs::components::spatial::*;
use bevy::prelude::*;

pub fn setup_spatial_system(app: &mut App) {
    // Register types for reflection (needed for networking)
    app.register_type::<Position>()
        .register_type::<SectorCoords>()
        .register_type::<ClusterCoords>()   
       .register_type::<ShadowEntity>()
       .register_type::<VisualOnly>()
       .register_type::<BoundaryDirection>();
    
    // Add events
    app.add_event::<EntityApproachingBoundary>();
}
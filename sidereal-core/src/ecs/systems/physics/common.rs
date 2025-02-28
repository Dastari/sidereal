use bevy::prelude::*;
use crate::ecs::components::*;
use crate::ecs::components::physics::{Velocity, AngularVelocity, Mass, Fixed};

/// Resource to control gravity strength
#[derive(Resource, Clone, Reflect)]
pub struct GravityMultiplier {
    pub value: f32,
}

impl Default for GravityMultiplier {
    fn default() -> Self {
        Self { value: 0.0 }
    }
}

/// Register the components for reflection
pub fn register_reflection(world: &mut World) {
    let registry = world.resource_mut::<AppTypeRegistry>();
    let mut registry = registry.write();
    
    registry.register::<Position>();
    registry.register::<Velocity>();
    registry.register::<Mass>();
    registry.register::<Rotation>();
    registry.register::<AngularVelocity>();
    registry.register::<Fixed>();
    registry.register::<GravityMultiplier>();
} 
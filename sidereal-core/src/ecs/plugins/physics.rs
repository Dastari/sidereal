use crate::ecs::plugins::serialization::EntitySerializationExt;
use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Register types
        app.register_serializable_component::<Transform>()
            .register_serializable_component::<GlobalTransform>()
            .register_serializable_component::<Sleeping>()
            .register_serializable_component::<Velocity>()
            .register_serializable_component::<Damping>();
    }
}

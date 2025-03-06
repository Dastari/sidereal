use crate::ecs::components::Hull;
use crate::ecs::plugins::serialization::EntitySerializationExt;
use bevy::prelude::*;

pub struct ShipPlugin;

impl Plugin for ShipPlugin {
    fn build(&self, app: &mut App) {
        app.register_serializable_component::<Hull>();
    }
}

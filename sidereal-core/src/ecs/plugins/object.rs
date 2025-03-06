use crate::ecs::components::{Id, Object};
use crate::ecs::plugins::serialization::EntitySerializationExt;
use bevy::prelude::*;

pub struct ObjectPlugin;

impl Plugin for ObjectPlugin {
    fn build(&self, app: &mut App) {
        app.register_serializable_component::<Id>()
            .register_serializable_component::<Object>();
    }
}

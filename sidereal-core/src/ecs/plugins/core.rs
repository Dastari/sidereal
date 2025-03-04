use crate::ecs::components::hull::{Block, Hull};
use crate::ecs::components::name::Name;
use crate::ecs::plugins::serialization::EntitySerializationExt;
use bevy::prelude::*;

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.register_serializable_component::<Hull>()
            .register_serializable_component::<Block>()
            .register_serializable_component::<Name>();
    }
}

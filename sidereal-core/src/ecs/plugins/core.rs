use crate::ecs::components::hull::{Block, Direction, Hull};
use crate::ecs::components::name::Name;
use bevy::prelude::*;

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Hull>()
            .register_type::<Block>()
            .register_type::<Direction>()
            .register_type::<Name>();
    }
}

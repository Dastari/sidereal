use bevy::prelude::*;
use crate::ecs::components::*;

pub fn spawn_entity(commands: &mut Commands) {
    commands.spawn(())
        .insert(Position { x: 0.0, y: 0.0 })
        .insert(Velocity { x: 1.0, y: 1.0 });
}
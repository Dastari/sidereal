use bevy::prelude::*;
use crate::ecs::systems::physics::physics_system;

pub struct MyPlugin;
impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (physics_system,));
    }
}
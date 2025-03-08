use crate::ecs::plugins::{
    entities::EntitiesPlugin, object::ObjectPlugin
};
use bevy::prelude::*;

pub struct SiderealGamePlugin;

impl Plugin for SiderealGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((ObjectPlugin, EntitiesPlugin));
    }
}

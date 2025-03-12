use bevy::prelude::*;
use crate::ecs::systems::sectors::*;

// Plugin to register the sector system
pub struct SectorPlugin;

impl Plugin for SectorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SectorManager>()
           .add_systems(Update, update_entity_sectors);
    }
}

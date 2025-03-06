mod ship;

use bevy::prelude::*;
use ship::*;

pub struct EntitiesPlugin;

impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ShipPlugin);
    }
}

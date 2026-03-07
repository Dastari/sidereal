use bevy::prelude::*;
use sidereal_game::SiderealGameCorePlugin;

pub(crate) fn configure_shared_client_core(app: &mut App) {
    app.add_plugins(SiderealGameCorePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
}

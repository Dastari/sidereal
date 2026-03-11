use bevy::prelude::*;
use sidereal_core::SIM_TICK_HZ;
use sidereal_game::SiderealGameCorePlugin;

pub(crate) fn configure_shared_client_core(app: &mut App) {
    app.add_plugins(SiderealGameCorePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(f64::from(SIM_TICK_HZ)));
}

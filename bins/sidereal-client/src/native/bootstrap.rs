//! Bootstrap watchdog: arm on enter in-world, optional failure watch.

use bevy::log::info;
use bevy::prelude::*;

use super::resources::BootstrapWatchdogState;

pub fn reset_bootstrap_watchdog_on_enter_in_world(
    time: Res<'_, Time>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
) {
    info!("client entered in-world state; bootstrap watchdog armed");
    *watchdog = BootstrapWatchdogState {
        in_world_entered_at_s: Some(time.elapsed_secs_f64()),
        last_bootstrap_progress_at_s: time.elapsed_secs_f64(),
        ..Default::default()
    };
}

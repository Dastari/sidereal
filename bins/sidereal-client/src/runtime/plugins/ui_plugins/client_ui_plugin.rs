use bevy::prelude::*;

use crate::runtime::dev_console;

use super::{in_world, logout, menu_loading, post_update};

pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        dev_console::register_console(app);
        menu_loading::add_audio_state_systems(app);
        menu_loading::add_menu_and_loading_ui_systems(app);
        in_world::add_in_world_ui_update_systems(app);
        post_update::add_in_world_post_update_systems(app);
        post_update::add_in_world_last_stage_systems(app);
        logout::add_logout_systems(app);
    }
}

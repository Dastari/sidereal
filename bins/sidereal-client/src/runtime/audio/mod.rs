mod backend;
mod catalog;
#[cfg(not(target_arch = "wasm32"))]
mod native_backend;
mod null_backend;
mod settings;
mod state;
mod systems;

use bevy::prelude::*;

pub(crate) use catalog::AudioCatalogState;
pub(crate) use settings::AudioSettings;
pub(crate) use systems::{
    ensure_menu_music_system, ensure_world_music_system, queue_audio_asset_demands_system,
    receive_local_destruction_audio_system, receive_local_weapon_fire_audio_system,
    receive_remote_destruction_audio_system, receive_remote_weapon_fire_audio_system,
    sync_audio_catalog_defaults_system, sync_audio_listener_system, sync_audio_runtime_system,
};

pub(crate) fn init_audio_runtime(app: &mut App) {
    app.insert_resource(AudioCatalogState::default());
    app.insert_resource(AudioSettings::default());
    app.insert_resource(state::AudioAssetDemandState::default());
    app.insert_non_send_resource(backend::AudioBackendResource::default());
}

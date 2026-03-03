use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

#[derive(Resource, Clone)]
pub(super) struct MenuLoopAudioHandle {
    pub handle: Handle<AudioSource>,
}

#[derive(Resource, Default)]
pub(super) struct MenuLoopPlaybackState {
    pub player_entity: Option<Entity>,
}

pub(super) fn insert_embedded_menu_loop_audio(app: &mut App) {
    static MENU_LOOP_MP3: &[u8] = include_bytes!("../../../../data/music/menu-loop.mp3");
    let handle = {
        let mut audio_sources = app.world_mut().resource_mut::<Assets<AudioSource>>();
        audio_sources.add(AudioSource {
            bytes: MENU_LOOP_MP3.to_vec().into(),
        })
    };
    app.insert_resource(MenuLoopAudioHandle { handle });
    app.insert_resource(MenuLoopPlaybackState::default());
}

pub(super) fn start_menu_loop_music_system(
    mut commands: Commands<'_, '_>,
    mut playback_state: ResMut<'_, MenuLoopPlaybackState>,
    menu_loop_audio: Res<'_, MenuLoopAudioHandle>,
) {
    if playback_state.player_entity.is_some() {
        return;
    }
    let player_entity = commands
        .spawn((
            AudioPlayer::new(menu_loop_audio.handle.clone()),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.45)),
            Name::new("MenuLoopMusic"),
        ))
        .id();
    playback_state.player_entity = Some(player_entity);
}

pub(super) fn stop_menu_loop_music_system(
    mut commands: Commands<'_, '_>,
    mut playback_state: ResMut<'_, MenuLoopPlaybackState>,
) {
    if let Some(player_entity) = playback_state.player_entity.take()
        && let Ok(mut entity_commands) = commands.get_entity(player_entity)
    {
        entity_commands.try_despawn();
    }
}

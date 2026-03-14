use crate::{AudioClipPlaybackDefaults, AudioPlaybackDefinition, AudioRegistry};
use std::collections::HashMap;

pub fn apply_clip_defaults(registry: &mut AudioRegistry) {
    let clip_defaults_by_asset_id = registry
        .clips
        .iter()
        .map(|clip| (clip.clip_asset_id.as_str(), &clip.defaults))
        .collect::<HashMap<_, _>>();

    for profile in &mut registry.profiles {
        for cue in profile.cues.values_mut() {
            let Some(clip_asset_id) = cue.playback.clip_asset_id.as_deref() else {
                continue;
            };
            let Some(defaults) = clip_defaults_by_asset_id.get(clip_asset_id) else {
                continue;
            };
            apply_playback_defaults(&mut cue.playback, defaults);
        }
    }
}

fn apply_playback_defaults(
    playback: &mut AudioPlaybackDefinition,
    defaults: &AudioClipPlaybackDefaults,
) {
    if playback.intro_start_s.is_none() {
        playback.intro_start_s = defaults.intro_start_s;
    }
    if playback.loop_start_s.is_none() {
        playback.loop_start_s = defaults.loop_start_s;
    }
    if playback.loop_end_s.is_none() {
        playback.loop_end_s = defaults.loop_end_s;
    }
    if playback.outro_start_s.is_none() {
        playback.outro_start_s = defaults.outro_start_s;
    }
    if playback.clip_end_s.is_none() {
        playback.clip_end_s = defaults.clip_end_s;
    }
    if playback.loop_region.is_none() {
        playback.loop_region = defaults.loop_region.clone();
    }
}

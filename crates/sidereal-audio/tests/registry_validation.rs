use sidereal_audio::{
    AudioBusDefinition, AudioClipDefinition, AudioClipPlaybackDefaults, AudioCueDefinition,
    AudioPlaybackDefinition, AudioProfileDefinition, AudioRegistry, AudioRouteDefinition,
    AudioSpatialDefinition, apply_clip_defaults, validate_audio_registry,
};
use std::collections::BTreeMap;

fn base_registry() -> AudioRegistry {
    let mut cues = BTreeMap::new();
    cues.insert(
        "click".to_string(),
        AudioCueDefinition {
            playback: AudioPlaybackDefinition {
                kind: "one_shot".to_string(),
                clip_asset_id: Some("audio.ui.click_01".to_string()),
                variants: Vec::new(),
                intro_start_s: None,
                loop_start_s: None,
                loop_end_s: None,
                outro_start_s: None,
                clip_end_s: None,
                loop_region: None,
            },
            route: AudioRouteDefinition {
                bus: "ui".to_string(),
                sends: Vec::new(),
            },
            spatial: AudioSpatialDefinition {
                mode: "screen_nonpositional".to_string(),
                min_distance_m: None,
                max_distance_m: None,
                rolloff: None,
                pan_strength: None,
                distance_lowpass: None,
            },
            concurrency: None,
            ducking: None,
        },
    );
    AudioRegistry {
        schema_version: 1,
        buses: vec![AudioBusDefinition {
            bus_id: "ui".to_string(),
            parent: Some("master".to_string()),
            default_volume_db: Some(-3.0),
            muted: None,
        }],
        sends: Vec::new(),
        environments: Vec::new(),
        concurrency_groups: Vec::new(),
        clips: Vec::new(),
        profiles: vec![AudioProfileDefinition {
            profile_id: "ui.menu.standard".to_string(),
            kind: "ui".to_string(),
            cues,
        }],
    }
}

#[test]
fn validates_minimal_registry() {
    validate_audio_registry(&base_registry()).expect("registry should validate");
}

#[test]
fn rejects_unknown_bus_reference() {
    let mut registry = base_registry();
    registry.profiles[0]
        .cues
        .get_mut("click")
        .unwrap()
        .route
        .bus = "missing".to_string();
    let err = validate_audio_registry(&registry).expect_err("expected validation failure");
    assert!(
        err.to_string().contains("unknown bus=missing"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_invalid_segmented_loop_markers() {
    let mut registry = base_registry();
    let cue = registry.profiles[0].cues.get_mut("click").unwrap();
    cue.playback.kind = "segmented_loop".to_string();
    cue.playback.intro_start_s = Some(0.0);
    cue.playback.loop_start_s = Some(2.0);
    cue.playback.loop_end_s = Some(1.0);
    cue.playback.outro_start_s = Some(3.0);
    cue.playback.clip_end_s = Some(4.0);
    let err = validate_audio_registry(&registry).expect_err("expected validation failure");
    assert!(
        err.to_string().contains("segmented_loop markers"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_duplicate_clip_defaults_entries() {
    let mut registry = base_registry();
    registry.clips = vec![
        AudioClipDefinition {
            clip_asset_id: "audio.ui.click_01".to_string(),
            defaults: AudioClipPlaybackDefaults::default(),
        },
        AudioClipDefinition {
            clip_asset_id: "audio.ui.click_01".to_string(),
            defaults: AudioClipPlaybackDefaults::default(),
        },
    ];
    let err = validate_audio_registry(&registry).expect_err("expected validation failure");
    assert!(
        err.to_string().contains("duplicates clip_asset_id"),
        "unexpected error: {err}"
    );
}

#[test]
fn clip_defaults_fill_missing_playback_markers_without_overwriting_cue_overrides() {
    let mut registry = base_registry();
    registry.clips = vec![AudioClipDefinition {
        clip_asset_id: "audio.ui.click_01".to_string(),
        defaults: AudioClipPlaybackDefaults {
            intro_start_s: Some(0.0),
            loop_start_s: Some(0.5),
            loop_end_s: Some(0.8),
            outro_start_s: Some(0.9),
            clip_end_s: Some(1.2),
            loop_region: None,
        },
    }];
    let cue = registry.profiles[0].cues.get_mut("click").unwrap();
    cue.playback.kind = "segmented_loop".to_string();
    cue.playback.loop_end_s = Some(0.75);

    apply_clip_defaults(&mut registry);

    let playback = &registry.profiles[0].cues.get("click").unwrap().playback;
    assert_eq!(playback.intro_start_s, Some(0.0));
    assert_eq!(playback.loop_start_s, Some(0.5));
    assert_eq!(playback.loop_end_s, Some(0.75));
    assert_eq!(playback.outro_start_s, Some(0.9));
    assert_eq!(playback.clip_end_s, Some(1.2));
}

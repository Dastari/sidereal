use sidereal_scripting::{
    load_audio_registry_from_root, load_audio_registry_from_source, resolve_scripts_root,
};
use std::path::{Path, PathBuf};

fn shared_scripts_root() -> PathBuf {
    resolve_scripts_root(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn loads_shared_audio_registry_from_workspace_scripts() {
    let registry = load_audio_registry_from_root(&shared_scripts_root()).expect("audio registry");
    assert!(registry.buses.iter().any(|bus| bus.bus_id == "sfx"));
    assert!(
        registry
            .profiles
            .iter()
            .any(|profile| profile.profile_id == "weapon.ballistic_gatling")
    );
}

#[test]
fn rejects_audio_registry_with_unknown_bus_reference() {
    let err = load_audio_registry_from_source(
        r#"
return {
  schema_version = 1,
  buses = {},
  sends = {},
  environments = {},
  concurrency_groups = {},
  profiles = {
    {
      profile_id = "ui.menu.standard",
      kind = "ui",
      cues = {
        click = {
          playback = {
            kind = "one_shot",
            clip_asset_id = "audio.ui.click_01",
          },
          route = {
            bus = "missing",
          },
          spatial = {
            mode = "screen_nonpositional",
          },
        },
      },
    },
  },
}
"#,
        Path::new("audio/registry.lua"),
    )
    .expect_err("expected validation error");
    assert!(
        err.to_string().contains("unknown bus=missing"),
        "unexpected error: {err}"
    );
}

#[test]
fn applies_clip_defaults_when_loading_audio_registry() {
    let registry = load_audio_registry_from_source(
        r#"
return {
  schema_version = 1,
  buses = {
    {
      bus_id = "sfx",
      parent = "master",
    },
  },
  sends = {},
  environments = {},
  concurrency_groups = {},
  clips = {
    {
      clip_asset_id = "audio.sfx.weapon.ballistic_fire",
      defaults = {
        intro_start_s = 0.0,
        loop_start_s = 0.25,
        loop_end_s = 0.5,
        outro_start_s = 0.75,
        clip_end_s = 1.0,
      },
    },
  },
  profiles = {
    {
      profile_id = "weapon.ballistic_gatling",
      kind = "weapon",
      cues = {
        fire = {
          playback = {
            kind = "segmented_loop",
            clip_asset_id = "audio.sfx.weapon.ballistic_fire",
            loop_end_s = 0.55,
          },
          route = {
            bus = "sfx",
          },
          spatial = {
            mode = "world_2d",
          },
        },
      },
    },
  },
}
"#,
        Path::new("audio/registry.lua"),
    )
    .expect("audio registry");

    let playback = &registry
        .profiles
        .iter()
        .find(|profile| profile.profile_id == "weapon.ballistic_gatling")
        .expect("profile")
        .cues["fire"]
        .playback;
    assert_eq!(playback.intro_start_s, Some(0.0));
    assert_eq!(playback.loop_start_s, Some(0.25));
    assert_eq!(playback.loop_end_s, Some(0.55));
    assert_eq!(playback.outro_start_s, Some(0.75));
    assert_eq!(playback.clip_end_s, Some(1.0));
}

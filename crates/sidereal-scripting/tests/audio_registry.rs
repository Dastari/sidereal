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

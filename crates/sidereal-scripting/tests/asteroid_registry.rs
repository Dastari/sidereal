use sidereal_scripting::{
    LuaSandboxPolicy, load_asteroid_registry_from_root, load_asteroid_registry_from_source,
    resolve_scripts_root,
};
use std::path::{Path, PathBuf};

fn shared_scripts_root() -> PathBuf {
    resolve_scripts_root(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn loads_shared_asteroid_registry_from_workspace_scripts() {
    let registry =
        load_asteroid_registry_from_root(&shared_scripts_root()).expect("asteroid registry");
    assert_eq!(registry.schema_version, 1);
    assert!(
        registry
            .field_profiles
            .iter()
            .any(|profile| profile.field_profile_id == "asteroid.field.starter_belt")
    );
    assert!(
        registry
            .resource_profiles
            .iter()
            .any(|profile| profile.resource_profile_id == "asteroid.resource.common_ore")
    );
}

#[test]
fn rejects_duplicate_asteroid_resource_profile_ids() {
    let source = r#"
return {
  schema_version = 1,
  resource_profiles = {
    { resource_profile_id = "asteroid.resource.duplicate", depletion_pool_units = 1.0, yield_table = {
      { item_id = "resource.iron_ore", weight = 1.0, min_units = 1.0, max_units = 2.0 },
    }},
    { resource_profile_id = "asteroid.resource.duplicate", depletion_pool_units = 1.0, yield_table = {
      { item_id = "resource.nickel_ore", weight = 1.0, min_units = 1.0, max_units = 2.0 },
    }},
  },
}
"#;
    let err = load_asteroid_registry_from_source(
        source,
        Path::new("asteroids/registry.lua"),
        &LuaSandboxPolicy::default(),
    )
    .expect_err("duplicate resource profile should fail");
    assert!(
        err.to_string()
            .contains("duplicate resource_profile_id=asteroid.resource.duplicate"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_unknown_field_profile_resource_reference() {
    let source = r#"
return {
  schema_version = 1,
  field_profiles = {
    {
      field_profile_id = "asteroid.field.bad",
      display_name = "Bad Field",
      shape = "ClusterPatch",
      radius_m = 100.0,
      density = 0.5,
      layout_seed = 1,
      sprite_profile_id = "asteroid.sprite.ok",
      fracture_profile_id = "asteroid.fracture.ok",
      resource_profile_id = "asteroid.resource.missing",
    },
  },
  sprite_profiles = {
    {
      sprite_profile_id = "asteroid.sprite.ok",
      generator_id = "asteroid_rocky_v1",
      surface_styles = { "Rocky" },
      pixel_step_px = 2,
      crack_intensity_range = { 0.0, 1.0 },
      mineral_vein_intensity_range = { 0.0, 1.0 },
    },
  },
  fracture_profiles = {
    {
      fracture_profile_id = "asteroid.fracture.ok",
      break_massive_into_large = { 2, 3 },
      break_large_into_medium = { 2, 4 },
      break_medium_into_small = { 2, 5 },
      child_impulse_mps = { 0.1, 1.0 },
      mass_retention_ratio = 0.8,
      terminal_debris_loss_ratio = 0.6,
    },
  },
}
"#;
    let err = load_asteroid_registry_from_source(
        source,
        Path::new("asteroids/registry.lua"),
        &LuaSandboxPolicy::default(),
    )
    .expect_err("unknown resource profile should fail");
    assert!(
        err.to_string().contains(
            "field_profile_id=asteroid.field.bad references unknown resource_profile_id=asteroid.resource.missing"
        ),
        "unexpected error: {err}"
    );
}

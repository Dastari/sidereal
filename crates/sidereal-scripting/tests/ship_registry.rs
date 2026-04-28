use sidereal_scripting::{
    ScriptAssetRegistry, ScriptAssetRegistryEntry, load_ship_module_registry_from_root,
    load_ship_module_registry_from_sources, load_ship_registry_from_root,
    load_ship_registry_from_sources, resolve_scripts_root,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn shared_scripts_root() -> PathBuf {
    resolve_scripts_root(env!("CARGO_MANIFEST_DIR"))
}

fn module_registry_source() -> &'static str {
    r#"
return {
  schema_version = 1,
  modules = {
    { module_id = "module.engine.test", script = "ship_modules/engine_test.lua" },
  },
}
"#
}

fn engine_module_source() -> &'static str {
    r#"
return {
  module_id = "module.engine.test",
  display_name = "Test Engine",
  category = "engine",
  entity_labels = { "Module", "Engine" },
  compatible_slot_kinds = { "engine" },
  tags = { "engine" },
  components = {
    { kind = "mass_kg", properties = 25.0 },
    { kind = "engine", properties = {
      thrust = 1200.0,
      reverse_thrust = 800.0,
      torque_thrust = 600.0,
      burn_rate_kg_s = 0.1,
    } },
  },
}
"#
}

fn valid_ship_registry_source() -> &'static str {
    r#"
return {
  schema_version = 1,
  ships = {
    {
      ship_id = "ship.test",
      bundle_id = "ship.test",
      script = "ships/test.lua",
      spawn_enabled = true,
      tags = { "test" },
    },
  },
}
"#
}

fn valid_ship_source() -> &'static str {
    r#"
return {
  ship_id = "ship.test",
  bundle_id = "ship.test",
  display_name = "Test Ship",
  entity_labels = { "Ship" },
  tags = { "test" },
  visual = {
    visual_asset_id = "test_ship",
    map_icon_asset_id = "map_icon_ship_svg",
  },
  dimensions = {
    length_m = 12.0,
    width_m = 6.0,
    height_m = 2.5,
    collision_mode = "Aabb",
    collision_from_texture = false,
  },
  root = {
    base_mass_kg = 1000.0,
    max_velocity_mps = 80.0,
    health_pool = { current = 100.0, maximum = 100.0 },
    destructible = { destruction_profile_id = "test", destroy_delay_s = 0.1 },
    flight_computer = { profile = "test", throttle = 0.0, yaw_input = 0.0, brake_active = false, turn_rate_deg_s = 90.0 },
    flight_tuning = { max_linear_accel_mps2 = 10.0, passive_brake_accel_mps2 = 1.0, active_brake_accel_mps2 = 2.0, drag_per_s = 0.1 },
    visibility_range_buff_m = { additive_m = 10.0, multiplier = 1.0 },
  },
  hardpoints = {
    {
      hardpoint_id = "engine_aft",
      display_name = "Engine Aft",
      slot_kind = "engine",
      offset_m = { 0.0, -5.0, 0.0 },
      local_rotation_rad = 0.0,
      compatible_tags = { "engine" },
    },
  },
  mounted_modules = {
    {
      hardpoint_id = "engine_aft",
      module_id = "module.engine.test",
      component_overrides = {},
    },
  },
}
"#
}

fn module_sources() -> HashMap<String, String> {
    HashMap::from([(
        "ship_modules/engine_test.lua".to_string(),
        engine_module_source().to_string(),
    )])
}

fn ship_sources(source: &str) -> HashMap<String, String> {
    HashMap::from([("ships/test.lua".to_string(), source.to_string())])
}

fn asset_registry(content_type: &str) -> ScriptAssetRegistry {
    ScriptAssetRegistry {
        schema_version: 1,
        assets: vec![ScriptAssetRegistryEntry {
            asset_id: "test_ship".to_string(),
            shader_family: None,
            source_path: "sprites/test_ship.png".to_string(),
            content_type: content_type.to_string(),
            dependencies: Vec::new(),
            bootstrap_required: false,
            startup_required: false,
            editor_schema: None,
        }],
    }
}

fn module_registry() -> sidereal_game::ShipModuleRegistry {
    load_ship_module_registry_from_sources(
        module_registry_source(),
        Path::new("ship_modules/registry.lua"),
        &module_sources(),
    )
    .expect("module registry")
}

#[test]
fn loads_workspace_ship_and_module_registries() {
    let root = shared_scripts_root();
    let modules = load_ship_module_registry_from_root(&root).expect("module registry");
    let ships = load_ship_registry_from_root(&root).expect("ship registry");

    assert!(
        modules
            .definitions
            .iter()
            .any(|module| module.module_id == "module.engine.main_mk1")
    );
    assert!(
        ships
            .definitions
            .iter()
            .any(|ship| ship.ship_id == "ship.corvette")
    );
    assert!(
        ships
            .definitions
            .iter()
            .any(|ship| ship.ship_id == "ship.rocinante")
    );
}

#[test]
fn rejects_duplicate_ship_ids() {
    let source = r#"
return {
  schema_version = 1,
  ships = {
    { ship_id = "ship.duplicate", bundle_id = "ship.a", script = "ships/a.lua" },
    { ship_id = "ship.duplicate", bundle_id = "ship.b", script = "ships/b.lua" },
  },
}
"#;
    let err = load_ship_registry_from_sources(
        source,
        Path::new("ships/registry.lua"),
        &HashMap::new(),
        &module_registry(),
        &asset_registry("image/png"),
    )
    .expect_err("expected duplicate ship ids to fail");
    assert!(
        err.to_string().contains("duplicate ship_id=ship.duplicate"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_missing_ship_scripts() {
    let err = load_ship_registry_from_sources(
        valid_ship_registry_source(),
        Path::new("ships/registry.lua"),
        &HashMap::new(),
        &module_registry(),
        &asset_registry("image/png"),
    )
    .expect_err("expected missing ship script to fail");
    assert!(
        err.to_string()
            .contains("ship_id=ship.test references missing script=ships/test.lua"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_duplicate_hardpoints() {
    let source = valid_ship_source().replace(
        r#"{
      hardpoint_id = "engine_aft",
      display_name = "Engine Aft",
      slot_kind = "engine",
      offset_m = { 0.0, -5.0, 0.0 },
      local_rotation_rad = 0.0,
      compatible_tags = { "engine" },
    }"#,
        r#"{
      hardpoint_id = "engine_aft",
      display_name = "Engine Aft",
      slot_kind = "engine",
      offset_m = { 0.0, -5.0, 0.0 },
      local_rotation_rad = 0.0,
      compatible_tags = { "engine" },
    },
    {
      hardpoint_id = "engine_aft",
      display_name = "Engine Duplicate",
      slot_kind = "engine",
      offset_m = { 1.0, -5.0, 0.0 },
      local_rotation_rad = 0.0,
      compatible_tags = { "engine" },
    }"#,
    );
    let err = load_ship_registry_from_sources(
        valid_ship_registry_source(),
        Path::new("ships/registry.lua"),
        &ship_sources(&source),
        &module_registry(),
        &asset_registry("image/png"),
    )
    .expect_err("expected duplicate hardpoints to fail");
    assert!(
        err.to_string()
            .contains("duplicate hardpoint_id=engine_aft"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_missing_mounted_module_ids() {
    let source = valid_ship_source().replace("module.engine.test", "module.engine.missing");
    let err = load_ship_registry_from_sources(
        valid_ship_registry_source(),
        Path::new("ships/registry.lua"),
        &ship_sources(&source),
        &module_registry(),
        &asset_registry("image/png"),
    )
    .expect_err("expected missing module id to fail");
    assert!(
        err.to_string()
            .contains("mounted module references unknown module_id=module.engine.missing"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_incompatible_module_to_hardpoint_slot_kind() {
    let source = valid_ship_source().replace(r#"slot_kind = "engine""#, r#"slot_kind = "weapon""#);
    let err = load_ship_registry_from_sources(
        valid_ship_registry_source(),
        Path::new("ships/registry.lua"),
        &ship_sources(&source),
        &module_registry(),
        &asset_registry("image/png"),
    )
    .expect_err("expected incompatible module slot to fail");
    assert!(
        err.to_string()
            .contains("is incompatible with hardpoint_id=engine_aft slot_kind=weapon"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_missing_or_non_image_visual_assets() {
    let err = load_ship_registry_from_sources(
        valid_ship_registry_source(),
        Path::new("ships/registry.lua"),
        &ship_sources(valid_ship_source()),
        &module_registry(),
        &asset_registry("text/plain"),
    )
    .expect_err("expected non-image visual asset to fail");
    assert!(
        err.to_string()
            .contains("visual.visual_asset_id=test_ship must reference an image asset"),
        "unexpected error: {err}"
    );

    let err = load_ship_registry_from_sources(
        valid_ship_registry_source(),
        Path::new("ships/registry.lua"),
        &ship_sources(valid_ship_source()),
        &module_registry(),
        &ScriptAssetRegistry {
            schema_version: 1,
            assets: Vec::new(),
        },
    )
    .expect_err("expected missing visual asset to fail");
    assert!(
        err.to_string()
            .contains("visual.visual_asset_id=test_ship is not in asset registry"),
        "unexpected error: {err}"
    );
}

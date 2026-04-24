use sidereal_scripting::{
    load_planet_registry_from_root, load_planet_registry_from_sources, resolve_scripts_root,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn shared_scripts_root() -> PathBuf {
    resolve_scripts_root(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn loads_shared_planet_registry_from_workspace_scripts() {
    let registry = load_planet_registry_from_root(&shared_scripts_root()).expect("planet registry");
    assert_eq!(registry.schema_version, 1);
    assert!(
        registry
            .definitions
            .iter()
            .any(|planet| planet.planet_id == "planet.aurelia")
    );
    assert!(
        registry
            .definitions
            .iter()
            .any(|planet| planet.planet_id == "planet.helion")
    );
}

#[test]
fn rejects_duplicate_planet_registry_ids() {
    let source = r#"
return {
  schema_version = 1,
  planets = {
    { planet_id = "planet.duplicate", script = "planets/a.lua" },
    { planet_id = "planet.duplicate", script = "planets/b.lua" },
  },
}
"#;
    let err = load_planet_registry_from_sources(
        source,
        Path::new("planets/registry.lua"),
        &HashMap::new(),
    )
    .expect_err("expected duplicate planet_id to fail");
    assert!(
        err.to_string()
            .contains("duplicate planet_id=planet.duplicate"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_missing_planet_definition_script() {
    let source = r#"
return {
  schema_version = 1,
  planets = {
    { planet_id = "planet.missing", script = "planets/missing.lua" },
  },
}
"#;
    let err = load_planet_registry_from_sources(
        source,
        Path::new("planets/registry.lua"),
        &HashMap::new(),
    )
    .expect_err("expected missing script to fail");
    assert!(
        err.to_string()
            .contains("planet_id=planet.missing references missing script=planets/missing.lua"),
        "unexpected error: {err}"
    );
}

use bevy::prelude::*;
use sidereal_game::SiderealGamePlugin;
use sidereal_game::generated::components::{
    GeneratedComponentRegistry, generated_component_registry,
};
use std::collections::HashSet;

#[test]
fn generated_component_registry_has_unique_component_kinds() {
    let registry = generated_component_registry();
    let mut seen = HashSet::<&str>::new();
    for entry in &registry {
        assert!(
            seen.insert(entry.component_kind),
            "duplicate component_kind detected: {}",
            entry.component_kind
        );
    }
}

#[test]
fn generated_component_registry_has_unique_type_paths() {
    let registry = generated_component_registry();
    let mut seen = HashSet::<&str>::new();
    for entry in &registry {
        assert!(
            seen.insert(entry.type_path),
            "duplicate type_path detected: {}",
            entry.type_path
        );
    }
}

#[test]
fn flight_computer_mapping_is_stable() {
    let registry = generated_component_registry();
    let mapping = registry
        .iter()
        .find(|entry| entry.component_kind == "flight_computer")
        .expect("flight_computer mapping should exist");
    assert!(mapping.type_path.ends_with("FlightComputer"));
}

#[test]
fn cost_mapping_exists() {
    let registry = generated_component_registry();
    let mapping = registry
        .iter()
        .find(|entry| entry.component_kind == "cost")
        .expect("cost mapping should exist");
    assert!(mapping.type_path.ends_with("Cost"));
}

#[test]
fn sidereal_game_plugin_inserts_generated_registry_resource() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, SiderealGamePlugin));
    assert!(
        app.world()
            .contains_resource::<GeneratedComponentRegistry>()
    );
}

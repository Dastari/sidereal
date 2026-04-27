use bevy::prelude::*;
use sidereal_game::SiderealGamePlugin;
use sidereal_game::generated::components::{
    GeneratedComponentRegistry, generated_component_registry,
};
use sidereal_game::{ComponentEditorValueKind, SiderealGameCorePlugin};
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
fn visibility_v2_component_mappings_exist() {
    let registry = generated_component_registry();
    let signal = registry
        .iter()
        .find(|entry| entry.component_kind == "signal_signature")
        .expect("signal_signature mapping should exist");
    assert!(signal.type_path.ends_with("SignalSignature"));

    let resolution = registry
        .iter()
        .find(|entry| entry.component_kind == "contact_resolution_m")
        .expect("contact_resolution_m mapping should exist");
    assert!(resolution.type_path.ends_with("ContactResolutionM"));
}

#[test]
fn scanner_component_mapping_exists() {
    let registry = generated_component_registry();
    let mapping = registry
        .iter()
        .find(|entry| entry.component_kind == "scanner_component")
        .expect("scanner_component mapping should exist");
    assert!(mapping.type_path.ends_with("ScannerComponent"));
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

#[test]
fn generated_registry_resource_infers_editor_schema() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, SiderealGameCorePlugin));
    let registry = app.world().resource::<GeneratedComponentRegistry>();
    assert!(registry.shader_entries.is_empty());
    let mapping = registry
        .entries
        .iter()
        .find(|entry| entry.component_kind == "planet_body_shader_settings")
        .expect("planet_body_shader_settings mapping should exist");
    assert_eq!(
        mapping.editor_schema.root_value_kind,
        ComponentEditorValueKind::Struct
    );
    let seed_field = mapping
        .editor_schema
        .fields
        .iter()
        .find(|field| field.field_path == "seed")
        .expect("seed field should be inferred");
    assert_eq!(
        seed_field.value_kind,
        ComponentEditorValueKind::UnsignedInteger
    );
    let sun_direction_field = mapping
        .editor_schema
        .fields
        .iter()
        .find(|field| field.field_path == "sun_direction_xy")
        .expect("sun_direction_xy field should be inferred");
    assert_eq!(
        sun_direction_field.value_kind,
        ComponentEditorValueKind::Vec2
    );
    let primary_color_field = mapping
        .editor_schema
        .fields
        .iter()
        .find(|field| field.field_path == "color_primary_rgb")
        .expect("color_primary_rgb field should be inferred");
    assert_eq!(
        primary_color_field.value_kind,
        ComponentEditorValueKind::ColorRgb
    );
}

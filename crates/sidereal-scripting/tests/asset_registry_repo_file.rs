use sidereal_scripting::load_asset_registry_from_root;
use std::path::PathBuf;

#[test]
fn repository_asset_registry_loads_preview_planet_shader_entry() {
    let scripts_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/scripts");
    let registry = load_asset_registry_from_root(&scripts_root)
        .expect("repository asset registry should load");
    let preview_asset = registry
        .assets
        .iter()
        .find(|asset| asset.asset_id == "planet_preview_study_wgsl")
        .expect("preview planet shader asset should be registered");

    assert_eq!(
        preview_asset.source_path,
        "shaders/planet_preview_study.wgsl"
    );
    assert_eq!(
        preview_asset.shader_family.as_deref(),
        Some("preview_fullscreen_planet_study")
    );
    let schema = preview_asset
        .editor_schema
        .as_ref()
        .expect("preview planet shader should expose editor schema");
    assert!(
        schema
            .uniforms
            .iter()
            .any(|field| field.field_path == "surface_family")
    );
    assert!(
        schema
            .presets
            .iter()
            .any(|preset| preset.preset_id == "mars_dust")
    );
}

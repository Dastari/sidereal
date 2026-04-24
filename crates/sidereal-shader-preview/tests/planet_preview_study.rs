use sidereal_shader_preview::validate_wgsl_source;
use std::path::PathBuf;

#[test]
fn planet_preview_study_shader_validates_for_preview_runtime() {
    let shader_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/shaders/planet_preview_study.wgsl");
    let source = std::fs::read_to_string(shader_path).expect("shader source should exist");
    let result = validate_wgsl_source(&source).expect("shader should validate");
    assert!(result.ok, "preview shader validation should succeed");
}

#[test]
fn active_planet_visual_shader_validates_for_preview_runtime() {
    let shader_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/shaders/planet_visual.wgsl");
    let source = std::fs::read_to_string(shader_path).expect("shader source should exist");
    let result = validate_wgsl_source(&source).expect("shader should validate");
    assert!(result.ok, "active planet shader validation should succeed");
}

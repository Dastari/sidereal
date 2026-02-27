use sidereal_gateway::api::{parse_vec3_property, resolve_asset_stream_path};

#[test]
fn resolve_asset_stream_path_knows_corvette_and_starfield() {
    assert!(resolve_asset_stream_path("corvette_01").is_some());
    assert!(resolve_asset_stream_path("starfield_wgsl").is_some());
    assert!(resolve_asset_stream_path("space_background_wgsl").is_some());
    assert!(resolve_asset_stream_path("sprite_pixel_effect_wgsl").is_some());
    assert!(resolve_asset_stream_path("unknown").is_none());
}

#[test]
fn parse_vec3_property_defaults_when_missing() {
    let value = serde_json::json!({});
    assert_eq!(parse_vec3_property(&value, "position_m"), [0.0, 0.0, 0.0]);
}

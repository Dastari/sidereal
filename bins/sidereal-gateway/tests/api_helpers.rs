use sidereal_gateway::api::parse_vec3_property;

#[test]
fn parse_vec3_property_defaults_when_missing() {
    let value = serde_json::json!({});
    assert_eq!(parse_vec3_property(&value, "position_m"), [0.0, 0.0, 0.0]);
}

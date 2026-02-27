use sidereal_persistence::{
    GraphEntityRecord, cypher_literal, decode_reflect_component, encode_reflect_component,
    parse_agtype_json, parse_agtype_string, validate_runtime_guid_uniqueness,
};

#[test]
fn cypher_literal_renders_nested_maps_and_arrays() {
    let value = serde_json::json!({"a": 1, "b": [true, "x"], "c": {"k": "v"}});
    let out = cypher_literal(&value);
    assert!(out.contains("a:1"));
    assert!(out.contains("b:[true,'x']"));
    assert!(out.contains("c:{k:'v'}"));
}

#[test]
fn parse_agtype_helpers_handle_suffix() {
    let s = parse_agtype_string("\"player:1\"::agtype".to_string()).expect("string");
    assert_eq!(s, "player:1");
    let json = parse_agtype_json("{\"x\":1}::agtype".to_string()).expect("json");
    assert_eq!(json["x"], 1);
}

#[test]
fn validate_runtime_guid_uniqueness_rejects_collisions() {
    let guid = "316c04e7-a139-4b36-afdb-8a607b565fec";
    let records = vec![
        GraphEntityRecord {
            entity_id: format!("player:{guid}"),
            labels: vec!["Entity".to_string()],
            properties: serde_json::json!({}),
            components: Vec::new(),
        },
        GraphEntityRecord {
            entity_id: format!("ship:{guid}"),
            labels: vec!["Entity".to_string()],
            properties: serde_json::json!({}),
            components: Vec::new(),
        },
    ];
    let err = validate_runtime_guid_uniqueness(&records).expect_err("should reject collision");
    assert!(format!("{err}").contains("runtime GUID collision"));
}

#[test]
fn validate_runtime_guid_uniqueness_accepts_distinct_guids() {
    let records = vec![
        GraphEntityRecord {
            entity_id: "player:316c04e7-a139-4b36-afdb-8a607b565fec".to_string(),
            labels: vec!["Entity".to_string()],
            properties: serde_json::json!({}),
            components: Vec::new(),
        },
        GraphEntityRecord {
            entity_id: "ship:199d6542-2603-4576-a510-7fa7eaddbe3d".to_string(),
            labels: vec!["Entity".to_string()],
            properties: serde_json::json!({}),
            components: Vec::new(),
        },
    ];
    validate_runtime_guid_uniqueness(&records).expect("distinct GUIDs should pass");
}

#[test]
fn reflect_envelope_roundtrip() {
    let payload = serde_json::json!({"fuel_kg": 42.0});
    let envelope = encode_reflect_component("sidereal_game::FuelTank", payload.clone());
    let decoded = decode_reflect_component(&envelope, "sidereal_game::FuelTank").expect("decode");
    assert_eq!(decoded, &payload);
}

use bevy::math::DVec2;
use serde_json::json;
use uuid::Uuid;

use crate::replication::runtime_scripting::{ScriptIntent, parse_intent};

#[test]
fn set_navigation_target_preserves_f64_coordinates() {
    let entity_id = Uuid::new_v4();
    let intent = parse_intent(
        "set_navigation_target",
        &json!({
            "entity_id": entity_id.to_string(),
            "target_position": {
                "x": 5_000_000_000_000.25_f64,
                "y": -7_000_000_000_000.5_f64,
            },
        }),
    )
    .unwrap();

    let ScriptIntent::SetNavigationTarget {
        entity_id: parsed_entity_id,
        target_position,
    } = intent
    else {
        panic!("expected set_navigation_target intent");
    };
    assert_eq!(parsed_entity_id, entity_id);
    assert_eq!(
        target_position,
        DVec2::new(5_000_000_000_000.25, -7_000_000_000_000.5)
    );
}

#[test]
fn set_navigation_target_rejects_non_finite_coordinates() {
    let entity_id = Uuid::new_v4();
    let err = parse_intent(
        "set_navigation_target",
        &json!({
            "entity_id": entity_id.to_string(),
            "target_position": {
                "x": f64::INFINITY,
                "y": 1.0,
            },
        }),
    )
    .unwrap_err();

    assert!(err.contains("target_position.x must be finite number"));
}

use sidereal_persistence::GraphPersistence;
use sidereal_persistence::{GraphComponentRecord, GraphEntityRecord};
use uuid::Uuid;

fn test_database_url() -> String {
    std::env::var("SIDEREAL_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("REPLICATION_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
}

fn unique_graph_name(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4().simple())
}

fn make_ship_batch(ship_id: &str, hardpoint_id: &str, engine_id: &str) -> Vec<GraphEntityRecord> {
    vec![
        GraphEntityRecord {
            entity_id: ship_id.to_string(),
            labels: vec!["Entity".to_string(), "Ship".to_string()],
            properties: serde_json::json!({
                "name": "ISS Persistence",
                "position_m": [100.0, 20.0, -5.0],
                "velocity_mps": [12.0, 0.0, 0.0],
                "mass_kg": 4200.0,
            }),
            components: vec![
                GraphComponentRecord {
                    component_id: format!("{ship_id}:display_name"),
                    component_kind: "display_name".to_string(),
                    properties: serde_json::json!({"value": "ISS Persistence"}),
                },
                GraphComponentRecord {
                    component_id: format!("{ship_id}:flight_computer"),
                    component_kind: "flight_computer".to_string(),
                    properties: serde_json::json!({"profile": "CruiseAssist", "throttle": 0.58}),
                },
                GraphComponentRecord {
                    component_id: format!("{ship_id}:health_pool"),
                    component_kind: "health_pool".to_string(),
                    properties: serde_json::json!({"hp": 98.0, "max_hp": 100.0}),
                },
            ],
        },
        GraphEntityRecord {
            entity_id: hardpoint_id.to_string(),
            labels: vec!["Entity".to_string(), "Hardpoint".to_string()],
            properties: serde_json::json!({
                "owner_entity_id": ship_id,
                "parent_entity_id": ship_id,
                "hardpoint_id": "engine_main",
                "offset_m": [0.0, 0.0, -4.0],
            }),
            components: vec![GraphComponentRecord {
                component_id: format!("{hardpoint_id}:hardpoint"),
                component_kind: "hardpoint".to_string(),
                properties: serde_json::json!({"hardpoint_id": "engine_main", "offset_m": [0.0, 0.0, -4.0]}),
            }],
        },
        GraphEntityRecord {
            entity_id: engine_id.to_string(),
            labels: vec!["Entity".to_string(), "Engine".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_id,
                "mounted_on_entity_id": hardpoint_id,
                "thrust": 280000.0,
                "reverse_thrust": 140000.0,
                "torque_thrust": 112000.0,
            }),
            components: vec![GraphComponentRecord {
                component_id: format!("{engine_id}:engine"),
                component_kind: "engine".to_string(),
                properties: serde_json::json!({
                    "thrust": 280000.0,
                    "reverse_thrust": 140000.0,
                    "torque_thrust": 112000.0,
                    "burn_rate_kg_s": 18.0,
                }),
            }],
        },
    ]
}

#[test]
fn graph_persistence_full_lifecycle_ship_hardpoint_engine() {
    let database_url = test_database_url();
    let graph_name = unique_graph_name("sidereal_persistence_lifecycle");
    let mut persistence = match GraphPersistence::connect_with_graph(&database_url, &graph_name) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!("skipping graph lifecycle test; postgres unavailable: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        tracing::warn!("skipping graph lifecycle test; AGE schema unavailable: {err}");
        return;
    }

    let ship_id = Uuid::new_v4().to_string();
    let hardpoint_id = Uuid::new_v4().to_string();
    let engine_id = Uuid::new_v4().to_string();

    let mut updates = make_ship_batch(&ship_id, &hardpoint_id, &engine_id);
    persistence
        .persist_graph_records(&updates, 100)
        .expect("initial graph records should persist");

    let mut hydrated = persistence
        .load_graph_records()
        .expect("load graph records should succeed");
    hydrated.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
    assert_eq!(hydrated.len(), 3);

    let ship = hydrated
        .iter()
        .find(|r| r.entity_id == ship_id)
        .expect("ship should hydrate");
    assert_eq!(ship.properties["name"], "ISS Persistence");
    assert_eq!(ship.components.len(), 3);

    updates[0].properties["velocity_mps"] = serde_json::json!([19.0, 0.0, 0.0]);
    updates[2].properties["thrust"] = serde_json::json!(300000.0);
    persistence
        .persist_graph_records(&updates, 101)
        .expect("second graph records should persist");
    persistence
        .remove_graph_entities(std::slice::from_ref(&hardpoint_id))
        .expect("hardpoint removal should persist");

    let after = persistence
        .load_graph_records()
        .expect("load graph records should succeed");
    assert!(after.iter().any(|r| r.entity_id == ship_id));
    assert!(after.iter().any(|r| r.entity_id == engine_id));
    assert!(!after.iter().any(|r| r.entity_id == hardpoint_id));
    let ship_after = after
        .iter()
        .find(|r| r.entity_id == ship_id)
        .expect("ship should still exist");
    assert_eq!(
        ship_after.properties["velocity_mps"],
        serde_json::json!([19.0, 0.0, 0.0])
    );

    persistence.drop_graph().expect("test graph should drop");
}

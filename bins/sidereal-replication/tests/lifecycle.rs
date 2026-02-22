use sidereal_persistence::GraphPersistence;
use sidereal_persistence::{
    ChannelClass, GraphComponentRecord, GraphEntityRecord, NetEnvelope, decode_envelope_json,
    encode_envelope_json,
};
use sidereal_replication::state::{
    GraphDeltaBatch, flush_pending_updates, hydrate_known_entity_ids, ingest_graph_envelope,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

fn test_database_url() -> String {
    std::env::var("SIDEREAL_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("REPLICATION_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
}

fn unique_graph_name(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4().simple())
}

fn make_envelope(tick: u64, batch: GraphDeltaBatch) -> NetEnvelope<GraphDeltaBatch> {
    NetEnvelope {
        protocol_version: 1,
        channel: ChannelClass::Control,
        source_shard_id: 7,
        lease_epoch: 1,
        seq: tick,
        tick,
        payload: batch,
    }
}

#[test]
fn replication_ingest_persist_hydrate_lifecycle() {
    let database_url = test_database_url();
    let graph_name = unique_graph_name("sidereal_replication_lifecycle");
    let mut persistence = match GraphPersistence::connect_with_graph(&database_url, &graph_name) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("skipping replication lifecycle test; postgres unavailable: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        eprintln!("skipping replication lifecycle test; AGE schema unavailable: {err}");
        return;
    }

    let ship_id = format!("ship:{}", Uuid::new_v4());
    let hardpoint_id = format!("hardpoint:{}", Uuid::new_v4());
    let engine_id = format!("engine:{}", Uuid::new_v4());

    let updates = vec![
        GraphEntityRecord {
            entity_id: ship_id.clone(),
            labels: vec!["Entity".to_string(), "Ship".to_string()],
            properties: serde_json::json!({
                "name": "ISS Replication",
                "position_m": [10.0, 0.0, 0.0],
                "velocity_mps": [2.0, 0.0, 0.0],
            }),
            components: vec![
                GraphComponentRecord {
                    component_id: format!("{ship_id}:display_name"),
                    component_kind: "display_name".to_string(),
                    properties: serde_json::json!({"value": "ISS Replication"}),
                },
                GraphComponentRecord {
                    component_id: format!("{ship_id}:flight_computer"),
                    component_kind: "flight_computer".to_string(),
                    properties: serde_json::json!({"profile": "CruiseAssist", "throttle": 0.41}),
                },
            ],
        },
        GraphEntityRecord {
            entity_id: hardpoint_id.clone(),
            labels: vec!["Entity".to_string(), "Hardpoint".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_id,
                "owner_entity_id": ship_id,
                "hardpoint_id": "engine_main",
                "offset_m": [0.0, 0.0, -2.5]
            }),
            components: vec![GraphComponentRecord {
                component_id: format!("{hardpoint_id}:hardpoint"),
                component_kind: "hardpoint".to_string(),
                properties: serde_json::json!({"hardpoint_id": "engine_main", "offset_m": [0.0, 0.0, -2.5]}),
            }],
        },
        GraphEntityRecord {
            entity_id: engine_id.clone(),
            labels: vec!["Entity".to_string(), "Engine".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_id,
                "mounted_on_entity_id": hardpoint_id,
                "thrust": 180000.0,
                "reverse_thrust": 90000.0,
                "torque_thrust": 72000.0
            }),
            components: vec![GraphComponentRecord {
                component_id: format!("{engine_id}:engine"),
                component_kind: "engine".to_string(),
                properties: serde_json::json!({
                    "thrust": 180000.0,
                    "reverse_thrust": 90000.0,
                    "torque_thrust": 72000.0,
                    "burn_rate_kg_s": 14.0,
                }),
            }],
        },
    ];

    let encoded = encode_envelope_json(&make_envelope(
        500,
        GraphDeltaBatch {
            upserts: updates,
            removals: Vec::new(),
        },
    ))
    .expect("encode should work");
    let decoded = decode_envelope_json::<GraphDeltaBatch>(&encoded).expect("decode should work");
    let mut known_entities = hydrate_known_entity_ids(&mut persistence).expect("hydrate ids");
    let mut pending_updates = HashMap::<String, GraphEntityRecord>::new();
    let mut pending_removals = HashSet::<String>::new();

    let has_removals = ingest_graph_envelope(
        &mut known_entities,
        &mut pending_updates,
        &mut pending_removals,
        decoded,
    );
    assert!(!has_removals);
    assert_eq!(pending_updates.len(), 3);

    flush_pending_updates(
        &mut persistence,
        &mut pending_updates,
        &mut pending_removals,
        500,
    )
    .expect("flush should work");
    assert!(pending_updates.is_empty());
    assert!(pending_removals.is_empty());
    assert_eq!(known_entities.len(), 3);

    let hydrated_ids = hydrate_known_entity_ids(&mut persistence).expect("hydrate ids after flush");
    assert_eq!(hydrated_ids.len(), 3);
    assert!(hydrated_ids.iter().any(|id| id.starts_with("ship:")));

    let encoded = encode_envelope_json(&make_envelope(
        501,
        GraphDeltaBatch {
            upserts: Vec::new(),
            removals: vec![hardpoint_id.clone()],
        },
    ))
    .expect("encode removal envelope");
    let decoded =
        decode_envelope_json::<GraphDeltaBatch>(&encoded).expect("decode removal envelope");
    let has_removals = ingest_graph_envelope(
        &mut known_entities,
        &mut pending_updates,
        &mut pending_removals,
        decoded,
    );
    assert!(has_removals);

    flush_pending_updates(
        &mut persistence,
        &mut pending_updates,
        &mut pending_removals,
        501,
    )
    .expect("flush removal should work");
    let hydrated_records = persistence
        .load_graph_records()
        .expect("graph records should load");
    assert!(hydrated_records.iter().any(|r| r.entity_id == ship_id));
    assert!(hydrated_records.iter().any(|r| r.entity_id == engine_id));
    assert!(!hydrated_records.iter().any(|r| r.entity_id == hardpoint_id));

    persistence.drop_graph().expect("test graph should drop");
}

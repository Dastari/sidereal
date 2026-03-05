use sidereal_persistence::GraphEntityRecord;
use sidereal_replication::persistence_helpers::{GraphDeltaBatch, ingest_graph_batch};
use std::collections::{HashMap, HashSet};

#[test]
fn ingest_graph_batch_tracks_add_remove() {
    let mut cache = HashSet::<String>::new();
    let mut pending = HashMap::<String, GraphEntityRecord>::new();
    let mut removals = HashSet::<String>::new();
    let add = GraphEntityRecord {
        entity_id: "11111111-1111-1111-1111-111111111111".to_string(),
        labels: vec!["Entity".to_string()],
        properties: serde_json::json!({}),
        components: Vec::new(),
    };
    let has_removals = ingest_graph_batch(
        &mut cache,
        &mut pending,
        &mut removals,
        GraphDeltaBatch {
            upserts: vec![add],
            removals: Vec::new(),
        },
    );
    assert!(!has_removals);
    assert!(cache.contains("11111111-1111-1111-1111-111111111111"));
    assert!(pending.contains_key("11111111-1111-1111-1111-111111111111"));
    assert!(removals.is_empty());

    let has_removals = ingest_graph_batch(
        &mut cache,
        &mut pending,
        &mut removals,
        GraphDeltaBatch {
            upserts: Vec::new(),
            removals: vec!["11111111-1111-1111-1111-111111111111".to_string()],
        },
    );
    assert!(has_removals);
    assert!(!cache.contains("11111111-1111-1111-1111-111111111111"));
    assert!(!pending.contains_key("11111111-1111-1111-1111-111111111111"));
    assert!(removals.contains("11111111-1111-1111-1111-111111111111"));
}

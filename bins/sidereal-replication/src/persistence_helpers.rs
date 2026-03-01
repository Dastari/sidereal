use sidereal_persistence::{GraphEntityRecord, GraphPersistence, NetEnvelope, PersistenceError};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Default)]
pub struct GraphDeltaBatch {
    #[serde(default)]
    pub upserts: Vec<GraphEntityRecord>,
    #[serde(default)]
    pub removals: Vec<String>,
}

pub fn hydrate_known_entity_ids(
    persistence: &mut GraphPersistence,
) -> std::result::Result<HashSet<String>, PersistenceError> {
    let records = persistence.load_graph_records()?;
    Ok(records
        .into_iter()
        .map(|record| record.entity_id)
        .collect::<HashSet<_>>())
}

pub fn ingest_graph_batch(
    known_entities: &mut HashSet<String>,
    pending_upserts: &mut HashMap<String, GraphEntityRecord>,
    pending_removals: &mut HashSet<String>,
    batch: GraphDeltaBatch,
) -> bool {
    let mut has_removals = false;
    for entity_id in batch.removals {
        known_entities.remove(&entity_id);
        pending_upserts.remove(&entity_id);
        pending_removals.insert(entity_id);
        has_removals = true;
    }
    for record in batch.upserts {
        if pending_removals.remove(&record.entity_id) {
            has_removals = true;
        }
        known_entities.insert(record.entity_id.clone());
        pending_upserts.insert(record.entity_id.clone(), record);
    }
    has_removals
}

pub fn ingest_graph_envelope(
    known_entities: &mut HashSet<String>,
    pending_upserts: &mut HashMap<String, GraphEntityRecord>,
    pending_removals: &mut HashSet<String>,
    envelope: NetEnvelope<GraphDeltaBatch>,
) -> bool {
    ingest_graph_batch(
        known_entities,
        pending_upserts,
        pending_removals,
        envelope.payload,
    )
}

pub fn flush_pending_updates(
    persistence: &mut GraphPersistence,
    pending_upserts: &mut HashMap<String, GraphEntityRecord>,
    pending_removals: &mut HashSet<String>,
    tick: u64,
) -> std::result::Result<usize, PersistenceError> {
    if pending_upserts.is_empty() && pending_removals.is_empty() {
        return Ok(0);
    }
    let batch = pending_upserts
        .drain()
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    let removals = pending_removals.drain().collect::<Vec<_>>();
    let count = batch.len() + removals.len();
    persistence.persist_graph_records(&batch, tick)?;
    persistence.remove_graph_entities(&removals)?;
    Ok(count)
}

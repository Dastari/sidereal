//! Tests for the replication binary (lifecycle, remote inspect, state ingest).

use bevy::prelude::*;
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_persistence::GraphEntityRecord;
use sidereal_replication::state::{GraphDeltaBatch, ingest_graph_batch};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};

use crate::replication::lifecycle::configure_remote;
use crate::BrpAuthToken;

#[test]
fn remote_endpoint_registers_when_enabled() {
    let cfg = RemoteInspectConfig {
        enabled: true,
        bind_addr: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: 15713,
        auth_token: Some("0123456789abcdef".to_string()),
    };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    configure_remote(&mut app, &cfg);

    assert!(app
        .world()
        .contains_resource::<bevy_remote::http::HostPort>());
    assert!(app.world().contains_resource::<BrpAuthToken>());
}

#[test]
fn ingest_graph_batch_tracks_add_remove() {
    let mut cache = HashSet::<String>::new();
    let mut pending = HashMap::<String, GraphEntityRecord>::new();
    let mut removals = HashSet::<String>::new();
    let add = GraphEntityRecord {
        entity_id: "ship:1".to_string(),
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
    assert!(cache.contains("ship:1"));
    assert!(pending.contains_key("ship:1"));
    assert!(removals.is_empty());

    let has_removals = ingest_graph_batch(
        &mut cache,
        &mut pending,
        &mut removals,
        GraphDeltaBatch {
            upserts: Vec::new(),
            removals: vec!["ship:1".to_string()],
        },
    );
    assert!(has_removals);
    assert!(!cache.contains("ship:1"));
    assert!(!pending.contains_key("ship:1"));
    assert!(removals.contains("ship:1"));
}

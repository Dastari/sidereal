use postgres::{Client, NoTls, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::HashMap;
use thiserror::Error;

const DEFAULT_GRAPH_NAME: &str = "sidereal";
const SCRIPT_CATALOG_DOCUMENTS_TABLE: &str = "script_catalog_documents";
const SCRIPT_CATALOG_VERSIONS_TABLE: &str = "script_catalog_versions";
const SCRIPT_CATALOG_DRAFTS_TABLE: &str = "script_catalog_drafts";
const PLAYER_NOTIFICATIONS_TABLE: &str = "player_notifications";

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, PersistenceError>;

/// Sanitize a Rust type path into a key safe for AGE/Cypher property names.
/// AGE strips characters like `:` from property keys, so we replace `::` with `__`.
fn sanitize_type_path_key(type_path: &str) -> String {
    type_path.replace("::", "__")
}

pub fn encode_reflect_component(type_path: &str, component_value: JsonValue) -> JsonValue {
    let mut envelope = JsonMap::new();
    envelope.insert(sanitize_type_path_key(type_path), component_value);
    JsonValue::Object(envelope)
}

pub fn decode_reflect_component<'a>(
    payload: &'a JsonValue,
    expected_type_path: &str,
) -> Option<&'a JsonValue> {
    let key = sanitize_type_path_key(expected_type_path);
    payload.as_object()?.get(&key)
}

pub fn ensure_player_notifications_schema(client: &mut Client) -> Result<()> {
    client
        .batch_execute(&format!(
            "
                CREATE TABLE IF NOT EXISTS {PLAYER_NOTIFICATIONS_TABLE} (
                    notification_id UUID PRIMARY KEY,
                    player_entity_id TEXT NOT NULL,
                    notification_kind TEXT NOT NULL,
                    severity TEXT NOT NULL,
                    title TEXT NOT NULL,
                    body TEXT NOT NULL,
                    image_asset_id TEXT NULL,
                    image_alt_text TEXT NULL,
                    placement TEXT NOT NULL,
                    payload JSONB NOT NULL DEFAULT '{{}}'::jsonb,
                    created_at_epoch_s BIGINT NOT NULL,
                    delivered_at_epoch_s BIGINT NULL,
                    dismissed_at_epoch_s BIGINT NULL
                );
                CREATE INDEX IF NOT EXISTS player_notifications_player_created_idx
                    ON {PLAYER_NOTIFICATIONS_TABLE} (player_entity_id, created_at_epoch_s DESC);
                "
        ))
        .map_err(db_err("create player notifications table"))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphComponentRecord {
    pub component_id: String,
    pub component_kind: String,
    pub properties: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEntityRecord {
    pub entity_id: String,
    pub labels: Vec<String>,
    pub properties: JsonValue,
    pub components: Vec<GraphComponentRecord>,
}

pub struct GraphPersistence {
    client: Client,
    graph_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerNotificationRecord {
    pub notification_id: String,
    pub player_entity_id: String,
    pub notification_kind: String,
    pub severity: String,
    pub title: String,
    pub body: String,
    pub image_asset_id: Option<String>,
    pub image_alt_text: Option<String>,
    pub placement: String,
    pub payload: JsonValue,
    pub created_at_epoch_s: i64,
    pub delivered_at_epoch_s: Option<i64>,
    pub dismissed_at_epoch_s: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct StaleComponentCleanupRow {
    entity_id: String,
    incoming_component_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct EntityComponentEdgeRow {
    entity_id: String,
    component_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct ChildEdgeRow {
    parent_entity_id: String,
    child_entity_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct EntityIdRow {
    entity_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct HardpointEdgeRow {
    owner_entity_id: String,
    hardpoint_entity_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct MountedOnEdgeRow {
    module_entity_id: String,
    mount_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScriptCatalogRecord {
    pub script_path: String,
    pub source: String,
    pub revision: u64,
    pub origin: String,
    pub family: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScriptCatalogDocumentSummary {
    pub script_path: String,
    pub family: String,
    pub active_revision: Option<u64>,
    pub has_draft: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScriptCatalogDocumentDetail {
    pub script_path: String,
    pub family: String,
    pub active_revision: Option<u64>,
    pub active_source: Option<String>,
    pub active_origin: Option<String>,
    pub draft_source: Option<String>,
    pub draft_origin: Option<String>,
    pub draft_updated_at_epoch_s: Option<u64>,
}

impl GraphPersistence {
    pub fn connect(database_url: &str) -> Result<Self> {
        Self::connect_with_graph(database_url, DEFAULT_GRAPH_NAME)
    }

    pub fn connect_with_graph(database_url: &str, graph_name: impl Into<String>) -> Result<Self> {
        let client = Client::connect(database_url, NoTls)
            .map_err(|err| PersistenceError::Database(format!("postgres connect failed: {err}")))?;
        Ok(Self {
            client,
            graph_name: graph_name.into(),
        })
    }

    pub fn graph_name(&self) -> &str {
        &self.graph_name
    }

    pub fn ensure_schema(&mut self) -> Result<()> {
        self.client
            .batch_execute("CREATE EXTENSION IF NOT EXISTS age;")
            .map_err(db_err("create age extension"))?;
        self.client
            .batch_execute("LOAD 'age';")
            .map_err(db_err("load age extension"))?;
        self.client
            .batch_execute("SET search_path = ag_catalog, \"$user\", public;")
            .map_err(db_err("set age search_path"))?;

        let graph_exists = self
            .client
            .query_opt(
                "SELECT 1 FROM ag_catalog.ag_graph WHERE name = $1 LIMIT 1",
                &[&self.graph_name],
            )
            .map_err(db_err("query graph existence"))?
            .is_some();
        if !graph_exists {
            let query = format!(
                "SELECT * FROM ag_catalog.create_graph('{}');",
                escape_cypher_string(&self.graph_name)
            );
            self.client
                .batch_execute(&query)
                .map_err(db_err("create graph"))?;
        }

        self.client
            .batch_execute("SET search_path = public;")
            .map_err(db_err("reset search_path"))?;

        self.client
            .batch_execute(
                "
                CREATE TABLE IF NOT EXISTS replication_snapshot_markers (
                    snapshot_id BIGSERIAL PRIMARY KEY,
                    snapshot_tick BIGINT NOT NULL,
                    entity_count BIGINT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL
                );
                ",
            )
            .map_err(db_err("create snapshot marker table"))?;
        self.client
            .batch_execute(
                "
                CREATE TABLE IF NOT EXISTS script_world_init_state (
                    init_key TEXT PRIMARY KEY,
                    script_path TEXT NOT NULL,
                    applied_at_epoch_s BIGINT NOT NULL
                );
                ",
            )
            .map_err(db_err("create script world init state table"))?;
        ensure_script_catalog_schema(&mut self.client)?;
        ensure_player_notifications_schema(&mut self.client)?;

        Ok(())
    }

    pub fn ensure_player_notifications_schema(&mut self) -> Result<()> {
        ensure_player_notifications_schema(&mut self.client)
    }

    pub fn insert_player_notification(&mut self, record: &PlayerNotificationRecord) -> Result<()> {
        let notification_id = record.notification_id.to_string();
        let payload = serde_json::to_string(&record.payload)
            .map_err(|err| PersistenceError::Serialization(err.to_string()))?;
        self.client
            .execute(
                &format!(
                    "
                    INSERT INTO {PLAYER_NOTIFICATIONS_TABLE} (
                        notification_id,
                        player_entity_id,
                        notification_kind,
                        severity,
                        title,
                        body,
                        image_asset_id,
                        image_alt_text,
                        placement,
                        payload,
                        created_at_epoch_s,
                        delivered_at_epoch_s,
                        dismissed_at_epoch_s
                    )
                    VALUES (
                        $1::text::uuid,
                        $2,
                        $3,
                        $4,
                        $5,
                        $6,
                        $7,
                        $8,
                        $9,
                        $10::text::jsonb,
                        $11,
                        $12,
                        $13
                    )
                    ON CONFLICT (notification_id) DO NOTHING
                    "
                ),
                &[
                    &notification_id,
                    &record.player_entity_id,
                    &record.notification_kind,
                    &record.severity,
                    &record.title,
                    &record.body,
                    &record.image_asset_id,
                    &record.image_alt_text,
                    &record.placement,
                    &payload,
                    &record.created_at_epoch_s,
                    &record.delivered_at_epoch_s,
                    &record.dismissed_at_epoch_s,
                ],
            )
            .map_err(db_err("insert player notification"))?;
        Ok(())
    }

    pub fn mark_player_notification_delivered(
        &mut self,
        player_entity_id: &str,
        notification_id: &str,
        delivered_at_epoch_s: i64,
    ) -> Result<bool> {
        self.client
            .execute(
                &format!(
                    "
                    UPDATE {PLAYER_NOTIFICATIONS_TABLE}
                    SET delivered_at_epoch_s = COALESCE(delivered_at_epoch_s, $3)
                    WHERE player_entity_id = $1 AND notification_id = $2::text::uuid
                    "
                ),
                &[&player_entity_id, &notification_id, &delivered_at_epoch_s],
            )
            .map(|count| count > 0)
            .map_err(db_err("mark player notification delivered"))
    }

    pub fn mark_player_notification_dismissed(
        &mut self,
        player_entity_id: &str,
        notification_id: &str,
        dismissed_at_epoch_s: i64,
    ) -> Result<bool> {
        self.client
            .execute(
                &format!(
                    "
                    UPDATE {PLAYER_NOTIFICATIONS_TABLE}
                    SET dismissed_at_epoch_s = COALESCE(dismissed_at_epoch_s, $3)
                    WHERE player_entity_id = $1 AND notification_id = $2::text::uuid
                    "
                ),
                &[&player_entity_id, &notification_id, &dismissed_at_epoch_s],
            )
            .map(|count| count > 0)
            .map_err(db_err("mark player notification dismissed"))
    }

    pub fn script_world_init_state_exists(&mut self, init_key: &str) -> Result<bool> {
        self.client
            .query_opt(
                "SELECT 1 FROM script_world_init_state WHERE init_key = $1 LIMIT 1",
                &[&init_key],
            )
            .map_err(db_err("query script world init state"))
            .map(|row| row.is_some())
    }

    pub fn insert_script_world_init_state(
        &mut self,
        init_key: &str,
        script_path: &str,
        applied_at_epoch_s: i64,
    ) -> Result<()> {
        self.client
            .execute(
                "INSERT INTO script_world_init_state (init_key, script_path, applied_at_epoch_s)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (init_key) DO NOTHING",
                &[&init_key, &script_path, &applied_at_epoch_s],
            )
            .map_err(db_err("insert script world init state"))?;
        Ok(())
    }

    pub fn persist_graph_records(
        &mut self,
        records: &[GraphEntityRecord],
        tick: u64,
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        validate_runtime_guid_uniqueness(records)?;
        self.client
            .batch_execute("LOAD 'age'; SET search_path = ag_catalog, \"$user\", public;")
            .map_err(db_err("prep age for graph persist"))?;

        for record in records {
            let labels = sanitize_labels(&record.labels);
            let mut set_parts = vec![format!("e.last_tick={tick}")];
            set_parts.push(format!(
                "e.entity_labels={}",
                cypher_literal(&JsonValue::Array(
                    labels
                        .iter()
                        .cloned()
                        .map(JsonValue::String)
                        .collect::<Vec<_>>()
                ))
            ));
            set_parts.extend(cypher_set_clauses("e", &record.properties));

            let query = format!(
                "MERGE (e:Entity {{entity_id:'{}'}}) SET {}",
                escape_cypher_string(&record.entity_id),
                set_parts.join(", "),
            );
            self.run_cypher(&query)?;

            let incoming_component_ids = JsonValue::Array(
                record
                    .components
                    .iter()
                    .map(|c| JsonValue::String(c.component_id.clone()))
                    .collect::<Vec<_>>(),
            );
            self.run_cypher(&format!(
                "MATCH (e:Entity {{entity_id:'{}'}}) \
                 OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(c:Component) \
                 WHERE c IS NOT NULL AND NOT c.component_id IN {} \
                 DETACH DELETE c",
                escape_cypher_string(&record.entity_id),
                cypher_literal(&incoming_component_ids),
            ))?;

            for component in &record.components {
                let reset_props_clause = format!(
                    "c = {{component_id:{}, component_kind:{}, last_tick:{tick}}}",
                    cypher_literal(&JsonValue::String(component.component_id.clone())),
                    cypher_literal(&JsonValue::String(component.component_kind.clone())),
                );
                let mut comp_set = vec![reset_props_clause];
                append_component_property_clauses(&mut comp_set, &component.properties);
                self.run_cypher(&format!(
                    "MERGE (c:Component {{component_id:'{}'}}) SET {}",
                    escape_cypher_string(&component.component_id),
                    comp_set.join(", ")
                ))?;
                self.run_cypher(&format!(
                    "MATCH (e:Entity {{entity_id:'{}'}}), (c:Component {{component_id:'{}'}}) MERGE (e)-[:HAS_COMPONENT]->(c)",
                    escape_cypher_string(&record.entity_id),
                    escape_cypher_string(&component.component_id),
                ))?;
            }

            self.persist_relationship_edges(record)?;
        }

        self.client
            .batch_execute("SET search_path = public;")
            .map_err(db_err("reset search_path after graph persist"))?;

        Ok(())
    }

    /// Persists a graph batch atomically under one SQL transaction.
    ///
    /// This keeps the existing AGE persistence shape, so it does not magically turn the batch
    /// into one Cypher statement. It does ensure the replication worker commits a whole snapshot
    /// or none of it, which is the right baseline before attempting deeper batching work.
    pub fn persist_graph_records_transactional(
        &mut self,
        records: &[GraphEntityRecord],
        tick: u64,
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let mut tx = self
            .client
            .transaction()
            .map_err(db_err("start graph persist transaction"))?;
        persist_graph_records_in_transaction(&mut tx, &self.graph_name, records, tick)?;
        tx.commit()
            .map_err(db_err("commit graph persist transaction"))?;
        Ok(())
    }

    pub fn remove_graph_entities(&mut self, entity_ids: &[String]) -> Result<()> {
        if entity_ids.is_empty() {
            return Ok(());
        }
        self.client
            .batch_execute("LOAD 'age'; SET search_path = ag_catalog, \"$user\", public;")
            .map_err(db_err("prep age for graph remove"))?;

        for entity_id in entity_ids {
            self.run_cypher(&format!(
                "MATCH (e:Entity {{entity_id:'{}'}}) OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(c:Component) DETACH DELETE c, e",
                escape_cypher_string(entity_id),
            ))?;
        }

        self.client
            .batch_execute("SET search_path = public;")
            .map_err(db_err("reset search_path after graph remove"))?;
        Ok(())
    }

    pub fn persist_snapshot_marker(
        &mut self,
        snapshot_tick: u64,
        entity_count: usize,
    ) -> Result<()> {
        let now = now_epoch_s() as i64;
        self.client
            .execute(
                "INSERT INTO replication_snapshot_markers (snapshot_tick, entity_count, created_at_epoch_s) VALUES ($1, $2, $3)",
                &[&(snapshot_tick as i64), &(entity_count as i64), &now],
            )
            .map_err(db_err("insert snapshot marker"))?;
        Ok(())
    }

    pub fn drop_graph(mut self) -> Result<()> {
        self.client
            .batch_execute("LOAD 'age'; SET search_path = ag_catalog, \"$user\", public;")
            .map_err(db_err("prep age for graph drop"))?;
        let sql = format!(
            "SELECT * FROM ag_catalog.drop_graph('{}', true);",
            escape_cypher_string(&self.graph_name)
        );
        self.client
            .batch_execute(&sql)
            .map_err(db_err("drop graph"))?;
        self.client
            .batch_execute("SET search_path = public;")
            .map_err(db_err("reset search_path after graph drop"))?;
        Ok(())
    }

    pub fn load_graph_records(&mut self) -> Result<Vec<GraphEntityRecord>> {
        self.client
            .batch_execute("LOAD 'age'; SET search_path = ag_catalog, \"$user\", public;")
            .map_err(db_err("prep age for graph load"))?;

        let query = format!(
            "SELECT entity_id::text AS entity_id, labels::text AS labels, props::text AS props, component_id::text AS component_id, component_kind::text AS component_kind, component_props::text AS component_props \
             FROM ag_catalog.cypher('{}', $$ \
                MATCH (e:Entity) \
                OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(c:Component) \
                RETURN e.entity_id, labels(e), properties(e), c.component_id, c.component_kind, properties(c) \
             $$) AS (entity_id agtype, labels agtype, props agtype, component_id agtype, component_kind agtype, component_props agtype);",
            escape_cypher_string(&self.graph_name)
        );
        let rows = self
            .client
            .query(&query, &[])
            .map_err(db_err("load graph records"))?;

        self.client
            .batch_execute("SET search_path = public;")
            .map_err(db_err("reset search_path after graph load"))?;

        let mut by_entity = HashMap::<String, GraphEntityRecord>::new();
        for row in rows {
            let Some(entity_id) = parse_agtype_string(row.get::<_, String>("entity_id")) else {
                continue;
            };
            let mut labels = parse_agtype_json(row.get::<_, String>("labels"))
                .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
                .unwrap_or_else(|| vec!["Entity".to_string()]);
            let properties = parse_agtype_json(row.get::<_, String>("props"))
                .unwrap_or(JsonValue::Object(JsonMap::new()));
            if let Some(extra_labels) = properties.get("entity_labels").and_then(|v| v.as_array()) {
                labels.extend(
                    extra_labels
                        .iter()
                        .filter_map(|v| v.as_str().map(ToString::to_string)),
                );
                labels.sort();
                labels.dedup();
            }
            let entry = by_entity
                .entry(entity_id.clone())
                .or_insert_with(|| GraphEntityRecord {
                    entity_id: entity_id.clone(),
                    labels,
                    properties,
                    components: Vec::new(),
                });

            let component_id = row
                .try_get::<_, Option<String>>("component_id")
                .ok()
                .flatten()
                .and_then(parse_agtype_string);
            let component_kind = row
                .try_get::<_, Option<String>>("component_kind")
                .ok()
                .flatten()
                .and_then(parse_agtype_string);
            if let (Some(component_id), Some(component_kind)) = (component_id, component_kind) {
                let component_props = row
                    .try_get::<_, Option<String>>("component_props")
                    .ok()
                    .flatten()
                    .and_then(parse_agtype_json)
                    .unwrap_or(JsonValue::Object(JsonMap::new()));
                let component_props = unwrap_scalar_component_props(component_props);
                if !entry
                    .components
                    .iter()
                    .any(|c| c.component_id == component_id)
                {
                    entry.components.push(GraphComponentRecord {
                        component_id,
                        component_kind,
                        properties: component_props,
                    });
                }
            }
        }

        let mut out = by_entity.into_values().collect::<Vec<_>>();
        out.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
        Ok(out)
    }

    fn persist_relationship_edges(&mut self, record: &GraphEntityRecord) -> Result<()> {
        if let Some(parent_id) = record
            .properties
            .get("parent_entity_id")
            .and_then(JsonValue::as_str)
        {
            // Keep HAS_CHILD single-parent: remove stale incoming parent edges first.
            self.run_cypher(&format!(
                "MATCH (e:Entity {{entity_id:'{}'}}) \
                 OPTIONAL MATCH (old:Entity)-[r:HAS_CHILD]->(e) \
                 WHERE old.entity_id <> '{}' \
                 DELETE r",
                escape_cypher_string(&record.entity_id),
                escape_cypher_string(parent_id),
            ))?;
            self.run_cypher(&format!(
                "MATCH (p:Entity {{entity_id:'{}'}}), (e:Entity {{entity_id:'{}'}}) MERGE (p)-[:HAS_CHILD]->(e)",
                escape_cypher_string(parent_id),
                escape_cypher_string(&record.entity_id),
            ))?;
        } else {
            // Root entities should not keep stale HAS_CHILD incoming edges.
            self.run_cypher(&format!(
                "MATCH (e:Entity {{entity_id:'{}'}}) \
                 OPTIONAL MATCH (:Entity)-[r:HAS_CHILD]->(e) \
                 DELETE r",
                escape_cypher_string(&record.entity_id),
            ))?;
        }

        if record.labels.iter().any(|l| l == "Hardpoint")
            && let Some(owner_id) = record
                .properties
                .get("owner_entity_id")
                .and_then(JsonValue::as_str)
        {
            self.run_cypher(&format!(
                "MATCH (s:Entity {{entity_id:'{}'}}), (h:Entity {{entity_id:'{}'}}) MERGE (s)-[:HAS_HARDPOINT]->(h)",
                escape_cypher_string(owner_id),
                escape_cypher_string(&record.entity_id),
            ))?;
        }

        if let Some(mounted_on) = record
            .properties
            .get("mounted_on_entity_id")
            .and_then(JsonValue::as_str)
        {
            self.run_cypher(&format!(
                "MATCH (m:Entity {{entity_id:'{}'}}), (h:Entity {{entity_id:'{}'}}) MERGE (m)-[:MOUNTED_ON]->(h)",
                escape_cypher_string(&record.entity_id),
                escape_cypher_string(mounted_on),
            ))?;
        }

        Ok(())
    }

    fn run_cypher(&mut self, cypher: &str) -> Result<()> {
        let sql = format!(
            "SELECT * FROM ag_catalog.cypher('{}', $$ {cypher} $$) AS (v agtype);",
            escape_cypher_string(&self.graph_name)
        );
        self.client.query(&sql, &[]).map_err(|err| {
            PersistenceError::Database(format!("cypher execution failed: {err}; query={cypher}"))
        })?;
        Ok(())
    }
}

pub fn ensure_schema_in_transaction(tx: &mut Transaction<'_>, graph_name: &str) -> Result<()> {
    tx.batch_execute("CREATE EXTENSION IF NOT EXISTS age;")
        .map_err(db_err("create age extension"))?;
    tx.batch_execute("LOAD 'age';")
        .map_err(db_err("load age extension"))?;
    tx.batch_execute("SET search_path = ag_catalog, \"$user\", public;")
        .map_err(db_err("set age search_path"))?;

    let graph_exists = tx
        .query_opt(
            "SELECT 1 FROM ag_catalog.ag_graph WHERE name = $1 LIMIT 1",
            &[&graph_name],
        )
        .map_err(db_err("query graph existence"))?
        .is_some();
    if !graph_exists {
        let query = format!(
            "SELECT * FROM ag_catalog.create_graph('{}');",
            escape_cypher_string(graph_name)
        );
        tx.batch_execute(&query).map_err(db_err("create graph"))?;
    }

    tx.batch_execute("SET search_path = public;")
        .map_err(db_err("reset search_path"))?;

    tx.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS replication_snapshot_markers (
            snapshot_id BIGSERIAL PRIMARY KEY,
            snapshot_tick BIGINT NOT NULL,
            entity_count BIGINT NOT NULL,
            created_at_epoch_s BIGINT NOT NULL
        );
        ",
    )
    .map_err(db_err("create snapshot marker table"))?;
    tx.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS script_world_init_state (
            init_key TEXT PRIMARY KEY,
            script_path TEXT NOT NULL,
            applied_at_epoch_s BIGINT NOT NULL
        );
        ",
    )
    .map_err(db_err("create script world init state table"))?;
    tx.batch_execute(script_catalog_schema_sql())
        .map_err(db_err("create script catalog tables"))?;

    Ok(())
}

pub fn script_catalog_schema_sql() -> &'static str {
    concat!(
        "CREATE TABLE IF NOT EXISTS ",
        "script_catalog_documents",
        " (",
        "script_path TEXT PRIMARY KEY,",
        "script_family TEXT NOT NULL,",
        "active_revision BIGINT NOT NULL,",
        "created_at_epoch_s BIGINT NOT NULL,",
        "updated_at_epoch_s BIGINT NOT NULL",
        ");",
        "CREATE TABLE IF NOT EXISTS ",
        "script_catalog_versions",
        " (",
        "script_path TEXT NOT NULL REFERENCES script_catalog_documents(script_path) ON DELETE CASCADE,",
        "revision BIGINT NOT NULL,",
        "source TEXT NOT NULL,",
        "origin TEXT NOT NULL,",
        "created_at_epoch_s BIGINT NOT NULL,",
        "PRIMARY KEY (script_path, revision)",
        ");",
        "CREATE INDEX IF NOT EXISTS script_catalog_documents_family_idx ON script_catalog_documents(script_family);",
        "CREATE TABLE IF NOT EXISTS ",
        "script_catalog_drafts",
        " (",
        "script_path TEXT PRIMARY KEY,",
        "script_family TEXT NOT NULL,",
        "source TEXT NOT NULL,",
        "origin TEXT NOT NULL,",
        "updated_at_epoch_s BIGINT NOT NULL",
        ");"
    )
}

pub fn ensure_script_catalog_schema(client: &mut Client) -> Result<()> {
    client
        .batch_execute(script_catalog_schema_sql())
        .map_err(db_err("create script catalog tables"))?;
    Ok(())
}

pub fn infer_script_family(script_path: &str) -> String {
    if script_path == "world/world_init.lua" {
        return "world_init".to_string();
    }
    if script_path == "assets/registry.lua" {
        return "asset_registry".to_string();
    }
    if script_path == "audio/registry.lua" {
        return "audio_registry".to_string();
    }
    if script_path == "planets/registry.lua" {
        return "planet_registry".to_string();
    }
    if script_path == "bundles/bundle_registry.lua" {
        return "bundle_registry".to_string();
    }
    if script_path == "accounts/player_init.lua" {
        return "player_init".to_string();
    }
    if script_path.starts_with("bundles/") {
        return "bundle".to_string();
    }
    if script_path.starts_with("planets/") {
        return "planet".to_string();
    }
    if script_path.starts_with("ai/") {
        return "ai".to_string();
    }
    if script_path.starts_with("world/") {
        return "world".to_string();
    }
    "misc".to_string()
}

pub fn load_active_script_catalog(client: &mut Client) -> Result<Vec<ScriptCatalogRecord>> {
    ensure_script_catalog_schema(client)?;
    let rows = client
        .query(
            &format!(
                "SELECT d.script_path, d.script_family, d.active_revision, v.source, v.origin
                 FROM {SCRIPT_CATALOG_DOCUMENTS_TABLE} d
                 JOIN {SCRIPT_CATALOG_VERSIONS_TABLE} v
                   ON v.script_path = d.script_path
                  AND v.revision = d.active_revision
                 ORDER BY d.script_path ASC"
            ),
            &[],
        )
        .map_err(db_err("load active script catalog"))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let revision: i64 = row.get(2);
        out.push(ScriptCatalogRecord {
            script_path: row.get(0),
            family: row.get(1),
            revision: revision.max(0) as u64,
            source: row.get(3),
            origin: row.get(4),
        });
    }
    Ok(out)
}

pub fn replace_active_script_catalog(
    client: &mut Client,
    records: &[ScriptCatalogRecord],
) -> Result<()> {
    ensure_script_catalog_schema(client)?;
    let now_epoch_s = now_epoch_s() as i64;
    let mut tx = client
        .transaction()
        .map_err(db_err("begin script catalog transaction"))?;

    let keep_paths = records
        .iter()
        .map(|record| record.script_path.clone())
        .collect::<Vec<_>>();
    if keep_paths.is_empty() {
        tx.execute(
            &format!("DELETE FROM {SCRIPT_CATALOG_DOCUMENTS_TABLE}"),
            &[],
        )
        .map_err(db_err("delete script catalog documents"))?;
        tx.commit()
            .map_err(db_err("commit empty script catalog transaction"))?;
        return Ok(());
    }

    tx.execute(
        &format!(
            "DELETE FROM {SCRIPT_CATALOG_DOCUMENTS_TABLE}
             WHERE NOT (script_path = ANY($1))"
        ),
        &[&keep_paths],
    )
    .map_err(db_err("prune script catalog documents"))?;

    for record in records {
        let revision = record.revision as i64;
        tx.execute(
            &format!(
                "INSERT INTO {SCRIPT_CATALOG_DOCUMENTS_TABLE}
                    (script_path, script_family, active_revision, created_at_epoch_s, updated_at_epoch_s)
                 VALUES ($1, $2, $3, $4, $4)
                 ON CONFLICT (script_path) DO UPDATE
                 SET script_family = EXCLUDED.script_family,
                     active_revision = EXCLUDED.active_revision,
                     updated_at_epoch_s = EXCLUDED.updated_at_epoch_s"
            ),
            &[
                &record.script_path,
                &record.family,
                &revision,
                &now_epoch_s,
            ],
        )
        .map_err(db_err("upsert script catalog document"))?;
        tx.execute(
            &format!(
                "INSERT INTO {SCRIPT_CATALOG_VERSIONS_TABLE}
                    (script_path, revision, source, origin, created_at_epoch_s)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (script_path, revision) DO UPDATE
                 SET source = EXCLUDED.source,
                     origin = EXCLUDED.origin"
            ),
            &[
                &record.script_path,
                &revision,
                &record.source,
                &record.origin,
                &now_epoch_s,
            ],
        )
        .map_err(db_err("upsert script catalog version"))?;
    }

    tx.commit()
        .map_err(db_err("commit script catalog transaction"))?;
    Ok(())
}

pub fn list_script_catalog_documents(
    client: &mut Client,
) -> Result<Vec<ScriptCatalogDocumentSummary>> {
    ensure_script_catalog_schema(client)?;
    let doc_rows = client
        .query(
            &format!(
                "SELECT script_path, script_family, active_revision
                 FROM {SCRIPT_CATALOG_DOCUMENTS_TABLE}
                 ORDER BY script_path ASC"
            ),
            &[],
        )
        .map_err(db_err("list script catalog documents"))?;
    let draft_rows = client
        .query(
            &format!(
                "SELECT script_path, script_family
                 FROM {SCRIPT_CATALOG_DRAFTS_TABLE}
                 ORDER BY script_path ASC"
            ),
            &[],
        )
        .map_err(db_err("list script catalog drafts"))?;

    let mut by_path = HashMap::<String, ScriptCatalogDocumentSummary>::new();
    for row in doc_rows {
        let revision: i64 = row.get(2);
        let script_path: String = row.get(0);
        by_path.insert(
            script_path.clone(),
            ScriptCatalogDocumentSummary {
                script_path,
                family: row.get(1),
                active_revision: Some(revision.max(0) as u64),
                has_draft: false,
            },
        );
    }
    for row in draft_rows {
        let script_path: String = row.get(0);
        let family: String = row.get(1);
        by_path
            .entry(script_path.clone())
            .and_modify(|entry| {
                entry.has_draft = true;
                if entry.family.is_empty() {
                    entry.family = family.clone();
                }
            })
            .or_insert(ScriptCatalogDocumentSummary {
                script_path,
                family,
                active_revision: None,
                has_draft: true,
            });
    }
    let mut out = by_path.into_values().collect::<Vec<_>>();
    out.sort_by(|a, b| a.script_path.cmp(&b.script_path));
    Ok(out)
}

pub fn load_script_catalog_document(
    client: &mut Client,
    script_path: &str,
) -> Result<Option<ScriptCatalogDocumentDetail>> {
    ensure_script_catalog_schema(client)?;
    let doc_row = client
        .query_opt(
            &format!(
                "SELECT d.script_path, d.script_family, d.active_revision, v.source, v.origin
                 FROM {SCRIPT_CATALOG_DOCUMENTS_TABLE} d
                 LEFT JOIN {SCRIPT_CATALOG_VERSIONS_TABLE} v
                   ON v.script_path = d.script_path
                  AND v.revision = d.active_revision
                 WHERE d.script_path = $1"
            ),
            &[&script_path],
        )
        .map_err(db_err("load script catalog document"))?;
    let draft_row = client
        .query_opt(
            &format!(
                "SELECT script_path, script_family, source, origin, updated_at_epoch_s
                 FROM {SCRIPT_CATALOG_DRAFTS_TABLE}
                 WHERE script_path = $1"
            ),
            &[&script_path],
        )
        .map_err(db_err("load script catalog draft"))?;

    if doc_row.is_none() && draft_row.is_none() {
        return Ok(None);
    }

    let mut detail = if let Some(row) = doc_row {
        let revision: i64 = row.get(2);
        ScriptCatalogDocumentDetail {
            script_path: row.get(0),
            family: row.get(1),
            active_revision: Some(revision.max(0) as u64),
            active_source: row.get(3),
            active_origin: row.get(4),
            draft_source: None,
            draft_origin: None,
            draft_updated_at_epoch_s: None,
        }
    } else {
        ScriptCatalogDocumentDetail {
            script_path: script_path.to_string(),
            family: String::new(),
            active_revision: None,
            active_source: None,
            active_origin: None,
            draft_source: None,
            draft_origin: None,
            draft_updated_at_epoch_s: None,
        }
    };

    if let Some(row) = draft_row {
        let updated_at: i64 = row.get(4);
        detail.script_path = row.get(0);
        if detail.family.is_empty() {
            detail.family = row.get(1);
        }
        detail.draft_source = row.get(2);
        detail.draft_origin = row.get(3);
        detail.draft_updated_at_epoch_s = Some(updated_at.max(0) as u64);
    }
    if detail.family.is_empty() {
        detail.family = infer_script_family(script_path);
    }
    Ok(Some(detail))
}

pub fn upsert_script_catalog_draft(
    client: &mut Client,
    script_path: &str,
    family: &str,
    source: &str,
    origin: &str,
) -> Result<()> {
    ensure_script_catalog_schema(client)?;
    let family = if family.trim().is_empty() {
        infer_script_family(script_path)
    } else {
        family.trim().to_string()
    };
    let now_epoch_s = now_epoch_s() as i64;
    client
        .execute(
            &format!(
                "INSERT INTO {SCRIPT_CATALOG_DRAFTS_TABLE}
                    (script_path, script_family, source, origin, updated_at_epoch_s)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (script_path) DO UPDATE
                 SET script_family = EXCLUDED.script_family,
                     source = EXCLUDED.source,
                     origin = EXCLUDED.origin,
                     updated_at_epoch_s = EXCLUDED.updated_at_epoch_s"
            ),
            &[&script_path, &family, &source, &origin, &now_epoch_s],
        )
        .map_err(db_err("upsert script catalog draft"))?;
    Ok(())
}

pub fn discard_script_catalog_draft(client: &mut Client, script_path: &str) -> Result<bool> {
    ensure_script_catalog_schema(client)?;
    let deleted = client
        .execute(
            &format!("DELETE FROM {SCRIPT_CATALOG_DRAFTS_TABLE} WHERE script_path = $1"),
            &[&script_path],
        )
        .map_err(db_err("discard script catalog draft"))?;
    Ok(deleted > 0)
}

pub fn publish_script_catalog_draft(client: &mut Client, script_path: &str) -> Result<Option<u64>> {
    ensure_script_catalog_schema(client)?;
    let mut tx = client
        .transaction()
        .map_err(db_err("begin script draft publish transaction"))?;
    let draft_row = tx
        .query_opt(
            &format!(
                "SELECT script_family, source, origin
                 FROM {SCRIPT_CATALOG_DRAFTS_TABLE}
                 WHERE script_path = $1"
            ),
            &[&script_path],
        )
        .map_err(db_err("load script draft for publish"))?;
    let Some(draft_row) = draft_row else {
        tx.commit()
            .map_err(db_err("commit empty script draft publish transaction"))?;
        return Ok(None);
    };
    let family: String = draft_row.get(0);
    let source: String = draft_row.get(1);
    let origin: String = draft_row.get(2);
    let next_revision = tx
        .query_one(
            &format!(
                "SELECT COALESCE(MAX(revision), 0) + 1
                 FROM {SCRIPT_CATALOG_VERSIONS_TABLE}
                 WHERE script_path = $1"
            ),
            &[&script_path],
        )
        .map_err(db_err("compute next script revision"))?
        .get::<_, i64>(0)
        .max(1);
    let now_epoch_s = now_epoch_s() as i64;
    tx.execute(
        &format!(
            "INSERT INTO {SCRIPT_CATALOG_VERSIONS_TABLE}
                (script_path, revision, source, origin, created_at_epoch_s)
             VALUES ($1, $2, $3, $4, $5)"
        ),
        &[&script_path, &next_revision, &source, &origin, &now_epoch_s],
    )
    .map_err(db_err("insert published script version"))?;
    tx.execute(
        &format!(
            "INSERT INTO {SCRIPT_CATALOG_DOCUMENTS_TABLE}
                (script_path, script_family, active_revision, created_at_epoch_s, updated_at_epoch_s)
             VALUES ($1, $2, $3, $4, $4)
             ON CONFLICT (script_path) DO UPDATE
             SET script_family = EXCLUDED.script_family,
                 active_revision = EXCLUDED.active_revision,
                 updated_at_epoch_s = EXCLUDED.updated_at_epoch_s"
        ),
        &[&script_path, &family, &next_revision, &now_epoch_s],
    )
    .map_err(db_err("upsert published script document"))?;
    tx.execute(
        &format!("DELETE FROM {SCRIPT_CATALOG_DRAFTS_TABLE} WHERE script_path = $1"),
        &[&script_path],
    )
    .map_err(db_err("delete published script draft"))?;
    tx.commit()
        .map_err(db_err("commit script draft publish transaction"))?;
    Ok(Some(next_revision as u64))
}

pub fn persist_graph_records_in_transaction(
    tx: &mut Transaction<'_>,
    graph_name: &str,
    records: &[GraphEntityRecord],
    tick: u64,
) -> Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    validate_runtime_guid_uniqueness(records)?;
    tx.batch_execute("LOAD 'age'; SET search_path = ag_catalog, \"$user\", public;")
        .map_err(db_err("prep age for graph persist"))?;

    let mut entity_rows = Vec::<JsonValue>::with_capacity(records.len());
    let mut stale_component_rows = Vec::<StaleComponentCleanupRow>::with_capacity(records.len());
    let mut component_rows = Vec::<JsonValue>::new();
    let mut entity_component_rows = Vec::<EntityComponentEdgeRow>::new();
    let mut child_edge_rows = Vec::<ChildEdgeRow>::new();
    let mut root_entity_rows = Vec::<EntityIdRow>::new();
    let mut hardpoint_edge_rows = Vec::<HardpointEdgeRow>::new();
    let mut mounted_on_edge_rows = Vec::<MountedOnEdgeRow>::new();

    for record in records {
        let labels = sanitize_labels(&record.labels);
        entity_rows.push(flatten_row_properties(
            [
                ("entity_id", JsonValue::String(record.entity_id.clone())),
                ("last_tick", JsonValue::from(tick)),
                (
                    "entity_labels",
                    JsonValue::Array(labels.into_iter().map(JsonValue::String).collect()),
                ),
            ],
            &record.properties,
        ));
        stale_component_rows.push(StaleComponentCleanupRow {
            entity_id: record.entity_id.clone(),
            incoming_component_ids: record
                .components
                .iter()
                .map(|component| component.component_id.clone())
                .collect(),
        });

        for component in &record.components {
            component_rows.push(flatten_row_properties(
                [
                    (
                        "component_id",
                        JsonValue::String(component.component_id.clone()),
                    ),
                    (
                        "component_kind",
                        JsonValue::String(component.component_kind.clone()),
                    ),
                    ("last_tick", JsonValue::from(tick)),
                ],
                &component_properties_object(&component.properties),
            ));
            entity_component_rows.push(EntityComponentEdgeRow {
                entity_id: record.entity_id.clone(),
                component_id: component.component_id.clone(),
            });
        }

        if let Some(parent_id) = record
            .properties
            .get("parent_entity_id")
            .and_then(JsonValue::as_str)
        {
            child_edge_rows.push(ChildEdgeRow {
                parent_entity_id: parent_id.to_string(),
                child_entity_id: record.entity_id.clone(),
            });
        } else {
            root_entity_rows.push(EntityIdRow {
                entity_id: record.entity_id.clone(),
            });
        }

        if record.labels.iter().any(|l| l == "Hardpoint")
            && let Some(owner_id) = record
                .properties
                .get("owner_entity_id")
                .and_then(JsonValue::as_str)
        {
            hardpoint_edge_rows.push(HardpointEdgeRow {
                owner_entity_id: owner_id.to_string(),
                hardpoint_entity_id: record.entity_id.clone(),
            });
        }

        if let Some(mounted_on) = record
            .properties
            .get("mounted_on_entity_id")
            .and_then(JsonValue::as_str)
        {
            mounted_on_edge_rows.push(MountedOnEdgeRow {
                module_entity_id: record.entity_id.clone(),
                mount_entity_id: mounted_on.to_string(),
            });
        }
    }

    run_batched_cypher_in_transaction(tx, graph_name, &entity_rows, |rows| {
        format!(
            "UNWIND {} AS row \
                 MERGE (e:Entity {{entity_id: row.entity_id}}) \
                 SET e += row",
            cypher_literal(&rows)
        )
    })?;
    run_batched_cypher_in_transaction(tx, graph_name, &stale_component_rows, |rows| {
        format!(
            "UNWIND {} AS row \
                 MATCH (e:Entity {{entity_id: row.entity_id}}) \
                 OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(c:Component) \
                 WHERE c IS NOT NULL AND NOT c.component_id IN row.incoming_component_ids \
                 DETACH DELETE c",
            cypher_literal(&rows)
        )
    })?;
    run_batched_cypher_in_transaction(tx, graph_name, &component_rows, |rows| {
        format!(
            "UNWIND {} AS row \
                 MERGE (c:Component {{component_id: row.component_id}}) \
                 SET c += row",
            cypher_literal(&rows)
        )
    })?;
    run_batched_cypher_in_transaction(tx, graph_name, &entity_component_rows, |rows| {
        format!(
            "UNWIND {} AS row \
                 MATCH (e:Entity {{entity_id: row.entity_id}}), (c:Component {{component_id: row.component_id}}) \
                 MERGE (e)-[:HAS_COMPONENT]->(c)",
            cypher_literal(&rows)
        )
    })?;
    run_batched_cypher_in_transaction(tx, graph_name, &child_edge_rows, |rows| {
        format!(
            "UNWIND {} AS row \
                 MATCH (e:Entity {{entity_id: row.child_entity_id}}) \
                 OPTIONAL MATCH (old:Entity)-[r:HAS_CHILD]->(e) \
                 WHERE old.entity_id <> row.parent_entity_id \
                 DELETE r",
            cypher_literal(&rows)
        )
    })?;
    run_batched_cypher_in_transaction(tx, graph_name, &child_edge_rows, |rows| {
        format!(
            "UNWIND {} AS row \
                 MATCH (p:Entity {{entity_id: row.parent_entity_id}}), (e:Entity {{entity_id: row.child_entity_id}}) \
                 MERGE (p)-[:HAS_CHILD]->(e)",
            cypher_literal(&rows)
        )
    })?;
    run_batched_cypher_in_transaction(tx, graph_name, &root_entity_rows, |rows| {
        format!(
            "UNWIND {} AS row \
                 MATCH (e:Entity {{entity_id: row.entity_id}}) \
                 OPTIONAL MATCH (:Entity)-[r:HAS_CHILD]->(e) \
                 DELETE r",
            cypher_literal(&rows)
        )
    })?;
    run_batched_cypher_in_transaction(tx, graph_name, &hardpoint_edge_rows, |rows| {
        format!(
            "UNWIND {} AS row \
                 MATCH (s:Entity {{entity_id: row.owner_entity_id}}), (h:Entity {{entity_id: row.hardpoint_entity_id}}) \
                 MERGE (s)-[:HAS_HARDPOINT]->(h)",
            cypher_literal(&rows)
        )
    })?;
    run_batched_cypher_in_transaction(tx, graph_name, &mounted_on_edge_rows, |rows| {
        format!(
            "UNWIND {} AS row \
                 MATCH (m:Entity {{entity_id: row.module_entity_id}}), (h:Entity {{entity_id: row.mount_entity_id}}) \
                 MERGE (m)-[:MOUNTED_ON]->(h)",
            cypher_literal(&rows)
        )
    })?;

    tx.batch_execute("SET search_path = public;")
        .map_err(db_err("reset search_path after graph persist"))?;
    Ok(())
}

fn run_cypher_in_transaction(
    tx: &mut Transaction<'_>,
    graph_name: &str,
    cypher: &str,
) -> Result<()> {
    let sql = format!(
        "SELECT * FROM ag_catalog.cypher('{}', $$ {cypher} $$) AS (v agtype);",
        escape_cypher_string(graph_name)
    );
    tx.query(&sql, &[]).map_err(|err| {
        PersistenceError::Database(format!("cypher execution failed: {err}; query={cypher}"))
    })?;
    Ok(())
}

fn run_batched_cypher_in_transaction<T, F>(
    tx: &mut Transaction<'_>,
    graph_name: &str,
    rows: &[T],
    build_cypher: F,
) -> Result<()>
where
    T: Serialize,
    F: FnOnce(JsonValue) -> String,
{
    if rows.is_empty() {
        return Ok(());
    }
    let rows = serde_json::to_value(rows).map_err(|err| {
        PersistenceError::Serialization(format!("serialize batched rows failed: {err}"))
    })?;
    let cypher = build_cypher(rows);
    run_cypher_in_transaction(tx, graph_name, &cypher)
}

fn sanitize_labels(labels: &[String]) -> Vec<String> {
    labels
        .iter()
        .filter_map(|label| {
            let cleaned = label
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>();
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            }
        })
        .collect::<Vec<_>>()
}

/// Appends Cypher SET clauses for component properties.
/// Object values are flattened into individual node properties.
/// Non-object values (strings, numbers, booleans, arrays, null) are stored
/// as a serialised JSON string in a `value` property to prevent data loss.
fn append_component_property_clauses(comp_set: &mut Vec<String>, properties: &JsonValue) {
    if properties.is_object() {
        comp_set.extend(cypher_set_clauses("c", properties));
    } else {
        let json_str = serde_json::to_string(properties).unwrap_or_default();
        comp_set.push(format!("c.value='{}'", escape_cypher_string(&json_str)));
    }
}

fn component_properties_object(properties: &JsonValue) -> JsonValue {
    if properties.is_object() {
        properties.clone()
    } else {
        JsonValue::Object(JsonMap::from_iter([(
            "value".to_string(),
            JsonValue::String(serde_json::to_string(properties).unwrap_or_default()),
        )]))
    }
}

fn flatten_row_properties<const N: usize>(
    base_fields: [(&str, JsonValue); N],
    properties: &JsonValue,
) -> JsonValue {
    let mut object = JsonMap::new();
    for (key, value) in base_fields {
        object.insert(key.to_string(), value);
    }
    if let Some(map) = properties.as_object() {
        for (key, value) in map {
            object.insert(key.clone(), value.clone());
        }
    }
    JsonValue::Object(object)
}

/// Recovers component properties that were stored as non-object JSON.
/// If the loaded properties object contains a `value` key, parse and return
/// the original value; otherwise return the properties unchanged.
fn unwrap_scalar_component_props(props: JsonValue) -> JsonValue {
    if let Some(json_str) = props
        .as_object()
        .and_then(|obj| obj.get("value"))
        .and_then(|v| v.as_str())
    {
        serde_json::from_str(json_str).unwrap_or(props)
    } else {
        props
    }
}

fn cypher_set_clauses(prefix: &str, value: &JsonValue) -> Vec<String> {
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    obj.iter()
        .map(|(key, val)| {
            let ident = cypher_property_ident(key);
            format!("{prefix}.{ident}={}", cypher_literal(val))
        })
        .collect::<Vec<_>>()
}

#[doc(hidden)]
pub fn cypher_literal(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "null".to_string(),
        JsonValue::Bool(v) => v.to_string(),
        JsonValue::Number(v) => v.to_string(),
        JsonValue::String(v) => format!("'{}'", escape_cypher_string(v)),
        JsonValue::Array(values) => {
            let rendered = values.iter().map(cypher_literal).collect::<Vec<_>>();
            format!("[{}]", rendered.join(","))
        }
        JsonValue::Object(map) => {
            let rendered = map
                .iter()
                .map(|(k, v)| format!("{}:{}", cypher_property_ident(k), cypher_literal(v)))
                .collect::<Vec<_>>();
            format!("{{{}}}", rendered.join(","))
        }
    }
}

fn cypher_property_ident(raw_key: &str) -> String {
    let clean_key = raw_key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>();
    format!("`{clean_key}`")
}

#[doc(hidden)]
pub fn parse_agtype_string(raw: String) -> Option<String> {
    let trimmed = raw.trim();
    if let Ok(parsed) = serde_json::from_str::<String>(trimmed) {
        return Some(parsed);
    }
    let stripped = strip_trailing_agtype_suffix(trimmed);
    if let Ok(parsed) = serde_json::from_str::<String>(stripped) {
        return Some(parsed);
    }
    if stripped.is_empty() || stripped == "null" {
        return None;
    }
    Some(stripped.trim_matches('"').to_string())
}

#[doc(hidden)]
pub fn parse_agtype_json(raw: String) -> Option<JsonValue> {
    let trimmed = raw.trim();
    if let Ok(parsed) = serde_json::from_str::<JsonValue>(trimmed) {
        return Some(parsed);
    }
    let stripped = strip_trailing_agtype_suffix(trimmed);
    serde_json::from_str::<JsonValue>(stripped).ok()
}

fn strip_trailing_agtype_suffix(raw: &str) -> &str {
    let Some((left, suffix)) = raw.rsplit_once("::") else {
        return raw;
    };
    if matches!(suffix, "agtype" | "vertex" | "edge" | "path") {
        left
    } else {
        raw
    }
}

fn escape_cypher_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn parse_runtime_guid(entity_id: &str) -> Option<&str> {
    is_uuid_like(entity_id).then_some(entity_id)
}

fn is_uuid_like(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (idx, ch) in bytes.iter().enumerate() {
        let is_dash = matches!(idx, 8 | 13 | 18 | 23);
        if is_dash {
            if *ch != b'-' {
                return false;
            }
        } else if !ch.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

#[doc(hidden)]
pub fn validate_runtime_guid_uniqueness(records: &[GraphEntityRecord]) -> Result<()> {
    let mut entity_ids_by_guid = HashMap::<String, Vec<String>>::new();
    for record in records {
        let Some(guid) = parse_runtime_guid(&record.entity_id) else {
            continue;
        };
        entity_ids_by_guid
            .entry(guid.to_string())
            .or_default()
            .push(record.entity_id.clone());
    }
    let collisions = entity_ids_by_guid
        .into_iter()
        .filter_map(|(guid, mut entity_ids)| {
            (entity_ids.len() > 1).then(|| {
                entity_ids.sort();
                format!("guid {guid} reused by {:?}", entity_ids)
            })
        })
        .collect::<Vec<_>>();
    if collisions.is_empty() {
        return Ok(());
    }
    Err(PersistenceError::Validation(format!(
        "runtime GUID collision detected: {}",
        collisions.join("; ")
    )))
}

fn now_epoch_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs()
}

fn db_err(action: &'static str) -> impl Fn(postgres::Error) -> PersistenceError {
    move |err| PersistenceError::Database(format!("{action} failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{cypher_literal, cypher_set_clauses, infer_script_family};
    use serde_json::json;

    #[test]
    fn infer_script_family_classifies_known_roots() {
        assert_eq!(infer_script_family("world/world_init.lua"), "world_init");
        assert_eq!(infer_script_family("assets/registry.lua"), "asset_registry");
        assert_eq!(infer_script_family("audio/registry.lua"), "audio_registry");
        assert_eq!(
            infer_script_family("planets/registry.lua"),
            "planet_registry"
        );
        assert_eq!(
            infer_script_family("bundles/bundle_registry.lua"),
            "bundle_registry"
        );
        assert_eq!(
            infer_script_family("accounts/player_init.lua"),
            "player_init"
        );
        assert_eq!(
            infer_script_family("bundles/starter/planet_body.lua"),
            "bundle"
        );
        assert_eq!(infer_script_family("planets/aurelia.lua"), "planet");
        assert_eq!(infer_script_family("ai/pirate_patrol.lua"), "ai");
        assert_eq!(infer_script_family("world/something_else.lua"), "world");
        assert_eq!(infer_script_family("misc/foo.lua"), "misc");
    }

    #[test]
    fn cypher_set_clauses_quote_keyword_like_property_names() {
        let clauses = cypher_set_clauses(
            "c",
            &json!({
                "order": -190,
                "display_name": "StarField"
            }),
        );
        assert!(clauses.iter().any(|value| value == "c.`order`=-190"));
        assert!(
            clauses
                .iter()
                .any(|value| value == "c.`display_name`='StarField'")
        );
    }

    #[test]
    fn cypher_literal_quotes_object_keys() {
        let rendered = cypher_literal(&json!({
            "order": -190,
            "phase": "fullscreen_background"
        }));
        assert!(rendered.contains("`order`:-190"));
        assert!(rendered.contains("`phase`:'fullscreen_background'"));
    }
}
